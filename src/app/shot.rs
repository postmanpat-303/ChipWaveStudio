//! Hidden developer flag `--screenshot <out.png> [--view v] [--theme dark|amiga]
//! [--size WxH]`: opens the real app window with the demo song, sets up one view,
//! lets playback run for a moment, captures the window contents
//! (egui ViewportCommand::Screenshot) to a PNG and exits. Used for the README
//! screenshots (scripts/screenshots.sh). Settings are isolated in a temp folder.
use super::*;

pub struct Shot {
    pub out: PathBuf,
    pub view: String,
    frame: u32,
    saved: bool,
    /// `--hold <secs>`: keep running (for screen recording) instead of capturing
    pub hold: Option<f32>,
    start: Option<std::time::Instant>,
    last: Option<std::time::Instant>,
    image: std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<egui::ColorImage>>>>,
}

impl Shot {
    pub fn new(out: PathBuf, view: &str) -> Self {
        Self {
            out,
            view: view.to_string(),
            frame: 0,
            saved: false,
            hold: None,
            start: None,
            last: None,
            image: Default::default(),
        }
    }
}

/// views `--view` accepts
pub const VIEWS: [&str; 12] = [
    "pattern",
    "arranger",
    "mixer",
    "fx",
    "fill",
    "instrument",
    "settings",
    "about",
    "themes",
    "voice",
    "library",
    "chords",
];
const SETUP_AT: u32 = 4;
const CAPTURE_AT: u32 = 150;

fn save_png(path: &std::path::Path, img: &egui::ColorImage) -> Result<(), String> {
    let f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(f),
        img.size[0] as u32,
        img.size[1] as u32,
    );
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    w.write_image_data(&bytes).map_err(|e| e.to_string())
}

impl App {
    fn shot_setup(&mut self, ctx: &egui::Context, view: &str) {
        // "Drop 2" (block 08) of the Lumen Drift demo, looping
        self.pattern = 8.min(self.with(|s| s.song.patterns.len().saturating_sub(1)));
        self.row = 0;
        match view {
            "arranger" => self.view = View::Arranger,
            "mixer" => self.view = View::Mixer,
            "fx" => {
                self.instr = 7; // Supersaw Pad: filter + chorus
                self.view = View::Fx;
            }
            "fill" => self.open_fill(),
            "instrument" => self.instr = 7,
            "voice" => {
                // a Voice (glitch) instrument added to the demo, selected
                let mut p = crate::voice::preset("Glitch Babble").unwrap_or_default();
                p.base.name = "Glitch Babble".into();
                self.add_instrument("Glitch Babble", InstrumentKind::Voice(p));
                self.dirty = false;
            }
            "settings" => {
                self.view = View::Settings;
                self.set_card = 1; // Look (themes, scale, accent)
                self.set_open = Some(1);
            }
            "about" => self.show_about = true,
            "library" => self.open_library(),
            "chords" => {
                self.chords.open = true;
                self.detect_key();
                // i - VI - III - VII (Am F C G), the trance staple
                self.chords.prog = [0usize, 5, 2, 6]
                    .iter()
                    .map(|&d| crate::chords::Step::new(d, crate::chords::ChordKind::Triad))
                    .collect();
                self.chords.prog[3].beats = 8.0;
            }
            "themes" => {}
            _ => {}
        }
        if view == "arranger" {
            // whole song from half-way through "Build" (order 13), so the
            // playhead crosses into "Drop 2" and the riser automation
            self.pattern = 7;
            self.with(|s| {
                let Shared { song, engine, .. } = s;
                let pos = 13.min(song.order.len().saturating_sub(1));
                engine.start(song, pos, 32, false, 0);
            });
        } else if !matches!(view, "fill" | "settings" | "about" | "library" | "chords") {
            self.play(3);
        }
        ctx.request_repaint();
    }

    /// recording mode: time-based changes (theme flips, instrument browsing)
    fn shot_animate(&mut self, ctx: &egui::Context, view: &str, t: f32) {
        match view {
            "themes" => {
                // Dark Circuit / Amiga every 2.5 s, over pattern, mixer and arranger
                let step = (t / 2.5) as usize;
                let want = if step.is_multiple_of(2) {
                    theme::DARK
                } else {
                    theme::AMIGA
                };
                if self.theme != want {
                    self.theme = want;
                    self.apply_theme(ctx);
                }
                self.view = [
                    View::Pattern,
                    View::Pattern,
                    View::Mixer,
                    View::Mixer,
                    View::Arranger,
                    View::Arranger,
                ][(t / 2.5) as usize % 6];
            }
            "instrument" => {
                // browse the demo's synth sounds, one every 1.6 s
                let order = [7u8, 8, 2, 3, 6, 10, 7];
                self.instr = order[((t / 1.6) as usize).min(order.len() - 1)];
            }
            _ => {}
        }
    }

    /// raw input for the capture: scroll the instrument panel down to the LFO
    pub(super) fn shot_input(&mut self, raw: &mut egui::RawInput) {
        let Some(sh) = &self.shot else { return };
        if sh.view == "instrument" && (SETUP_AT + 6..SETUP_AT + 30).contains(&sh.frame) {
            let p = egui::pos2(120.0, 560.0);
            raw.events.push(egui::Event::PointerMoved(p));
            raw.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -120.0),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
                modifiers: Default::default(),
            });
        }
        if sh.frame == SETUP_AT + 40 {
            // park the pointer away from the widgets (no hover highlights)
            raw.events.push(egui::Event::PointerGone);
        }
    }

    /// once per frame in screenshot mode
    pub(super) fn shot_frame(&mut self, ctx: &egui::Context) {
        let Some(sh) = &mut self.shot else { return };
        sh.frame += 1;
        let (f, view) = (sh.frame, sh.view.clone());
        ctx.request_repaint();
        if f == SETUP_AT {
            self.shot_setup(ctx, &view);
        }
        // no audio device: advance the engine from the UI at real-time speed
        let now = std::time::Instant::now();
        let Some(sh) = &mut self.shot else { return };
        let dt = sh.last.map(|l| (now - l).as_secs_f32()).unwrap_or(0.0);
        sh.last = Some(now);
        if self.audio.is_none() && f > SETUP_AT {
            let n = ((dt.min(0.1) * 48000.0) as usize).max(1);
            let mut buf = vec![0.0f32; n * 2];
            self.with(|s| {
                let Shared { song, engine, .. } = s;
                if engine.playing {
                    engine.render(song, &mut buf);
                }
            });
        }
        // recording mode: run for `hold` seconds, animating some views, then exit
        let Some(sh) = &mut self.shot else { return };
        if let Some(hold) = sh.hold {
            let t = sh.start.get_or_insert(now).elapsed().as_secs_f32();
            if f > SETUP_AT {
                self.shot_animate(ctx, &view, t);
            }
            if t > hold {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }
        if f == CAPTURE_AT {
            let Some(sh) = &self.shot else { return };
            let slot = sh.image.clone();
            let c = ctx.clone();
            ctx.request_screenshot(move |img| {
                if let Ok(mut g) = slot.lock() {
                    *g = Some(img);
                }
                c.request_repaint();
            });
        }
        let Some(sh) = &mut self.shot else { return };
        let got = sh.image.lock().ok().and_then(|mut g| g.take());
        if let Some(img) = &got {
            if !sh.saved {
                sh.saved = true;
                match save_png(&sh.out, img) {
                    Ok(()) => println!(
                        "screenshot {} ({}x{}) -> {}",
                        sh.view,
                        img.size[0],
                        img.size[1],
                        sh.out.display()
                    ),
                    Err(e) => eprintln!("screenshot: cannot write {}: {e}", sh.out.display()),
                }
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        } else if f > CAPTURE_AT + 300 {
            eprintln!("screenshot: no image from the renderer");
            std::process::exit(3);
        }
    }
}
