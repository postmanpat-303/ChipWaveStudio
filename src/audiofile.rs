//! Audio files for timeline audio clips (M4, docs/TIMELINE.md "Audio
//! clips"): decoding WAV / FLAC / MP3 / Ogg Vorbis with symphonia into
//! interleaved f32 at the file's own rate (no resampling at import: the
//! engine converts the rate while it plays, so a file always plays at its
//! own speed whatever the output or export rate), waveform peaks for
//! drawing, and the interpolated read the audio thread uses.
use std::path::Path;

/// file extensions the timeline imports as audio clips
pub const EXTS: [&str; 6] = ["wav", "wave", "flac", "mp3", "ogg", "oga"];
/// longest file that is imported (it is held in memory, 32-bit float)
pub const MAX_SECONDS: f64 = 20.0 * 60.0;
/// longer files import with a memory warning
pub const LONG_SECONDS: f64 = 5.0 * 60.0;
/// frames per waveform peak
pub const PEAK_BLOCK: usize = 256;

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// Decoded audio: interleaved f32, 1 or 2 channels (more are folded down
/// to stereo), plus min/max peaks every [`PEAK_BLOCK`] frames.
#[derive(Clone, Default, PartialEq)]
pub struct Pcm {
    pub rate: u32,
    pub channels: u8,
    pub data: Vec<f32>,
    pub peaks: Vec<(f32, f32)>,
}

impl std::fmt::Debug for Pcm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Pcm({} Hz, {} ch, {} frames)",
            self.rate,
            self.channels,
            self.frames()
        )
    }
}

impl Pcm {
    pub fn new(rate: u32, channels: u8, data: Vec<f32>) -> Self {
        let mut p = Self {
            rate: rate.max(1),
            channels: channels.clamp(1, 2),
            data,
            peaks: vec![],
        };
        p.peaks = peaks_of(&p);
        p
    }
    pub fn frames(&self) -> usize {
        self.data.len() / self.channels.max(1) as usize
    }
    /// bytes held in memory
    pub fn mem_bytes(&self) -> usize {
        self.data.len() * 4
    }

    /// Stereo sample at fractional frame `p` (4-point Hermite; silence
    /// outside the file). Allocation-free: called on the audio thread.
    #[inline]
    pub fn read(&self, p: f64) -> (f32, f32) {
        let n = self.frames() as i64;
        if !(p >= 0.0) || p >= n as f64 {
            return (0.0, 0.0);
        }
        let i = p as i64;
        let t = (p - i as f64) as f32;
        let ch = self.channels as usize;
        let at = |k: i64, c: usize| -> f32 {
            if k < 0 || k >= n {
                0.0
            } else {
                self.data[k as usize * ch + c]
            }
        };
        let h = |c: usize| {
            let (y0, y1, y2, y3) = (at(i - 1, c), at(i, c), at(i + 1, c), at(i + 2, c));
            let c1 = 0.5 * (y2 - y0);
            let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
            let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
            ((c3 * t + c2) * t + c1) * t + y1
        };
        if ch == 1 {
            let v = h(0);
            (v, v)
        } else {
            (h(0), h(1))
        }
    }

    /// (min, max) over frames [a, b) from the peak table (whole blocks;
    /// exact samples when the range is under one block)
    pub fn peak_range(&self, a: f64, b: f64) -> (f32, f32) {
        let n = self.frames();
        let (a, b) = (a.max(0.0) as usize, (b.max(0.0) as usize).min(n));
        if a >= b {
            return (0.0, 0.0);
        }
        let ch = self.channels as usize;
        if b - a <= PEAK_BLOCK {
            let mut m = (f32::MAX, f32::MIN);
            for x in &self.data[a * ch..b * ch] {
                m = (m.0.min(*x), m.1.max(*x));
            }
            return m;
        }
        let (ka, kb) = (a / PEAK_BLOCK, b.div_ceil(PEAK_BLOCK).min(self.peaks.len()));
        self.peaks[ka..kb.max(ka + 1).min(self.peaks.len())]
            .iter()
            .fold((f32::MAX, f32::MIN), |m, p| (m.0.min(p.0), m.1.max(p.1)))
    }
}

fn peaks_of(p: &Pcm) -> Vec<(f32, f32)> {
    let ch = p.channels as usize;
    p.data
        .chunks(PEAK_BLOCK * ch)
        .map(|c| {
            c.iter()
                .fold((f32::MAX, f32::MIN), |m, x| (m.0.min(*x), m.1.max(*x)))
        })
        .collect()
}

/// Decode a whole file held in memory. `ext` is a format hint ("wav",
/// "flac", "mp3", "ogg"); the content is probed either way.
pub fn decode(bytes: &[u8], ext: &str) -> Result<Pcm, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    let cur = std::io::Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cur), Default::default());
    let mut hint = Hint::new();
    if !ext.is_empty() {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("not a supported audio file ({e})"))?;
    let mut fmt = probed.format;
    let tr = fmt
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("no audio track in the file")?;
    let id = tr.id;
    let mut rate = tr.codec_params.sample_rate.unwrap_or(0);
    let mut dec = symphonia::default::get_codecs()
        .make(&tr.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("unsupported codec ({e})"))?;
    let mut data: Vec<f32> = vec![];
    let mut out_ch = 0usize;
    let mut sb: Option<SampleBuffer<f32>> = None;
    loop {
        let p = match fmt.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => {
                if data.is_empty() {
                    return Err(format!("could not read the file ({e})"));
                }
                break;
            }
        };
        if p.track_id() != id {
            continue;
        }
        let buf = match dec.decode(&p) {
            Ok(b) => b,
            // a damaged packet: skip it (as players do)
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("could not decode the file ({e})")),
        };
        let spec = *buf.spec();
        if rate == 0 {
            rate = spec.rate;
        }
        let ch = spec.channels.count().max(1);
        if out_ch == 0 {
            out_ch = ch.min(2);
        }
        let need = buf.capacity() as u64;
        if sb
            .as_ref()
            .is_none_or(|s| s.capacity() < need as usize * ch)
        {
            sb = Some(SampleBuffer::<f32>::new(need, spec));
        }
        let s = sb.as_mut().unwrap();
        s.copy_interleaved_ref(buf);
        let smp = s.samples();
        for f in smp.chunks(ch) {
            match (ch, out_ch) {
                (1, _) => data.push(f[0]),
                (_, 2) if ch == 2 => data.extend_from_slice(f),
                _ => {
                    // fold 3+ channels: even ones left, odd ones right
                    let (mut l, mut r) = (0.0, 0.0);
                    for (k, x) in f.iter().enumerate() {
                        if k % 2 == 0 {
                            l += x;
                        } else {
                            r += x;
                        }
                    }
                    let (nl, nr) = (ch.div_ceil(2) as f32, (ch / 2).max(1) as f32);
                    data.push(l / nl);
                    data.push(r / nr);
                }
            }
        }
        if rate > 0 && (data.len() / out_ch.max(1)) as f64 > MAX_SECONDS * rate as f64 {
            return Err(format!(
                "the file is longer than {} minutes",
                MAX_SECONDS as u32 / 60
            ));
        }
    }
    if out_ch == 0 || data.is_empty() || rate == 0 {
        return Err("the file has no audio".into());
    }
    for x in &mut data {
        if !x.is_finite() {
            *x = 0.0;
        }
    }
    Ok(Pcm::new(rate, out_ch as u8, data))
}

/// A 32-bit float WAV of interleaved `data` (tests, generated assets).
pub fn wav_f32(rate: u32, channels: u16, data: &[f32]) -> Vec<u8> {
    let mut cur = std::io::Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    if let Ok(mut w) = hound::WavWriter::new(&mut cur, spec) {
        for x in data {
            let _ = w.write_sample(*x);
        }
        let _ = w.finalize();
    }
    cur.into_inner()
}

/// lower-case extension of `path` ("" when none)
pub fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn wav_bytes(rate: u32, ch: u16, frames: &[f32], float: bool) -> Vec<u8> {
        let mut cur = std::io::Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels: ch,
            sample_rate: rate,
            bits_per_sample: if float { 32 } else { 16 },
            sample_format: if float {
                hound::SampleFormat::Float
            } else {
                hound::SampleFormat::Int
            },
        };
        let mut w = hound::WavWriter::new(&mut cur, spec).unwrap();
        for x in frames {
            if float {
                w.write_sample(*x).unwrap();
            } else {
                w.write_sample((x * 32767.0) as i16).unwrap();
            }
        }
        w.finalize().unwrap();
        cur.into_inner()
    }

    fn sine(rate: u32, hz: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (i as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.5)
            .collect()
    }

    #[test]
    fn decodes_float_and_int_wav_exactly() {
        let s = sine(22050, 441.0, 2205);
        let p = decode(&wav_bytes(22050, 1, &s, true), "wav").unwrap();
        assert_eq!((p.rate, p.channels, p.frames()), (22050, 1, 2205));
        assert_eq!(p.data, s, "float wav must round-trip exactly");
        let st: Vec<f32> = s.iter().flat_map(|x| [*x, -*x]).collect();
        let p = decode(&wav_bytes(48000, 2, &st, false), "wav").unwrap();
        assert_eq!((p.rate, p.channels, p.frames()), (48000, 2, 2205));
        for (a, b) in p.data.iter().zip(&st) {
            assert!((a - b).abs() < 1e-3);
        }
    }

    #[test]
    fn decodes_fixture_flac_mp3_ogg() {
        // 0.25 s 440 Hz stereo at 44.1 kHz, made with ffmpeg
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio");
        for (f, exact_len) in [
            ("tone.flac", true),
            ("tone.mp3", false),
            ("tone.ogg", false),
            ("tone.wav", true),
        ] {
            let b = std::fs::read(dir.join(f)).unwrap();
            let p = decode(&b, &ext_of(Path::new(f))).unwrap_or_else(|e| panic!("{f}: {e}"));
            assert_eq!(p.rate, 44100, "{f}");
            assert_eq!(p.channels, 2, "{f}");
            let n = p.frames() as i64;
            if exact_len {
                assert_eq!(n, 11025, "{f}");
            } else {
                assert!((n - 11025).abs() < 3000, "{f}: {n} frames");
            }
            let pk = p.data.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(pk > 0.3 && pk < 0.7, "{f}: peak {pk}");
            // the hint is only a hint: the content is probed
            assert!(decode(&b, "").is_ok(), "{f} without hint");
        }
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(decode(b"definitely not audio at all", "mp3").is_err());
        assert!(decode(&[], "wav").is_err());
        let mut b = wav_bytes(44100, 1, &sine(44100, 100.0, 1000), false);
        b.truncate(60);
        let _ = decode(&b, "wav");
    }

    #[test]
    fn hermite_read_hits_samples_and_is_silent_outside() {
        let p = Pcm::new(100, 1, vec![0.0, 1.0, 0.5, -0.5]);
        assert_eq!(p.read(1.0), (1.0, 1.0));
        assert_eq!(p.read(2.0), (0.5, 0.5));
        assert_eq!(p.read(-0.5), (0.0, 0.0));
        assert_eq!(p.read(4.0), (0.0, 0.0));
        assert_eq!(p.read(f64::NAN), (0.0, 0.0));
        let m = p.read(1.5).0;
        assert!(m > 0.5 && m < 1.0, "{m}");
        let s = Pcm::new(100, 2, vec![0.1, 0.2, 0.3, 0.4]);
        assert_eq!(s.read(1.0), (0.3, 0.4));
    }

    #[test]
    fn peaks_cover_the_file() {
        let s = sine(1000, 3.0, 5000);
        let p = Pcm::new(1000, 1, s);
        assert_eq!(p.peaks.len(), 5000usize.div_ceil(PEAK_BLOCK));
        let (lo, hi) = p.peak_range(0.0, 5000.0);
        assert!(lo < -0.49 && hi > 0.49);
        let (lo, hi) = p.peak_range(0.0, 10.0);
        assert!(lo >= 0.0 && hi < 0.1);
    }
}
