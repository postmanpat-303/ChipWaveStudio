//! Audio output: an engine thread renders into a lock-free ring buffer (rtrb);
//! the cpal callback only pops samples (no locks, no allocation).
//! Underruns are counted and the target latency grows automatically.
use crate::engine::Engine;
use crate::model::Song;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct Shared {
    pub song: Song,
    pub engine: Engine,
}

pub type SharedRef = Arc<Mutex<Shared>>;

/// Requested device buffer: 0 = Auto (device default), otherwise frames.
pub const BUFFER_CHOICES: [u32; 5] = [0, 256, 512, 1024, 2048];

#[derive(Default)]
pub struct Stats {
    pub underruns: AtomicU32,
    /// target ring fill in frames (latency)
    pub target_frames: AtomicUsize,
    pub callback_frames: AtomicUsize,
}

pub struct Audio {
    _stream: cpal::Stream,
    pub sample_rate: u32,
    pub device_name: String,
    pub buffer: String,
    pub stats: Arc<Stats>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Audio {
    pub fn latency_ms(&self) -> f32 {
        self.stats.target_frames.load(Ordering::Relaxed) as f32 * 1000.0 / self.sample_rate as f32
    }
}

const RING_FRAMES: usize = 16384;
const CHUNK: usize = 256;

pub fn start(shared: SharedRef, buffer_frames: u32) -> Result<Audio, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no audio output device")?;
    let device_name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "default".into());
    let cfg = device.default_output_config().map_err(|e| e.to_string())?;
    let sample_rate = cfg.sample_rate();
    let channels = cfg.channels() as usize;
    let mut config: cpal::StreamConfig = cfg.into();
    let mut buffer = "Auto (device default)".to_string();
    if buffer_frames > 0 {
        let ok = match cfg.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => (*min..=*max).contains(&buffer_frames),
            cpal::SupportedBufferSize::Unknown => true,
        };
        if ok {
            config.buffer_size = cpal::BufferSize::Fixed(buffer_frames);
            buffer = format!("{buffer_frames} frames");
        } else {
            buffer = format!("{buffer_frames} unsupported, using device default");
        }
    }
    {
        let mut s = shared.lock().unwrap();
        let was_playing = s.engine.playing;
        s.engine = Engine::new(sample_rate as f32);
        s.engine.playing = false;
        let _ = was_playing;
    }
    let stats = Arc::new(Stats::default());
    // start with ~2x the device buffer, at least 1024 frames (~21 ms @ 48k)
    let base = (buffer_frames as usize * 2).max(2048);
    stats.target_frames.store(base, Ordering::Relaxed);
    let (mut prod, mut cons) = rtrb::RingBuffer::<f32>::new(RING_FRAMES * 2);

    // engine thread: keeps the ring filled to the target
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let stop = stop.clone();
        let stats = stats.clone();
        let shared = shared.clone();
        std::thread::Builder::new()
            .name("chipwave-engine".into())
            .spawn(move || {
                realtime_thread();
                let mut buf = vec![0.0f32; CHUNK * 2];
                // prefill with silence so playback starts on a full buffer
                let pre = stats.target_frames.load(Ordering::Relaxed) * 2;
                for _ in 0..pre.min(prod.slots()) {
                    let _ = prod.push(0.0);
                }
                while !stop.load(Ordering::Relaxed) {
                    let target = stats.target_frames.load(Ordering::Relaxed) * 2;
                    let filled = RING_FRAMES * 2 - prod.slots();
                    if filled + CHUNK * 2 <= target {
                        {
                            let mut s = shared.lock().unwrap();
                            let Shared { song, engine } = &mut *s;
                            engine.render(song, &mut buf);
                        }
                        if let Ok(chunk) = prod.write_chunk_uninit(buf.len()) {
                            chunk.fill_from_iter(buf.iter().copied());
                        }
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
            })
            .map_err(|e| e.to_string())?
    };

    let err = |e| eprintln!("audio stream error: {e}");
    let st = stats.clone();
    // shared callback body: pop stereo frames, declick on underrun
    let mut last = [0.0f32; 2];
    let mut callbacks = 0u32;
    let mut fill = move |n_frames: usize, write: &mut dyn FnMut(usize, usize, f32)| {
        st.callback_frames.store(n_frames, Ordering::Relaxed);
        callbacks = callbacks.saturating_add(1);
        // keep at least two device buffers queued (not an underrun, just sizing)
        let t = st.target_frames.load(Ordering::Relaxed);
        if t < n_frames * 2 {
            st.target_frames.store(
                (n_frames * 2).min(RING_FRAMES - CHUNK * 2),
                Ordering::Relaxed,
            );
        }
        let mut under = false;
        for i in 0..n_frames {
            let (l, r) = if cons.slots() >= 2 {
                (cons.pop().unwrap_or(0.0), cons.pop().unwrap_or(0.0))
            } else {
                under = true;
                // fade towards silence instead of a hard drop
                (last[0] * 0.995, last[1] * 0.995)
            };
            last = [l, r];
            for c in 0..channels {
                write(i, c, if c & 1 == 0 { l } else { r });
            }
        }
        if under {
            // the first callbacks happen while the stream is still starting up
            if callbacks > 3 {
                st.underruns.fetch_add(1, Ordering::Relaxed);
                eprintln!("audio underrun: growing latency");
            }
            let t = st.target_frames.load(Ordering::Relaxed);
            let grow = (t + n_frames.max(256)).min(RING_FRAMES - CHUNK * 2);
            st.target_frames.store(grow, Ordering::Relaxed);
        }
    };
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            config,
            move |out: &mut [f32], _| {
                let n = out.len() / channels;
                fill(n, &mut |i, c, v| out[i * channels + c] = v);
            },
            err,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            config,
            move |out: &mut [i16], _| {
                let n = out.len() / channels;
                fill(n, &mut |i, c, v| {
                    out[i * channels + c] = (v.clamp(-1.0, 1.0) * 32767.0) as i16
                });
            },
            err,
            None,
        ),
        other => return Err(format!("unsupported sample format {other:?}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Audio {
        _stream: stream,
        sample_rate,
        device_name,
        buffer,
        stats,
        stop,
        thread: Some(thread),
    })
}

/// Make the engine thread wake up on time: 1 ms timer resolution and a
/// high thread priority on Windows (the default 15.6 ms sleep granularity
/// plus UI load caused underruns).
#[cfg(windows)]
fn realtime_thread() {
    #[link(name = "winmm")]
    extern "system" {
        fn timeBeginPeriod(ms: u32) -> u32;
    }
    extern "system" {
        fn GetCurrentThread() -> isize;
        fn SetThreadPriority(h: isize, prio: i32) -> i32;
    }
    unsafe {
        timeBeginPeriod(1);
        // THREAD_PRIORITY_TIME_CRITICAL
        SetThreadPriority(GetCurrentThread(), 15);
    }
}

#[cfg(not(windows))]
fn realtime_thread() {}
