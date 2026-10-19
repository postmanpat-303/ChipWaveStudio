use super::*;
use crate::model::{Cell, Pattern, Song, NOTE_OFF};

fn fixture(name: &str) -> Song {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    crate::fileio::load_cwt(&p).expect("fixture loads")
}

fn note(n: u8) -> Cell {
    Cell {
        note: n,
        instr: 1,
        ..Default::default()
    }
}

/// a small synthetic song: 3 channels, repeats, a pattern break, an
/// odd-length block, an empty column and a 300-row block
fn test_song() -> Song {
    let mut s = crate::fileio::demo_song();
    s.order.clear();
    s.channels = 3;
    s.channel_names = vec!["A".into(), "B".into(), "C".into()];
    s.automation.clear();
    let mut a = Pattern::new("A", 16, 3);
    a.cells[0][0] = note(49);
    a.cells[4][1] = note(52);
    a.cells[8][0] = Cell {
        note: NOTE_OFF,
        ..Default::default()
    };
    let mut brk = Pattern::new("Break", 32, 3);
    brk.cells[0][2] = note(37);
    brk.cells[3][1] = note(40);
    brk.cells[9][2] = Cell {
        fx: 0x0F,
        fxp: 0,
        ..Default::default()
    };
    brk.cells[20][0] = note(61); // after the break: never played
    let mut odd = Pattern::new("Odd", 13, 3);
    odd.cells[2][0] = note(55);
    odd.cells[12][2] = Cell {
        fx: 0x0F,
        fxp: 150, // tempo
        ..Default::default()
    };
    let mut long = Pattern::new("Long", 300, 3);
    for r in (0..300).step_by(7) {
        long.cells[r][r % 3] = note(40 + (r % 24) as u8);
    }
    s.patterns = vec![a, brk, odd, long];
    s.order = vec![0, 0, 1, 2, 0, 3, 1, 1];
    s
}

#[test]
fn velocity_scaling_is_m2_115_and_reversible() {
    assert_eq!(vel_from_7(0), 0);
    assert_eq!(vel_from_7(64), 0x8000);
    assert_eq!(vel_from_7(127), 0xFFFF);
    let mut prev = 0;
    for v in 0..=127u8 {
        let w = vel_from_7(v);
        assert_eq!(vel_to_7(w), v, "round trip {v}");
        assert!(v == 0 || w > prev, "monotonic at {v}");
        prev = w;
    }
}

#[test]
fn note_expressions_match_clap_ids() {
    use clap_sys::events::*;
    let ids = [
        CLAP_NOTE_EXPRESSION_VOLUME,
        CLAP_NOTE_EXPRESSION_PAN,
        CLAP_NOTE_EXPRESSION_TUNING,
        CLAP_NOTE_EXPRESSION_VIBRATO,
        CLAP_NOTE_EXPRESSION_EXPRESSION,
        CLAP_NOTE_EXPRESSION_BRIGHTNESS,
        CLAP_NOTE_EXPRESSION_PRESSURE,
    ];
    for (k, id) in ExprKind::ALL.iter().zip(ids) {
        assert_eq!(k.clap_id(), id);
    }
}

#[test]
fn tempo_map_steps_and_ramps() {
    let mut tl = Timeline::default();
    // no map: the song tempo; 120 BPM = 0.5 s per beat
    assert!((tl.seconds_at(4 * PPQ, 120.0) - 2.0).abs() < 1e-9);
    tl.tempo = vec![
        TempoPoint {
            tick: 0,
            bpm: 120.0,
            ramp: false,
        },
        TempoPoint {
            tick: 4 * PPQ,
            bpm: 120.0,
            ramp: true,
        },
        TempoPoint {
            tick: 8 * PPQ,
            bpm: 240.0,
            ramp: false,
        },
    ];
    assert_eq!(tl.bpm_at(2 * PPQ, 99.0), 120.0);
    assert!((tl.bpm_at(6 * PPQ, 99.0) - 180.0).abs() < 1e-4);
    assert_eq!(tl.bpm_at(9 * PPQ, 99.0), 240.0);
    // ramp 120 -> 240 over 4 beats: 2 ln 2 s (closed form)
    let ramp = tl.seconds_at(8 * PPQ, 0.0) - tl.seconds_at(4 * PPQ, 0.0);
    assert!((ramp - 2.0 * 2f64.ln()).abs() < 1e-9, "{ramp}");
    // and a brute-force sum agrees
    let mut sum = 0.0;
    for t in 4 * PPQ..8 * PPQ {
        sum += 60.0 / (PPQ as f64 * tl.bpm_at(t, 0.0) as f64);
    }
    assert!((sum - ramp).abs() < 1e-3, "{sum} vs {ramp}");
    // after the ramp: 240 BPM = 0.25 s per beat
    let after = tl.seconds_at(10 * PPQ, 0.0) - tl.seconds_at(8 * PPQ, 0.0);
    assert!((after - 0.5).abs() < 1e-9);
    // part-way into the ramp
    assert!(tl.seconds_at(6 * PPQ, 0.0) > 2.0 && tl.seconds_at(6 * PPQ, 0.0) < 3.0);
}

#[test]
fn signature_map_bars_and_positions() {
    let mut tl = Timeline::default();
    let bar = 4 * PPQ;
    assert_eq!(tl.bars_in(0, 3 * bar), vec![(0, 1), (bar, 2), (2 * bar, 3)]);
    assert_eq!(
        tl.bar_pos(bar + PPQ + 7),
        BarPos {
            bar: 2,
            beat: 2,
            tick: 7
        }
    );
    // two bars of 4/4, then 3/4, then 7/8
    tl.signature = vec![
        SigPoint {
            tick: 0,
            num: 4,
            den: 4,
        },
        SigPoint {
            tick: 2 * bar,
            num: 3,
            den: 4,
        },
        SigPoint {
            tick: 2 * bar + 2 * 3 * PPQ,
            num: 7,
            den: 8,
        },
    ];
    let b = tl.bars_in(0, 2 * bar + 6 * PPQ + 8 * PPQ);
    let t34 = 2 * bar;
    let t78 = t34 + 6 * PPQ;
    assert_eq!(
        b,
        vec![
            (0, 1),
            (bar, 2),
            (t34, 3),
            (t34 + 3 * PPQ, 4),
            (t78, 5),
            (t78 + 7 * PPQ / 2, 6),
            (t78 + 7 * PPQ, 7)
        ]
    );
    assert_eq!(
        tl.bars_in(t34 + 1, t78 + 1),
        vec![(t34 + 3 * PPQ, 4), (t78, 5)]
    );
    assert_eq!(tl.bar_pos(t34 + 3 * PPQ + PPQ).bar, 4);
    assert_eq!(tl.bar_pos(t34 + 3 * PPQ + PPQ).beat, 2);
    assert_eq!(
        tl.bar_pos(t78 + PPQ / 2).beat,
        2,
        "eighth-note beats in 7/8"
    );
    assert_eq!(tl.sig_at(t78 + 5).num, 7);
}

#[test]
fn clip_source_loops_passes_and_seeded_randomness() {
    let c = Clip {
        start: 1000 * ROW,
        len: 40 * ROW,
        offset: 4 * ROW,
        loop_len: Some(16 * ROW),
        seed: 42,
        ..Default::default()
    };
    assert_eq!(c.source_at(c.start), 4 * ROW);
    assert_eq!(c.source_at(c.start + 12 * ROW), 0);
    assert_eq!(c.pass_at(c.start + 11 * ROW), 0);
    assert_eq!(c.pass_at(c.start + 12 * ROW), 1);
    let lp: Vec<Tick> = c.loop_points().collect();
    assert_eq!(lp, vec![c.start + 12 * ROW, c.start + 28 * ROW]);
    let plain = Clip::default();
    assert_eq!(plain.loop_points().count(), 0);
    // the same seed and pass always give the same numbers
    let draw = |seed, pass| {
        let mut r = ClipRng::new(seed, pass);
        (0..8).map(|_| r.next_u32()).collect::<Vec<_>>()
    };
    assert_eq!(draw(42, 1), draw(42, 1));
    assert_ne!(draw(42, 1), draw(42, 2));
    assert_ne!(draw(42, 1), draw(43, 1));
    let mut r = ClipRng::new(7, 0);
    assert!((0..1000)
        .map(|_| r.next_f32())
        .all(|x| (0.0..1.0).contains(&x)));
    assert_eq!(seed_for(5), seed_for(5));
    assert_ne!(seed_for(5), seed_for(6));
}

#[test]
fn innermost_lane_wins() {
    assert_eq!(innermost(Some(0.2), Some(0.5), Some(0.9)), Some(0.2));
    assert_eq!(innermost(None, Some(0.5), Some(0.9)), Some(0.5));
    assert_eq!(innermost(None, None, Some(0.9)), Some(0.9));
    assert_eq!(innermost(None, None, None), None);
}

#[test]
fn timeline_toml_round_trip_with_every_field_and_shared_notes() {
    let (mut tl, _) = from_order(&test_song());
    tl.tempo.push(TempoPoint {
        tick: 64 * ROW,
        bpm: 140.0,
        ramp: true,
    });
    tl.signature.push(SigPoint {
        tick: 0,
        num: 7,
        den: 8,
    });
    tl.markers.push(Marker {
        name: "Filter opens".into(),
        tick: 32 * ROW,
        colour: Some(0x336699),
        track: Some(1),
    });
    tl.loop_range = Some((0, 64 * ROW));
    tl.notes.push(Shared::new(NoteClipData {
        name: "Lead".into(),
        len: 16 * ROW,
        notes: vec![Note {
            start: 0,
            len: 3 * ROW + 17,
            key: 61,
            vel: 51234,
            rel_vel: 800,
            instr: 2,
            fx: Some((0x0C, 0x20)),
            tune: -0.25,
            chance: 70,
            expr: vec![NoteExpr {
                kind: ExprKind::Brightness,
                points: vec![(0, 0.1), (ROW, 0.9)],
            }],
        }],
    }));
    let id = tl.new_id();
    tl.clips.push(Clip {
        id,
        track: 2,
        start: 128 * ROW,
        len: 32 * ROW,
        loop_len: Some(16 * ROW),
        kind: ClipKind::Notes { data: 0 },
        name: Some("Lead loop".into()),
        colour: Some(0xAA5500),
        muted: true,
        gain: 0.5,
        seed: 99,
        auto: vec![crate::automation::Lane {
            target: "mixer.ch.3.volume".into(),
            label: "vol".into(),
            points: vec![crate::automation::Point {
                pos: 1.5,
                value: 0.75,
                shape: crate::automation::Shape::Smooth,
            }],
            enabled: true,
        }],
        ..Default::default()
    });
    tl.tracks[1].auto = tl.clips.last().unwrap().auto.clone();
    tl.sort();
    let text = to_toml(&tl).unwrap();
    let back = from_toml(&text).unwrap();
    assert_eq!(back, tl, "{text}");
    // defaults stay out of the file (compact, and old fields can't clash)
    assert!(!text.contains("fade_in"), "{text}");
    // a minimal hand-written file fills every default
    let min = from_toml("[[clips]]\nid = 3\ntrack = 1\nstart = 0\nlen = 960\n[clips.kind]\ntype = \"block\"\npattern = 0\ncol = 1\n").unwrap();
    assert_eq!(min.clips[0].gain, 1.0);
    assert_eq!(min.next_id, 4);
    // pooled notes are shared by clones (cheap undo) until one is edited
    let mut copy = tl.clone();
    assert!(copy.notes[0].same(&tl.notes[0]));
    copy.notes[0].make_mut().name = "Edited".into();
    assert!(!copy.notes[0].same(&tl.notes[0]));
    assert_eq!(tl.notes[0].name, "Lead");
}

#[test]
fn derived_timeline_merges_runs_skips_empty_columns_and_maps_positions() {
    let s = test_song();
    let (tl, map) = from_order(&s);
    assert_eq!(tl.tracks.len(), 3);
    assert_eq!(tl.tracks[1].name, "B");
    // effective lengths: Break ends at row 9 (its 0F00 row)
    assert_eq!(map.lens, vec![16, 16, 10, 13, 16, 300, 10, 10]);
    assert_eq!(map.full, vec![16, 16, 32, 13, 16, 300, 32, 32]);
    // track 0: A A (merged, looped) | Break? no (col 0 only has a note after the break) | Odd | A | Long
    let t0: Vec<(Tick, Tick, Option<Tick>)> = tl
        .clips
        .iter()
        .filter(|c| c.track == 0)
        .map(|c| (c.start / ROW, c.len / ROW, c.loop_len.map(|l| l / ROW)))
        .collect();
    assert_eq!(
        t0,
        vec![
            (0, 32, Some(16)),
            (42, 13, None),
            (55, 16, None),
            (71, 300, None)
        ]
    );
    // track 2: the two Break entries at the end merge into one looped clip
    let last = tl.clips.iter().rev().find(|c| c.track == 2).unwrap();
    assert_eq!(
        (last.start / ROW, last.len / ROW, last.loop_len),
        (371, 20, Some(10 * ROW))
    );
    // ids unique, seeds set, sorted
    let ids: BTreeSet<u32> = tl.clips.iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), tl.clips.len());
    assert!(tl.clips.iter().all(|c| c.seed == seed_for(c.id)));
    assert!(tl
        .clips
        .windows(2)
        .all(|w| (w[0].track, w[0].start) <= (w[1].track, w[1].start)));
    // positions
    assert_eq!(map.end(), rows_to_ticks(391));
    assert_eq!(map.entry_at(rows_to_ticks(37)), (2, 5));
    assert_eq!(map.tick_of(3, 2), rows_to_ticks(44));
    // the engine counts full block lengths: entry 3 starts at line 64 there
    assert_eq!(
        map.tick_of_song_pos(3, 64.5),
        rows_to_ticks(42) as f64 + 0.5 * ROW as f64
    );
    // visible-range query
    let idx = tl.index();
    let v: Vec<Tick> = idx
        .visible(&tl, 0, rows_to_ticks(50), rows_to_ticks(60))
        .map(|i| tl.clips[i].start / ROW)
        .collect();
    assert_eq!(v, vec![42, 55]);
    assert_eq!(idx.visible(&tl, 9, 0, 100).count(), 0);
    // demo markers come from block roles
    let (dt, _) = from_order(&crate::fileio::demo_song_n(2));
    assert!(!dt.markers.is_empty());
    assert_eq!(dt.markers[0].tick, 0);
}

#[test]
fn consolidate_reproduces_the_row_stream_of_every_song() {
    let mut songs: Vec<(String, Song)> = (0..crate::fileio::DEMOS.len())
        .map(|i| {
            (
                crate::fileio::DEMOS[i].0.to_string(),
                crate::fileio::demo_song_n(i),
            )
        })
        .collect();
    songs.push(("test song".into(), test_song()));
    for f in ["chipwave-0.2.3.5-song.cwt", "chipwave-0.2.3.6-song.cwt"] {
        songs.push((f.into(), fixture(f)));
    }
    for (name, s) in &songs {
        let (tl, _) = from_order(s);
        let c = consolidated_song(s, &tl).unwrap();
        assert_eq!(row_stream(&c), row_stream(s), "{name}");
        assert!(
            c.patterns.iter().all(|p| p.rows <= Pattern::MAX_EDIT_ROWS),
            "{name}"
        );
        // shared segments: never more blocks than order entries need
        assert!(c.patterns.len() <= c.order.len(), "{name}");
        // and a second round trip is stable
        let (tl2, _) = from_order(&c);
        let c2 = consolidated_song(&c, &tl2).unwrap();
        assert_eq!(row_stream(&c2), row_stream(s), "{name} twice");
    }
}

#[test]
fn consolidate_handles_offsets_loops_overlaps_gaps_and_other_columns() {
    let s = test_song();
    let mut tl = Timeline {
        tracks: vec![Track::default(); 3],
        ..Default::default()
    };
    // track 0: block A from row 4, looping its 16 rows, for 20 rows
    tl.clips.push(Clip {
        id: 1,
        track: 0,
        start: 0,
        len: 20 * ROW,
        offset: 4 * ROW,
        loop_len: Some(16 * ROW),
        ..Default::default()
    });
    // track 1: plays column 0 of block A at row 8, overlapped by a later clip
    tl.clips.push(Clip {
        id: 2,
        track: 1,
        start: 8 * ROW,
        len: 16 * ROW,
        kind: ClipKind::Block { pattern: 0, col: 0 },
        ..Default::default()
    });
    tl.clips.push(Clip {
        id: 3,
        track: 1,
        start: 12 * ROW,
        len: 4 * ROW,
        kind: ClipKind::Block { pattern: 0, col: 1 },
        ..Default::default()
    });
    // a muted clip plays nothing
    tl.clips.push(Clip {
        id: 4,
        track: 2,
        start: 0,
        len: 16 * ROW,
        muted: true,
        kind: ClipKind::Block { pattern: 1, col: 2 },
        ..Default::default()
    });
    tl.sort();
    let c = consolidated_song(&s, &tl).unwrap();
    let st = row_stream(&c);
    // timeline is 24 rows long, padded to one 64-row block minimum
    assert_eq!(st.len(), 64);
    let a = &s.patterns[0].cells;
    for r in 0..20 {
        assert_eq!(st[r][0], a[(r + 4) % 16][0], "track 0 row {r}");
    }
    assert!(st[20..].iter().all(|row| row[0].is_empty()));
    for r in 8..24 {
        let want = if (12..16).contains(&r) {
            a[r - 12][1]
        } else {
            a[r - 8][0]
        };
        assert_eq!(st[r][1], want, "track 1 row {r}");
    }
    assert!(st.iter().all(|row| row[2].is_empty()), "muted clip");
    // off-grid clips are refused
    tl.clips[0].start = 7;
    assert!(consolidate(&s, &tl).is_err());
    // note clips cannot be consolidated yet
    tl.clips[0].start = 0;
    tl.clips[0].kind = ClipKind::Notes { data: 0 };
    assert!(consolidate(&s, &tl).is_err());
    // an empty timeline gives one empty block
    let (b, o) = consolidate(&s, &Timeline::default()).unwrap();
    assert_eq!((b.len(), o.len(), b[0].rows), (1, 1, 64));
}

/// The big one: order list -> timeline -> consolidated order list renders
/// sample-identical audio, for every demo song, the old-version fixtures
/// and the synthetic song (break, odd lengths, long block).
#[test]
fn round_trip_renders_bit_exact() {
    let mut songs: Vec<(String, Song)> = (0..crate::fileio::DEMOS.len())
        .map(|i| {
            (
                crate::fileio::DEMOS[i].0.to_string(),
                crate::fileio::demo_song_n(i),
            )
        })
        .collect();
    songs.push(("test song".into(), test_song()));
    for f in ["chipwave-0.2.3.5-song.cwt", "chipwave-0.2.3.6-song.cwt"] {
        songs.push((f.into(), fixture(f)));
    }
    std::thread::scope(|sc| {
        for (name, s) in &songs {
            sc.spawn(move || {
                let t = std::time::Instant::now();
                let (tl, _) = from_order(s);
                let c = consolidated_song(s, &tl).unwrap();
                let a = crate::engine::render_song(s, 22050);
                let b = crate::engine::render_song(&c, 22050);
                assert!(a.len() > 44100, "{name}");
                assert_eq!(a.len(), b.len(), "{name}: length");
                let first = a
                    .iter()
                    .zip(&b)
                    .position(|(x, y)| x.to_bits() != y.to_bits());
                assert_eq!(first, None, "{name}: first differing sample");
                eprintln!(
                    "{name}: {} blocks -> {} blocks, {:.1} s audio, bit-exact ({:.1} s)",
                    s.patterns.len(),
                    c.patterns.len(),
                    a.len() as f64 / 44100.0,
                    t.elapsed().as_secs_f64()
                );
            });
        }
    });
}

#[test]
fn derived_timeline_is_fast_for_big_songs() {
    // 32 channels x 400 order entries over 40 distinct blocks: ~12 800 clips
    let mut s = Song::empty();
    s.channels = 32;
    s.channel_names = (1..=32).map(|i| format!("Ch {i}")).collect();
    s.patterns = (0..40)
        .map(|b| {
            let mut p = Pattern::new(&format!("B{b}"), 64, 32);
            for r in (0..64).step_by(4) {
                for c in 0..32 {
                    if (r + c + b) % 3 == 0 {
                        p.cells[r][c] = note(30 + (c as u8));
                    }
                }
            }
            p
        })
        .collect();
    s.order = (0..400).map(|i| (i * 7) % 40).collect();
    let t = std::time::Instant::now();
    let (tl, _) = from_order(&s);
    let dt = t.elapsed();
    assert!(tl.clips.len() > 10_000);
    assert!(dt.as_millis() < 2000, "{dt:?}");
    let idx = tl.index();
    let t = std::time::Instant::now();
    let mut n = 0;
    for tr in 0..32 {
        n += idx
            .visible(&tl, tr, rows_to_ticks(5000), rows_to_ticks(5256))
            .count();
    }
    assert!(n > 0 && n < 32 * 8);
    assert!(t.elapsed().as_millis() < 50);
}

#[test]
fn lines_per_beat_is_display_only_and_never_changes_timing() {
    // a row is a 16th note whatever lines_per_beat says (the engine's
    // tick length ignores it), so seconds, bars and the timeline agree
    let mut s = crate::fileio::demo_song();
    s.lines_per_beat = 4;
    let (len4, row4, bar4) = (
        s.length_seconds(),
        s.row_seconds(),
        crate::arrange::bar_secs(&s),
    );
    s.lines_per_beat = 8;
    assert_eq!(s.length_seconds(), len4);
    assert_eq!(s.row_seconds(), row4);
    assert_eq!(crate::arrange::bar_secs(&s), bar4);
    assert_eq!(crate::arrange::rows_per_bar(&s), 16);
    assert!((bar4 - 16.0 * row4).abs() < 1e-12);
    let (tl, _) = from_order(&s);
    let rows: usize = s.order.iter().map(|&p| s.patterns[p].rows).sum();
    let secs = tl.seconds_at(rows_to_ticks(rows), s.bpm);
    assert!((secs - len4).abs() < 1e-9, "{secs} vs {len4}");
    // and what the engine actually renders: one row = tick_len * speed samples
    let sr = 48000.0;
    let per_row = sr * 60.0 / (s.bpm as f64 * 4.0);
    assert!((row4 * sr - per_row).abs() < 1e-6);
}
