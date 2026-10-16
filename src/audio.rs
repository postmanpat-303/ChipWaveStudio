//! cpal output stream driving the shared engine.
use crate::engine::Engine;
use crate::model::Song;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

pub struct Shared {
    pub song: Song,
    pub engine: Engine,
}

pub type SharedRef = Arc<Mutex<Shared>>;

pub struct Audio {
    _stream: cpal::Stream,
    pub sample_rate: u32,
    pub device_name: String,
}

pub fn start(shared: SharedRef) -> Result<Audio, String> {
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
    {
        let mut s = shared.lock().unwrap();
        s.engine = Engine::new(sample_rate as f32);
    }
    let mut buf: Vec<f32> = Vec::new();
    let err = |e| eprintln!("audio stream error: {e}");
    let config: cpal::StreamConfig = cfg.into();
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            config,
            move |out: &mut [f32], _| {
                let frames = out.len() / channels;
                buf.resize(frames * 2, 0.0);
                if let Ok(mut s) = shared.try_lock() {
                    let Shared { song, engine } = &mut *s;
                    engine.render(song, &mut buf);
                } else {
                    buf.iter_mut().for_each(|x| *x = 0.0);
                }
                for (i, f) in out.chunks_mut(channels).enumerate() {
                    for (c, x) in f.iter_mut().enumerate() {
                        *x = buf[i * 2 + (c & 1)];
                    }
                }
            },
            err,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            config,
            move |out: &mut [i16], _| {
                let frames = out.len() / channels;
                buf.resize(frames * 2, 0.0);
                if let Ok(mut s) = shared.try_lock() {
                    let Shared { song, engine } = &mut *s;
                    engine.render(song, &mut buf);
                } else {
                    buf.iter_mut().for_each(|x| *x = 0.0);
                }
                for (i, f) in out.chunks_mut(channels).enumerate() {
                    for (c, x) in f.iter_mut().enumerate() {
                        *x = (buf[i * 2 + (c & 1)] * 32767.0) as i16;
                    }
                }
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
    })
}
