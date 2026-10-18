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
pub const VIEWS: [&str; 24] = [
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
    "speech",
    "slice",
    "transform",
    "drums",
    "kit",
    "chipfx",
    "analog",
    "fm",
    "noise",
    "cue",
    "plugin",
    "plugins",
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
        // CHIPWAVE_SHOT_BLOCK=<n>: another block (e.g. the disco demo's Chorus)
        if let Some(n) = std::env::var("CHIPWAVE_SHOT_BLOCK")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
        {
            self.pattern = n.min(self.with(|s| s.song.patterns.len().saturating_sub(1)));
        }
        self.row = 0;
        match view {
            "arranger" | "cue" => self.view = View::Arranger,
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
            "speech" => {
                // a Throat-mode Voice that speaks words (0.2.3.10)
                let mut p = crate::voice::preset("Demon Words").unwrap_or_default();
                p.base.name = "Demon Words".into();
                self.add_instrument("Demon Words", InstrumentKind::Voice(p));
                self.dirty = false;
            }
            "transform" => {
                // a Voice with the Transform section in use (0.2.3.11)
                let mut p = crate::voice::preset("Metal Demon").unwrap_or_default();
                p.base.name = "Metal Demon".into();
                self.add_instrument("Metal Demon", InstrumentKind::Voice(p));
                self.dirty = false;
                super::voiceview::TF_SCROLL.with(|f| f.set(true));
            }
            "drums" | "kit" | "chipfx" | "analog" | "fm" | "noise" => {
                // a Generator (0.2.3.12): the Zap kick or the Psy Kit;
                // Chip FX (0.2.3.13): the Psy Laser
                let name = match view {
                    "kit" => "Psy Kit",
                    "chipfx" => "Psy Laser",
                    "analog" => "Psy Rolling Bass",
                    "fm" => "Metallic Bell",
                    "noise" => "White Riser",
                    _ => "Psy Zap Kick",
                };
                let p = crate::gen::preset(name).unwrap_or_default();
                self.add_instrument(name, InstrumentKind::Gen(p));
                self.dirty = false;
            }
            "slice" => {
                // a breakbeat in the sample editor, sliced at its transients
                self.add_instrument(
                    "Break",
                    InstrumentKind::Sample {
                        file: "break.wav".into(),
                        rate: 48000,
                        data: crate::sample::demo_break(48000).into(),
                        base_note: 49,
                        lp: Default::default(),
                    },
                );
                let idx = self.instr as usize - 1;
                self.view = View::Sample;
                self.slice_sample(idx, None);
                self.select_slice(idx, 5);
                self.dirty = false;
            }
            "settings" => {
                self.view = View::Settings;
                self.set_card = 1; // Look (themes, scale, accent)
                self.set_open = Some(1);
            }
            "about" => {
                self.show_about = true;
                // CHIPWAVE_SHOT_ABOUT_TAB=credits|licences, CHIPWAVE_SHOT_ABOUT_FILTER=<text>
                self.about.tab = match std::env::var("CHIPWAVE_SHOT_ABOUT_TAB").as_deref() {
                    Ok("credits") => about::AboutTab::Credits,
                    Ok("licences") => about::AboutTab::Licences,
                    _ => about::AboutTab::About,
                };
                if let Ok(f) = std::env::var("CHIPWAVE_SHOT_ABOUT_FILTER") {
                    self.about.filter = f;
                }
            }
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
            // --song with a plugin instrument: select it (panel + parameters);
            // CHIPWAVE_SHOT_EDITOR=1 also opens its editor window
            "plugin" => {
                let i = self.with(|s| {
                    s.song
                        .instruments
                        .iter()
                        .position(|x| matches!(x.kind, InstrumentKind::Plugin(_)))
                });
                if let Some(i) = i {
                    self.instr = i as u8 + 1;
                    let extra = Some((i, self.ch));
                    crate::plugin::sync(&self.shared, extra);
                    if std::env::var_os("CHIPWAVE_SHOT_EDITOR").is_some() {
                        let r = self.with(|s| {
                            let k = s.engine.plugins.primary(i)?;
                            Some(
                                s.engine.plugins.slots[k]
                                    .inst
                                    .open_editor("Plugin – ChipWave"),
                            )
                        });
                        eprintln!("editor: {r:?}");
                    }
                }
            }
            // the "Add plugin instrument" browser (cache: CHIPWAVE_PLUGIN_CACHE)
            "plugins" => {
                self.plug.scan_msg = " ".into();
                self.plug.browser = true;
            }
            _ => {}
        }
        // CHIPWAVE_SHOT_INSTR=<n>: select instrument n (1-based, as in the list)
        if let Some(n) = std::env::var("CHIPWAVE_SHOT_INSTR")
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
        {
            let len = self.with(|s| s.song.instruments.len()) as u8;
            self.instr = n.clamp(1, len.max(1));
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
            // the play marker (where Play song started) stays behind it
            self.cue = (13.min(self.order_len().saturating_sub(1)), 32);
        } else if view == "cue" {
            // stopped, play marker moved by a ruler click into the 6th clip, bar 3
            let n = self.order_len();
            self.seek_to((5.min(n.saturating_sub(1)), 32));
        } else if !matches!(
            view,
            "fill" | "settings" | "about" | "library" | "chords" | "slice"
        ) && std::env::var_os("CHIPWAVE_SHOT_NOPLAY").is_none()
        {
            self.play(3);
        }
        // the lower panel (0.2.3.19): CHIPWAVE_SHOT_LOWER=instrument | arranger |
        // automation | full, CHIPWAVE_SHOT_TAB=main | modulation | fx | advanced,
        // CHIPWAVE_SHOT_SEARCH=<query>, CHIPWAVE_SHOT_CLASSIC=1 (old sidebar editor)
        if let Ok(m) = std::env::var("CHIPWAVE_SHOT_LOWER") {
            match m.as_str() {
                "instrument" => self.show_instrument_panel(),
                "arranger" => self.set_lower(0),
                "automation" => self.set_lower(2),
                "full" => {
                    self.set_lower(0);
                    self.toggle_full_height();
                }
                _ => {}
            }
        }
        if let Ok(tb) = std::env::var("CHIPWAVE_SHOT_TAB") {
            self.ip.tab = match tb.as_str() {
                "modulation" => super::params::Tab::Modulation,
                "fx" => super::params::Tab::Fx,
                "advanced" => super::params::Tab::Advanced,
                _ => super::params::Tab::Main,
            };
        }
        if let Ok(q) = std::env::var("CHIPWAVE_SHOT_SEARCH") {
            self.ip.query = q;
            self.ip.search_open = true;
        }
        if std::env::var_os("CHIPWAVE_SHOT_CLASSIC").is_some() {
            self.extra.classic_editor = true;
        }
        // CHIPWAVE_SHOT_MIXSCROLL=end: the mixer scrolled to its last strip
        if std::env::var("CHIPWAVE_SHOT_MIXSCROLL").as_deref() == Ok("end") {
            ctx.data_mut(|d| d.insert_temp(egui::Id::unique("mixer_scroll_to_end"), true));
        }
        // CHIPWAVE_SHOT_SEL=r0,c0,r1,c1: a pattern selection (0-based rows and
        // channels), e.g. for the Block bar transpose buttons; add
        // CHIPWAVE_SHOT_NOPLAY=1 so the view stays at the top of the block
        if let Ok(v) = std::env::var("CHIPWAVE_SHOT_SEL") {
            let n: Vec<usize> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if let [r0, c0, r1, c1] = n[..] {
                self.sel = Some(((r0, c0), (r1, c1)));
                self.ch = c0;
            }
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
