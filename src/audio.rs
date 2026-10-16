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
    /// Fill-to-length preview: when set, the audio thread plays this song instead
    pub preview: Option<Song>,
}

pub type SharedRef = Arc<Mutex<Shared>>;

/// Lock the shared state, recovering from a poisoned mutex (a panic in one
/// thread must not take the UI or the audio thread down with it).
pub fn lock(s: &SharedRef) -> std::sync::MutexGuard<'_, Shared> {
    s.lock().unwrap_or_else(|p| {
        s.clear_poison();
        p.into_inner()
    })
}

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

fn dev_name(d: &cpal::Device) -> String {
    d.description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "default".into())
}

/// names of the output devices of the default host
pub fn output_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.output_devices()
        .map(|it| it.map(|d| dev_name(&d)).collect())
        .unwrap_or_default()
}

/// sample rates commonly supported by the chosen device (empty = unknown)
pub fn device_rates(device: &str) -> Vec<u32> {
    let host = cpal::default_host();
    let d = if device.is_empty() {
        host.default_output_device()
    } else {
        host.output_devices()
            .ok()
            .and_then(|mut it| it.find(|d| dev_name(d) == device))
    };
    let Some(d) = d else { return vec![] };
    let Ok(cfgs) = d.supported_output_configs() else {
        return vec![];
    };
    let cfgs: Vec<_> = cfgs.collect();
    [44100u32, 48000, 88200, 96000]
        .into_iter()
        .filter(|r| cfgs.iter().any(|c| (*c).try_with_sample_rate(*r).is_some()))
        .collect()
}

pub fn start(shared: SharedRef, buffer_frames: u32) -> Result<Audio, String> {
    start_with(shared, buffer_frames, "", 0)
}

/// `device` "" = system default; `rate` 0 = device default
pub fn start_with(
    shared: SharedRef,
    buffer_frames: u32,
    device: &str,
    rate: u32,
) -> Result<Audio, String> {
    let host = cpal::default_host();
    let device = if device.is_empty() {
        None
    } else {
        host.output_devices()
            .ok()
            .and_then(|mut it| it.find(|d| dev_name(d) == device))
    }
    .or_else(|| host.default_output_device())
    .ok_or("no audio output device")?;
    let device_name = dev_name(&device);
    let mut cfg = device.default_output_config().map_err(|e| e.to_string())?;
    if rate > 0 {
        if let Ok(mut it) = device.supported_output_configs() {
            if let Some(c) = it.find_map(|c| {
                (c.channels() == cfg.channels() && c.sample_format() == cfg.sample_format())
                    .then(|| c.try_with_sample_rate(rate))
                    .flatten()
            }) {
                cfg = c;
            }
        }
    }
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
        let mut s = lock(&shared);
        // a device change restarts the engine (playback stops)
        s.engine = Engine::new(sample_rate as f32);
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
                let _timer = realtime_thread();
                let mut buf = vec![0.0f32; CHUNK * 2];
                // prefill with silence so playback starts on a full buffer
                let pre = stats.target_frames.load(Ordering::Relaxed) * 2;
                for _ in 0..pre.min(prod.slots()) {
                    let _ = prod.push(0.0);
                }
                let mut logged_underruns = 0u32;
                while !stop.load(Ordering::Relaxed) {
                    // underruns are counted in the callback, reported here (no I/O there)
                    let u = stats.underruns.load(Ordering::Relaxed);
                    if u != logged_underruns {
                        logged_underruns = u;
                        eprintln!("audio underrun ({u} total): growing latency");
                    }
                    let target = stats.target_frames.load(Ordering::Relaxed) * 2;
                    let filled = RING_FRAMES * 2 - prod.slots();
                    if filled + CHUNK * 2 <= target {
                        render_chunk(&shared, &mut buf, sample_rate as f32);
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

/// Render one chunk under the lock. A panic inside the engine must not kill
/// the audio thread: output silence, reset the engine and keep going.
pub(crate) fn render_chunk(shared: &SharedRef, buf: &mut [f32], sr: f32) {
    let mut s = lock(shared);
    let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Shared {
            song,
            engine,
            preview,
        } = &mut *s;
        engine.render(preview.as_ref().unwrap_or(song), buf);
    }))
    .is_ok();
    if !ok {
        buf.fill(0.0);
        s.engine = Engine::new(sr);
    }
}

/// Make the engine thread wake up on time: 1 ms timer resolution and a
/// high thread priority on Windows (the default 15.6 ms sleep granularity
/// plus UI load caused underruns).
/// Restores the timer resolution when the engine thread ends.
pub(crate) struct TimerGuard;

#[cfg(windows)]
impl Drop for TimerGuard {
    fn drop(&mut self) {
        #[link(name = "winmm")]
        extern "system" {
            fn timeEndPeriod(ms: u32) -> u32;
        }
        // SAFETY: plain Win32 call, paired with timeBeginPeriod(1) below
        unsafe {
            timeEndPeriod(1);
        }
    }
}

#[cfg(windows)]
fn realtime_thread() -> TimerGuard {
    #[link(name = "winmm")]
    extern "system" {
        fn timeBeginPeriod(ms: u32) -> u32;
    }
    extern "system" {
        fn GetCurrentThread() -> isize;
        fn SetThreadPriority(h: isize, prio: i32) -> i32;
    }
    // SAFETY: plain Win32 calls on the current thread
    unsafe {
        timeBeginPeriod(1);
        // THREAD_PRIORITY_HIGHEST (TIME_CRITICAL risks priority inversion on
        // the mutex shared with the UI)
        SetThreadPriority(GetCurrentThread(), 2);
    }
    TimerGuard
}

#[cfg(not(windows))]
fn realtime_thread() -> TimerGuard {
    TimerGuard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_lock_recovers() {
        let shared: SharedRef = Arc::new(Mutex::new(Shared {
            song: crate::fileio::demo_song(),
            engine: Engine::new(48000.0),
            preview: None,
        }));
        let s2 = shared.clone();
        let _ = std::thread::spawn(move || {
            let _g = s2.lock().unwrap();
            panic!("boom");
        })
        .join();
        assert!(shared.is_poisoned());
        let g = lock(&shared);
        assert!(!g.song.patterns.is_empty());
        drop(g);
        let mut buf = vec![1.0f32; 64];
        render_chunk(&shared, &mut buf, 48000.0);
        assert!(buf.iter().all(|x| x.is_finite()));
    }
}
