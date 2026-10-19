//! Track and clip automation (M5): innermost lane wins (clip > track >
//! song), clip lanes follow their clip (moves, trims, loops, splits),
//! curves, save / load, tracker-mode baking, export, allocation-free
//! rendering, and songs without timeline lanes staying bit-exact.
use super::*;
use crate::audiofile::{wav_f32, Pcm};
use crate::automation::{Lane, Point, Shape, Target};
use crate::engine::{alloc_count, render_song, Engine};
use crate::model::{Pattern, Song};

const SR: u32 = 22050;
/// 125 BPM at 22 050 Hz: a row is exactly 2 646 frames
const ROW_FRAMES: usize = 2646;

/// 2 channels in timeline mode, a quiet steady sine "audio clip" on track 1
/// for 64 rows (so channel 1's level is easy to read), and an empty block
/// clip on track 1 over rows 0..16
fn song() -> Song {
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.modulation = Default::default();
    s.bpm = 125.0;
    s.speed = 6;
    s.channels = 2;
    s.channel_names = vec!["A".into(), "B".into()];
    s.patterns = vec![Pattern::new("A", 16, 2)];
    s.order = vec![0];
    s.mixer.channels.resize(2, Default::default());
    s.mixer.channels[1].volume = 1.0;
    s.mixer.limiter.enabled = false;
    enter_timeline_mode(&mut s);
    // 441 Hz: exactly 50 frames per cycle
    let data: Vec<f32> = (0..SR as usize)
        .map(|i| (i as f64 * std::f64::consts::TAU / 50.0).sin() as f32 * 0.05)
        .collect();
    let a = AudioAsset::from_pcm(
        "dc",
        "",
        "wav",
        Pcm::new(SR, 1, data.clone()),
        wav_f32(SR, 1, &data),
    );
    let k = add_audio_asset(tl(&mut s), a);
    let id = new_audio_clip(&mut s, 1, 0, k).unwrap();
    let c = tl(&mut s).clips.iter_mut().find(|c| c.id == id).unwrap();
    c.loop_len = Some(ROW * 8);
    c.len = ROW * 64;
    tl(&mut s).end = ROW * 64;
    tl(&mut s).clips.retain(|c| c.audio().is_some());
    let id = tl(&mut s).new_id();
    tl(&mut s).clips.push(Clip {
        id,
        track: 1,
        start: 0,
        len: 16 * ROW,
        kind: ClipKind::Block { pattern: 0, col: 1 },
        ..Default::default()
    });
    tl(&mut s).sort();
    s
}

fn tl(s: &mut Song) -> &mut Timeline {
    s.timeline.as_deref_mut().unwrap()
}

/// the empty block clip on track 1 (rows 0..16)
fn block_clip(s: &mut Song) -> &mut Clip {
    tl(s)
        .clips
        .iter_mut()
        .find(|c| c.track == 1 && c.block().is_some())
        .unwrap()
}

fn pt(pos: f32, value: f32, shape: Shape) -> Point {
    Point {
        pos,
        value,
        shape,
        curve: 0.0,
    }
}

/// a channel-1 volume lane: (row, linear gain) steps
fn vol_lane(steps: &[(f32, f32)]) -> Lane {
    let t = Target::ChanVolume(1);
    let mut l = Lane::new(t, "vol".into());
    l.points = steps
        .iter()
        .map(|&(r, g)| pt(r, t.to_norm(g), Shape::Step))
        .collect();
    l
}

fn run(s: &Song, frames: usize) -> Vec<f32> {
    let mut e = Engine::new(SR as f32);
    e.start(s, 0, 0, false, 0);
    let mut out = vec![0.0f32; frames * 2];
    for c in out.chunks_mut(256) {
        e.render(s, c);
    }
    out.chunks(2).map(|f| f[0]).collect()
}

/// channel 1's peak level in the middle half of row `r`
fn level(out: &[f32], r: usize) -> f32 {
    let a = r * ROW_FRAMES + ROW_FRAMES / 4;
    out[a..a + ROW_FRAMES / 2]
        .iter()
        .fold(0.0f32, |m, x| m.max(x.abs()))
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn songs_without_timeline_lanes_render_bit_exact() {
    let s0 = song();
    let a0 = render_song(&s0, SR);
    // a disabled lane, an empty lane and a lane on a muted clip: no layers
    let mut s = s0.clone();
    let mut off = vol_lane(&[(0.0, 0.1)]);
    off.enabled = false;
    tl(&mut s).tracks[1].auto.push(off);
    tl(&mut s).tracks[0]
        .auto
        .push(Lane::new(Target::ChanPan(0), String::new()));
    let mut b = s.clone();
    block_clip(&mut b).muted = true;
    block_clip(&mut b).auto.push(vol_lane(&[(0.0, 0.1)]));
    for s in [&s, &b] {
        assert!(build_auto(s, s.timeline.as_deref().unwrap()).is_empty());
        assert!(same(&render_song(s, SR), &a0));
    }
}

#[test]
fn a_track_lane_drives_its_target() {
    let mut s = song();
    let flat = run(&s, 4 * ROW_FRAMES);
    tl(&mut s).tracks[1]
        .auto
        .push(vol_lane(&[(0.0, 1.0), (8.0, 0.25)]));
    let out = run(&s, 12 * ROW_FRAMES);
    let g = level(&flat, 2);
    assert!(g > 0.01, "{g}");
    assert!((level(&out, 2) / g - 1.0).abs() < 1e-3);
    assert!(
        (level(&out, 10) / g - 0.25).abs() < 1e-3,
        "{}",
        level(&out, 10) / g
    );
}

#[test]
fn innermost_lane_wins_clip_over_track_over_song() {
    let mut s = song();
    let g = level(&run(&s, 4 * ROW_FRAMES), 2);
    s.automation.push(vol_lane(&[(0.0, 0.1)]));
    let out = run(&s, 20 * ROW_FRAMES);
    assert!((level(&out, 2) / g - 0.1).abs() < 1e-3, "song lane alone");
    tl(&mut s).tracks[1].auto.push(vol_lane(&[(0.0, 0.5)]));
    // the block clip (rows 0..16) loops every 8 rows: 4 rows quiet, 4 loud
    {
        let c = block_clip(&mut s);
        c.loop_len = Some(8 * ROW);
        c.auto.push(vol_lane(&[(0.0, 0.25), (4.0, 1.0)]));
    }
    let out = run(&s, 20 * ROW_FRAMES);
    let r = |k: usize| level(&out, k) / g;
    for (row, want) in [(1, 0.25), (5, 1.0), (9, 0.25), (13, 1.0), (17, 0.5)] {
        assert!(
            (r(row) - want).abs() < 1e-3,
            "row {row}: {} not {want}",
            r(row)
        );
    }
    // moving the clip by 4 rows moves its automation with it
    let id = block_clip(&mut s).id;
    move_clips(tl(&mut s), &[id], 4 * ROW, 0, 2);
    let out = run(&s, 24 * ROW_FRAMES);
    let r = |k: usize| level(&out, k) / g;
    for (row, want) in [
        (1, 0.5),
        (5, 0.25),
        (9, 1.0),
        (13, 0.25),
        (17, 1.0),
        (21, 0.5),
    ] {
        assert!(
            (r(row) - want).abs() < 1e-3,
            "moved, row {row}: {} not {want}",
            r(row)
        );
    }
    // a muted clip's lane is off: the track lane plays
    block_clip(&mut s).muted = true;
    let out = run(&s, 12 * ROW_FRAMES);
    assert!((level(&out, 6) / g - 0.5).abs() < 1e-3);
}

#[test]
fn clip_lanes_follow_trims_and_splits() {
    let mut s = song();
    {
        let c = block_clip(&mut s);
        let mut l = vol_lane(&[]);
        l.points = vec![pt(0.0, 0.0, Shape::Linear), pt(16.0, 1.0, Shape::Linear)];
        c.auto.push(l);
    }
    let plan = |s: &Song| build_auto(s, s.timeline.as_deref().unwrap());
    let before = plan(&s);
    let vals = |p: &AutoPlan| -> Vec<Option<f32>> {
        (0..16 * 4)
            .map(|q| p.value(0, (q * ROW / 4) as f64))
            .collect()
    };
    let v0 = vals(&before);
    // split at row 6: both halves keep the lane, the values don't change
    let id = block_clip(&mut s).id;
    let other = split(tl(&mut s), id, 6 * ROW).unwrap();
    assert!(tl(&mut s)
        .clips
        .iter()
        .any(|c| c.id == other && !c.auto.is_empty()));
    assert_eq!(vals(&plan(&s)), v0);
    // trimming the start keeps the lane on the source
    set_start(tl(&mut s), id, 2 * ROW);
    let after = vals(&plan(&s));
    assert_eq!(after[..8], [None; 8]);
    assert_eq!(after[8..], v0[8..]);
}

#[test]
fn curves_and_targets_of_every_kind_compile() {
    let mut s = song();
    let mut l = vol_lane(&[]);
    l.points = vec![
        Point {
            curve: 0.7,
            ..pt(0.0, 0.0, Shape::Linear)
        },
        pt(8.0, 1.0, Shape::Linear),
    ];
    tl(&mut s).tracks[1].auto.push(l);
    for id in [
        "instrument.1.fx.1.p0",
        "instrument.1.cutoff",
        "instrument.1.gen.2",
        "instrument.1.macro.3",
        "mixer.ch.1.send_a",
        "mixer.return_b.mix",
        "mod.1.amount",
        "tempo",
        "nonsense",
    ] {
        let mut l = Lane::new(Target::Master, String::new());
        l.target = id.into();
        l.points = vec![pt(0.0, 0.5, Shape::Linear)];
        tl(&mut s).tracks[0].auto.push(l);
    }
    let p = build_auto(&s, s.timeline.as_deref().unwrap());
    // tempo is allowed on track / Song-Master lanes (design A); "nonsense" is dropped
    assert!(p.targets.contains(&Target::Tempo));
    assert!(p.targets.contains(&Target::ModAmount(0)));
    assert!(p.targets.contains(&Target::Fx(0, 0, 0)));
    assert_eq!(p.layers.len(), p.targets.len());
    let k = p.slot(Target::ChanVolume(1)).unwrap();
    let mid = p.value(k, 4.0 * ROW as f64).unwrap();
    assert!(mid > 0.5 + 0.1, "bent upwards: {mid}");
    // a curved segment plays the curve: louder than the straight one
    let mut straight = s.clone();
    tl(&mut straight).tracks[1].auto[0].points[0].curve = 0.0;
    let bent = level(&run(&s, 6 * ROW_FRAMES), 4);
    let flat = level(&run(&straight, 6 * ROW_FRAMES), 4);
    assert!(bent > flat * 1.15, "{bent} vs {flat}");
}

#[test]
fn rendering_timeline_lanes_is_allocation_free() {
    let mut s = song();
    tl(&mut s).tracks[1]
        .auto
        .push(vol_lane(&[(0.0, 1.0), (3.0, 0.3), (7.0, 1.2)]));
    let mut pan = Lane::new(Target::ChanPan(1), String::new());
    pan.points = (0..32)
        .map(|i| pt(i as f32, (i % 2) as f32, Shape::Smooth))
        .collect();
    tl(&mut s).tracks[1].auto.push(pan);
    block_clip(&mut s)
        .auto
        .push(vol_lane(&[(0.0, 0.25), (4.0, 1.0)]));
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 512];
    e.render(&s, &mut buf);
    let n = alloc_count::count(|| {
        for _ in 0..800 {
            e.render(&s, &mut buf);
        }
    });
    assert_eq!(n, 0, "timeline lanes allocated {n} times");
    assert!(buf.iter().all(|x| x.is_finite()));
}

#[test]
fn lanes_save_load_and_need_reader_version_4() {
    let dir = std::env::temp_dir().join(format!("chipwave_m5_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = song();
    tl(&mut s).tracks[1]
        .auto
        .push(vol_lane(&[(0.0, 1.0), (8.0, 0.25)]));
    tl(&mut s).tracks[1].auto[0].points[0].curve = -0.4;
    block_clip(&mut s).auto.push(vol_lane(&[(0.0, 0.5)]));
    tl(&mut s).view.auto_open = 0b10;
    let f = dir.join("lanes.cwt");
    crate::fileio::save_cwt(&s, &f).unwrap();
    let (l, w) = crate::fileio::load_cwt_warn(&f).unwrap();
    assert!(w.is_empty(), "{w:?}");
    let (a, b) = (
        s.timeline.as_deref().unwrap(),
        l.timeline.as_deref().unwrap(),
    );
    assert_eq!(a.tracks[1].auto, b.tracks[1].auto);
    assert_eq!(a.clips, b.clips);
    assert_eq!(b.view.auto_open, 0b10);
    assert!(same(&render_song(&s, SR), &render_song(&l, SR)));
    let mut z = zip::ZipArchive::new(std::fs::File::open(&f).unwrap()).unwrap();
    let mut m = String::new();
    std::io::Read::read_to_string(&mut z.by_name("manifest.toml").unwrap(), &mut m).unwrap();
    assert!(m.contains("min_reader_version = 4"), "{m}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn tracker_mode_bakes_track_and_clip_lanes_into_song_lanes() {
    let mut s = song();
    // tracker mode can't hold the audio clip: keep only blocks
    tl(&mut s).clips.retain(|c| c.audio().is_none());
    tl(&mut s).audio.clear();
    tl(&mut s).end = 0;
    s.automation.push(vol_lane(&[(0.0, 0.1)]));
    let mut ramp = vol_lane(&[]);
    ramp.points = vec![
        pt(2.0, 0.2, Shape::Linear),
        pt(10.0, 0.9, Shape::Smooth),
        pt(14.0, 0.3, Shape::Linear),
    ];
    tl(&mut s).tracks[1].auto.push(ramp);
    {
        let c = block_clip(&mut s);
        c.start = 4 * ROW;
        c.len = 4 * ROW;
        c.auto.push(vol_lane(&[(0.0, 0.7), (2.0, 0.6)]));
    }
    tl(&mut s).sort();
    let t = s.timeline.as_deref().unwrap().clone();
    let plan = build_auto(&s, &t);
    let outer = s.automation[0].points.clone();
    let mut b = s.clone();
    leave_timeline_mode(&mut b).unwrap();
    assert!(b.timeline.is_none());
    assert_eq!(b.automation.len(), 1, "the song lane was replaced");
    let baked = &b.automation[0].points;
    for q in 0..16 * 4 {
        let tk = q * ROW / 4;
        let want = plan
            .value(0, tk as f64)
            .or_else(|| crate::automation::value_at(&outer, (tk / ROW) as f32))
            .unwrap();
        let got = crate::automation::value_at(baked, tk as f32 / ROW as f32).unwrap();
        // exact on rows; between rows a smooth segment is sampled per row
        let tol = if tk % ROW == 0 { 1e-5 } else { 0.05 };
        assert!(
            (got - want).abs() < tol,
            "tick {tk}: {got} vs {want} {baked:?}"
        );
    }
}

#[test]
fn export_plays_timeline_lanes() {
    let mut s = song();
    tl(&mut s).tracks[1]
        .auto
        .push(vol_lane(&[(0.0, 1.0), (8.0, 0.0)]));
    let o = crate::export::Opts {
        format: crate::export::Format::Wav,
        sample_rate: SR,
        tail_secs: 0.0,
        normalise: false,
        limiter: false,
        ..Default::default()
    };
    let out = crate::export::render(&s, &o, &Default::default()).unwrap();
    let l: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
    assert!(level(&l, 2) > 0.01);
    assert!(level(&l, 12) < 1e-5, "{}", level(&l, 12));
}

#[test]
fn deleting_an_instrument_renumbers_timeline_lanes() {
    let mut s = song();
    while s.instruments.len() < 3 {
        s.instruments.push(s.instruments[0].clone());
    }
    let mut l = Lane::new(Target::Cutoff(2), String::new());
    l.points = vec![pt(0.0, 0.5, Shape::Linear)];
    tl(&mut s).tracks[0].auto.push(l.clone());
    let mut gone = l.clone();
    gone.target = Target::Cutoff(0).id();
    block_clip(&mut s).auto.push(gone);
    s.delete_instrument(0);
    let t = s.timeline.as_deref().unwrap();
    assert_eq!(t.tracks[0].auto[0].target, Target::Cutoff(1).id());
    assert!(t.clips.iter().all(|c| c.auto.is_empty()));
}

#[test]
fn glue_keeps_clip_automation() {
    use crate::timeline::edit::glue;
    let mut s = crate::fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let (id0, id1, pts) = {
        let tl = s.timeline.as_deref_mut().unwrap();
        let clips: Vec<_> = tl
            .clips
            .iter()
            .filter(|c| c.track == 0 && c.block().is_some())
            .take(2)
            .cloned()
            .collect();
        assert!(clips.len() >= 2, "need two block clips");
        let id0 = clips[0].id;
        let id1 = clips[1].id;
        let mut lane = vol_lane(&[(0.0, 0.2), (4.0, 0.8)]);
        lane.mode = crate::automation::ClipMode::Absolute;
        let pts = lane.points.clone();
        if let Some(c) = tl.clips.iter_mut().find(|c| c.id == id0) {
            c.auto = vec![lane];
        }
        (id0, id1, pts)
    };
    let nid = glue(&mut s, &[id0, id1]).expect("glue");
    let tl = s.timeline.as_deref().unwrap();
    let c = tl.clips.iter().find(|c| c.id == nid).expect("glued clip");
    assert!(
        !c.auto.is_empty(),
        "glued clip should keep automation lanes"
    );
    assert_eq!(c.auto[0].points.len(), pts.len());
}

#[test]
fn relative_clip_mode_combines_with_track() {
    use crate::automation::{ClipMode, Point, Shape, Target};
    use crate::timeline::autoplan::build_auto;
    let mut s = crate::fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let start = {
        let tl = s.timeline.as_deref_mut().unwrap();
        let mut track = Lane::new(Target::ChanVolume(0), "vol".into());
        track.points = vec![
            Point {
                pos: 0.0,
                value: 0.5,
                shape: Shape::Linear,
                curve: 0.0,
            },
            Point {
                pos: 64.0,
                value: 0.5,
                shape: Shape::Linear,
                curve: 0.0,
            },
        ];
        tl.tracks[0].auto = vec![track];
        let clip = tl.clips.iter_mut().find(|c| c.track == 0).unwrap();
        let mut cl = Lane::new(Target::ChanVolume(0), "vol".into());
        cl.mode = ClipMode::RelMul;
        cl.points = vec![
            Point {
                pos: 0.0,
                value: 0.5,
                shape: Shape::Linear,
                curve: 0.0,
            },
            Point {
                pos: (clip.len / crate::timeline::ROW) as f32,
                value: 0.5,
                shape: Shape::Linear,
                curve: 0.0,
            },
        ];
        let start = clip.start;
        clip.auto = vec![cl];
        start
    };
    let plan = build_auto(&s, s.timeline.as_deref().unwrap());
    let slot = plan.slot(Target::ChanVolume(0)).unwrap();
    let v = plan.value(slot, start as f64 + 1.0).unwrap();
    assert!((v - 0.25).abs() < 1e-4, "got {v}");
}

#[test]
fn absolute_clip_mode_stays_bit_exact_with_innermost() {
    use crate::automation::{ClipMode, Point, Shape, Target};
    use crate::timeline::autoplan::build_auto;
    let mut s = crate::fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let start = {
        let tl = s.timeline.as_deref_mut().unwrap();
        let mut track = Lane::new(Target::ChanVolume(0), "vol".into());
        track.points = vec![Point {
            pos: 0.0,
            value: 0.1,
            shape: Shape::Step,
            curve: 0.0,
        }];
        tl.tracks[0].auto = vec![track];
        let clip = tl.clips.iter_mut().find(|c| c.track == 0).unwrap();
        let mut cl = Lane::new(Target::ChanVolume(0), "vol".into());
        cl.mode = ClipMode::Absolute;
        cl.points = vec![Point {
            pos: 0.0,
            value: 0.9,
            shape: Shape::Step,
            curve: 0.0,
        }];
        let start = clip.start;
        clip.auto = vec![cl];
        start
    };
    let plan = build_auto(&s, s.timeline.as_deref().unwrap());
    let slot = plan.slot(Target::ChanVolume(0)).unwrap();
    let v = plan.value(slot, start as f64 + 1.0).unwrap();
    assert_eq!(v.to_bits(), 0.9f32.to_bits());
}
