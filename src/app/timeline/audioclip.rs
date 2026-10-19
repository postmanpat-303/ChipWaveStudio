//! Audio clips in the Timeline view (M4): importing files (drop on a track,
//! or right-click > Import audio file), decoding on a worker thread, placing
//! the clip, relinking a missing file, embed / reference, the clip's gain
//! and fade settings, and drawing the waveform.
use super::*;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// what a finished import is for
#[derive(Clone, Copy, Debug)]
pub(crate) enum AudioJob {
    /// a new clip on (track, tick)
    Place(usize, Tick),
    /// the file of pooled asset `n` (it was missing)
    Relink(usize),
}

pub(crate) struct Import {
    rx: mpsc::Receiver<Result<tl::AudioAsset, String>>,
    job: AudioJob,
    name: String,
}

/// a clip menu edit of the selected audio clips
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AudioOp {
    Gain(f32),
    Shape(tl::FadeShape),
    Embed(usize, bool),
    Locate(usize),
    Import(usize, Tick),
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".into())
}

/// read and decode one file (worker thread)
pub(crate) fn load_file(path: &Path) -> Result<tl::AudioAsset, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let ext = crate::audiofile::ext_of(path);
    let pcm = crate::audiofile::decode(&bytes, &ext)?;
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut p = abs.to_string_lossy().into_owned();
    // canonicalize on Windows gives \\?\C:\...; keep the familiar form
    if let Some(s) = p.strip_prefix(r"\\?\") {
        p = s.to_string();
    }
    Ok(tl::AudioAsset::from_pcm(
        &display_name(path),
        &p,
        &ext,
        pcm,
        bytes,
    ))
}

fn mb(bytes: usize) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

/// "0:04.2 · 44.1 kHz stereo"
pub(crate) fn describe_asset(a: &tl::AudioAsset) -> String {
    let s = a.seconds();
    format!(
        "{}:{:04.1} · {:.1} kHz {}",
        (s / 60.0) as u32,
        s % 60.0,
        a.rate as f64 / 1000.0,
        if a.channels == 2 { "stereo" } else { "mono" }
    )
}

impl App {
    /// where dropped audio files go: the track and grid position under the
    /// mouse, else the current track at the play marker
    pub(crate) fn tlv_drop_target(&self) -> (usize, Tick) {
        match self.tlv.hover_at {
            Some((tr, tk)) => (tr, tl::snap(tk, self.tlv.grid_now.max(1)).max(0)),
            None => (self.ch, self.tlv.cue),
        }
    }

    /// Import audio file `path` as a clip on (track, tick); decoding runs on
    /// a worker thread, the clip appears when it is done.
    pub(crate) fn import_audio(&mut self, path: PathBuf, track: usize, at: Tick) {
        self.start_audio_job(path, AudioJob::Place(track, at));
    }

    fn start_audio_job(&mut self, path: PathBuf, job: AudioJob) {
        let (tx, rx) = mpsc::channel();
        let name = display_name(&path);
        let p = path.clone();
        let spawned = std::thread::Builder::new()
            .name("audio-import".into())
            .spawn(move || {
                let _ = tx.send(load_file(&p));
            });
        if let Err(e) = spawned {
            self.status = format!("Import failed: {e}");
            return;
        }
        self.status = format!("Reading {name}…");
        self.tlv.imports.push(Import { rx, job, name });
    }

    /// right-click > Import audio file here…
    pub(crate) fn import_audio_dialog(&mut self, track: usize, at: Tick) {
        let Some(files) = rfd::FileDialog::new()
            .set_title("Import audio file")
            .add_filter("Audio (WAV, FLAC, MP3, Ogg)", &crate::audiofile::EXTS)
            .pick_files()
        else {
            return;
        };
        self.import_audio_files(files, track, at);
    }

    /// several files: one per track from `track` down
    pub(crate) fn import_audio_files(&mut self, files: Vec<PathBuf>, track: usize, at: Tick) {
        let nch = self.with(|s| s.song.channels.clamp(1, MAX_CHANNELS));
        for (k, f) in files.into_iter().enumerate() {
            self.import_audio(f, (track + k).min(nch - 1), at.max(0));
        }
    }

    /// right-click > Locate audio file… (a missing file)
    pub(crate) fn locate_audio(&mut self, asset: usize) {
        let name = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.audio.get(asset))
                .map(|a| a.name.clone())
                .unwrap_or_default()
        });
        let Some(p) = rfd::FileDialog::new()
            .set_title(format!("Locate {name}"))
            .add_filter("Audio (WAV, FLAC, MP3, Ogg)", &crate::audiofile::EXTS)
            .pick_file()
        else {
            return;
        };
        self.start_audio_job(p, AudioJob::Relink(asset));
    }

    /// finished imports: place or relink (once per frame)
    pub(crate) fn poll_audio_imports(&mut self, ctx: &egui::Context) {
        if self.tlv.imports.is_empty() {
            return;
        }
        let mut done = vec![];
        self.tlv.imports.retain(|im| match im.rx.try_recv() {
            Ok(r) => {
                done.push((r, im.job, im.name.clone()));
                false
            }
            Err(mpsc::TryRecvError::Empty) => true,
            Err(mpsc::TryRecvError::Disconnected) => {
                done.push((Err("the import stopped".into()), im.job, im.name.clone()));
                false
            }
        });
        if !self.tlv.imports.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        for (r, job, name) in done {
            match (r, job) {
                (Err(e), _) => self.status = format!("Can't import {name}: {e}"),
                (Ok(a), AudioJob::Place(tr, at)) => {
                    self.tlv.ready_audio.push((a, tr, at));
                    self.tl_apply(TlOp::PlaceAudio);
                }
                (Ok(a), AudioJob::Relink(k)) => {
                    let msg = self.with(|s| {
                        let Some(t) = s.song.timeline.as_deref_mut() else {
                            return "Not in timeline mode".to_string();
                        };
                        let Some(x) = t.audio.get_mut(k) else {
                            return "That audio file is gone".to_string();
                        };
                        let had = x.frames;
                        let embed = x.embed;
                        x.path = a.path.clone();
                        x.ext = a.ext.clone();
                        x.attach(
                            (*a.pcm.clone().unwrap()).clone(),
                            embed.then(|| (*a.bytes.clone().unwrap()).clone()),
                        );
                        let differs = x.frames != had;
                        let Shared { song, engine, .. } = s;
                        engine.ensure_plan(song);
                        if differs {
                            format!("Relinked to {name} (its length differs from the missing file)")
                        } else {
                            format!("Relinked to {name}")
                        }
                    });
                    self.dirty = true;
                    self.status = msg;
                }
            }
        }
    }

    /// place the decoded imports (inside `tl_apply`, in timeline mode)
    pub(crate) fn place_ready_audio(
        song: &mut Song,
        ready: Vec<(tl::AudioAsset, usize, Tick)>,
    ) -> (String, Vec<u32>) {
        let mut ids = vec![];
        let mut msg = String::new();
        for (a, tr, at) in ready {
            let info = describe_asset(&a);
            let mem = a.pcm.as_ref().map_or(0, |p| p.mem_bytes());
            let long = a.seconds() > crate::audiofile::LONG_SECONDS;
            let embed = a.embed;
            let name = a.name.clone();
            let Some(t) = song.timeline.as_deref_mut() else {
                break;
            };
            let k = tl::add_audio_asset(t, a);
            match tl::new_audio_clip(song, tr, at, k) {
                Some(id) => {
                    ids.push(id);
                    msg = format!(
                        "Imported {name} on track {:02} ({info}, {}){}",
                        tr + 1,
                        if embed {
                            "stored in the song"
                        } else {
                            "referenced: too large to store in the song"
                        },
                        if long {
                            format!(" · long file: {} in memory", mb(mem))
                        } else {
                            String::new()
                        }
                    );
                }
                None => msg = format!("Can't place {name}: no track there"),
            }
        }
        (msg, ids)
    }

    /// clip-menu edits of audio clips
    pub(crate) fn audio_op(&mut self, op: AudioOp) {
        let sels = self.tlv.sels.clone();
        match op {
            AudioOp::Locate(k) => return self.locate_audio(k),
            AudioOp::Import(tr, at) => return self.import_audio_dialog(tr, at),
            _ => {}
        }
        let msg = self.with(|s| {
            let Some(t) = s.song.timeline.as_deref_mut() else {
                return String::new();
            };
            let msg =
                match op {
                    AudioOp::Gain(g) => {
                        for c in t
                            .clips
                            .iter_mut()
                            .filter(|c| sels.contains(&c.id) && c.audio().is_some())
                        {
                            c.gain = g;
                        }
                        format!("Clip gain {:+.1} dB", 20.0 * g.max(1e-6).log10())
                    }
                    AudioOp::Shape(f) => {
                        for c in t
                            .clips
                            .iter_mut()
                            .filter(|c| sels.contains(&c.id) && c.audio().is_some())
                        {
                            c.fade_shape = f;
                        }
                        format!("Fades: {}", f.name())
                    }
                    AudioOp::Embed(k, on) => match t.audio.get_mut(k) {
                        Some(a) if on && a.bytes.is_none() => match std::fs::read(&a.path) {
                            Ok(b) => {
                                a.bytes = Some(std::sync::Arc::new(b));
                                a.embed = true;
                                format!("{} is stored in the song when you save", a.name)
                            }
                            Err(e) => format!("Can't read {}: {e}", a.path),
                        },
                        Some(a) if on => {
                            a.embed = true;
                            format!("{} is stored in the song when you save", a.name)
                        }
                        Some(a) if Path::new(&a.path).is_file() => {
                            a.embed = false;
                            a.bytes = None;
                            format!("{} is referenced from {}", a.name, a.path)
                        }
                        Some(a) => format!(
                        "{} stays stored in the song: its original file ({}) isn't there any more",
                        a.name,
                        if a.path.is_empty() { "unknown" } else { &a.path }
                    ),
                        None => String::new(),
                    },
                    AudioOp::Locate(_) | AudioOp::Import(..) => String::new(),
                };
            let Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            msg
        });
        self.dirty = true;
        if !msg.is_empty() {
            self.status = msg;
        }
    }
}

/// The audio part of the clip menu (the selected clip is an audio clip).
pub(crate) fn audio_menu(
    ui: &mut egui::Ui,
    clip: &tl::Clip,
    a: Option<&tl::AudioAsset>,
    ops: &mut Vec<AudioOp>,
) {
    let Some(k) = clip.audio() else { return };
    match a {
        Some(a) => {
            ui.label(egui::RichText::new(format!("{} · {}", a.name, describe_asset(a))).small());
            if a.missing() {
                ui.colored_label(th().brick, "File missing: the clip is silent");
                if ui.button("Locate audio file…").clicked() {
                    ops.push(AudioOp::Locate(k));
                    ui.close();
                }
            }
            let mut emb = a.embed;
            if ui
                .checkbox(&mut emb, "Store the file in the song")
                .on_hover_text("off: the song refers to the file where it is on disk (smaller song files; keep the file with the song)")
                .changed()
            {
                ops.push(AudioOp::Embed(k, emb));
            }
        }
        None => {
            ui.label("Audio clip (no file)");
        }
    }
    let mut db = 20.0 * clip.gain.max(1e-4).log10();
    if ui
        .add(
            egui::Slider::new(&mut db, -48.0..=12.0)
                .text("gain dB")
                .fixed_decimals(1),
        )
        .changed()
    {
        ops.push(AudioOp::Gain(10f32.powf(db / 20.0)));
    }
    ui.horizontal(|ui| {
        ui.label("Fades");
        for f in tl::FadeShape::ALL {
            if ui
                .selectable_label(clip.fade_shape == f, f.name())
                .clicked()
            {
                ops.push(AudioOp::Shape(f));
            }
        }
    });
    ui.label(
        egui::RichText::new("drag the top corners for fade in / out")
            .small()
            .weak(),
    );
}

/// Waveform of audio clip `clip` in `body` (`x_of` / `tick_of`: the view's
/// mapping), as min/max bars per pixel column into `mesh`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_waveform(
    mesh: &mut egui::Mesh,
    body: Rect,
    clip: &tl::Clip,
    t: &tl::Timeline,
    song_bpm: f32,
    x_of: &dyn Fn(f64) -> f32,
    tick_of: &dyn Fn(f32) -> f64,
    clip_rect: Rect,
    colour: Color32,
) {
    let Some(a) = clip.audio().and_then(|k| t.audio.get(k)) else {
        return;
    };
    let Some(pcm) = a.pcm.as_deref() else { return };
    let fpt = t.frames_per_tick(clip.start, song_bpm, a.rate);
    let full = t.audio_ticks(clip.audio().unwrap_or(0), clip.start, song_bpm);
    let loop_f = match clip.loop_len.filter(|l| *l > 0) {
        Some(l) if l == full => a.frames as f64,
        Some(l) => l as f64 * fpt,
        None => 0.0,
    };
    let src = |tk: f64| {
        let f = (clip.offset as f64 + (tk - clip.start as f64)) * fpt;
        if loop_f > 0.0 {
            f.rem_euclid(loop_f)
        } else {
            f
        }
    };
    let mid = body.center().y;
    let half = body.height() * 0.5;
    let g = clip.gain.clamp(0.0, 4.0);
    let xa = body.left().max(clip_rect.left()).floor();
    let xb = body.right().min(clip_rect.right()).ceil();
    let mut x = xa;
    while x < xb {
        let (ta, tb) = (tick_of(x), tick_of(x + 1.0));
        let (fa, fb) = (src(ta), src(tb));
        let (lo, hi) = if fb >= fa {
            pcm.peak_range(fa, fb.max(fa + 1.0))
        } else {
            // the loop wraps inside this pixel
            let (a1, b1) = pcm.peak_range(fa, loop_f);
            let (a2, b2) = pcm.peak_range(0.0, fb.max(1.0));
            (a1.min(a2), b1.max(b2))
        };
        if lo <= hi {
            let env = |tk: f64| {
                let mut e = g;
                let u = tk - clip.start as f64;
                if clip.fade_in > 0 && u < clip.fade_in as f64 {
                    e *= clip.fade_shape.gain((u / clip.fade_in as f64) as f32);
                }
                let v = clip.end() as f64 - tk;
                if clip.fade_out > 0 && v < clip.fade_out as f64 {
                    e *= clip.fade_shape.gain((v / clip.fade_out as f64) as f32);
                }
                e
            };
            let e = env(ta);
            let top = mid - (hi * e).clamp(-1.0, 1.0) * half;
            let bot = mid - (lo * e).clamp(-1.0, 1.0) * half;
            let (top, bot) = (top.min(mid - 0.5), bot.max(mid + 0.5));
            quad(
                mesh,
                Rect::from_min_max(Pos2::new(x, top), Pos2::new(x + 1.0, bot)),
                colour,
            );
        }
        x += 1.0;
    }
    let _ = x_of;
}

/// The fade curves and their handles (top corners) of audio clip `clip`.
pub(crate) fn paint_fades(
    p: &egui::Painter,
    r: Rect,
    head_h: f32,
    clip: &tl::Clip,
    x_of: &dyn Fn(f64) -> f32,
    hot: Option<bool>,
) {
    let ink = Color32::from_white_alpha(170);
    let shade = Color32::from_black_alpha(70);
    let body_top = r.top() + head_h;
    for inn in [true, false] {
        let len = if inn { clip.fade_in } else { clip.fade_out };
        let (t0, t1) = if inn {
            (clip.start, clip.start + len)
        } else {
            (clip.end() - len, clip.end())
        };
        let (x0, x1) = (x_of(t0 as f64), x_of(t1 as f64));
        if len > 0 && x1 - x0 > 2.0 {
            let n = ((x1 - x0) / 3.0).clamp(4.0, 64.0) as usize;
            let pts: Vec<Pos2> = (0..=n)
                .map(|k| {
                    let u = k as f32 / n as f32;
                    let g = clip.fade_shape.gain(if inn { u } else { 1.0 - u });
                    Pos2::new(x0 + (x1 - x0) * u, r.bottom() - g * (r.bottom() - body_top))
                })
                .collect();
            // shade the part the fade takes away
            let mut poly = pts.clone();
            poly.push(Pos2::new(if inn { x0 } else { x1 }, body_top));
            let mut m = egui::Mesh::default();
            for w in pts.windows(2) {
                let a = Rect::from_min_max(
                    Pos2::new(w[0].x, body_top),
                    Pos2::new(w[1].x, w[0].y.min(w[1].y)),
                );
                quad(&mut m, a, shade);
            }
            p.add(egui::Shape::mesh(m));
            p.add(egui::Shape::line(pts, Stroke::new(1.2, ink)));
        }
        // handle
        let hx = if inn {
            x1.max(r.left() + 3.0)
        } else {
            x0.min(r.right() - 3.0)
        };
        let on = hot == Some(inn);
        let hs = if on { 4.5 } else { 3.5 };
        p.rect_filled(
            Rect::from_center_size(
                Pos2::new(hx, r.top() + head_h * 0.5),
                egui::vec2(hs * 2.0, hs * 2.0),
            ),
            1,
            if on { Color32::WHITE } else { ink },
        );
    }
}

/// a fade handle under `p` (`Some(true)`: fade in) of audio clip `clip`
pub(crate) fn fade_handle_at(
    p: Pos2,
    r: Rect,
    head_h: f32,
    clip: &tl::Clip,
    x_of: &dyn Fn(f64) -> f32,
) -> Option<bool> {
    if clip.audio().is_none()
        || p.y > r.top() + head_h + 2.0
        || p.y < r.top() - 2.0
        || r.width() < 24.0
    {
        return None;
    }
    let hin = x_of((clip.start + clip.fade_in) as f64).max(r.left() + 3.0);
    let hout = x_of((clip.end() - clip.fade_out) as f64).min(r.right() - 3.0);
    let (din, dout) = ((p.x - hin).abs(), (p.x - hout).abs());
    if din <= 6.0 && din <= dout {
        Some(true)
    } else if dout <= 6.0 {
        Some(false)
    } else {
        None
    }
}
