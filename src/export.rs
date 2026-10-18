//! Audio export (docs/EXPORT.md): WAV and MP3 (LAME via mp3lame-encoder,
//! ID3v2.4 tags via id3), scope song / block / selection, normalise, limiter,
//! tail, progress + cancel. Rendering uses the same engine as playback.
use crate::model::Song;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Wav,
    Mp3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mp3Mode {
    /// constant bitrate in kbps (128 / 192 / 256 / 320)
    Cbr(u16),
    /// LAME VBR quality V0 (best) .. V9
    Vbr(u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scope {
    Song,
    /// one block (pattern index), played once
    Block(usize),
    /// order positions a..=b (arranger selection)
    Range(usize, usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Opts {
    pub format: Format,
    pub mp3: Mp3Mode,
    pub sample_rate: u32,
    /// normalise: true peak to the master limiter ceiling, or (limiter
    /// bypassed) sample peak to -0.3 dBFS
    pub normalise: bool,
    /// extra soft limiter (-0.3 dBFS), only used when the song's master
    /// limiter is bypassed; the master limiter itself is part of the render
    pub limiter: bool,
    /// seconds rendered after the end (release / delay / reverb tails)
    pub tail_secs: f32,
    pub scope: Scope,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            format: Format::Mp3,
            mp3: Mp3Mode::Vbr(2),
            sample_rate: 44100,
            normalise: false,
            limiter: true,
            tail_secs: 2.0,
            scope: Scope::Song,
        }
    }
}

/// Shared between the export thread and the UI.
#[derive(Default)]
pub struct Progress {
    frac: AtomicU32,
    pub cancel: AtomicBool,
}

impl Progress {
    pub fn get(&self) -> f32 {
        f32::from_bits(self.frac.load(Ordering::Relaxed))
    }
    fn set(&self, f: f32) {
        self.frac
            .store(f.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

pub const CANCELLED: &str = "cancelled";
const CEIL: f32 = 0.966; // -0.3 dBFS

/// The song as it should be rendered for a scope.
pub fn scoped(song: &Song, scope: Scope) -> Song {
    let mut s = song.clone();
    match scope {
        Scope::Song => {}
        Scope::Block(p) => s.order = vec![p.min(s.patterns.len().saturating_sub(1))],
        Scope::Range(a, b) => {
            let n = s.order.len();
            let (a, b) = (a.min(b).min(n - 1), a.max(b).min(n - 1));
            s.order = s.order[a..=b].to_vec();
        }
    }
    s
}

/// Render interleaved stereo for the options (progress 0..0.8).
pub fn render(song: &Song, o: &Opts, pr: &Progress) -> Result<Vec<f32>, String> {
    let s = scoped(song, o.scope);
    let sr = o.sample_rate;
    let mut e = crate::engine::Engine::new(sr as f32);
    // plugin instruments: fresh instances with the song's saved state
    crate::plugin::prepare(&mut e.plugins, &s, sr as f32);
    e.start(&s, 0, 0, false, 0);
    let est = (s.length_seconds() + o.tail_secs as f64).max(0.1) * sr as f64 * 2.0;
    let max =
        ((s.length_seconds() + 30.0) * sr as f64 * 2.0).min(60.0 * 60.0 * sr as f64 * 2.0) as usize;
    let mut out = Vec::with_capacity(est as usize + 4096);
    let mut buf = vec![0.0f32; 1024];
    while !e.song_ended && out.len() < max {
        e.render(&s, &mut buf);
        out.extend_from_slice(&buf);
        if out.len() % (1024 * 64) == 0 {
            if pr.cancelled() {
                return Err(CANCELLED.into());
            }
            pr.set(0.8 * (out.len() as f64 / est) as f32);
        }
    }
    e.playing = false;
    let tail = (o.tail_secs.clamp(0.0, 30.0) * sr as f32) as usize * 2;
    let end = out.len() + tail;
    while out.len() < end {
        e.render(&s, &mut buf);
        let n = (end - out.len()).min(buf.len());
        out.extend_from_slice(&buf[..n]);
    }
    // short fade over the last 50 ms so the cut is silent
    let f = ((sr as usize / 20) * 2).min(out.len());
    let len = out.len();
    for (k, x) in out[len - f..].iter_mut().enumerate() {
        *x *= 1.0 - k as f32 / f as f32;
    }
    let ml = s.mixer.limiter;
    post(
        &mut out,
        o,
        ml.enabled.then(|| crate::master::db_to_lin(ml.ceiling_db)),
    );
    pr.set(0.8);
    Ok(out)
}

/// normalise / limit in place. `master_ceiling` = the song's master limiter
/// ceiling (linear, true peak) when it is on: the render already went
/// through it, so only normalisation (by true peak) is left to do.
pub fn post(d: &mut [f32], o: &Opts, master_ceiling: Option<f32>) {
    if let Some(ceil) = master_ceiling {
        if o.normalise {
            let tp = crate::master::true_peak(d);
            if tp > 1e-6 {
                let g = ceil / tp;
                d.iter_mut().for_each(|x| *x *= g);
            }
        }
        return;
    }
    if o.limiter {
        // soft knee above -6 dB, never past the ceiling
        for x in d.iter_mut() {
            let a = x.abs();
            if a > 0.5 {
                let y = 0.5 + (CEIL - 0.5) * ((a - 0.5) / (CEIL - 0.5)).tanh();
                *x = y.copysign(*x);
            }
        }
    }
    if o.normalise {
        let pk = d.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        if pk > 1e-6 {
            let g = CEIL / pk;
            d.iter_mut().for_each(|x| *x *= g);
        }
    }
}

pub fn encode_mp3(d: &[f32], sr: u32, mode: Mp3Mode, pr: &Progress) -> Result<Vec<u8>, String> {
    use mp3lame_encoder::{Bitrate, Builder, FlushNoGap, InterleavedPcm, Quality, VbrMode};
    let mut b = Builder::new().ok_or("LAME init failed")?;
    let es = |e: mp3lame_encoder::BuildError| format!("LAME: {e:?}");
    b.set_num_channels(2).map_err(es)?;
    b.set_sample_rate(sr).map_err(es)?;
    b.set_quality(Quality::NearBest).map_err(es)?;
    match mode {
        Mp3Mode::Cbr(k) => {
            b.set_vbr_mode(VbrMode::Off).map_err(es)?;
            b.set_brate(match k {
                128 => Bitrate::Kbps128,
                192 => Bitrate::Kbps192,
                256 => Bitrate::Kbps256,
                _ => Bitrate::Kbps320,
            })
            .map_err(es)?;
        }
        Mp3Mode::Vbr(q) => {
            b.set_vbr_mode(VbrMode::Mtrh).map_err(es)?;
            b.set_vbr_quality(match q {
                0 => Quality::Best,
                1 => Quality::SecondBest,
                2 => Quality::NearBest,
                3 => Quality::VeryNice,
                4 => Quality::Nice,
                5 => Quality::Good,
                6 => Quality::Decent,
                7 => Quality::Ok,
                8 => Quality::SecondWorst,
                _ => Quality::Worst,
            })
            .map_err(es)?;
        }
    }
    b.set_to_write_vbr_tag(false).map_err(es)?;
    let mut enc = b.build().map_err(es)?;
    let mut out = Vec::with_capacity(d.len() / 4);
    let chunk = 1152 * 2 * 32;
    let mut pcm = Vec::with_capacity(chunk);
    for (k, c) in d.chunks(chunk).enumerate() {
        if pr.cancelled() {
            return Err(CANCELLED.into());
        }
        pcm.clear();
        pcm.extend(c.iter().map(|x| (x.clamp(-1.0, 1.0) * 32767.0) as i16));
        out.reserve(mp3lame_encoder::max_required_buffer_size(pcm.len() / 2));
        enc.encode_to_vec(InterleavedPcm(&pcm[..]), &mut out)
            .map_err(|e| format!("encode: {e:?}"))?;
        pr.set(0.8 + 0.18 * ((k * chunk) as f32 / d.len().max(1) as f32));
    }
    out.reserve(7200);
    enc.flush_to_vec::<FlushNoGap>(&mut out)
        .map_err(|e| format!("flush: {e:?}"))?;
    Ok(out)
}

/// ID3v2.4 tags from the song info.
pub fn write_tags(path: &Path, song: &Song) -> Result<(), String> {
    use id3::TagLike;
    let mut t = id3::Tag::new();
    if !song.title.is_empty() {
        t.set_title(song.title.clone());
    }
    if !song.info.artist.is_empty() {
        t.set_artist(song.info.artist.clone());
    }
    if !song.info.album.is_empty() {
        t.set_album(song.info.album.clone());
    }
    if let Ok(y) = song.info.year.trim().parse::<i32>() {
        t.set_year(y);
    }
    let c = if song.info.comment.trim().is_empty() {
        "Made with ChipWave".to_string()
    } else {
        format!("{} - Made with ChipWave", song.info.comment.trim())
    };
    t.add_frame(id3::frame::Comment {
        lang: "eng".into(),
        description: String::new(),
        text: c,
    });
    t.set_text("TSSE", format!("ChipWave Studio {}", crate::VERSION));
    t.write_to_path(path, id3::Version::Id3v24)
        .map_err(|e| e.to_string())
}

/// Full export to `path`; returns a status line.
pub fn export(song: &Song, o: &Opts, path: &Path, pr: &Progress) -> Result<String, String> {
    let d = render(song, o, pr)?;
    let secs = d.len() as f32 / 2.0 / o.sample_rate as f32;
    let lufs = crate::master::integrated_lufs(&d, o.sample_rate as f32);
    let tp = crate::master::lin_to_db(crate::master::true_peak(&d));
    let tmp = path.with_extension("part");
    match o.format {
        Format::Wav => crate::engine::write_wav(&tmp, &d, o.sample_rate)?,
        Format::Mp3 => {
            let bytes = encode_mp3(&d, o.sample_rate, o.mp3, pr)?;
            std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
            write_tags(&tmp, song)?;
        }
    }
    if pr.cancelled() {
        let _ = std::fs::remove_file(&tmp);
        return Err(CANCELLED.into());
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    pr.set(1.0);
    let lufs = if lufs.is_finite() {
        format!("{lufs:.1} LUFS")
    } else {
        "silent".into()
    };
    Ok(format!(
        "Exported {:.1} s to {} ({lufs}, true peak {tp:.1} dBTP)",
        secs,
        path.display()
    ))
}

/// Export on a background thread. Poll `Progress`; the result arrives on the receiver.
pub fn spawn(
    song: Song,
    o: Opts,
    path: std::path::PathBuf,
) -> (
    Arc<Progress>,
    std::sync::mpsc::Receiver<Result<String, String>>,
) {
    let pr = Arc::new(Progress::default());
    let (tx, rx) = std::sync::mpsc::channel();
    let p2 = pr.clone();
    std::thread::spawn(move || {
        let r = export(&song, &o, &path, &p2);
        if r.is_err() {
            let _ = std::fs::remove_file(path.with_extension("part"));
        }
        let _ = tx.send(r);
    });
    (pr, rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(path: &Path) -> (u32, usize, f32) {
        use symphonia::core::codecs::DecoderOptions;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;
        use symphonia::core::probe::Hint;
        let f = std::fs::File::open(path).unwrap();
        let mss = MediaSourceStream::new(Box::new(f), Default::default());
        let mut h = Hint::new();
        h.with_extension("mp3");
        let probed = symphonia::default::get_probe()
            .format(
                &h,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .unwrap();
        let mut fmt = probed.format;
        let tr = fmt.default_track().unwrap();
        let sr = tr.codec_params.sample_rate.unwrap();
        let id = tr.id;
        let mut dec = symphonia::default::get_codecs()
            .make(&tr.codec_params, &DecoderOptions::default())
            .unwrap();
        let (mut frames, mut peak) = (0usize, 0.0f32);
        while let Ok(p) = fmt.next_packet() {
            if p.track_id() != id {
                continue;
            }
            let buf = dec.decode(&p).unwrap();
            let mut sb = symphonia::core::audio::SampleBuffer::<f32>::new(
                buf.capacity() as u64,
                *buf.spec(),
            );
            sb.copy_interleaved_ref(buf);
            frames += sb.samples().len() / 2;
            peak = sb.samples().iter().fold(peak, |m, x| m.max(x.abs()));
        }
        (sr, frames, peak)
    }

    #[test]
    fn mp3_export_decodes_and_has_tags() {
        let mut song = crate::fileio::demo_song();
        song.title = "Test Song".into();
        song.info.artist = "Del".into();
        song.info.album = "Demos".into();
        song.info.year = "2026".into();
        for (k, o) in [
            Opts {
                scope: Scope::Block(0),
                tail_secs: 0.5,
                ..Default::default()
            },
            Opts {
                scope: Scope::Block(0),
                mp3: Mp3Mode::Cbr(192),
                sample_rate: 48000,
                normalise: true,
                tail_secs: 0.5,
                ..Default::default()
            },
        ]
        .into_iter()
        .enumerate()
        {
            let p = std::env::temp_dir().join(format!("chipwave_test_{k}.mp3"));
            let pr = Progress::default();
            let msg = export(&song, &o, &p, &pr).unwrap();
            assert!(msg.contains("Exported"));
            assert!((pr.get() - 1.0).abs() < 1e-6);
            let (sr, frames, peak) = decode(&p);
            assert_eq!(sr, o.sample_rate);
            let want = scoped(&song, o.scope).length_seconds() as f32 + o.tail_secs;
            let got = frames as f32 / sr as f32;
            assert!((got - want).abs() < 0.2, "length {got} vs {want}");
            assert!(peak > 0.05 && peak <= 1.01, "peak {peak}");
            let t = id3::Tag::read_from_path(&p).unwrap();
            use id3::TagLike;
            assert_eq!(t.title(), Some("Test Song"));
            assert_eq!(t.artist(), Some("Del"));
            assert_eq!(t.album(), Some("Demos"));
            assert_eq!(t.year(), Some(2026));
            assert!(t.comments().any(|c| c.text.contains("Made with ChipWave")));
            if let Some(dst) = std::env::var_os("CHIPWAVE_MP3_OUT") {
                let _ = std::fs::copy(&p, std::path::Path::new(&dst).join(format!("test_{k}.mp3")));
            }
        }
    }

    #[test]
    fn wav_export_scopes_and_cancel() {
        let song = crate::fileio::demo_song();
        let p = std::env::temp_dir().join("chipwave_test_scope.wav");
        let o = Opts {
            format: Format::Wav,
            scope: Scope::Range(0, 0),
            tail_secs: 0.0,
            sample_rate: 22050,
            ..Default::default()
        };
        export(&song, &o, &p, &Progress::default()).unwrap();
        let r = hound::WavReader::open(&p).unwrap();
        let secs = r.duration() as f32 / 22050.0;
        let want = scoped(&song, Scope::Range(0, 0)).length_seconds() as f32;
        assert!((secs - want).abs() < 0.1, "{secs} vs {want}");
        // cancelled before it starts: error and no file
        let pr = Progress::default();
        pr.cancel.store(true, Ordering::Relaxed);
        let p2 = std::env::temp_dir().join("chipwave_test_cancel.wav");
        let _ = std::fs::remove_file(&p2);
        let long = Opts {
            scope: Scope::Song,
            ..o
        };
        assert_eq!(export(&song, &long, &p2, &pr).unwrap_err(), CANCELLED);
        assert!(!p2.exists());
    }

    #[test]
    fn normalise_and_limiter() {
        let mut d = vec![0.1, -0.2, 0.05];
        post(
            &mut d,
            &Opts {
                normalise: true,
                limiter: false,
                ..Default::default()
            },
            None,
        );
        assert!((d[1].abs() - CEIL).abs() < 1e-4);
        let mut d = vec![3.0, -5.0, 0.3];
        post(
            &mut d,
            &Opts {
                normalise: false,
                limiter: true,
                ..Default::default()
            },
            None,
        );
        assert!(d.iter().all(|x| x.abs() <= CEIL + 1e-6));
        assert_eq!(d[2], 0.3);
        // master limiter on: the soft limiter is skipped, normalise is by
        // true peak to the ceiling
        let ceil = crate::master::db_to_lin(-1.0);
        let mut d: Vec<f32> = (0..2000)
            .flat_map(|i| {
                let v = (i as f32 * 0.3).sin() * 0.2;
                [v, v]
            })
            .collect();
        post(&mut d, &Opts::default(), Some(ceil));
        assert!(d.iter().all(|x| x.abs() <= 0.2 + 1e-6), "untouched");
        post(
            &mut d,
            &Opts {
                normalise: true,
                ..Default::default()
            },
            Some(ceil),
        );
        let tp = crate::master::true_peak(&d);
        assert!((tp - ceil).abs() < 1e-3, "{tp}");
    }

    /// export goes through the same engine and master limiter as playback:
    /// rendering with realtime-sized callbacks gives the same samples
    #[test]
    fn export_matches_realtime_rendering() {
        let mut song = crate::fileio::demo_song();
        song.mixer.limiter.gain_db = 9.0; // drive it into the limiter
        let o = Opts {
            tail_secs: 0.0,
            scope: Scope::Block(song.order[0]),
            ..Default::default()
        };
        let off = render(&song, &o, &Progress::default()).unwrap();
        let s = scoped(&song, o.scope);
        let mut e = crate::engine::Engine::new(o.sample_rate as f32);
        e.start(&s, 0, 0, false, 0);
        let mut rt = Vec::new();
        // odd callback sizes like a sound card (frames 113, 256, 512, ...)
        let sizes = [113usize, 256, 512, 441, 1024];
        let mut k = 0;
        while rt.len() < off.len() {
            let mut buf = vec![0.0f32; sizes[k % sizes.len()] * 2];
            e.render(&s, &mut buf);
            rt.extend_from_slice(&buf);
            k += 1;
        }
        // the export fades out its last 50 ms; compare everything before it
        let n = off.len() - (o.sample_rate as usize / 20) * 2 - 2;
        let diff = off[..n]
            .iter()
            .zip(&rt[..n])
            .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
        assert!(diff < 1e-6, "max diff {diff}");
        let tp = crate::master::lin_to_db(crate::master::true_peak(&off));
        assert!(tp <= -1.0 + 1e-3, "export true peak {tp}");
        assert!(
            e.loud.true_peak <= -1.0 + 1e-3,
            "meter {}",
            e.loud.true_peak
        );
    }
}
