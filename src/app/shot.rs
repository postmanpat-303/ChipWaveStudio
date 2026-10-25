//! Hidden developer flag `--screenshot <out.png> [--view v] [--theme slate|dark|amiga]
//! [--size WxH]`: opens the real app window with the demo song, sets up one view,
//! lets playback run for a moment, captures the window contents
//! (egui ViewportCommand::Screenshot) to a PNG and exits. Used for the README
//! screenshots (scripts/screenshots.sh). Settings are isolated in a temp folder.
use super::*;

pub struct Shot {
    pub out: PathBuf,
    pub view: String,
    pub(super) frame: u32,
    saved: bool,
    /// `--hold <secs>`: keep running (for screen recording) instead of capturing
    pub hold: Option<f32>,
    /// scripted input for piano roll shots: (frame, event)
    script: Vec<(u32, egui::Event)>,
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
            script: vec![],
            start: None,
            last: None,
            image: Default::default(),
        }
    }
}

/// views `--view` accepts
pub const VIEWS: [&str; 125] = [
    "pattern",
    "advedit",
    "advedit-headers",
    "arranger",
    "timeline",
    "timeline-new",
    "timeline-layout",
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
    "browser-instruments",
    "browser-samples",
    "browser-drop",
    "browser-patterns",
    "browser-patterns-styles",
    "browser-patterns-drop",
    "browser-files",
    "browser-plugins",
    "browser-effects",
    "browser-filter",
    "browser-more",
    "plugin-setup",
    "showcase",
    "showcase-progress",
    "devices",
    "devices-drop",
    "timeline-instrument",
    "timeline-drumkit",
    "trackpie",
    "trackpie-icons",
    "trackpie-colour",
    "trackpie-addring-1",
    "trackpie-addring-2",
    "trackpie-addring-3",
    "trackpie-addring-instr",
    "trackauto",
    "tracks-dock",
    "tracks-addpie",
    "tracks-addpie-kits",
    "tracks-addpie-instr",
    "tracks-trackpie",
    "tracks-trackpie-kits",
    "tracks-trackpie-melodic",
    "tracks-trackpie-sounds",
    "tracks-trackpie-empty",
    "tracks-arranger-off",
    "classic-arranger-off",
    "classic-arranger-off-master",
    "split-preview",
    "cliparranger",
    "cliparranger-drag",
    "cliparranger-notes",
    "drop-preview",
    "drop-ask",
    "cliparranger-launcher",
    "clip-pie",
    "lane-pie",
    "clip-pasted",
    "launcher-pie",
    "drum-pie",
    "chooser",
    "focus-playing",
    "focus-chord",
    "focus-prompt",
    "focus-window",
    "record-recording",
    "record-live-open",
    "record-live",
    "record-live-stopped",
    "record-typing",
    "record-idle",
    "record-playing",
    "record-metro-menu",
    "record-countin",
    "record-lcd-master",
    "record-master",
    "racks-panel",
    "racks-add-device",
    "racks-save",
    "racks-browser",
    "knob-mockup",
    "instskin-waveforge",
    "instskin-chains",
    "instskin-browser",
    "instskin-plugins",
    "stringwave-layers",
    "stringwave-filtermod",
    "stringwave-fx",
    "stringwave-full",
    "stringwave-pie",
    "stringwave-panelpie",
    "metronome-mockup",
    "rack-compact",
    "rack-empty",
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
        if view.starts_with("focus-") || view.starts_with("record-") {
            self.shot_focus(view);
            return;
        }
        if view.starts_with("stringwave-") {
            self.shot_stringwave(view);
            self.shot_quickmod();
            return;
        }
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
            "browser-instruments" => {
                // Browser › ChipWave Instruments; FM-4 just dropped on track 3:
                // selected, its editor open under the Timeline
                self.enter_timeline();
                self.default_layout();
                self.br.filter.tab = crate::browser::Tab::Instruments;
                self.br.filter.category = Some(crate::browser::TOOLS_CAT.into());
                self.br_refresh();
                self.track_new_instrument(2, super::trackpie::Builtin::Fm4);
                self.br.sel = Some("tool:FM-4".into());
                self.dirty = false;
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
            // Browser › Plugins with thumbnails; CHIPWAVE_SHOT_LOAD=<name>
            // [|clap|vst3] then double-click-loads it (new track + editor)
            "browser-plugins" => {
                self.view = View::Timeline;
                self.extra.browser_thumbs = true;
                self.br.filter.tab = crate::browser::Tab::Plugins;
                self.br_refresh();
                if let Ok(want) = std::env::var("CHIPWAVE_SHOT_LOAD") {
                    let (name, fmt) = want.split_once('|').unwrap_or((want.as_str(), ""));
                    let it = self
                        .br_result_items()
                        .iter()
                        .find(|i| match &i.src {
                            crate::browser::Src::Plugin(f) => {
                                f.desc.name == name && (fmt.is_empty() || f.format == fmt)
                            }
                            _ => false,
                        })
                        .map(|i| (*i).clone());
                    if let Some(it) = it {
                        self.br.sel = Some(it.key.clone());
                        self.br_load(&it, ctx);
                    } else {
                        eprintln!("CHIPWAVE_SHOT_LOAD: no plugin {want}");
                    }
                }
            }
            // Browser direction A: Effects with its "Adds to" line, the
            // Filter popover open (two filters on), the More menu open
            "browser-effects" | "browser-filter" | "browser-more" => {
                self.view = View::Timeline;
                self.ch = 1;
                let (tab, pop) = match view {
                    "browser-effects" => (
                        crate::browser::Tab::Effects,
                        super::browserpanel::BrPop::None,
                    ),
                    "browser-filter" => (
                        crate::browser::Tab::Instruments,
                        super::browserpanel::BrPop::Filter,
                    ),
                    _ => (
                        crate::browser::Tab::Wavetables,
                        super::browserpanel::BrPop::More,
                    ),
                };
                self.br.filter.tab = tab;
                if view == "browser-filter" {
                    self.br.filter.tags = vec!["Bass"];
                    self.br.filter.creator = Some("ChipWave".into());
                }
                self.br_refresh();
                self.br.sel = self.br_result_items().first().map(|i| i.key.clone());
                self.br.pop = pop;
            }
            "browser-drop" => {
                // dragging an instrument below the last track: "New track: …"
                self.view = View::Timeline;
                self.br.filter.tab = crate::browser::Tab::Instruments;
                self.br.filter.query = "808 Boom".into();
                self.br_refresh();
                let it = self.br_result_items().first().map(|i| (*i).clone());
                self.br.sel = it.as_ref().map(|i| i.key.clone());
                self.br.drag = it;
                self.br.shot_drop = true;
            }
            "browser-patterns" | "browser-patterns-styles" | "browser-patterns-drop" => {
                // Browser › Drum Patterns: the styles, or the Psytrance folder
                // with one pattern previewing (the song stays stopped)
                use crate::drumpat::PATTERNS;
                self.view = View::Timeline;
                self.br.filter.tab = crate::browser::Tab::Patterns;
                if view != "browser-patterns-styles" {
                    let i = PATTERNS.iter().position(|p| p.style == 0).unwrap_or(0);
                    self.br.pat.folder = Some(PATTERNS[i].style);
                    for p in PATTERNS.iter().filter(|p| p.style == 0).take(2) {
                        self.extra.browser_favs.push(p.key());
                    }
                    if view == "browser-patterns" {
                        self.pat_toggle(i + 1);
                    } else {
                        let it = super::drumpat::pattern_item(i);
                        self.br.sel = it.as_ref().map(|i| i.key.clone());
                        self.br.drag = it;
                        self.br.shot_drop = true;
                    }
                }
            }
            "browser-files" => {
                // Browser › Files in a made-up music folder
                self.view = View::Timeline;
                let d = std::env::temp_dir()
                    .join(format!("chipwave-shot-files-{}", std::process::id()))
                    .join("Music");
                for sub in ["Projects", "Samples", "Stems"] {
                    let _ = std::fs::create_dir_all(d.join(sub));
                }
                for (n, len) in [("Kick 909.wav", 9600usize), ("Lead loop.wav", 48000)] {
                    let data = vec![0.0f32; len * 2];
                    let _ = crate::engine::write_wav(&d.join(n), &data, 48000);
                }
                let _ = std::fs::write(d.join("Night drive.cwt"), b"");
                self.br.filter.tab = crate::browser::Tab::Files;
                self.files_go(d, false);
            }
            "advedit" | "advedit-headers" => {
                // the tracker: Advanced Edit docked on the right with a selection,
                // or the coloured channel headers (one muted, the lower panel hidden)
                self.view = View::Pattern;
                self.extra.full_height = true;
                if view == "advedit" {
                    self.extra.adv_edit_open = true;
                    self.extra.adv_edit_float = false;
                    self.sel = Some(((4, 0), (19, 1)));
                    self.row = 19;
                    self.adv.section = crate::advedit::Section::Selection;
                    self.adv.mask.set(crate::advedit::Field::FxVal, false);
                    self.status = "Advanced Edit: Ctrl+Shift+A".into();
                } else {
                    self.with(|s| s.song.mixer.channels[2].mute = true);
                    self.extra.full_height = true;
                }
            }
            "devices"
            | "devices-drop"
            | "timeline-instrument"
            | "instskin-waveforge"
            | "instskin-chains"
            | "instskin-browser"
            | "instskin-plugins" => {
                // the Devices tab on the Supersaw Pad: EQ, its filter and
                // chorus, a compressor and a reverb; Browser › Effects open
                self.enter_timeline();
                self.default_layout();
                self.instr = 7;
                let nch = self.with(|s| s.song.channels);
                if let Some(c) = (0..nch).find(|&c| self.track_slot(c) == 7) {
                    self.ch = c;
                }
                self.dev.follow_ch = self.ch;
                // the built-in skin (docs/INSTSKIN.md): a WaveForge on the
                // pad's track
                let wf = view.starts_with("instskin");
                let ii = if wf {
                    self.track_new_instrument(self.ch, super::trackpie::Builtin::WaveForge);
                    self.ip.advanced = false;
                    (self.instr as usize).saturating_sub(1)
                } else {
                    6
                };
                self.with(|s| {
                    use crate::fx::{Device, FxKind};
                    let mut eq = Device::new(FxKind::Eq);
                    for (k, v) in [
                        ("lo_freq", 90.0),
                        ("lo_gain", -6.0),
                        ("m1_freq", 700.0),
                        ("m1_gain", 3.5),
                        ("m2_freq", 3200.0),
                        ("m2_gain", -4.0),
                        ("m2_q", 2.0),
                        ("hi_gain", 5.0),
                    ] {
                        eq.set_real(k, v);
                    }
                    let mut comp = Device::new(FxKind::Compressor);
                    super::fxview::apply_preset(&mut comp, 0);
                    let rev = Device::new(FxKind::Reverb);
                    s.song.insert_device(ii, 0, eq);
                    let n = s.song.instruments.get(ii).map_or(0, |i| i.fx.len());
                    s.song.insert_device(ii, n, comp);
                    s.song.insert_device(ii, n + 1, rev);
                    s.song.insert_device(ii, n + 2, Device::new(FxKind::Delay));
                });
                // CHIPWAVE_SHOT_MORE=N: device N with its "…" open
                if let Some(n) = std::env::var("CHIPWAVE_SHOT_MORE")
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    let chain = self.with(|s| s.song.instruments[6].fx.clone());
                    if n < chain.len() {
                        self.dev
                            .open
                            .insert((6, super::devcards::open_key(&chain, n)));
                    }
                }
                // CHIPWAVE_SHOT_COLLAPSE=N: device N folded to a strip (what
                // a double-click on its side tab does)
                if let Some(n) = std::env::var("CHIPWAVE_SHOT_COLLAPSE")
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    self.with(|s| {
                        if let Some(d) = s.song.instruments[ii].fx.get_mut(n) {
                            d.collapsed = true;
                        }
                    });
                }
                self.set_lower(4);
                self.extra.lower_h = 430.0;
                self.dev.inst_folded = std::env::var_os("CHIPWAVE_SHOT_FOLD").is_some();
                self.extra.browser_open = std::env::var_os("CHIPWAVE_SHOT_NO_BROWSER").is_none();
                self.br.filter.tab = crate::browser::Tab::Effects;
                self.br_refresh();
                if view == "instskin-chains" {
                    self.extra.chains_cards = true;
                    self.extra.browser_open = false;
                }
                if matches!(
                    view,
                    "timeline-instrument"
                        | "instskin-waveforge"
                        | "instskin-browser"
                        | "instskin-plugins"
                ) {
                    // the Instrument tab (simple card) on the same pad
                    self.set_lower(1);
                    self.extra.browser_open =
                        view.ends_with("-browser") || view.ends_with("-plugins");
                    if self.extra.browser_open {
                        // Browser › Instruments (ChipWave Instruments first)
                        // or › VST3 & CLAP (neutral badges)
                        self.br.filter.tab = if view == "instskin-plugins" {
                            crate::browser::Tab::Plugins
                        } else {
                            crate::browser::Tab::Instruments
                        };
                        self.br_refresh();
                    }
                }
                if view == "devices-drop" {
                    let it = self.br.items.iter().find(|i| i.key == "fx:Delay").cloned();
                    self.br.sel = it.as_ref().map(|i| i.key.clone());
                    self.br.drag = it;
                    self.br.shot_drop = true;
                    self.dev.shot_at = Some(1);
                }
                self.dirty = false;
            }
            "racks-panel" | "racks-add-device" | "racks-save" | "racks-browser" => {
                self.shot_racks(view);
            }
            "plugin-setup" => {
                self.view = View::Timeline;
                self.open_plugin_setup(true);
            }
            "showcase" | "showcase-progress" => {
                // Browser › Plugins › Get showcase instruments… (no download runs)
                self.view = View::Timeline;
                self.extra.browser_open = true;
                self.br.filter.tab = crate::browser::Tab::Plugins;
                self.open_showcase();
                if view == "showcase-progress" {
                    use crate::showcase::{Job, Progress, State};
                    let p = |id, state, done: f64| {
                        let total = crate::showcase::item(id).map(|i| i.size()).unwrap_or(1);
                        Progress {
                            id,
                            state,
                            done: (total as f64 * done) as u64,
                            total,
                        }
                    };
                    if let Some(d) = self.showcase.as_mut() {
                        d.job = Some(Job {
                            progress: Arc::new(Mutex::new(vec![
                                p("dexed", State::Done, 1.0),
                                p("ob-xf", State::Downloading, 0.62),
                                p("monique", State::Waiting, 0.0),
                                p("adlplug", State::Waiting, 0.0),
                            ])),
                            cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                            done: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                        });
                    }
                }
            }
            "arranger" | "cue" => self.view = View::Arranger,
            "cliparranger" | "cliparranger-drag" => {
                // the default layout on the timeline-mode demo with the Clip
                // Arranger; scene 03 plays (more is queued just before the shot)
                self.enter_timeline();
                self.default_layout();
                self.extra.clip_arranger = true;
                self.extra.reduce_motion = true;
                self.ch = 5;
                if view == "cliparranger" {
                    self.ca_scene(3);
                }
                self.dirty = false;
            }
            "cliparranger-notes"
            | "drop-preview"
            | "drop-ask"
            | "cliparranger-launcher"
            | "clip-pie"
            | "lane-pie"
            | "clip-pasted"
            | "launcher-pie"
            | "drum-pie" => {
                // a new song: a take on track 01 at bars 1 and 5, a Drum Kit
                // pattern on track 02 (launched a few frames later)
                self.new_song();
                for k in 0..2 {
                    self.track_insert(k, if k == 0 { "Lead" } else { "Drums" });
                }
                self.view = View::Timeline;
                self.default_layout();
                self.extra.clip_arranger = true;
                self.extra.reduce_motion = true;
                self.track_new_instrument(0, super::trackpie::Builtin::WaveForge);
                self.with(|s| {
                    use crate::timeline::{self as tl, PPQ};
                    let bar = 4 * PPQ;
                    for (at, base) in [(0, 57u8), (4 * bar, 60)] {
                        let Some(id) = tl::new_note_clip(&mut s.song, 0, at) else {
                            continue;
                        };
                        let t = s.song.timeline.as_deref_mut().unwrap();
                        let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
                        c.len = 4 * bar;
                        let tl::ClipKind::Notes { data } = c.kind else {
                            continue;
                        };
                        let d = t.notes[data as usize].make_mut();
                        d.name = if base == 57 {
                            "Verse".into()
                        } else {
                            "Hook".into()
                        };
                        d.len = 4 * bar;
                        for k in 0..16i64 {
                            let step = [0u8, 3, 7, 10, 12, 10, 7, 3][k as usize % 8];
                            d.notes.push(tl::Note {
                                start: k * PPQ,
                                len: PPQ - PPQ / 4,
                                key: base + step,
                                instr: 1,
                                ..Default::default()
                            });
                        }
                    }
                });
                if let Some(i) = crate::drumpat::PATTERNS
                    .iter()
                    .position(|p| p.name == "Full-on Drive")
                {
                    self.pat_place_now(i, 1, 0);
                }
                self.ch = 0;
                self.dirty = false;
            }
            "chooser" => {
                // a new song's track without an instrument: the chooser
                self.new_song();
                self.track_insert(0, "Lead");
                self.view = View::Timeline;
                self.default_layout();
                self.tl_select_track(0, false);
                self.set_lower(1);
                self.dirty = false;
            }
            "tracks-dock"
            | "tracks-addpie"
            | "tracks-addpie-kits"
            | "tracks-addpie-instr"
            | "tracks-trackpie"
            | "tracks-trackpie-kits"
            | "tracks-trackpie-melodic"
            | "tracks-trackpie-sounds"
            | "tracks-trackpie-empty"
            | "tracks-arranger-off"
            | "classic-arranger-off"
            | "classic-arranger-off-master"
            | "rack-compact"
            | "rack-empty" => {
                // a new song with three tracks: Song / Master docked at the
                // bottom of the list, room under the tracks for the Add pie
                self.new_song();
                for (k, n) in ["Lead", "Bass", "Drums"].into_iter().enumerate() {
                    self.track_insert(k, n);
                }
                self.view = View::Timeline;
                self.default_layout();
                self.extra.clip_arranger = true;
                self.extra.reduce_motion = true;
                // classic-*: the classic toolbar with the Arranger hidden,
                // its compact Master meter (popover open in -master)
                let classic = view.starts_with("classic-");
                self.extra.side_hidden = view.ends_with("arranger-off") || classic;
                if classic {
                    self.extra.lcd_toolbar = false;
                    self.lcd.master_open = view == "classic-arranger-off-master";
                }
                self.track_new_instrument(0, super::trackpie::Builtin::WaveForge);
                self.track_new_instrument(1, super::trackpie::Builtin::Analog);
                self.track_set_pattern_instrument(
                    2,
                    super::trackpie::Builtin::DrumKit,
                    Some("909 Kit"),
                );
                // -empty: a fourth track with no instrument yet
                if view == "tracks-trackpie-empty" || view == "rack-empty" {
                    self.track_add(3);
                }
                self.tl_select_track(2, false);
                // rack-compact: the pie's Add › WaveForge on track 01, its
                // compact Rack card; rack-empty: the empty track's Rack
                if view == "rack-compact" {
                    self.track_new_instrument(0, super::trackpie::Builtin::WaveForge);
                }
                if view == "rack-empty" {
                    self.tl_select_track(3, false);
                    self.set_lower(4);
                }
                self.dirty = false;
            }
            "trackpie"
            | "trackpie-icons"
            | "trackpie-colour"
            | "trackpie-addring-1"
            | "trackpie-addring-2"
            | "trackpie-addring-3"
            | "trackpie-addring-instr"
            | "trackauto"
            | "split-preview" => {
                // the default layout on the timeline-mode demo; the pie
                // opens a few frames later (shot_pie)
                self.enter_timeline();
                self.default_layout();
                self.dirty = false;
            }
            "timeline"
            | "timeline-mode"
            | "timeline-notes"
            | "timeline-audio"
            | "timeline-automation" => self.view = View::Timeline,
            "timeline-switch" => {
                self.view = View::Timeline;
                self.tlv.ask_switch = Some(None);
            }
            // the Drum Kit editor: a library pattern dropped on a new track
            // (CHIPWAVE_SHOT_PAT=<name>, default "Full-on Drive"); the
            // generator's result when CHIPWAVE_SHOT_DK=generate (=added: then Add to track; =sound: the Sound tab)
            "timeline-drumkit" => {
                self.enter_timeline();
                let name = std::env::var("CHIPWAVE_SHOT_PAT").unwrap_or("Full-on Drive".into());
                if let Some(i) = crate::drumpat::PATTERNS.iter().position(|p| p.name == name) {
                    self.pat_place_now(i, u8::MAX, 0);
                }
                self.dk.inst = (self.instr as usize).saturating_sub(1);
                self.dk.sel = 3;
                let dkv = std::env::var("CHIPWAVE_SHOT_DK").unwrap_or_default();
                if dkv == "sound" {
                    self.dk.tab = 1;
                }
                if dkv == "generate" || dkv == "added" {
                    self.dk.tab = 2;
                    self.dk.gen.result = Some(crate::drumkit::generate(1, 7, 0.7, 1, 0));
                }
                // "added": Generate › Add to track with the marker in bar 5
                if dkv == "added" {
                    let bar = self.with(|s| {
                        s.song
                            .timeline
                            .as_deref()
                            .map_or(3840, |t| t.sig_at(0).bar_ticks())
                    });
                    self.tlv.cue = 4 * bar + 240;
                    if let Some((notes, steps, swing, _)) = self.dk.gen.result.clone() {
                        let d = crate::timeline::NoteClipData {
                            name: "Groove 2".into(),
                            len: steps as crate::timeline::Tick * crate::timeline::ROW,
                            notes,
                            chords: vec![],
                            drum: Some(crate::drumkit::DrumMeta { swing }),
                        };
                        self.dk_add_to_track(self.instr, None, super::drumkitview::AddSrc::New(d));
                    }
                }
            }
            "timeline-new" => {
                self.new_song();
                self.dirty = false;
            }
            "timeline-layout" => {
                // the default layout on a timeline-mode song, a clip with
                // notes selected (track CHIPWAVE_SHOT_SEL, default 2)
                self.enter_timeline();
                self.default_layout();
                let tr = std::env::var("CHIPWAVE_SHOT_SEL")
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(2);
                let id = self.with(|s| {
                    let t = s.song.timeline.as_deref()?;
                    t.clips
                        .iter()
                        .filter(|c| c.track as usize == tr)
                        .min_by_key(|c| c.start)
                        .map(|c| c.id)
                });
                self.ch = tr;
                self.tlv.sel = id;
                self.tlv.sels = id.into_iter().collect();
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
        self.shot_quickmod();
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
                // CHIPWAVE_SHOT_GEN=chords|melody|bass: the Generators panel
                // open on that clip
                if let Ok(g) = std::env::var("CHIPWAVE_SHOT_GEN") {
                    self.cl_open_gen_at(self.tlv.cue.max(0) as crate::timeline::Tick);
                    self.cl.gen.tab = match g.as_str() {
                        "melody" => super::genpanel::GenTab::Melody,
                        "bass" => super::genpanel::GenTab::Bass,
                        _ => super::genpanel::GenTab::Chords,
                    };
                }
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
                | "cliparranger"
                | "cliparranger-drag"
                | "cliparranger-notes"
                | "cliparranger-launcher"
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
        // CHIPWAVE_SHOT_FLAT=1: the Flat interface style
        if std::env::var_os("CHIPWAVE_SHOT_FLAT").is_some() {
            self.extra.flat_ui = true;
            self.apply_theme(ctx);
        }
        // CHIPWAVE_SHOT_INSPECTOR=track | clip: the left Inspector open on
        // the cursor track (CHIPWAVE_SHOT_SEL, 1-based) or the selected clip
        if let Ok(m) = std::env::var("CHIPWAVE_SHOT_INSPECTOR") {
            self.extra.inspector_open = true;
            if let Some(c) = std::env::var("CHIPWAVE_SHOT_SEL")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
            {
                self.ch = c.saturating_sub(1);
            }
            self.inspector_shot(m == "clip");
        }
        // CHIPWAVE_SHOT_ADVANCED=1 (or a tab): the full editor, not the simple card
        if std::env::var_os("CHIPWAVE_SHOT_ADVANCED").is_some()
            || std::env::var_os("CHIPWAVE_SHOT_TAB").is_some()
        {
            self.ip.advanced = true;
        }
        if let Ok(tb) = std::env::var("CHIPWAVE_SHOT_TAB") {
            // keep the tab (a Grid would open on its patch)
            self.gen_ui.inst = (self.instr as usize).saturating_sub(1);
            self.ip.tab = match tb.as_str() {
                "modulation" => super::params::Tab::Modulation,
                "fx" => super::params::Tab::Fx,
                "advanced" => super::params::Tab::Advanced,
                "grid" => super::params::Tab::Grid,
                "synth" => super::params::Tab::Synth,
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
    /// quick-mod shots: CHIPWAVE_SHOT_ADV=1 the full editor,
    /// CHIPWAVE_SHOT_TAB=modulation its Modulation tab, CHIPWAVE_SHOT_POLYSEC=<n>
    /// WaveForge's section, CHIPWAVE_SHOT_MOVES=1 three movements on the
    /// selected instrument, CHIPWAVE_SHOT_QUICK=<knob> that knob's
    /// "Modulate … with…" popover open
    fn shot_quickmod(&mut self) {
        use crate::quickmod::{self as qm, Source, Spec};
        if std::env::var_os("CHIPWAVE_SHOT_ADV").is_some() {
            self.ip.advanced = true;
        }
        if std::env::var("CHIPWAVE_SHOT_TAB").as_deref() == Ok("modulation") {
            self.ip.tab = super::params::Tab::Modulation;
        }
        if let Some(n) = std::env::var("CHIPWAVE_SHOT_POLYSEC")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            self.ip.poly.sec = n;
        }
        if std::env::var_os("CHIPWAVE_SHOT_MOVES").is_some() {
            let idx = (self.instr as usize).wrapping_sub(1);
            self.with(|s| {
                let bpm = s.song.bpm;
                let Some(inst) = s.song.instruments.get_mut(idx) else {
                    return;
                };
                let l = super::params::build(inst, idx, 0);
                let picks = [
                    (Source::Lfo, 0.35),
                    (Source::Envelope, -0.25),
                    (Source::Random, 0.2),
                    (Source::Velocity, 0.4),
                ];
                let mut k = 0;
                for want in ["Cutoff", "Detune", "Pan", "Level", "Width", "Resonance"] {
                    let Some(tg) = l
                        .params
                        .iter()
                        .find(|p| p.name == want)
                        .and_then(|p| p.target)
                    else {
                        continue;
                    };
                    let Some(&(source, amount)) = picks.get(k) else {
                        break;
                    };
                    let spec = Spec {
                        source,
                        amount,
                        ..Spec::default()
                    };
                    if qm::add(inst, idx, &mut s.song.modulation, tg, &spec, bpm).is_ok() {
                        k += 1;
                    }
                }
            });
        }
        self.ip.quick_req = std::env::var("CHIPWAVE_SHOT_QUICK").ok();
        // CHIPWAVE_SHOT_QUICKDOT=<s>: the preview's running dot, frozen
        self.ip.quick_dot = std::env::var("CHIPWAVE_SHOT_QUICKDOT")
            .ok()
            .and_then(|v| v.parse().ok());
        // CHIPWAVE_SHOT_QUICKSRC=envelope|random|…: the popover's source
        self.ip.quick_src = std::env::var("CHIPWAVE_SHOT_QUICKSRC").ok().and_then(|v| {
            Source::ALL
                .into_iter()
                .find(|s| s.name().eq_ignore_ascii_case(&v.replace('-', " ")))
        });
        self.dirty = false;
    }

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
                // Slate / Dark Circuit / Amiga every 2.5 s, over pattern, mixer and arranger
                let step = (t / 2.5) as usize;
                let want = theme::THEMES[step % theme::THEMES.len()];
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
        let f = sh.frame;
        let scripted = !sh.script.is_empty();
        if f == SETUP_AT + 40 && !scripted {
            // park the pointer away from the widgets (no hover highlights)
            raw.events.push(egui::Event::PointerGone);
        }
        if let Some(sh) = &mut self.shot {
            let (now, later): (Vec<_>, Vec<_>) =
                sh.script.drain(..).partition(|(at, _)| *at == f + 1);
            sh.script = later;
            for (_, e) in now {
                if let egui::Event::PointerButton { modifiers, .. } = &e {
                    raw.events.push(egui::Event::ModifiersChanged(*modifiers));
                }
                raw.events.push(e);
            }
            if sh.script.is_empty() && scripted {
                // keep a sentinel so the pointer stays where the script left it
                sh.script.push((u32::MAX, egui::Event::PointerGone));
            }
        }
    }

    /// once per frame in screenshot mode
    pub(super) fn shot_frame(&mut self, ctx: &egui::Context) {
        let Some(sh) = &mut self.shot else { return };
        sh.frame += 1;
        let (f, view) = (sh.frame, sh.view.clone());
        ctx.request_repaint();
        if f == 1 {
            // CHIPWAVE_SHOT_BROWSER_W=<pt>: the Browser at that width, from
            // its first frame (a panel keeps the width it first opened at)
            if let Some(w) = std::env::var("CHIPWAVE_SHOT_BROWSER_W")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
            {
                self.extra.browser_w = w;
            }
        }
        if f == SETUP_AT {
            self.shot_setup(ctx, &view);
        }
        if view == "knob-mockup" && f >= SETUP_AT {
            knob_mockup(ctx);
        }
        if view == "metronome-mockup" && f >= SETUP_AT {
            super::metroicon::metronome_mockup(ctx);
        }
        if f == SETUP_AT + 14 {
            if let Ok(m) = std::env::var("CHIPWAVE_SHOT_PR") {
                self.shot_pr(ctx, &m);
            }
        }
        if view == "cliparranger-notes" && f == SETUP_AT + 20 {
            // the Hook slot on track 01 plays (the drums held); the Bar 1
            // scene is queued for the next bar
            self.ca_slot(0, 1);
        }
        if view == "cliparranger-notes" && f == CAPTURE_AT - 6 {
            self.ca_scene(0);
        }
        if matches!(
            view.as_str(),
            "clip-pie" | "launcher-pie" | "drum-pie" | "lane-pie" | "clip-pasted"
        ) {
            self.shot_obj_pie(&view, f);
        }
        if view == "cliparranger-launcher" && f == SETUP_AT + 16 {
            // Capture, a variation of the Verse, then scene 1 plays
            self.ca_capture_now();
        }
        if view == "cliparranger-launcher" && f == SETUP_AT + 20 {
            self.ca_scene(0);
        }
        if view == "cliparranger" && f == CAPTURE_AT - 6 {
            // queued for the next bar: Pad 1 plays block 04, Kick stops
            self.ca_slot(5, 4);
            self.ca_stop(0);
        }
        if (view == "drop-preview" || view == "drop-ask") && f == SETUP_AT + 12 {
            // the Verse slot (track 01, bar 1) dragged onto its own track at
            // bar 3, over itself: the red preview, then the question
            use super::dropplan::DropSrc;
            let at = 2 * 4 * crate::timeline::PPQ;
            let id = self.with(|s| {
                s.song
                    .timeline
                    .as_deref()
                    .and_then(|t| t.clips.iter().find(|c| c.track == 0 && c.start == 0))
                    .map(|c| c.id)
            });
            if let (Some(id), Some((_, row))) = (id, self.tlv.track_rects(0)) {
                let p = Pos2::new(self.tlv.x_of(at) + 30.0, row.center().y);
                if view == "drop-ask" {
                    let plan = self.drop_plan(p, DropSrc::Clip(id), false);
                    self.drop_release(DropSrc::Clip(id), plan, false);
                } else {
                    self.ca.drag = Some(DropSrc::Clip(id));
                    self.ca.shot_drag = Some(p);
                }
            }
        }
        if view == "cliparranger-drag" && f == SETUP_AT + 12 {
            let at = 40 * 4 * crate::timeline::PPQ;
            if let Some((_, row)) = self.tlv.track_rects(4) {
                let x = self.tlv.x_of(at) + 30.0;
                self.ca.drag = Some(super::dropplan::DropSrc::Block { block: 2, col: 4 });
                self.ca.shot_drag = Some(Pos2::new(x, row.center().y));
            }
        }
        if (view.starts_with("trackpie") || view == "trackauto") && f == SETUP_AT + 12 {
            self.shot_pie(ctx, &view);
        }
        if view.starts_with("tracks-") && view.contains("pie") && f == SETUP_AT + 12 {
            self.shot_tracks_pie(ctx, &view);
        }
        if view == "timeline" && f == SETUP_AT + 12 {
            self.shot_tracksize();
        }
        if f == SETUP_AT + 12 || f == SETUP_AT + 24 {
            self.shot_dbl_loop(f == SETUP_AT + 12);
        }
        if f == SETUP_AT + 12 {
            self.shot_keys();
            self.shot_lcd();
            self.shot_loop();
            self.shot_kit_after(ctx);
        }
        if view == "split-preview" && f == SETUP_AT + 12 {
            self.shot_split();
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
                // the count-in shot holds its countdown
                if engine.playing && view != "record-countin" && view != "record-live-open" {
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
            // CHIPWAVE_SHOT_PRSEL=1: the first two bars' notes selected
            if std::env::var_os("CHIPWAVE_SHOT_PRSEL").is_some() {
                let data = self.with(|s| {
                    let t = s.song.timeline.as_deref()?;
                    match t.clips.iter().find(|c| c.id == id)?.kind {
                        crate::timeline::ClipKind::Notes { data } => Some(data as usize),
                        _ => None,
                    }
                });
                if let Some(data) = data {
                    self.pr.target = Some(super::pianoview::Target::Notes {
                        data,
                        clip: Some(id),
                    });
                    self.pr.sel = (0..18).collect();
                }
            }
        }
    }
}

impl App {
    /// CHIPWAVE_SHOT_PR=click | erase | select | line | chords | chord-pie |
    /// chord-block-pie | chord-generate | melody-generate[-adv] |
    /// bass-generate[-adv|-moved] (view timeline-notes): piano roll
    /// shots with scripted mouse input
    fn shot_pr(&mut self, ctx: &egui::Context, mode: &str) {
        use super::chordlane::PieOn;
        use crate::chordgen as cg;
        use crate::timeline::{self as tl, PPQ, ROW};
        self.with(|s| s.engine.stop());
        let Some(t) = self.pr_target() else { return };
        let bar = 4 * PPQ;
        self.pr.zoom = 22.0;
        self.pr.scroll_x = 0.0;
        self.pr.focus = true;
        let key_y = |a: &App, k: u8| a.pr.y_of(k) + a.pr.key_h / 2.0;
        let x = |a: &App, tk: tl::Tick| a.pr.x_of(tk);
        let mut sc: Vec<(u32, egui::Event)> = vec![];
        let base = CAPTURE_AT.saturating_sub(100);
        let mv = |p: Pos2| egui::Event::PointerMoved(p);
        let btn = |p: Pos2, b: egui::PointerButton, down: bool, m: egui::Modifiers| {
            egui::Event::PointerButton {
                pos: p,
                button: b,
                pressed: down,
                modifiers: m,
            }
        };
        let none = egui::Modifiers::NONE;
        let prim = egui::PointerButton::Primary;
        match mode {
            "click" | "erase" | "select" | "line" => {
                if mode == "click" {
                    // an empty clip: the script draws into it
                    if let Some(super::pianoview::Source::Notes(mut d)) = self.pr_read(t) {
                        d.notes.clear();
                        self.pr_write(t, super::pianoview::Source::Notes(d));
                    }
                }
                self.pr.scroll_y =
                    (120.0 - 62.0) * self.pr.key_h - self.pr.grid_rect.height() / 2.0;
            }
            _ => {
                self.pr.scroll_y =
                    (120.0 - 50.0) * self.pr.key_h - self.pr.grid_rect.height() / 2.0;
            }
        }
        // the layout follows the scroll next frame; positions from the new scroll
        let gy = self.pr.grid_rect;
        let _ = gy;
        match mode {
            "click" => {
                let (r, k0) = (ROW, 58u8);
                // 1: click a note; 2: drag its edge to 4 rows; 3..: single clicks
                let p0 = Pos2::new(x(self, 0) + 5.0, key_y(self, k0));
                let e0 = Pos2::new(x(self, r) - 2.0, key_y(self, k0));
                let e1 = Pos2::new(x(self, 4 * r) + 1.0, key_y(self, k0));
                let mut f = base;
                for e in [
                    mv(p0),
                    btn(p0, prim, true, none),
                    btn(p0, prim, false, none),
                ] {
                    f += 3;
                    sc.push((f, e));
                }
                f += 6;
                sc.push((f, mv(e0)));
                f += 3;
                sc.push((f, btn(e0, prim, true, none)));
                for k in 1..=6 {
                    f += 2;
                    sc.push((f, mv(e0 + (e1 - e0) * (k as f32 / 6.0))));
                }
                f += 2;
                sc.push((f, btn(e1, prim, false, none)));
                for (i, (row, key)) in [
                    (4usize, 61u8),
                    (8, 65),
                    (12, 63),
                    (16, 58),
                    (20, 61),
                    (24, 65),
                    (28, 70),
                ]
                .into_iter()
                .enumerate()
                {
                    let p = Pos2::new(x(self, row as i64 * r) + 6.0, key_y(self, key));
                    f += 4;
                    sc.push((f, mv(p)));
                    f += 2;
                    sc.push((f, btn(p, prim, true, none)));
                    f += 2;
                    sc.push((f, btn(p, prim, false, none)));
                    let _ = i;
                }
                f += 4;
                sc.push((f, mv(Pos2::new(x(self, 34 * r), key_y(self, 66)))));
            }
            "erase" => {
                // a right-drag over the arpeggio, still held at the capture
                let notes = match self.pr_read(t) {
                    Some(super::pianoview::Source::Notes(d)) => d.notes.clone(),
                    _ => vec![],
                };
                let mut arp: Vec<&tl::Note> = notes
                    .iter()
                    .filter(|n| n.len < PPQ && n.start < 2 * bar)
                    .collect();
                arp.sort_by_key(|n| n.start);
                let pts: Vec<Pos2> = arp
                    .iter()
                    .take(9)
                    .map(|n| Pos2::new(x(self, n.start) + 4.0, key_y(self, n.key)))
                    .collect();
                if pts.len() >= 2 {
                    // fast: the last notes are still smoking at the capture
                    let sec = egui::PointerButton::Secondary;
                    let mut f = CAPTURE_AT - 14;
                    sc.push((f, mv(pts[0])));
                    f += 1;
                    sc.push((f, btn(pts[0], sec, true, none)));
                    for w in pts.windows(2) {
                        f += 1;
                        sc.push((f, mv(w[0] + (w[1] - w[0]) * 0.5)));
                        f += 1;
                        sc.push((f, mv(w[1])));
                    }
                }
            }
            "select" => {
                // Select tool, a Shift box in progress
                self.pr.tool = super::pianoview::PrTool::Select;
                let a = Pos2::new(x(self, bar) - 6.0, key_y(self, 72));
                let b = Pos2::new(x(self, 3 * bar - PPQ), key_y(self, 54));
                let sh = egui::Modifiers::SHIFT;
                let mut f = CAPTURE_AT - 30;
                sc.push((f, mv(a)));
                f += 2;
                sc.push((f, btn(a, prim, true, sh)));
                for k in 1..=10 {
                    f += 1;
                    sc.push((f, mv(a + (b - a) * (k as f32 / 10.0))));
                }
            }
            "line" => {
                // Pointer tool, an Alt+drag line from empty space through
                // some of the notes, still held at the capture
                self.pr.tool = super::pianoview::PrTool::Pointer;
                let p = |a: &App, bars: f32, key: u8| {
                    Pos2::new(x(a, (bars * bar as f32) as tl::Tick), key_y(a, key))
                };
                let pts = [
                    p(self, 0.45, 57),
                    p(self, 0.8, 63),
                    p(self, 1.35, 73),
                    p(self, 1.75, 64),
                    p(self, 2.3, 71),
                    p(self, 2.6, 66),
                ];
                let alt = egui::Modifiers::ALT;
                let mut f = CAPTURE_AT - 60;
                sc.push((f, mv(pts[0])));
                f += 2;
                sc.push((f, btn(pts[0], prim, true, alt)));
                for w in pts.windows(2) {
                    for k in 1..=8 {
                        f += 1;
                        sc.push((f, mv(w[0] + (w[1] - w[0]) * (k as f32 / 8.0))));
                    }
                }
            }
            "marker" => {
                // bar 1 copied; a click in the ruler at bar 3 places the
                // paste marker, then Ctrl+V twice: two copies end to end,
                // the marker after them
                self.pr.zoom = 10.0;
                self.pr.tool = super::pianoview::PrTool::Pointer;
                if let Some(super::pianoview::Source::Notes(mut d)) = self.pr_read(t) {
                    d.notes.retain(|n| n.start < bar);
                    if let Some(k) = d.notes.iter().map(|n| n.key).max() {
                        // the copied notes in view
                        self.pr.scroll_y = ((120.0 - k as f32 - 3.0) * self.pr.key_h).max(0.0);
                    }
                    self.pr.sel = (0..d.notes.len()).collect();
                    self.pr_write(t, super::pianoview::Source::Notes(d));
                }
                self.pr_copy(false, ctx);
                let rp = Pos2::new(x(self, 2 * bar) + 3.0, self.pr.grid_rect.top() - 8.0);
                let cmd = egui::Modifiers::COMMAND;
                let kv = |down: bool| egui::Event::Key {
                    key: egui::Key::V,
                    physical_key: None,
                    pressed: down,
                    repeat: false,
                    modifiers: cmd,
                };
                let mut f = CAPTURE_AT - 60;
                sc.push((f, mv(rp)));
                f += 2;
                sc.push((f, btn(rp, prim, true, none)));
                f += 2;
                sc.push((f, btn(rp, prim, false, none)));
                for _ in 0..2 {
                    f += 6;
                    sc.push((f, kv(true)));
                    f += 2;
                    sc.push((f, kv(false)));
                }
                // the mouse off the ruler (no tooltip over the result)
                f += 4;
                sc.push((
                    f,
                    mv(Pos2::new(
                        x(self, bar / 2),
                        self.pr.grid_rect.bottom() - 4.0,
                    )),
                ));
            }
            _ => {
                // chord shots: a Cinematic progression in E minor on the lane
                self.with(|s| {
                    s.song.info.key = Some(crate::chords::Key::new(4, crate::chords::Scale::Minor))
                });
                if let Some(super::pianoview::Source::Notes(mut d)) = self.pr_read(t) {
                    d.notes.clear();
                    self.pr_write(t, super::pianoview::Source::Notes(d));
                }
                let o = cg::GenOpts {
                    style: 0,
                    root: 4,
                    mode: cg::Mode::Minor,
                    bars: 8,
                    prog: Some(1),
                    colour: 0.55,
                    variation: 0.3,
                    walking_bass: true,
                    seed: 3,
                    ..Default::default()
                };
                let marks = cg::generate(&o).marks;
                self.cl_insert(t, marks.clone(), true, cg::Voicing::Pad, 4);
                self.cl.gen.opts = o;
                self.pr.zoom = 10.0;
                let lane = self.cl.rect;
                let ly = lane.center().y;
                match mode {
                    "chord-pie" => {
                        let at = Pos2::new(x(self, 5 * bar) + 20.0, ly);
                        self.cl_open_menu(ctx, at, PieOn::Lane(4 * bar), t);
                        if let Some((pie, _)) = &self.cl.pie {
                            // hover Dark psy, then its second progression
                            let wi = 5;
                            let (w, c) = (pie.wedge_centre(wi, 75.0), pie.child_centre(wi, 1));
                            sc.push((CAPTURE_AT - 30, mv(w)));
                            sc.push((CAPTURE_AT - 20, mv(c)));
                        }
                    }
                    "chord-block-pie" => {
                        let m = marks.get(2).cloned().unwrap_or_default();
                        let at = Pos2::new(x(self, m.start) + 30.0, ly);
                        self.cl.sel = Some(2);
                        self.cl_open_menu(ctx, at, PieOn::Mark(2), t);
                        if let Some((pie, _)) = &self.cl.pie {
                            let wi = 7; // Write notes
                            let (w, c) = (pie.wedge_centre(wi, 75.0), pie.child_centre(wi, 6));
                            sc.push((CAPTURE_AT - 30, mv(w)));
                            sc.push((CAPTURE_AT - 20, mv(c)));
                        }
                    }
                    "chord-generate" => {
                        self.cl_open_gen_at(0);
                        self.cl.gen.opts = o;
                        self.cl.gen.write = true;
                    }
                    "melody-generate" | "melody-generate-adv" => {
                        self.gen_open_melody(0);
                        self.cl.gen.opts = o;
                        self.cl.gen.mel.adv = mode.ends_with("-adv");
                    }
                    "bass-generate" | "bass-generate-adv" | "bass-generate-moved" => {
                        self.gen_open_bass(0);
                        self.cl.gen.opts = o;
                        self.cl.gen.bass.opts.style = crate::bassgen::BassStyle::Syncopated;
                        self.cl.gen.bass.adv = mode.ends_with("-adv");
                        if mode.ends_with("-moved") {
                            // as if dragged by its grip (down and left)
                            self.extra.gen_pos = Some([40.0, 150.0]);
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(sh) = &mut self.shot {
            sh.script = sc;
        }
    }
}

impl App {
    /// `--view tracks-addpie[-kits|-instr]`: the Add pie on the empty
    /// space under the tracks (-kits: its Drum pattern ring, -instr: its
    /// Instrument ring); `tracks-trackpie[-kits]`: the drum track 03's pie,
    /// its Add ring open (-kits: Swap kit ›); -melodic / -sounds: the
    /// melodic track 01's (-sounds: its Sounds ›); -empty: the empty track
    /// 04's
    fn shot_tracks_pie(&mut self, ctx: &egui::Context, view: &str) {
        use super::trackring::{ring_slots, Level};
        let lanes = self.tlv.lanes;
        let heads = self.tlv.heads;
        let lay = self.tlv.layout.clone();
        let sy = self.tlv.scroll_y;
        let now = ctx.input(|i| i.time);
        let empty = view.starts_with("tracks-addpie");
        let tr = match view {
            "tracks-trackpie-melodic" | "tracks-trackpie-sounds" => 0usize,
            "tracks-trackpie-empty" => 3,
            _ => 2,
        };
        if empty {
            let y = heads.top() + lay.body_end - sy + 95.0;
            self.open_add_pie(Pos2::new(lanes.left() + 40.0, y), false, ctx);
        } else {
            let y = heads.top() + lay.top.get(tr).copied().unwrap_or(0.0) - sy + 20.0;
            self.open_track_pie(tr, Pos2::new(heads.left() + 140.0, y), false, ctx);
        }
        let info = self.ring_info_for(tr, empty);
        let n = |l| ring_slots(l, &info).len();
        let pos = |label: &str| {
            ring_slots(Level::Add, &info)
                .iter()
                .position(|s| s.label == label)
                .unwrap_or(0)
        };
        let into = match view {
            "tracks-addpie-kits" => Some((Level::DrumInstr, pos("Drum pattern"))),
            "tracks-addpie-instr" => Some((Level::Instrument, pos("Instrument track"))),
            "tracks-trackpie-kits" => Some((Level::DrumInstr, pos("Swap kit"))),
            "tracks-trackpie-sounds" => Some((Level::Sounds, pos("Sounds"))),
            _ => None,
        };
        if let Some(p) = &mut self.pie {
            p.open_ring(now, n(Level::Add));
            if let (Some(r), Some((l, k))) = (&mut p.ring, into) {
                r.enter(l, k, n(Level::Add), n(l), now);
                r.key = Some(1);
            }
        }
    }

    /// `--view trackpie[-icons]`: the track pie on track 2's header, Icon
    /// highlighted (trackpie-icons: its TRACK ICON grid popped out).
    /// CHIPWAVE_SHOT_PIE_T=<s> freezes the open animation at that time
    /// (frames for the animation GIF); CHIPWAVE_SHOT_PIE_KEY=<n|none> picks
    /// the highlighted segment (default 2 = Icon).
    fn shot_pie(&mut self, ctx: &egui::Context, view: &str) {
        let lanes = self.tlv.lanes;
        let tr = 1usize.min(self.with(|s| s.song.channels.saturating_sub(1)));
        let at = Pos2::new(lanes.left() - 120.0, lanes.top() + 175.0);
        self.open_track_pie(tr, at, false, ctx);
        let key = match std::env::var("CHIPWAVE_SHOT_PIE_KEY").ok().as_deref() {
            Some("none") => None,
            Some(k) => k.parse::<usize>().ok(),
            None => Some(2),
        };
        let frozen = std::env::var("CHIPWAVE_SHOT_PIE_T")
            .ok()
            .and_then(|v| v.parse::<f64>().ok());
        let now = ctx.input(|i| i.time);
        let info = self.pie_info(tr);
        if let Some(p) = &mut self.pie {
            p.key = key;
            p.frozen = frozen;
            if view == "trackpie-icons" {
                p.sub = Some((2, now));
                p.sub_key = Some(6);
            }
            // the Add rings: level 1, Drum pattern › and its Drum Kit ›
            if let Some(lv) = view.strip_prefix("trackpie-addring-") {
                use super::trackring::{ring_slots, Level};
                let n = |l| ring_slots(l, &info).len();
                p.open_ring(now, n(Level::Add));
                if let Some(r) = &mut p.ring {
                    // Instrument ›: the built-ins (StringWave too) and Browse…
                    if lv == "instr" {
                        let k = ring_slots(Level::Add, &info)
                            .iter()
                            .position(|s| s.label == "Instrument")
                            .unwrap_or(0);
                        r.enter(
                            Level::Instrument,
                            k,
                            n(Level::Add),
                            n(Level::Instrument),
                            now,
                        );
                    } else if lv == "3" {
                        // Sounds ›: the melodic track's presets
                        let k = ring_slots(Level::Add, &info)
                            .iter()
                            .position(|s| s.label == "Sounds")
                            .unwrap_or(1);
                        r.enter(Level::Sounds, k, n(Level::Add), n(Level::Sounds), now);
                    } else if lv != "1" {
                        r.enter(Level::DrumInstr, 1, n(Level::Add), n(Level::DrumInstr), now);
                    }
                }
            }
        }
        if view == "trackauto" {
            self.pie_apply(tr, super::trackpie::PieAct::Automation, ctx);
            self.pie = None;
        }
        if view == "trackpie-colour" {
            // the pointer goes from Colour straight to its panel, over the
            // ring's next segment: the panel stays open
            if let Some(p) = &mut self.pie {
                p.key = None;
                p.frozen = None;
            }
            let Some(c) = self.pie.as_ref().map(|p| p.centre) else {
                return;
            };
            let a = super::trackpie::seg_angle(1);
            let from = c + egui::vec2(a.cos(), a.sin()) * 110.0;
            let info = self.pie_info(tr);
            let items = super::trackpie::sub_items(super::trackpie::SEGS[1], &info);
            let (size, _) = super::trackpie::sub_layout(super::trackpie::SEGS[1], &items);
            let panel = super::trackpie::sub_rect(c, 1, size, ctx.content_rect());
            let to = panel.center();
            let mut sc = vec![];
            for i in 0..=12u32 {
                let t = i as f32 / 12.0 * 0.62;
                sc.push((
                    SETUP_AT + 30 + i * 2,
                    egui::Event::PointerMoved(from + (to - from) * t),
                ));
            }
            if let Some(sh) = &mut self.shot {
                sh.script = sc;
            }
        }
    }

    /// `--view split-preview`: S on the timeline with the pointer over a
    /// clip on track 2 (the amber split line, its label, the clip outlined)
    /// Stage 5: the pies on the Verse clip, its launcher copy and the
    /// Drum Kit pattern's step grid
    fn shot_obj_pie(&mut self, view: &str, f: u32) {
        use super::objpie::PieFor;
        let clip_on = |me: &Self, tr: u16| {
            me.with(|s| {
                s.song
                    .timeline
                    .as_deref()?
                    .clips
                    .iter()
                    .find(|c| c.track == tr && c.start == 0)
                    .map(|c| (c.id, c.notes()))
            })
        };
        match (view, f) {
            ("launcher-pie", x) if x == SETUP_AT + 12 => self.ca_capture_now(),
            ("drum-pie", x) if x == SETUP_AT + 12 => {
                if let Some((id, _)) = clip_on(self, 1) {
                    self.tlv.sel = Some(id);
                    self.tlv.sels = vec![id];
                    self.ch = 1;
                    self.instr = self.track_instr(1);
                    self.set_lower(1);
                }
            }
            // CHIPWAVE_SHOT_LINKED=1: the take duplicated twice and named
            // "Take 1" (three linked clips, 'Take 1 ×3')
            ("clip-pie", x)
                if x == SETUP_AT + 16 && std::env::var_os("CHIPWAVE_SHOT_LINKED").is_some() =>
            {
                if let Some((id, _)) = clip_on(self, 0) {
                    self.tlv.sel = Some(id);
                    self.tlv.sels = vec![id];
                    for _ in 0..2 {
                        self.tl_apply(super::timeline::TlOp::Duplicate);
                    }
                    self.with(|s| {
                        if let Some(c) = s
                            .song
                            .timeline
                            .as_deref_mut()
                            .and_then(|t| t.clips.iter_mut().find(|c| c.id == id))
                        {
                            c.name = Some("Take 1".into());
                        }
                    });
                }
            }
            ("clip-pie", x) if x == SETUP_AT + 20 => {
                if let (Some((id, _)), Some((_, row))) = (clip_on(self, 0), self.tlv.track_rects(0))
                {
                    self.tlv.sel = Some(id);
                    self.tlv.sels = vec![id];
                    let p = Pos2::new(self.tlv.x_of(2 * crate::timeline::PPQ), row.center().y);
                    self.opie_req = Some((PieFor::Clip(id), p));
                }
            }
            // the Verse copied (Ctrl+C); lane-pie: the empty lane on track
            // 01 after the Hook; clip-pasted: pasted there just before the shot
            ("lane-pie" | "clip-pasted", x) if x == SETUP_AT + 20 => {
                if let Some((id, _)) = clip_on(self, 0) {
                    self.tlv.sel = Some(id);
                    self.tlv.sels = vec![id];
                    self.tl_copy(false);
                }
            }
            ("lane-pie", x) if x == SETUP_AT + 24 => {
                let at = 8 * 4 * crate::timeline::PPQ;
                if let Some((_, row)) = self.tlv.track_rects(0) {
                    let p = Pos2::new(self.tlv.x_of(at + crate::timeline::PPQ), row.center().y);
                    self.opie_req = Some((PieFor::Lane { track: 0, at }, p));
                }
            }
            ("clip-pasted", x) if x == CAPTURE_AT - 3 => {
                self.tl_paste(Some((0, 8 * 4 * crate::timeline::PPQ)), false);
            }
            ("launcher-pie", x) if x == SETUP_AT + 24 => {
                let lid = self.with(|s| {
                    let t = s.song.timeline.as_deref()?;
                    crate::timeline::launch_at(t, 0, 0).map(|l| l.clip.id)
                });
                let r = self
                    .ca
                    .slot_rects
                    .iter()
                    .find(|x| x.0 == 0 && x.1 == 0)
                    .map(|x| x.3);
                if let (Some(id), Some(r)) = (lid, r) {
                    self.opie_req = Some((PieFor::Launch(id), r.center()));
                }
            }
            ("drum-pie", x) if x == SETUP_AT + 24 => {
                let cell = self.dk.cells.iter().find(|c| c.0 == 0 && c.1 == 4).copied();
                if let (Some((id, Some(data))), Some((p, _, r))) = (clip_on(self, 1), cell) {
                    self.opie_req = Some((
                        PieFor::Drum {
                            clip: id,
                            data,
                            track: 1,
                            pad: p,
                        },
                        r.center(),
                    ));
                }
            }
            // the outer ring of one wedge, as when the mouse is on it
            (v, x) if x == SETUP_AT + 30 => {
                let want = match v {
                    "clip-pie" => "Copy",
                    "lane-pie" => "Paste here",
                    "clip-pasted" => return,
                    "launcher-pie" => "Place",
                    _ => "Fill",
                };
                if let Some((pie, _)) = &mut self.opie {
                    pie.hover = pie.items.iter().position(|x| x.label == want);
                    // and the pointer on it, so it stays lit
                    if let Some(i) = pie.hover {
                        let c = pie.wedge_centre(
                            i,
                            (super::radial::r_in() + super::radial::r_mid()) / 2.0,
                        );
                        if let Some(sh) = &mut self.shot {
                            sh.script = vec![(x + 2, egui::Event::PointerMoved(c))];
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Racks (docs/racks/PLAN.md): the Supersaw Pad made a rack with a
    /// second layer whose chain has a Split (Drive | dry); the panel, Add
    /// Device, Save Custom Rack, or Browser › My Racks (racks saved in the
    /// Racks folder first)
    fn shot_racks(&mut self, view: &str) {
        use crate::fx::{Device, FxKind};
        self.enter_timeline();
        self.default_layout();
        self.instr = 7;
        let nch = self.with(|s| s.song.channels);
        if let Some(c) = (0..nch).find(|&c| self.track_slot(c) == 7) {
            self.ch = c;
        }
        self.dev.follow_ch = self.ch;
        self.make_rack(6);
        self.add_layer(6, None);
        let b = self.with(|s| match &s.song.instruments[6].kind {
            InstrumentKind::Rack(r) => r.layers.get(1).map(|l| l.slot),
            _ => None,
        });
        let Some(b) = b else { return };
        self.with(|s| s.song.instruments[b].name = "Glass Bell".into());
        self.apply_fx_ops(
            b,
            vec![super::fxview::FxOp::Insert(
                usize::MAX,
                Box::new(Device::split()),
            )],
        );
        let at = self.with(|s| s.song.instruments[b].fx.len() - 1);
        self.apply_fx_ops(
            b,
            vec![super::chains::wrap(
                &[at as u8, 0],
                super::fxview::FxOp::Add(FxKind::Drive, None),
            )],
        );
        self.apply_fx_ops(
            b,
            vec![super::chains::wrap(
                &[at as u8, 1],
                super::fxview::FxOp::Add(FxKind::Delay, None),
            )],
        );
        self.with(|s| s.song.instruments[6].name = "Glass Keys".into());
        self.set_lower(4);
        self.extra.browser_open = false;
        self.dev.sel = Some(super::chains::Sel::Dev(
            b,
            crate::fx::DevPath::from_slice(&[at as u8]).unwrap(),
        ));
        match view {
            "racks-add-device" => self.open_add_device(b, vec![at as u8, 1], usize::MAX),
            "racks-save" => {
                self.dev.sel = Some(super::chains::Sel::Inst(6));
                self.open_save_rack(6);
                if let Some(f) = &mut self.dev.save_rack {
                    f.description =
                        "bright keys with a bell on top, the bell through a Split".into();
                    f.sound_type = "Keys".into();
                    f.tags = "bright, glass, layered".into();
                    f.focus = false;
                }
            }
            "racks-browser" => {
                self.dev.sel = Some(super::chains::Sel::Inst(6));
                if let Some(d) = crate::racks::dir() {
                    let _ = std::fs::create_dir_all(&d);
                    for (name, desc, ty, tags) in [
                        (
                            "Glass Keys",
                            "bright keys with a bell on top",
                            "Keys",
                            "bright, glass",
                        ),
                        (
                            "Reese Wall",
                            "two detuned saws, one through a Split",
                            "Bass",
                            "dark, wide",
                        ),
                        ("Choir Pad", "voice and strings layered", "Pad", "soft"),
                    ] {
                        let info = crate::racks::RackInfo {
                            name: name.into(),
                            description: desc.into(),
                            sound_type: ty.into(),
                            tags: tags.split(", ").map(String::from).collect(),
                            created: crate::racks::today(),
                            updated: crate::racks::today(),
                        };
                        let p = d.join(crate::racks::file_name(name));
                        if let Ok(r) = self.with(|s| crate::racks::from_song(&s.song, 6, info)) {
                            let _ = crate::racks::save(&r, &p);
                        }
                    }
                    self.dev.my_racks.sel = Some(d.join(crate::racks::file_name("Glass Keys")));
                }
                self.dev.racks_dirty = true;
                self.extra.browser_open = true;
                self.br.filter.tab = crate::browser::Tab::Racks;
            }
            _ => {}
        }
        self.dirty = false;
    }

    fn shot_split(&mut self) {
        let Some(c) = self.tlv.cache.as_ref() else {
            return;
        };
        let Some(cl) =
            c.tl.clips
                .iter()
                .filter(|x| x.track == 1 && x.len >= 4 * crate::timeline::PPQ)
                .min_by_key(|x| x.start)
                .cloned()
        else {
            return;
        };
        // zoomed in so the line and its label read at a glance
        self.tlv.zoom = 40.0;
        self.tlv.scroll = 0.0;
        let Some((_, row)) = self.tlv.track_rects(1) else {
            return;
        };
        let ppq = crate::timeline::PPQ;
        let x = self.tlv.x_of(cl.start + (22 * ppq).min(cl.len * 5 / 8)) + 3.0;
        self.tlv.split_arm = true;
        self.status = "Split: click to cut · Esc cancels".into();
        let p = Pos2::new(x, row.center().y);
        if let Some(sh) = &mut self.shot {
            sh.script = (0..6)
                .map(|i| {
                    (
                        SETUP_AT + 40 + i * 3,
                        egui::Event::PointerMoved(p + egui::vec2(i as f32, 0.0)),
                    )
                })
                .collect();
        }
    }
}

impl App {
    /// `--view timeline` with CHIPWAVE_SHOT_TRACKS=resize (track 02 taller,
    /// track 04 shorter) or reorder (track 03's header dragged up between
    /// tracks 01 and 02, held: the insertion line)
    /// CHIPWAVE_SHOT_KEYS=on|off|hover: the toolbar's Keys → Notes button
    /// on, off, or on with the pointer over it (its hover)
    fn shot_keys(&mut self) {
        let Ok(m) = std::env::var("CHIPWAVE_SHOT_KEYS") else {
            return;
        };
        self.typing.on = m != "off";
        if m == "hover" {
            let c = self.kb_rect.center();
            if let Some(sh) = &mut self.shot {
                sh.script = vec![
                    (SETUP_AT + 30, egui::Event::PointerMoved(c)),
                    (SETUP_AT + 40, egui::Event::PointerMoved(c)),
                ];
            }
        }
    }

    /// CHIPWAVE_SHOT_LCD=on: the display's LOOP (bars 1–4 on a timeline) and
    /// REC pills lit
    fn shot_lcd(&mut self) {
        if std::env::var("CHIPWAVE_SHOT_LCD").as_deref() != Ok("on") {
            return;
        }
        self.with(|s| {
            if let Some(t) = s.song.timeline.as_deref_mut() {
                t.loop_range = Some((0, 4 * t.sig_at(0).bar_ticks()));
                t.loop_on = true;
            }
        });
        self.lcd.shot_lit = true;
    }

    /// CHIPWAVE_SHOT_LOOP=on | off: the transport's Loop with bars 1–4 set,
    /// on or off (the pointer on it: its tip)
    fn shot_loop(&mut self) {
        let Ok(m) = std::env::var("CHIPWAVE_SHOT_LOOP") else {
            return;
        };
        self.with(|s| {
            if let Some(t) = s.song.timeline.as_deref_mut() {
                t.loop_range = Some((0, 4 * t.sig_at(0).bar_ticks()));
                t.loop_on = m == "on";
            }
        });
    }

    /// CHIPWAVE_SHOT_DBLLOOP=1: a block clip on screen double-clicked: the
    /// loop exactly around it (the ruler band, the lane tint, the status)
    fn shot_dbl_loop(&mut self, first: bool) {
        if std::env::var("CHIPWAVE_SHOT_DBLLOOP").is_err() {
            return;
        }
        if first {
            // the song in timeline mode first (the double-click loops there)
            self.with(|s| {
                if s.song.timeline.is_none() {
                    crate::timeline::enter_timeline_mode(&mut s.song);
                }
            });
            return;
        }
        let pick = self.tlv.cache.as_ref().and_then(|c| {
            c.tl.clips
                .iter()
                .filter(|cl| cl.start > 0 && cl.block().is_some())
                .find(|cl| {
                    self.tlv
                        .clip_rect(cl.id)
                        .is_some_and(|r| self.tlv.lanes.contains(r.center()))
                })
                .map(|cl| cl.id)
        });
        if let Some(id) = pick {
            self.tlv.sels = vec![id];
            self.tlv.sel = Some(id);
            self.tl_apply(super::timeline::TlOp::LoopClip(id));
        }
    }

    /// CHIPWAVE_SHOT_KITAFTER=1 (with timeline-new): a StringWave on track
    /// 01 with a clip, then Drum kit › 909 here on track 02 (its starter
    /// groove), shown in the step sequencer
    fn shot_kit_after(&mut self, ctx: &egui::Context) {
        use super::trackpie::{Builtin, PieAct};
        use crate::timeline as tl;
        if std::env::var_os("CHIPWAVE_SHOT_KITAFTER").is_none() {
            return;
        }
        self.enter_timeline();
        self.view = View::Timeline;
        if !(self.track_add(0) && self.track_add(1)) {
            return;
        }
        self.track_new_instrument(0, Builtin::StringWave);
        self.with(|s| {
            if let Some(id) = tl::new_note_clip(&mut s.song, 0, 0) {
                let t = s.song.timeline.as_deref_mut().unwrap();
                if let Some(d) = t.clips.iter().find(|c| c.id == id).and_then(|c| c.notes()) {
                    t.notes[d].make_mut().notes.push(tl::Note {
                        key: 49,
                        len: 4 * tl::ROW,
                        instr: 1,
                        ..Default::default()
                    });
                }
            }
        });
        self.pie_apply(1, PieAct::DrumKitHere("909 Kit"), ctx);
        let id = self.with(|s| {
            let t = s.song.timeline.as_deref()?;
            t.clips.iter().find(|c| c.track == 1).map(|c| c.id)
        });
        if let Some(id) = id {
            self.tlv.sel = Some(id);
            self.tlv.sels = vec![id];
            self.ch = 1;
            self.instr = self.track_instr(1);
            self.set_lower(1);
        }
    }

    fn shot_tracksize(&mut self) {
        let Ok(m) = std::env::var("CHIPWAVE_SHOT_TRACKS") else {
            return;
        };
        match m.as_str() {
            "resize" => {
                self.set_track_height(1, 104.0);
                self.set_track_height(3, 36.0);
            }
            "reorder" => {
                let (Some((h2, _)), Some((h0, _))) =
                    (self.tlv.track_rects(2), self.tlv.track_rects(0))
                else {
                    return;
                };
                let from = Pos2::new(h2.left() + 70.0, h2.center().y);
                let to = Pos2::new(h0.left() + 90.0, h0.bottom() + 6.0);
                let mut sc = vec![
                    (SETUP_AT + 30, egui::Event::PointerMoved(from)),
                    (
                        SETUP_AT + 32,
                        egui::Event::PointerButton {
                            pos: from,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: Default::default(),
                        },
                    ),
                ];
                for i in 1..=10u32 {
                    let p = from + (to - from) * (i as f32 / 10.0);
                    sc.push((SETUP_AT + 34 + i * 2, egui::Event::PointerMoved(p)));
                }
                if let Some(sh) = &mut self.shot {
                    sh.script = sc;
                }
            }
            _ => {}
        }
    }
}

/// `--view knob-mockup`: the knobs as the app wires them (left) beside the
/// studio knob (`ember::knob_studio`, right) at its design sizes, painted 1:1
/// by the app's own painter over the window. A design reference board.
fn knob_mockup(ctx: &egui::Context) {
    use crate::ember::{self, StudioKnob};
    let t = th();
    egui::Area::new(egui::Id::unique("knob_mockup"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::ZERO)
        .show(ctx, |ui| {
            let full = ctx.content_rect();
            ui.set_min_size(full.size());
            let p = ui.painter().clone();
            p.rect_filled(full, 0.0, t.bg_bot);
            let head = |p: &egui::Painter, pos: Pos2, s: &str| {
                p.text(
                    pos,
                    egui::Align2::LEFT_TOP,
                    s,
                    theme::label_font(crate::theme::ts::LARGE),
                    t.ink,
                );
            };
            let note = |p: &egui::Painter, pos: Pos2, s: &str| {
                p.text(
                    pos,
                    egui::Align2::LEFT_TOP,
                    s,
                    crate::theme::prop(crate::theme::ts::LABEL),
                    t.ink2,
                );
            };
            head(
                &p,
                Pos2::new(24.0, 14.0),
                "KNOB BOARD · 1:1 · IN THE APP (LEFT) AND THE STUDIO KNOB SIZES (RIGHT)",
            );
            note(
                &p,
                Pos2::new(24.0, 34.0),
                "Painted by ChipWave's own painter at the app's sizes.",
            );
            let colw = (full.width() - 72.0) / 2.0;
            let lx = 24.0;
            let rx = lx + colw + 24.0;
            // ---- 1 Master panel: Volume / Drive / Ceiling (rad 27)
            let y0 = 64.0;
            for (x, now) in [(lx, true), (rx, false)] {
                let r = Rect::from_min_size(Pos2::new(x, y0), egui::vec2(colw, 176.0));
                theme::cartridge(&p, r, t.slate);
                head(
                    &p,
                    r.min + egui::vec2(14.0, 10.0),
                    if now {
                        "MASTER · IN THE APP"
                    } else {
                        "MASTER · STUDIO KNOB 27"
                    },
                );
                let knobs = [
                    ("Volume", 0.80, "+0.0", "-60", "+6", "+0.0 dB"),
                    ("Drive", 0.0, "0.0", "0", "+24", "0.0 dB"),
                    ("Ceiling", 0.92, "-1.0", "-12", "0", "-1.0 dB"),
                ];
                for (i, (name, f, rd, lo, hi, val)) in knobs.into_iter().enumerate() {
                    let c = Pos2::new(r.left() + 90.0 + i as f32 * 120.0, r.top() + 96.0);
                    if now {
                        theme::knob_hero(
                            &p,
                            c,
                            21.0,
                            &StudioKnob {
                                frac: f,
                                title: name,
                                value: rd,
                                min_max: (lo, hi),
                                dim: 1.0,
                                ..Default::default()
                            },
                        );
                    } else {
                        let mut v = f;
                        ember::knob_studio_ui(
                            ui,
                            c,
                            27.0,
                            egui::Id::unique(("mock_master", i)),
                            &mut v,
                            f,
                            StudioKnob {
                                frac: f,
                                title: name,
                                value: val,
                                min_max: (lo, hi),
                                dim: 1.0,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
            // ---- 2 a device card: Supersaw Cutoff / Detune / Attack / Release
            let y1 = y0 + 192.0;
            let dev = [
                ("Cutoff", 0.62, "2.40 kHz", false, "20", "20k"),
                ("Detune", 0.65, "+18 ct", true, "-50", "+50"),
                ("Attack", 0.15, "12 ms", false, "0", "2 s"),
                ("Release", 0.45, "320 ms", false, "0", "4 s"),
            ];
            for (x, now) in [(lx, true), (rx, false)] {
                let r = Rect::from_min_size(Pos2::new(x, y1), egui::vec2(colw, 232.0));
                theme::cartridge(&p, r, t.slate);
                head(
                    &p,
                    r.min + egui::vec2(14.0, 10.0),
                    if now {
                        "DEVICE CARD · SUPERSAW · IN THE APP (60 x 66 CELLS)"
                    } else {
                        "DEVICE CARD · SUPERSAW · STUDIO KNOB"
                    },
                );
                if now {
                    let mut ch = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(Rect::from_min_size(
                                r.min + egui::vec2(14.0, 40.0),
                                egui::vec2(colw - 28.0, 70.0),
                            ))
                            .layout(egui::Layout::left_to_right(egui::Align::Min)),
                    );
                    ch.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    for (k, (name, f, val, ..)) in dev.into_iter().enumerate() {
                        let mut v = f;
                        knob::knob(
                            &mut ch,
                            egui::Id::unique(("mock_old", k)),
                            &mut v,
                            knob::KnobArgs {
                                caption: None,
                                name,
                                value_text: val.to_string(),
                                default: 0.5,
                                steps: 0.0,
                                enabled: true,
                                highlight: false,
                                badge: None,
                                locked: None,
                                tip: "",
                                rings: &[],
                                live: None,
                                auto_led: None,
                                mod_dot: None,
                            },
                        );
                    }
                    note(
                        &p,
                        r.min + egui::vec2(14.0, 124.0),
                        "Slate: the studio knob in the 60 x 66 cell, the value in orange under it.",
                    );
                } else {
                    // the same 60 x 66 cells (rad 14), then a roomier 18 with end labels
                    for (row, rad, cw, cy) in
                        [(0, 14.0f32, 60.0f32, 82.0f32), (1, 18.0, 92.0, 178.0)]
                    {
                        for (k, (name, f, val, bip, lo, hi)) in dev.into_iter().enumerate() {
                            let c =
                                Pos2::new(r.left() + 14.0 + cw / 2.0 + k as f32 * cw, r.top() + cy);
                            ember::knob_studio(
                                &p,
                                c,
                                rad,
                                &StudioKnob {
                                    frac: f,
                                    bipolar: bip,
                                    title: name,
                                    value: val,
                                    min_max: (lo, hi),
                                    dim: 1.0,
                                    ..Default::default()
                                },
                            );
                        }
                        note(
                            &p,
                            Pos2::new(r.left() + 14.0 + 4.0 * cw + 16.0, r.top() + cy - 8.0),
                            if row == 0 {
                                "rad 14 in the 60 x 66 cell"
                            } else {
                                "rad 18 with end labels (92 wide)"
                            },
                        );
                    }
                }
            }
            // ---- 3 the scale variant and the other states (studio knob only)
            let y2 = y1 + 248.0;
            let r = Rect::from_min_size(
                Pos2::new(lx, y2),
                egui::vec2(full.width() - 48.0, full.height() - y2 - 16.0),
            );
            theme::cartridge(&p, r, t.slate);
            head(
                &p,
                r.min + egui::vec2(14.0, 10.0),
                "STUDIO KNOB · SCALE NUMBERS, BIPOLAR, STEPPED, DISABLED",
            );
            let cy = r.top() + 118.0;
            let ms: [(&str, f32); 7] = [
                ("2", 0.0),
                ("8", 0.17),
                ("14", 0.33),
                ("19", 0.5),
                ("25", 0.67),
                ("30", 0.83),
                ("40ms", 1.0),
            ];
            let cases: [(f32, StudioKnob); 5] = [
                (
                    30.0,
                    StudioKnob {
                        frac: 0.02,
                        title: "Attack",
                        value: "2 ms",
                        scale: &ms,
                        dim: 1.0,
                        ..Default::default()
                    },
                ),
                (
                    24.0,
                    StudioKnob {
                        frac: 0.5,
                        bipolar: true,
                        title: "Pan",
                        value: "C",
                        min_max: ("L", "R"),
                        dim: 1.0,
                        ..Default::default()
                    },
                ),
                (
                    24.0,
                    StudioKnob {
                        frac: 0.3,
                        bipolar: true,
                        title: "Ratio",
                        value: "-40",
                        min_max: ("-100", "+100"),
                        dim: 1.0,
                        ..Default::default()
                    },
                ),
                (
                    22.0,
                    StudioKnob {
                        frac: 4.0 / 6.0,
                        title: "Voices",
                        value: "5",
                        min_max: ("1", "7"),
                        dots: 7,
                        dim: 1.0,
                        ..Default::default()
                    },
                ),
                (
                    22.0,
                    StudioKnob {
                        frac: 0.55,
                        title: "Mix",
                        value: "55 %",
                        min_max: ("Raw", "Done"),
                        dim: 0.45,
                        ..Default::default()
                    },
                ),
            ];
            for (i, (rad, k)) in cases.iter().enumerate() {
                let c = Pos2::new(r.left() + 110.0 + i as f32 * 200.0, cy);
                ember::knob_studio(&p, c, *rad, k);
            }
            note(
                &p,
                Pos2::new(r.left() + 14.0, r.bottom() - 22.0),
                "Mix is shown disabled (dimmed).",
            );
        });
}

impl App {
    /// StringWave's panel on a new track instrument in the Timeline at
    /// the automatic lower-panel height (a laptop's view):
    /// stringwave-layers / -filtermod / -fx (the sub-tabs), -full (the full
    /// view), -pie (a layer level's knob pie), -panelpie (the empty panel's
    /// pie); CHIPWAVE_SHOT_SWTEXT=125|150|200 for the text size
    fn shot_stringwave(&mut self, view: &str) {
        self.psetup = None;
        self.enter_timeline();
        self.default_layout();
        self.track_new_instrument(self.ch, super::trackpie::Builtin::StringWave);
        self.ip.advanced = false;
        self.set_lower(1);
        self.extra.lower_h = 0.0;
        self.extra.browser_open = false;
        self.ip.sw.tab = match view {
            "stringwave-filtermod" => 1,
            "stringwave-fx" => 2,
            _ => 0,
        };
        self.ip.sw.full = view == "stringwave-full";
        self.extra.sw_text = std::env::var("CHIPWAVE_SHOT_SWTEXT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        self.ip.sw.pie_req = match view {
            "stringwave-pie" => Some(Some(format!(
                "sw.{}",
                crate::strings::LAYER[1] + crate::strings::L_LVL
            ))),
            "stringwave-panelpie" => Some(None),
            _ => None,
        };
        self.dirty = false;
    }
}
