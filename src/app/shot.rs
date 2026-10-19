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
pub const VIEWS: [&str; 40] = [
    "pattern",
    "arranger",
    "timeline",
    "timeline-new",
    "timeline-mode",
    "timeline-switch",
    "timeline-notes",
    "timeline-audio",
    "timeline-automation",
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
    "plugin-fx",
    "modulate",
    "modulate-fx",
    "wave",
    "grid",
    "grid-fx",
    "browser",
    "browser-samples",
    "plugin-setup",
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
        // the Browser shows in its own views only (the others stay as before)
        self.extra.browser_open = view.starts_with("browser");
        self.psetup = None;
        match view {
            "browser" => {
                self.view = View::Timeline;
                self.ch = 1;
                self.br.filter.tab = crate::browser::Tab::Instruments;
                self.br.filter.tags = vec!["Bass"];
                self.br_refresh();
                let keys: Vec<String> = self
                    .br_result_items()
                    .iter()
                    .map(|i| i.key.clone())
                    .collect();
                for (k, key) in keys.iter().enumerate().take(6) {
                    self.extra
                        .browser_ratings
                        .insert(key.clone(), [3, 2, 3, 1, 2, 3][k]);
                    if k % 2 == 0 {
                        self.extra.browser_favs.push(key.clone());
                    }
                }
                self.br.sel = keys.first().cloned();
            }
            "browser-samples" => {
                self.view = View::Timeline;
                let d = std::env::temp_dir()
                    .join(format!("chipwave-shot-samples-{}", std::process::id()));
                for (sub, names) in [
                    (
                        "Drums",
                        &["Kick 909", "Snare Tight", "Hat Closed", "Clap Big"][..],
                    ),
                    ("Bass", &["Sub Sine C", "Reese Low"][..]),
                    ("FX", &["Riser 8 bars", "Impact Boom"][..]),
                ] {
                    let _ = std::fs::create_dir_all(d.join(sub));
                    for (k, n) in names.iter().enumerate() {
                        let f = 55.0 * (k as f32 + 1.0);
                        let data: Vec<f32> = (0..9600)
                            .flat_map(|i| {
                                let v = (i as f32 / 48000.0 * f * std::f32::consts::TAU).sin()
                                    * (1.0 - i as f32 / 9600.0)
                                    * 0.5;
                                [v, v]
                            })
                            .collect();
                        let _ = crate::engine::write_wav(
                            &d.join(sub).join(format!("{n}.wav")),
                            &data,
                            48000,
                        );
                    }
                }
                self.br.filter.tab = crate::browser::Tab::Samples;
                self.br_add_folder(d);
            }
            "plugin-setup" => {
                self.view = View::Timeline;
                self.open_plugin_setup(true);
            }
            "arranger" | "cue" => self.view = View::Arranger,
            "timeline"
            | "timeline-mode"
            | "timeline-notes"
            | "timeline-audio"
            | "timeline-automation" => self.view = View::Timeline,
            "timeline-switch" => {
                self.view = View::Timeline;
                self.tlv.ask_switch = Some(None);
            }
            "timeline-new" => {
                self.new_song();
                self.dirty = false;
            }
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
            "drums" | "kit" | "chipfx" | "analog" | "fm" | "noise" | "wave" | "grid" => {
                // a Generator (0.2.3.12): the Zap kick or the Psy Kit;
                // Chip FX (0.2.3.13): the Psy Laser
                let name = match view {
                    "kit" => "Psy Kit",
                    "chipfx" => "Psy Laser",
                    "analog" => "Psy Rolling Bass",
                    "fm" => "Metallic Bell",
                    "noise" => "White Riser",
                    "wave" => "Wave Vowel Bass",
                    "grid" => "Grid Saw Bass",
                    _ => "Psy Zap Kick",
                };
                // CHIPWAVE_SHOT_PRESET=<name>: another generator preset
                let pick = std::env::var("CHIPWAVE_SHOT_PRESET").ok();
                let name = match &pick {
                    Some(n) if crate::gen::preset(n).is_some() => n.as_str(),
                    _ => name,
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
            // FX view of the first instrument with a plugin effect; with
            // CHIPWAVE_SHOT_FX=<file.clap> that effect is added to the
            // Supersaw Pad first; CHIPWAVE_SHOT_EDITOR=1 opens its editor
            "plugin-fx" => {
                if let Some(p) = std::env::var_os("CHIPWAVE_SHOT_FX") {
                    let p = PathBuf::from(p);
                    let r = crate::plugin::scan::check_file(&p);
                    if let Some(f) = r.plugins.into_iter().find(|x| x.desc.is_effect()) {
                        let mut f = f;
                        f.path = p.to_string_lossy().into_owned();
                        let patch = super::fxplugview::patch_of(f);
                        self.with(|s| {
                            if let Some(i) = s.song.instruments.get_mut(6) {
                                i.fx.push(crate::fx::Device::plugin(patch));
                            }
                        });
                    }
                }
                let found = self.with(|s| {
                    s.song.instruments.iter().enumerate().find_map(|(i, x)| {
                        x.fx.iter().find(|d| d.plugin.is_some()).map(|d| (i, d.uid))
                    })
                });
                if let Some((i, uid)) = found {
                    self.instr = i as u8 + 1;
                    self.view = View::Fx;
                    self.sync_fx_plugins();
                    if std::env::var_os("CHIPWAVE_SHOT_EDITOR").is_some() {
                        let r = self.with(|s| {
                            let k = s.engine.fx_plugins.primary(i, uid)?;
                            Some(
                                s.engine.fx_plugins.slots[k]
                                    .inst
                                    .open_editor("Plugin effect – ChipWave"),
                            )
                        });
                        eprintln!("editor: {r:?}");
                    }
                }
            }
            // a Grid FX device (Comb Resonator patch) on the Supersaw Pad,
            // its patch window open in the FX view
            "grid-fx" => {
                self.with(|s| {
                    if let Some(i) = s.song.instruments.get_mut(6) {
                        let mut d = crate::fx::Device::new(crate::fx::FxKind::Grid);
                        d.grid = crate::grid::fx_presets()
                            .into_iter()
                            .nth(2)
                            .map(|x| Box::new(x.2));
                        i.fx.push(d);
                    }
                });
                self.instr = 7;
                self.view = View::Fx;
                let n = self.with(|s| s.song.instruments.get(6).map_or(1, |i| i.fx.len()));
                self.ip.grid_fx = Some((6, n - 1));
                self.dirty = false;
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
        // drag-to-modulate: modulators with routes on the selected
        // instrument's knobs and FX (instrument 7 unless CHIPWAVE_SHOT_INSTR)
        if view == "modulate" || view == "modulate-fx" {
            if std::env::var("CHIPWAVE_SHOT_INSTR").is_err() {
                self.instr = 7;
            }
            self.shot_modulation();
            if view == "modulate-fx" {
                self.view = View::Fx;
            }
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
        } else if view == "timeline-mode"
            || view == "timeline-notes"
            || view == "timeline-audio"
            || view == "timeline-automation"
        {
            // timeline mode with a few edits: a loop region, a marker, a
            // muted clip, a track speed, two selected clips; playing from
            // order entry CHIPWAVE_SHOT_POS (default 8)
            let pos = std::env::var("CHIPWAVE_SHOT_POS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(8);
            let row = self.with(|s| {
                let (_, map) = crate::timeline::from_order(&s.song);
                let k = pos.min(map.starts.len().saturating_sub(1));
                map.starts.get(k).copied().unwrap_or(0) / crate::timeline::ROW
            }) as usize;
            self.enter_timeline();
            let sels = self.with(|s| {
                let t = s.song.timeline.as_deref_mut().unwrap();
                let bar = 4 * crate::timeline::PPQ;
                let a = (row as i64 * crate::timeline::ROW / bar) * bar;
                crate::timeline::set_loop(t, a, a + 8 * bar);
                let i = crate::timeline::add_marker(t, a + 4 * bar);
                t.markers[i].name = "Hook".into();
                if let Some(t3) = t.tracks.get_mut(3) {
                    t3.speed = 3;
                }
                let on: Vec<u32> = t
                    .clips
                    .iter()
                    .filter(|c| c.start <= a + 2 * bar && c.end() > a + 2 * bar)
                    .map(|c| c.id)
                    .collect();
                if let Some(&m) = on.get(4) {
                    crate::timeline::toggle_mute(t, &[m]);
                }
                let Shared { song, engine, .. } = s;
                engine.ensure_plan(song);
                engine.start(song, 0, row, false, 0);
                on.into_iter().take(2).collect::<Vec<u32>>()
            });
            self.tlv.sel = sels.last().copied();
            self.tlv.sels = sels;
            self.tlv.cue = (row as i64) * crate::timeline::ROW;
            if view == "timeline-notes" {
                self.shot_note_clip(row);
            }
            if view == "timeline-audio" {
                self.shot_audio_clips(row);
            }
            if view == "timeline-automation" {
                self.shot_automation(row);
            }
        } else if view == "timeline" {
            // play the song from order entry CHIPWAVE_SHOT_POS (default 8)
            let pos = std::env::var("CHIPWAVE_SHOT_POS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(8);
            self.with(|s| {
                let Shared { song, engine, .. } = s;
                let pos = pos.min(song.order.len().saturating_sub(1));
                engine.start(song, pos, 0, false, 0);
            });
        } else if view == "cue" {
            // stopped, play marker moved by a ruler click into the 6th clip, bar 3
            let n = self.order_len();
            self.seek_to((5.min(n.saturating_sub(1)), 32));
        } else if !matches!(
            view,
            "fill"
                | "settings"
                | "about"
                | "library"
                | "chords"
                | "slice"
                | "timeline-new"
                | "timeline-switch"
        ) && std::env::var_os("CHIPWAVE_SHOT_NOPLAY").is_none()
        {
            self.play(3);
        }
        // the lower panel (0.2.3.19): CHIPWAVE_SHOT_LOWER=instrument | arranger |
        // automation | full, CHIPWAVE_SHOT_TAB=main | modulation | fx | advanced | grid,
        // CHIPWAVE_SHOT_SEARCH=<query>, CHIPWAVE_SHOT_CLASSIC=1 (old sidebar editor)
        if let Ok(m) = std::env::var("CHIPWAVE_SHOT_LOWER") {
            match m.as_str() {
                "instrument" => self.show_instrument_panel(),
                "arranger" => self.set_lower(0),
                "automation" => self.set_lower(2),
                "piano" => self.set_lower(3),
                "piano-ch" => {
                    // the piano roll on channel CHIPWAVE_SHOT_SEL (1-based)
                    self.set_lower(3);
                    if let Some(c) = std::env::var("CHIPWAVE_SHOT_SEL")
                        .ok()
                        .and_then(|v| v.parse::<usize>().ok())
                    {
                        self.ch = c.saturating_sub(1);
                    }
                }
                "full" => {
                    self.set_lower(0);
                    self.toggle_full_height();
                }
                _ => {}
            }
        }
        if view.starts_with("timeline") {
            self.view = View::Timeline;
        }
        // CHIPWAVE_SHOT_LOWER_H=<px>: the lower panel's height
        if let Some(h) = std::env::var("CHIPWAVE_SHOT_LOWER_H")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            self.extra.lower_h = h;
        }
        if let Ok(tb) = std::env::var("CHIPWAVE_SHOT_TAB") {
            // keep the tab (a Grid would open on its patch)
            self.gen_ui.inst = (self.instr as usize).saturating_sub(1);
            self.ip.tab = match tb.as_str() {
                "modulation" => super::params::Tab::Modulation,
                "fx" => super::params::Tab::Fx,
                "advanced" => super::params::Tab::Advanced,
                "grid" => super::params::Tab::Grid,
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
    /// modulators and routes for the "modulate" screenshots
    fn shot_modulation(&mut self) {
        use crate::modulation::{LfoShape, ModKind};
        let idx = (self.instr as usize).wrapping_sub(1);
        let sel = self.with(|s| {
            let inst = s.song.instruments.get(idx)?;
            let nfx = inst.fx.len();
            let m = &mut s.song.modulation;
            let n = idx + 1;
            let lfo = m.add(idx, ModKind::Lfo)?;
            m.mods[lfo].sync = 2.0;
            m.mods[lfo].shape = LfoShape::Triangle;
            let env = m.add(idx, ModKind::Envelope)?;
            m.mods[env].a = 0.05;
            m.mods[env].d = 0.4;
            let mac = m.add(idx, ModKind::Macro)?;
            m.mods[mac].name = "Brightness".into();
            m.mods[mac].value = 0.7;
            let rnd = m.add(idx, ModKind::Random)?;
            m.mods[rnd].rate = 4.0;
            m.mods[rnd].smooth = 0.6;
            let id = |k: usize| m.mods[k].id;
            let (l, e, c, r) = (id(lfo), id(env), id(mac), id(rnd));
            m.connect(l, &format!("instrument.{n}.cutoff"), 0.3);
            m.connect(e, &format!("instrument.{n}.cutoff"), -0.2);
            m.connect(e, &format!("instrument.{n}.resonance"), 0.35);
            m.connect(c, &format!("instrument.{n}.patch.detune_cents"), 0.4);
            m.connect(r, &format!("instrument.{n}.patch.width"), 0.5);
            m.connect(c, &format!("instrument.{n}.patch.lfo-amp"), 0.3);
            if nfx > 0 {
                m.connect(l, &format!("instrument.{n}.fx.1.p1"), 0.25);
                m.connect(c, &format!("instrument.{n}.fx.1.p2"), -0.3);
            }
            if nfx > 1 {
                m.connect(r, &format!("instrument.{n}.fx.2.p1"), 0.4);
            }
            Some(l)
        });
        self.ip.mod_sel = sel;
        self.dirty = false;
    }

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

impl App {
    /// timeline-notes: a looped note clip (chords + an arpeggio in the song
    /// key) on track 5 at the play position, selected, in the roll
    /// timeline-audio: a looped drum break with a fade-in, a pad with
    /// fades and gain, and a clip whose file is missing (generated audio)
    fn shot_audio_clips(&mut self, row: usize) {
        use crate::audiofile::{wav_f32, Pcm};
        use crate::timeline::{self as tl, PPQ};
        let bar = 4 * PPQ;
        let at = (row as i64 * tl::ROW / bar) * bar;
        let ids = self.with(|s| {
            let nch = s.song.channels;
            let bpm = s.song.bpm as f64;
            let sr = 44100u32;
            let bar_f = (4.0 * 60.0 / bpm * sr as f64) as usize;
            // a one-bar break: kicks, snares, hats (deterministic noise)
            let mut seed = 0x1234_5678u32;
            let mut noise = move || {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed as f32 / u32::MAX as f32 * 2.0 - 1.0
            };
            let mut drums = vec![0.0f32; bar_f * 2];
            let step = bar_f / 16;
            for k in 0..16 {
                let (kick, snare) = (k % 8 == 0 || k == 10, k % 8 == 4);
                for i in 0..step.min(9000) {
                    let t = i as f32 / sr as f32;
                    let mut v = noise() * 0.12 * (-t * 60.0).exp();
                    if kick {
                        v += (t * std::f32::consts::TAU * (50.0 + 90.0 * (-t * 30.0).exp())).sin()
                            * 0.9
                            * (-t * 9.0).exp();
                    }
                    if snare {
                        v += noise() * 0.55 * (-t * 18.0).exp();
                    }
                    let j = (k * step + i).min(bar_f - 1);
                    drums[j * 2] += v;
                    drums[j * 2 + 1] += v * 0.9;
                }
            }
            let pad: Vec<f32> = (0..sr as usize * 8)
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let f = |hz: f32| (t * hz * std::f32::consts::TAU).sin();
                    (f(220.0) + 0.6 * f(277.2) + 0.5 * f(329.6) + 0.2 * f(440.0))
                        * 0.22
                        * (0.7 + 0.3 * f(0.25))
                })
                .collect();
            let tracks = [4.min(nch - 1), 5.min(nch - 1), 6.min(nch - 1)];
            if let Some(t) = s.song.timeline.as_deref_mut() {
                t.clips.retain(|c| {
                    !(tracks.contains(&(c.track as usize))
                        && c.start < at + 8 * bar
                        && c.end() > at)
                });
            }
            let mut ids = vec![];
            let assets = [
                ("drum break.wav", 2u8, drums),
                ("pad chords.flac", 1u8, pad),
                ("guitar take 3.wav", 1u8, vec![0.0; sr as usize * 4]),
            ];
            for (k, (name, ch, data)) in assets.into_iter().enumerate() {
                let bytes = wav_f32(sr, ch as u16, &data);
                let mut a = tl::AudioAsset::from_pcm(
                    name,
                    &format!("/home/del/samples/{name}"),
                    "wav",
                    Pcm::new(sr, ch, data),
                    bytes,
                );
                if k == 2 {
                    a.pcm = None;
                    a.bytes = None;
                    a.embed = false;
                }
                let t = s.song.timeline.as_deref_mut()?;
                let n = tl::add_audio_asset(t, a);
                let id =
                    tl::new_audio_clip(&mut s.song, tracks[k], at + [0, 2 * bar, 4 * bar][k], n)?;
                let t = s.song.timeline.as_deref_mut()?;
                let c = t.clips.iter_mut().find(|c| c.id == id)?;
                match k {
                    0 => {
                        c.loop_len = Some(c.len);
                        c.len = 8 * bar;
                        c.fade_in = 2 * PPQ;
                    }
                    1 => {
                        c.len = c.len.min(6 * bar);
                        c.fade_in = bar;
                        c.fade_out = 2 * bar;
                        c.gain = 0.7;
                    }
                    _ => {}
                }
                ids.push(id);
            }
            let crate::audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some(ids)
        });
        if let Some(ids) = ids {
            self.tlv.sel = ids.first().copied();
            self.tlv.sels = ids.into_iter().take(1).collect();
        }
        // close enough to see the waveforms
        self.tlv.zoom = 30.0;
        self.tlv.scroll = (at - bar) as f64;
        self.tlv.track_h = 62.0;
    }

    /// timeline-automation: the Bass track unfolded with a volume lane
    /// (a curved swell), a filter lane with a looping clip lane on one
    /// clip (it wins inside the clip), and a pan lane on the Hats track
    fn shot_automation(&mut self, row: usize) {
        use crate::automation::{Lane, Point, Shape, Target};
        use crate::timeline::{self as tl, PPQ};
        let bar = 4 * PPQ;
        let at = (row as i64 * tl::ROW / bar) * bar;
        let r0 = (at / tl::ROW) as f32;
        let pt = |pos: f32, value: f32, shape: Shape, curve: f32| Point {
            pos,
            value,
            shape,
            curve,
        };
        let sel = self.with(|s| {
            let t = s.song.timeline.as_deref_mut()?;
            let bass = 1.min(t.tracks.len() - 1);
            let hats = 2.min(t.tracks.len() - 1);
            let mut vol = Lane::new(Target::ChanVolume(bass), "Bass - Volume".into());
            let v = |g: f32| Target::ChanVolume(bass).to_norm(g);
            vol.points = vec![
                pt(0.0, v(0.9), Shape::Linear, 0.0),
                pt(r0, v(0.9), Shape::Linear, 0.0),
                pt(r0 + 16.0, v(0.35), Shape::Linear, -0.55),
                pt(r0 + 64.0, v(1.2), Shape::Linear, 0.0),
                pt(r0 + 80.0, v(1.2), Shape::Step, 0.0),
                pt(r0 + 96.0, v(0.8), Shape::Smooth, 0.0),
                pt(r0 + 128.0, v(1.0), Shape::Linear, 0.0),
            ];
            let cut = Target::Cutoff(1);
            let mut flt = Lane::new(cut, "02 Octave Bass - Filter cutoff".into());
            flt.points = vec![
                pt(0.0, cut.to_norm(900.0), Shape::Linear, 0.0),
                pt(r0 + 32.0, cut.to_norm(600.0), Shape::Smooth, 0.0),
                pt(r0 + 100.0, cut.to_norm(4000.0), Shape::Linear, 0.0),
            ];
            t.tracks[bass].auto = vec![vol, flt];
            let mut pan = Lane::new(Target::ChanPan(hats), "Hats - Pan".into());
            pan.points = (0..17)
                .map(|k| {
                    pt(
                        r0 + k as f32 * 8.0,
                        if k % 2 == 0 { 0.2 } else { 0.8 },
                        Shape::Smooth,
                        0.0,
                    )
                })
                .collect();
            t.tracks[hats].auto = vec![pan];
            // a clip lane: a filter wobble that loops with the clip
            let id = t
                .clips
                .iter()
                .filter(|c| c.track as usize == bass && c.start >= at + 2 * bar)
                .min_by_key(|c| c.start)
                .map(|c| c.id);
            if let Some(c) = id.and_then(|id| t.clips.iter_mut().find(|c| c.id == id)) {
                let src = (c.offset / tl::ROW) as f32;
                let rows = (c.loop_len.unwrap_or(c.len) / tl::ROW) as usize;
                let mut w = Lane::new(cut, "02 Octave Bass - Filter cutoff".into());
                w.points = (0..=rows / 4)
                    .map(|k| {
                        pt(
                            src + k as f32 * 4.0,
                            cut.to_norm(if k % 2 == 0 { 300.0 } else { 5000.0 }),
                            Shape::Linear,
                            if k % 2 == 0 { 0.5 } else { -0.5 },
                        )
                    })
                    .collect();
                c.auto = vec![w];
            }
            t.view.auto_open = 1 << bass | 1 << hats;
            let crate::audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            id
        });
        self.tlv.sel = sel;
        self.tlv.sels = sel.into_iter().collect();
        self.tlv.zoom = 9.0;
        self.tlv.scroll = (at - bar) as f64;
        self.tlv.track_h = 46.0;
    }

    fn shot_note_clip(&mut self, row: usize) {
        use crate::chords::{self, ChordKind};
        use crate::timeline::{self as tl, PPQ};
        let bar = 4 * PPQ;
        let at = (row as i64 * tl::ROW / bar) * bar;
        let id = self.with(|s| {
            let key = *s
                .song
                .info
                .key
                .get_or_insert(chords::Key::new(9, chords::Scale::Minor));
            let tr = 4.min(s.song.channels.saturating_sub(1));
            // room for it: the block clips there make way
            if let Some(t) = s.song.timeline.as_deref_mut() {
                t.clips.retain(|c| {
                    !(c.track as usize == tr && c.start < at + 8 * bar && c.end() > at)
                });
            }
            let pad = s
                .song
                .instruments
                .iter()
                .position(|i| i.kind.patch().is_some_and(|p| p.amp.s >= 0.5))
                .map_or(1, |i| i as u8 + 1);
            let id = tl::new_note_clip(&mut s.song, tr, at)?;
            let t = s.song.timeline.as_deref_mut()?;
            let c = t.clips.iter_mut().find(|c| c.id == id)?;
            c.loop_len = Some(4 * bar);
            c.len = 8 * bar;
            let tl::ClipKind::Notes { data } = c.kind else {
                return None;
            };
            let d = t.notes[data as usize].make_mut();
            d.name = "Chords".into();
            d.len = 4 * bar;
            for (b, deg) in [0usize, 5, 2, 6].into_iter().enumerate() {
                let ch = chords::diatonic(key, deg, ChordKind::Triad);
                let base = b as i64 * bar;
                for (k, n) in ch.notes(4, 0).into_iter().enumerate() {
                    d.notes.push(tl::Note {
                        start: base,
                        len: bar - PPQ / 4,
                        key: n,
                        vel: tl::roll::vel_u16(0.55 + 0.1 * k as f32),
                        instr: pad,
                        ..Default::default()
                    });
                }
                // an off-grid arpeggio an octave up (triplet eighths)
                for (k, n) in ch.notes(5, 0).into_iter().cycle().take(6).enumerate() {
                    d.notes.push(tl::Note {
                        start: base + k as i64 * PPQ / 3 * 2,
                        len: PPQ / 3,
                        key: n,
                        vel: tl::roll::vel_u16(0.9 - 0.08 * k as f32),
                        instr: pad,
                        ..Default::default()
                    });
                }
            }
            let crate::audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some(id)
        });
        if let Some(id) = id {
            self.tlv.sel = Some(id);
            self.tlv.sels = vec![id];
            self.set_lower(3);
            self.pr.focus = false;
        }
    }
}
