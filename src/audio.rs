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
    /// engine render time per chunk as ‰ of the chunk's real time (smoothed);
    /// above 1000 the song is too heavy to play live on this machine
    pub load_permille: AtomicU32,
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
    /// The latency the engine keeps queued ahead of the device: the ring's
    /// target fill (it starts at 2x the buffer and grows by a device buffer
    /// on each underrun), not including the device's own buffer.
    pub fn latency_ms(&self) -> f32 {
        self.stats.target_frames.load(Ordering::Relaxed) as f32 * 1000.0 / self.sample_rate as f32
    }

    /// the latency has grown to its ceiling (`MAX_TARGET`, 331 ms at 48 kHz):
    /// more buffering cannot help, the engine is not keeping up
    pub fn latency_maxed(&self) -> bool {
        self.stats.target_frames.load(Ordering::Relaxed) >= MAX_TARGET
    }

    /// smoothed engine load in % of real time
    pub fn load_pct(&self) -> f32 {
        self.stats.load_permille.load(Ordering::Relaxed) as f32 / 10.0
    }

    /// one line for the status bar / Settings when playback is in trouble
    pub fn advice(&self) -> Option<String> {
        let load = self.load_pct();
        if load >= 90.0 || (self.latency_maxed() && load >= 60.0) {
            Some(format!(
                "audio engine at {load:.0} % of real time: this song is too heavy for this \
                 computer. Close other programs, or freeze/mute heavy tracks (voices, many FX)"
            ))
        } else if self.stats.underruns.load(Ordering::Relaxed) > 20 {
            Some("many underruns: try a bigger buffer in Settings › Audio".into())
        } else {
            None
        }
    }
}

const RING_FRAMES: usize = 16384;
const CHUNK: usize = 256;
/// most frames the engine keeps queued (the latency ceiling)
pub const MAX_TARGET: usize = RING_FRAMES - CHUNK * 2;

/// smoothed load update: `prev` and the result in ‰, `secs` the render time
/// of one chunk of `frames` at `sr`
pub(crate) fn load_step(prev: u32, secs: f64, frames: usize, sr: f32) -> u32 {
    let now = secs * sr as f64 / frames as f64 * 1000.0;
    // ~0.25 s time constant at 48 kHz / 256 frames
    let k = 0.02;
    let next = prev as f64 + (now - prev as f64) * k;
    // round towards the new value so the integer ‰ settles on it
    let next = if now > prev as f64 {
        next.ceil()
    } else {
        next.floor()
    };
    next.clamp(0.0, 100_000.0) as u32
}

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
        // a device change restarts the engine (playback stops); the live
        // MIDI queue stays connected
        let rx = s.engine.take_live_input();
        // plugin instances survive a device change (re-activated at the new rate)
        let mut rack = std::mem::take(&mut s.engine.plugins);
        let mut fxr = std::mem::take(&mut s.engine.fx_plugins);
        rack.all_off();
        if s.engine.sr != sample_rate as f32 {
            rack.set_sample_rate(sample_rate as f32);
            fxr.set_sample_rate(sample_rate as f32);
        }
        s.engine = Engine::new(sample_rate as f32);
        s.engine.set_live_input(rx);
        s.engine.plugins = rack;
        s.engine.fx_plugins = fxr;
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
                        let t0 = std::time::Instant::now();
                        render_chunk(&shared, &mut buf, sample_rate as f32);
                        let l = load_step(
                            stats.load_permille.load(Ordering::Relaxed),
                            t0.elapsed().as_secs_f64(),
                            CHUNK,
                            sample_rate as f32,
                        );
                        stats.load_permille.store(l, Ordering::Relaxed);
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
            st.target_frames
                .store((n_frames * 2).min(MAX_TARGET), Ordering::Relaxed);
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
            let grow = (t + n_frames.max(256)).min(MAX_TARGET);
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
        let rx = s.engine.take_live_input();
        // never destroy plugin instances on the audio thread: keep the rack
        let mut rack = std::mem::take(&mut s.engine.plugins);
        let fxr = std::mem::take(&mut s.engine.fx_plugins);
        rack.all_off();
        s.engine = Engine::new(sr);
        s.engine.set_live_input(rx);
        s.engine.plugins = rack;
        s.engine.fx_plugins = fxr;
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

    /// the "331 ms" in the status bar is the ring ceiling at 48 kHz
    #[test]
    fn latency_ceiling_is_331_ms_at_48k() {
        let ms = MAX_TARGET as f32 * 1000.0 / 48000.0;
        assert_eq!(format!("{ms:.0}"), "331");
    }

    #[test]
    fn load_follows_render_time() {
        let mut l = 0;
        // a chunk of 256 @ 48 kHz is 5.33 ms; rendering it in 8 ms = 150 %
        for _ in 0..1000 {
            l = load_step(l, 0.008, 256, 48000.0);
        }
        assert!((1490..=1500).contains(&l), "{l}");
        for _ in 0..1000 {
            l = load_step(l, 0.0016, 256, 48000.0);
        }
        assert!((300..=310).contains(&l), "{l}");
    }
}

#[cfg(test)]
mod rt_bench {
    /// per-chunk render cost of each demo (256-frame chunks @ 48 kHz, budget 5.33 ms)
    #[test]
    #[ignore]
    fn rt_bench_demos() {
        let secs: f64 = std::env::var("RT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60.0);
        for (name, _, text) in crate::fileio::DEMOS {
            if std::env::var("RT_ONLY").is_ok_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let s = crate::fileio::song_from_text(text).unwrap();
            let mut e = crate::engine::Engine::new(48000.0);
            e.start(&s, 0, 0, false, 0);
            let mut buf = vec![0.0f32; 512];
            let n = (secs * 48000.0 / 256.0) as usize;
            let mut t = Vec::with_capacity(n);
            let mut h: u64 = 0xcbf29ce484222325;
            for _ in 0..n {
                let t0 = std::time::Instant::now();
                e.render(&s, &mut buf);
                t.push(t0.elapsed().as_secs_f64() * 1e6);
                for v in &buf {
                    h = (h ^ v.to_bits() as u64).wrapping_mul(0x100000001b3);
                }
            }
            let tot: f64 = t.iter().sum();
            let mut so = t.clone();
            so.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let p = |q: f64| so[((so.len() as f64 - 1.0) * q) as usize];
            println!(
                "RTB {name:20} ch {:2} avg {:7.1} us p50 {:7.1} p99 {:7.1} max {:8.1} | {:5.1}% of RT | hash {h:016x}",
                s.channels,
                tot / n as f64,
                p(0.5),
                p(0.99),
                so[so.len() - 1],
                tot / (n as f64 * 256.0 / 48000.0 * 1e6) * 100.0
            );
        }
    }
}
