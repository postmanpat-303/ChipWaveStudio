//! Audio clips (M4): spans in the plan, sample-accurate starts, playback
//! at the file's own speed at any output rate, trim / loop / split / fades
//! / gain, seeks, save and load (embedded and referenced), missing files,
//! allocation-free rendering, and songs without audio staying bit-exact.
use super::*;
use crate::audiofile::{wav_f32, Pcm};
use crate::engine::{alloc_count, render_song, Engine};
use crate::model::{Pattern, Song};

const SR: u32 = 22050;
/// 125 BPM at 22 050 Hz: a row is exactly 2 646 frames
const ROW_FRAMES: usize = 2646;

/// 2 silent channels in timeline mode (one empty block clip per track)
fn song() -> Song {
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.bpm = 125.0;
    s.speed = 6;
    s.channels = 2;
    s.channel_names = vec!["A".into(), "B".into()];
    s.patterns = vec![Pattern::new("A", 16, 2)];
    s.order = vec![0];
    s.mixer.channels.resize(2, Default::default());
    s.mixer.limiter.enabled = false;
    enter_timeline_mode(&mut s);
    s
}

fn tl(s: &mut Song) -> &mut Timeline {
    s.timeline.as_deref_mut().unwrap()
}

fn asset(name: &str, rate: u32, ch: u8, data: Vec<f32>) -> AudioAsset {
    let bytes = wav_f32(rate, ch as u16, &data);
    AudioAsset::from_pcm(name, "", "wav", Pcm::new(rate, ch, data), bytes)
}

/// an audio clip of `a` on track 1 at `start`; returns its id
fn audio_clip(s: &mut Song, start: Tick, a: AudioAsset) -> u32 {
    let k = add_audio_asset(tl(s), a);
    new_audio_clip(s, 1, start, k).unwrap()
}

fn clip(s: &mut Song, id: u32) -> &mut Clip {
    tl(s).clips.iter_mut().find(|c| c.id == id).unwrap()
}

fn run(e: &mut Engine, s: &Song, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; frames * 2];
    for c in out.chunks_mut(256) {
        e.render(s, c);
    }
    out
}

fn left(a: &[f32]) -> Vec<f32> {
    a.chunks(2).map(|f| f[0]).collect()
}

fn onset(a: &[f32]) -> usize {
    a.iter().position(|x| x.abs() > 1e-6).unwrap_or(usize::MAX)
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// rising zero crossings per second of `x` at `sr`
fn freq(x: &[f32], sr: u32) -> f64 {
    let n = x.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
    n as f64 * sr as f64 / x.len() as f64
}

fn sine(rate: u32, hz: f64, secs: f64) -> Vec<f32> {
    (0..(rate as f64 * secs) as usize)
        .map(|i| (i as f64 * hz * std::f64::consts::TAU / rate as f64).sin() as f32 * 0.5)
        .collect()
}

#[test]
fn songs_without_audio_or_with_silent_audio_clips_render_bit_exact() {
    let s0 = song();
    let a0 = render_song(&s0, SR);
    // a muted clip and a clip of a missing file change nothing
    let mut s = s0.clone();
    let id = audio_clip(&mut s, 0, asset("dc", SR, 1, vec![0.5; 4000]));
    clip(&mut s, id).muted = true;
    let mut m = asset("gone", SR, 1, vec![0.5; 4000]);
    m.pcm = None;
    let k = add_audio_asset(tl(&mut s), m);
    new_audio_clip(&mut s, 0, ROW, k).unwrap();
    let p = build_plan(&s, s.timeline.as_deref().unwrap());
    assert!(!p.has_audio());
    // the clips don't reach past the empty block, so the length is the same
    assert!(same(&render_song(&s, SR), &a0));
}

#[test]
fn a_clip_starts_on_its_frame_and_plays_the_file_exactly() {
    let mut s = song();
    // an impulse train: 1.0 every 100 frames
    let data: Vec<f32> = (0..2000)
        .map(|i| if i % 100 == 0 { 1.0 } else { 0.0 })
        .collect();
    audio_clip(&mut s, 4 * ROW, asset("clicks", SR, 1, data));
    s.mixer.channels[1].pan = 0.0;
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    let out = left(&run(&mut e, &s, 6 * ROW_FRAMES));
    let on = onset(&out);
    assert!(
        (on as i64 - 4 * ROW_FRAMES as i64).abs() <= 1,
        "clip at row 4 starts at frame {on}, not {}",
        4 * ROW_FRAMES
    );
    // the same rate: every click lands 100 frames after the last, at the
    // same level (no interpolation smear)
    let peaks: Vec<usize> = (0..out.len())
        .filter(|&i| out[i].abs() > 0.5 * out[on].abs())
        .collect();
    assert_eq!(peaks.len(), 20, "{peaks:?}");
    assert!(peaks.windows(2).all(|w| w[1] - w[0] == 100), "{peaks:?}");
    assert!(peaks.iter().all(|&i| (out[i] - out[on]).abs() < 1e-4));
    assert_eq!(e.audio_voices(1), 0, "the clip ended with the file");
}

#[test]
fn files_play_at_their_own_speed_at_any_output_rate() {
    // 441 Hz at 11 025 Hz for 1 s, rendered at 22 050 and 44 100 Hz
    for out_sr in [22050u32, 44100, 48000] {
        let mut s = song();
        let id = audio_clip(&mut s, 0, asset("tone", 11025, 1, sine(11025, 441.0, 1.0)));
        let len = clip(&mut s, id).len;
        // 1 s at 125 BPM = 2 000 ticks
        assert_eq!(len, 2000);
        let mut e = Engine::new(out_sr as f32);
        e.start(&s, 0, 0, false, 0);
        let out = left(&run(&mut e, &s, out_sr as usize * 6 / 5));
        let on = onset(&out);
        let end = out.iter().rposition(|x| x.abs() > 1e-3).unwrap();
        let secs = (end - on) as f64 / out_sr as f64;
        assert!((secs - 1.0).abs() < 0.01, "{out_sr}: {secs} s");
        let f = freq(&out[on + 100..end - 100], out_sr);
        assert!((f - 441.0).abs() < 3.0, "{out_sr}: {f} Hz");
    }
}

#[test]
fn trim_loop_split_fades_and_gain() {
    let mut s = song();
    let data: Vec<f32> = (0..ROW_FRAMES * 4)
        .map(|i| (i / ROW_FRAMES) as f32 * 0.01 + 0.01)
        .collect();
    let id = audio_clip(&mut s, 0, asset("steps", SR, 1, data));
    assert_eq!(clip(&mut s, id).len, 4 * ROW);
    // left trim by one row: the clip now starts with step 2
    set_start(tl(&mut s), id, ROW);
    let p = build_plan(&s, s.timeline.as_deref().unwrap());
    let sp = p.audio[1][0];
    assert_eq!((sp.start, sp.end), (ROW, 4 * ROW));
    assert!((sp.off - ROW_FRAMES as f64).abs() < 1e-6, "{}", sp.off);
    // drag the end past the file: it loops the whole file, frame-exact
    let s2 = {
        let mut s = s.clone();
        let snap = s.clone();
        set_end(&snap, tl(&mut s), id, 10 * ROW);
        s
    };
    let c = s2
        .timeline
        .as_deref()
        .unwrap()
        .clips
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .clone();
    assert_eq!(c.loop_len, Some(4 * ROW));
    let p = build_plan(&s2, s2.timeline.as_deref().unwrap());
    assert_eq!(p.audio[1][0].loop_len, (ROW_FRAMES * 4) as f64);
    let mut e = Engine::new(SR as f32);
    e.start(&s2, 0, 0, false, 0);
    let out = left(&run(&mut e, &s2, 10 * ROW_FRAMES));
    let lvl = |row: usize| out[row * ROW_FRAMES + ROW_FRAMES / 2];
    // rows 1..10 play steps 2,3,4,1,2,3,4,1,2 (offset 1 row, loop of 4)
    let r: Vec<f32> = (1..10).map(|k| lvl(k) / lvl(4)).collect();
    let want = [2.0, 3.0, 4.0, 1.0, 2.0, 3.0, 4.0, 1.0, 2.0];
    for (a, b) in r.iter().zip(want) {
        assert!((a - b).abs() < 0.01 * b, "{r:?}");
    }
    // split: two clips play exactly what one played
    let mut s3 = s2.clone();
    let nid = split(tl(&mut s3), id, 5 * ROW).unwrap();
    let mut e3 = Engine::new(SR as f32);
    e3.start(&s3, 0, 0, false, 0);
    let out3 = left(&run(&mut e3, &s3, 10 * ROW_FRAMES));
    for i in 0..out.len() {
        assert!((out[i] - out3[i]).abs() < 1e-5, "split differs at {i}");
    }
    assert_ne!(nid, id);
    // gain and a fade-in
    let mut s4 = s.clone();
    clip(&mut s4, id).gain = 0.5;
    set_fades(tl(&mut s4), id, ROW, 0);
    let p = build_plan(&s4, s4.timeline.as_deref().unwrap());
    let sp = p.audio[1][0];
    assert_eq!(sp.env(ROW as f64), 0.0);
    assert!((sp.env(1.5 * ROW as f64) - 0.5 * FadeShape::EqualPower.gain(0.5)).abs() < 1e-6);
    assert_eq!(sp.env(3.0 * ROW as f64), 0.5);
    // fades never overlap or leave the clip
    set_fades(tl(&mut s4), id, 100 * ROW, 100 * ROW);
    let c = clip(&mut s4, id);
    assert_eq!(c.fade_in + c.fade_out, c.len);
}

#[test]
fn fade_shapes_run_from_zero_to_one() {
    for f in FadeShape::ALL {
        assert_eq!(f.gain(0.0), 0.0, "{f:?}");
        assert!((f.gain(1.0) - 1.0).abs() < 1e-6, "{f:?}");
        let mut last = 0.0;
        for k in 1..=100 {
            let g = f.gain(k as f32 / 100.0);
            assert!(g >= last, "{f:?} not rising");
            last = g;
        }
    }
    // equal power: in^2 + out^2 = 1 through a crossfade
    let e = FadeShape::EqualPower;
    for k in 0..=10 {
        let x = k as f32 / 10.0;
        assert!((e.gain(x).powi(2) + e.gain(1.0 - x).powi(2) - 1.0).abs() < 1e-5);
    }
}

#[test]
fn seeking_into_a_clip_plays_from_the_right_frame() {
    let mut s = song();
    // a ramp: the value says where in the file we are
    let n = ROW_FRAMES * 8;
    let data: Vec<f32> = (0..n).map(|i| 0.05 * i as f32 / n as f32).collect();
    audio_clip(&mut s, 0, asset("ramp", SR, 1, data.clone()));
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 4, false, 0);
    let out = left(&run(&mut e, &s, ROW_FRAMES));
    assert_eq!(e.audio_voices(1), 1, "the clip under the playhead sounds");
    // after the de-click fade-in, the level is the file's at row 4
    let a = out[200];
    let b = out[300];
    let slope = (b - a) / 100.0;
    let at0 = a - slope * 200.0;
    assert!(
        (at0 / slope - 4.0 * ROW_FRAMES as f32).abs() < 8.0,
        "seek landed at frame {} of the file",
        at0 / slope
    );
    // stop fades out
    e.stop();
    run(&mut e, &s, 256);
    assert_eq!(e.audio_voices(1), 0);
}

#[test]
fn edits_while_playing_keep_unchanged_clips_running() {
    let mut s = song();
    let id = audio_clip(&mut s, 0, asset("dc", SR, 1, vec![0.5; ROW_FRAMES * 8]));
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    run(&mut e, &s, ROW_FRAMES);
    // a gain change rebuilds the plan; the voice plays on at the new gain
    clip(&mut s, id).gain = 0.25;
    e.ensure_plan(&s);
    let out = left(&run(&mut e, &s, ROW_FRAMES));
    assert_eq!(e.audio_voices(1), 1);
    let before = out[0];
    let after = out[ROW_FRAMES - 1];
    assert!(
        before.abs() > 1e-3 && (before - after).abs() < 1e-6,
        "{before} {after}"
    );
}

#[test]
fn overlapping_clips_mix_and_crossfade() {
    let mut s = song();
    let a = audio_clip(&mut s, 0, asset("a", SR, 1, vec![0.5; ROW_FRAMES * 4]));
    let b = audio_clip(
        &mut s,
        2 * ROW,
        asset("b", SR, 1, vec![0.5; ROW_FRAMES * 4]),
    );
    set_fades(tl(&mut s), a, 0, 2 * ROW);
    set_fades(tl(&mut s), b, 2 * ROW, 0);
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    let out = left(&run(&mut e, &s, 6 * ROW_FRAMES));
    let flat = out[ROW_FRAMES];
    // equal power: the sum dips by at most 3 dB in the middle
    for i in (2 * ROW_FRAMES + 10..4 * ROW_FRAMES - 10).step_by(50) {
        let r = out[i] / flat;
        assert!((0.99..=1.42).contains(&r), "frame {i}: {r}");
    }
}

#[test]
fn rendering_audio_clips_is_allocation_free() {
    let mut s = song();
    let id = audio_clip(&mut s, 0, asset("tone", 44100, 2, sine(44100, 300.0, 4.0)));
    clip(&mut s, id).loop_len = Some(ROW * 3);
    clip(&mut s, id).len = ROW * 64;
    set_fades(tl(&mut s), id, ROW, ROW);
    audio_clip(
        &mut s,
        2 * ROW,
        asset("t2", 8000, 1, sine(8000, 200.0, 1.0)),
    );
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 512];
    e.render(&s, &mut buf);
    let n = alloc_count::count(|| {
        for _ in 0..800 {
            e.render(&s, &mut buf);
        }
    });
    assert_eq!(n, 0, "audio clips allocated {n} times");
    assert!(buf.iter().all(|x| x.is_finite()));
}

#[test]
fn embedded_and_referenced_audio_round_trip_and_missing_files_warn() {
    let dir = std::env::temp_dir().join(format!("chipwave_m4_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = song();
    let data = sine(SR, 300.0, 0.5);
    let id = audio_clip(&mut s, ROW, asset("emb", SR, 1, data.clone()));
    clip(&mut s, id).gain = 0.7;
    clip(&mut s, id).fade_shape = FadeShape::SCurve;
    // a referenced file next to the song
    let wav = dir.join("ref.wav");
    std::fs::write(&wav, wav_f32(SR, 2, &sine(SR, 200.0, 0.25).repeat(2))).unwrap();
    let b = std::fs::read(&wav).unwrap();
    let mut r = AudioAsset::from_pcm(
        "ref.wav",
        &wav.to_string_lossy(),
        "wav",
        crate::audiofile::decode(&b, "wav").unwrap(),
        b,
    );
    r.embed = false;
    r.bytes = None;
    let k = add_audio_asset(tl(&mut s), r);
    new_audio_clip(&mut s, 0, 0, k).unwrap();
    let f = dir.join("song.cwt");
    crate::fileio::save_cwt(&s, &f).unwrap();
    let z = zip::ZipArchive::new(std::fs::File::open(&f).unwrap()).unwrap();
    let names: Vec<String> = z.file_names().map(|n| n.unwrap().to_string()).collect();
    assert!(names.iter().any(|n| n == "audio/000.wav"), "{names:?}");
    assert!(
        !names.iter().any(|n| n.starts_with("audio/001")),
        "{names:?}"
    );
    let (l, w) = crate::fileio::load_cwt_warn(&f).unwrap();
    assert!(w.is_empty(), "{w:?}");
    let (a, b2) = (
        s.timeline.as_deref().unwrap(),
        l.timeline.as_deref().unwrap(),
    );
    assert_eq!(a.clips, b2.clips);
    assert_eq!(b2.audio.len(), 2);
    assert_eq!(b2.audio[0].pcm.as_ref().unwrap().data, data);
    assert!(!b2.audio[1].missing());
    assert!(
        same(&render_song(&s, SR), &render_song(&l, SR)),
        "loaded song plays the same"
    );
    // the manifest needs reader version 3
    let mut z = zip::ZipArchive::new(std::fs::File::open(&f).unwrap()).unwrap();
    let mut m = String::new();
    std::io::Read::read_to_string(&mut z.by_name("manifest.toml").unwrap(), &mut m).unwrap();
    assert!(m.contains("min_reader_version = 3"), "{m}");
    // the referenced file moved next to the song under another folder
    let moved = dir.join("moved");
    std::fs::create_dir_all(&moved).unwrap();
    std::fs::copy(&f, moved.join("song.cwt")).unwrap();
    std::fs::rename(&wav, moved.join("ref.wav")).unwrap();
    let (l, w) = crate::fileio::load_cwt_warn(&moved.join("song.cwt")).unwrap();
    assert!(w.is_empty(), "found next to the song: {w:?}");
    assert!(!l.timeline.as_deref().unwrap().audio[1].missing());
    // gone altogether: a warning, the clip stays (silent), saving keeps it
    std::fs::remove_file(moved.join("ref.wav")).unwrap();
    let (l, w) = crate::fileio::load_cwt_warn(&moved.join("song.cwt")).unwrap();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].contains("ref.wav") && w[0].contains("missing"),
        "{w:?}"
    );
    let t = l.timeline.as_deref().unwrap();
    assert!(t.audio[1].missing());
    assert_eq!(
        t.audio[1].frames,
        s.timeline.as_deref().unwrap().audio[1].frames
    );
    assert_eq!(t.clips, s.timeline.as_deref().unwrap().clips);
    let f2 = dir.join("again.cwt");
    crate::fileio::save_cwt(&l, &f2).unwrap();
    let (l2, _) = crate::fileio::load_cwt_warn(&f2).unwrap();
    assert_eq!(l2.timeline.as_deref().unwrap().clips, t.clips);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn audio_clips_block_tracker_mode_and_survive_fix() {
    let mut s = song();
    audio_clip(&mut s, 0, asset("a", SR, 1, vec![0.1; 100]));
    let e = leave_timeline_mode(&mut s.clone()).unwrap_err();
    assert!(e.contains("audio clip"), "{e}");
    let mut t = s.timeline.as_deref().unwrap().clone();
    assert!(t.fix(&s).is_empty());
    t.audio.clear();
    assert_eq!(
        t.fix(&s).len(),
        1,
        "a clip of a missing pool entry is dropped"
    );
    // undo compares the pool by pointer, not by sample
    let a = s.timeline.as_deref().unwrap();
    assert!(a.same_content(&a.clone()));
}
