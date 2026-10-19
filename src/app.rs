//! The egui application: toolbar, instrument panel, pattern editor.
use crate::audio::{self, Shared, SharedRef};
use crate::engine::Engine;
use crate::fileio;
use crate::input;
use crate::model::*;
use crate::synth::{FilterMode, Wave};
use crate::theme::{self, hex, th};
use eframe::egui::{self, Color32, FontId, Key, Pos2, Rect, Sense, Stroke};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod about;
mod arranger;
#[cfg(test)]
mod audioclip_tests;
#[cfg(test)]
mod autolanes_tests;
mod blocklen;
#[cfg(test)]
mod browser_tests;
mod browserpanel;
mod browserview;
mod chordview;
mod dialogs;
mod edit;
mod file;
mod fillview;
mod fxplugview;
mod fxview;
mod genview;
mod instpanel;
#[cfg(test)]
mod instpanel_tests;
mod instruments;
mod keys;
mod knob;
mod lanes;
mod library;
mod midiview;
mod mixer;
mod modview;
#[cfg(test)]
mod modview_tests;
mod noterec;
mod params;
mod pattern;
#[cfg(test)]
mod piano_tests;
mod pianoview;
mod pluginview;
mod plugsetup;
mod sampleview;
mod settingsview;
pub mod shot;
mod timeline;
#[cfg(test)]
mod timeline_m2_tests;
#[cfg(test)]
mod timeline_tests;
mod topbar;
mod trackops;
mod transpose;
#[cfg(test)]
mod ui_tests;
mod undo;
mod updates;
mod voiceview;

/// cursor sub-columns: 0 note, 1-2 instr, 3-4 vol, 5-8 effect
const SUBCOLS: usize = 9;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Pattern,
    Arranger,
    /// tracks x time (docs/TIMELINE.md), app/timeline.rs
    Timeline,
    Mixer,
    Fx,
    Sample,
    Settings,
}

/// muted clip palette for the arranger (block id -> colour)
/// hover text of the Channels boxes (also how UI tests find them)
pub(crate) const CHANNELS_TIP: &str =
    "channels (tracks) in this song, 1-32; the pattern editor and mixer scroll when they don't fit";

const CLIP_COLOURS: [u32; 8] = [
    // no greens: the theme accent is green, so clips must never look "selected"
    0x516D96, 0x8A6D3B, 0x7A4E5C, 0x5E6B7A, 0x6B5E8A, 0x7A6A4E, 0x8A5A4E, 0x4E6A86,
];

const LANE_H: f32 = 64.0;
/// automation curve colours: functional, saturated but muted
const LANE_COLOURS: [u32; 6] = [0x4FC3D9, 0xE8C35A, 0xD98AD0, 0x8FD9A3, 0xE08A5C, 0x9AA8F0];

enum LaneOp {
    Snapshot,
    Add(usize, f32, f32),
    Delete(usize, usize),
    Move(usize, usize, f32, f32),
    Pencil(usize, f32, f32),
}
type Pos = (usize, usize); // (row, channel)

pub struct App {
    shared: SharedRef,
    audio: Option<audio::Audio>,
    audio_err: Option<String>,
    row: usize,
    ch: usize,
    col: usize,
    pattern: usize,
    octave: u8,
    step: usize,
    edit: bool,
    follow: bool,
    instr: u8,
    /// song undo/redo history (see app/undo.rs)
    hist: undo::History,
    /// >0 while a compound action runs: inner snapshot() calls are ignored
    undo_hold: u32,
    path: Option<PathBuf>,
    status: String,
    dirty: bool,
    /// selection anchor and end (row, channel), inclusive
    sel: Option<(Pos, Pos)>,
    /// mouse drag-select anchor
    drag_anchor: Option<Pos>,
    clip: Vec<Vec<Cell>>,
    view: View,
    dock_arranger: bool,
    arr_sel: usize,
    arr_drag: Option<usize>,
    fx_drag: Option<usize>,
    recent: Vec<String>,
    session: Option<crate::recovery::Session>,
    last_autosave: std::time::Instant,
    /// background save in flight: target path + result channel
    save_job: Option<(PathBuf, std::sync::mpsc::Receiver<Result<(), String>>)>,
    save_was_dirty: bool,
    /// problems found while opening a song (shown in a window until dismissed)
    load_warnings: Vec<String>,
    last_heartbeat: std::time::Instant,
    /// crashed sessions found at start (recovery dialog)
    orphans: Vec<crate::recovery::Orphan>,
    smp: sampleview::SampleEd,
    extra: crate::settings::Extra,
    /// update check state (Help > Check for updates…, startup check)
    upd: updates::UpdateUi,
    /// `--screenshot` capture plan (developer flag)
    shot: Option<shot::Shot>,
    keymap: crate::keymap::Keymap,
    /// settings screen: focused card, expanded card, key being rebound
    set_card: usize,
    set_open: Option<usize>,
    rebinding: Option<crate::keymap::Action>,
    bind_msg: String,
    export_dlg: Option<dialogs::ExportDlg>,
    /// Song > Fill to length dialog state
    fill: Option<fillview::FillState>,
    /// chord helper window (0.2.3.9)
    chords: chordview::ChordHelper,
    /// instrument library window (0.2.3.9)
    library: library::Library,
    /// the Browser panel (docs/BROWSER.md)
    br: browserview::BrowserUi,
    /// Find plugins dialog (first run, Settings › Plugins, Browser)
    psetup: Option<plugsetup::PlugSetup>,
    /// instrument list search
    instr_query: String,
    /// Generator editor state: kit piece, Dice history (0.2.3.12)
    gen_ui: genview::GenUi,
    /// lower-panel instrument editor state (0.2.3.19)
    ip: instpanel::InstUi,
    /// instrument being renamed in the list (slot, text)
    renaming: Option<(u8, String)>,
    /// the rename field has been focused (losing focus after that commits)
    rename_focus: bool,
    /// view when the rename started (switching views commits)
    rename_view: View,
    show_info: bool,
    /// action waiting for the Save / Don't save / Cancel answer
    prompt: Option<crate::prompt::Pending>,
    /// user chose "Don't save" (recovery copy may be deleted on exit)
    discarded: bool,
    allow_close: bool,
    /// clip to highlight briefly after duplicate/insert, with start time
    arr_flash: Option<(Vec<usize>, f64)>,
    /// selected song-order entries (multi-select); arr_sel is the anchor
    arr_multi: std::collections::BTreeSet<usize>,
    /// arranger clipboard: block ids
    arr_clip: Vec<usize>,
    /// keyboard focus is in the docked arranger (the full view always has it)
    arr_focus: bool,
    /// modifiers at the last input event (Shift for Ctrl+Shift+V)
    mods: egui::Modifiers,
    /// debug key log (Ctrl+Shift+K): last 10 input events
    key_log: Option<std::collections::VecDeque<String>>,
    show_auto: bool,
    auto_pencil: bool,
    auto_shape: crate::automation::Shape,
    /// selected lane (for Delete lane / shape) and selected point
    auto_lane: usize,
    auto_sel: Option<(usize, usize)>,
    auto_drag: Option<(usize, usize)>,
    auto_pencil_lane: Option<usize>,
    /// wheel accumulators (fractional notches) for pattern / arranger
    wheel_pat: f32,
    /// first channel shown in the pattern editor (horizontal scroll)
    pub(crate) ch_view: usize,
    /// the pattern editor's channel scroll bar (empty when all channels fit)
    pub(crate) ch_bar: egui::Rect,
    /// mixer strip scroll: pending offset from the bar, and the bar's rect
    mix_scroll: Option<f32>,
    pub(crate) mix_bar: egui::Rect,
    wheel_arr: f32,
    arr_scroll: f32,
    arr_scroll_set: Option<f32>,
    arr_area: Rect,
    /// arranger play marker (song-order entry, row): Play song starts here;
    /// set by clicking / dragging the arranger's timeline ruler
    pub(crate) cue: (usize, usize),
    /// the arranger ruler's screen rect and the x of row 0 (tests, shots)
    pub(crate) arr_ruler: Rect,
    pub(crate) arr_x0: f32,
    /// a ruler scrub is in progress (suspends follow-scrolling)
    ruler_drag: bool,
    zoom: f32,
    buffer: u32,
    win_max: bool,
    win_rect: Option<[f32; 4]>,
    max_zoom: f32,
    zoom_checked: bool,
    themed: bool,
    theme: theme::Theme,
    show_help: bool,
    show_about: bool,
    about: about::AboutState,
    /// key events taken from egui before it can use them for focus navigation
    pending: Vec<egui::Event>,
    /// undo/redo keys pressed while a text field had focus: run next frame,
    /// after the field has given up focus (and applied its value)
    deferred_keys: Vec<egui::Event>,
    /// a mouse press/release happened since the last undo commit
    mouse_since_commit: bool,
    /// delete-instrument confirmation: (slot, name, notes using it)
    instr_delete: Option<(u8, String, usize)>,
    /// block length change waiting for "cut N cells?" (block, op, cells lost)
    len_confirm: Option<(usize, blocklen::LenOp, usize)>,
    /// MIDI keyboard input (app/midiview.rs)
    midi: midiview::MidiUi,
    /// instrument FX chain clipboard (Copy / Paste effects)
    fx_clip: Option<Vec<crate::fx::Device>>,
    /// instrument the list last scrolled to
    instr_shown: u8,
    /// plugin instruments: browser, scan, panel state (app/pluginview.rs)
    plug: pluginview::PluginUi,
    /// Timeline view state and its derived-timeline cache (app/timeline.rs)
    pub(crate) tlv: timeline::TimelineUi,
    /// piano roll in the lower panel (app/pianoview.rs)
    pub(crate) pr: pianoview::PianoUi,
    /// MIDI recording into note clips and Capture (app/noterec.rs)
    pub(crate) noterec: noterec::NoteRec,
    /// "made with a newer ChipWave Studio" message (open refused)
    too_new: Option<String>,
}

pub fn run(open: Option<PathBuf>) -> Result<(), String> {
    run_with(open, None)
}

/// `--screenshot`: open the window at `size` (points), capture one view, exit
pub fn screenshot(sh: shot::Shot, size: [f32; 2], song: Option<PathBuf>) -> Result<(), String> {
    run_with(song, Some((sh, size)))
}

fn run_with(open: Option<PathBuf>, shot: Option<(shot::Shot, [f32; 2])>) -> Result<(), String> {
    let song = match &open {
        Some(p) if p.to_string_lossy().ends_with(".toml") => fileio::load_text_song(p)?,
        Some(p) => fileio::load_cwt(p)?,
        None => fileio::demo_song(),
    };
    let shared: SharedRef = Arc::new(Mutex::new(Shared {
        song,
        engine: Engine::new(48000.0),
        preview: None,
    }));
    // crash recovery: find sessions left behind, then start our own
    let rroot = crate::recovery::root();
    let orphans = rroot
        .as_deref()
        .filter(|_| shot.is_none())
        .map(|r| crate::recovery::find_orphans(r, None))
        .unwrap_or_default();
    let session = rroot.as_deref().filter(|_| shot.is_none()).and_then(|r| {
        let t = shared
            .lock()
            .map(|s| s.song.title.clone())
            .unwrap_or_default();
        crate::recovery::Session::start(r, &t, open.as_deref())
    });
    if let Some(sess) = &session {
        install_panic_hook(shared.clone(), sess.dir.clone());
    }
    let st0 = crate::settings::load();
    let started = if shot.is_some() {
        // screenshots: no audio device; the UI advances the engine (shot.rs)
        Err("audio off for this capture".to_string())
    } else {
        audio::start_with(
            shared.clone(),
            st0.buffer,
            &st0.extra.device,
            st0.extra.rate,
        )
    };
    let (audio, audio_err) = match started {
        Ok(a) => (Some(a), None),
        Err(e) => (None, Some(e)),
    };
    let status = match (&audio, &audio_err) {
        (Some(a), _) => format!(
            "Audio: {} @ {} Hz. Press Space to play the demo.",
            a.device_name, a.sample_rate
        ),
        (_, Some(e)) => format!("No audio ({e}). Editing and WAV export still work."),
        _ => String::new(),
    };
    let status = if shot.is_some() {
        match &open {
            Some(p) => format!(
                "Opened {}",
                p.file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default()
            ),
            None => "Lumen Drift demo loaded.".to_string(),
        }
    } else {
        status
    };
    let mut app = App::new(shared, audio, audio_err, open, status, session, orphans);
    let set = crate::settings::load();
    let mut vb = egui::ViewportBuilder::default().with_inner_size([1360.0, 800.0]);
    if let Some((sh, size)) = shot {
        app.shot = Some(sh);
        vb = vb.with_inner_size(size).with_resizable(false);
    } else if set.maximized {
        vb = vb.with_maximized(true);
    } else if let Some([x, y, w, h]) = set.window {
        vb = vb.with_inner_size([w.max(800.0), h.max(500.0)]);
        if x > -200.0 && y > -50.0 {
            vb = vb.with_position([x, y]);
        }
    }
    let opts = eframe::NativeOptions {
        viewport: vb
            .with_title("ChipWave Studio")
            .with_min_inner_size([800.0, 500.0])
            .with_drag_and_drop(true)
            .with_icon(std::sync::Arc::new(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../assets/icon/chipwave_studio_256.png"
                ))
                .unwrap_or_default(),
            )),
        ..Default::default()
    };
    eframe::run_native("ChipWave Studio", opts, Box::new(|_cc| Ok(Box::new(app))))
        .map_err(|e| e.to_string())
}

/// On panic: try one emergency snapshot of the song, then the default hook.
fn install_panic_hook(shared: SharedRef, dir: PathBuf) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let song = match shared.try_lock() {
            Ok(g) => Some(g.song.clone()),
            Err(std::sync::TryLockError::Poisoned(p)) => Some(p.into_inner().song.clone()),
            Err(_) => None,
        };
        crate::recovery::emergency(&dir, song.as_ref(), &info.to_string());
        prev(info);
    }));
}

/// file-name-safe version of a song title
fn sanitize(t: &str) -> String {
    let s: String = t
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "song".into()
    } else {
        s
    }
}

/// keyboard-focus ring: 2 px light outline with a soft outer glow
fn focus_ring(p: &egui::Painter, r: Rect) {
    let c = th().focus_ring;
    for (w, a) in [(7.0, 0.10), (4.0, 0.22)] {
        p.rect_stroke(
            r,
            8,
            Stroke::new(w, c.gamma_multiply(a)),
            egui::StrokeKind::Outside,
        );
    }
    p.rect_stroke(r, 8, Stroke::new(2.0, c), egui::StrokeKind::Outside);
}

impl App {
    /// Build the app state (also used by the UI tests, without audio or a session).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        shared: SharedRef,
        audio: Option<audio::Audio>,
        audio_err: Option<String>,
        open: Option<PathBuf>,
        status: String,
        session: Option<crate::recovery::Session>,
        orphans: Vec<crate::recovery::Orphan>,
    ) -> App {
        let hist = undo::History::new(&audio::lock(&shared).song);
        let (midi, live_rx) = midiview::MidiUi::new();
        audio::lock(&shared).engine.set_live_input(Some(live_rx));
        let mut app = App {
            shared,
            audio,
            audio_err,
            row: 0,
            ch: 0,
            col: 0,
            pattern: 0,
            octave: 4,
            step: 1,
            edit: true,
            follow: true,
            instr: 1,
            hist,
            undo_hold: 0,
            path: open,
            status,
            dirty: false,
            sel: None,
            drag_anchor: None,
            clip: vec![],
            view: View::Pattern,
            dock_arranger: true,
            arr_sel: 0,
            arr_drag: None,
            fx_drag: None,
            recent: crate::settings::load().recent,
            session,
            last_autosave: std::time::Instant::now(),
            save_job: None,
            save_was_dirty: false,
            load_warnings: Vec::new(),
            last_heartbeat: std::time::Instant::now(),
            orphans,
            smp: Default::default(),
            extra: crate::settings::load().extra,
            upd: Default::default(),
            shot: None,
            keymap: crate::keymap::Keymap::load(),
            set_card: 0,
            set_open: Some(0),
            rebinding: None,
            bind_msg: String::new(),
            export_dlg: None,
            fill: None,
            chords: Default::default(),
            library: Default::default(),
            br: Default::default(),
            psetup: None,
            instr_query: String::new(),
            gen_ui: Default::default(),
            ip: Default::default(),
            renaming: None,
            rename_focus: false,
            rename_view: View::Pattern,
            show_info: false,
            prompt: None,
            discarded: false,
            allow_close: false,
            arr_flash: None,
            arr_multi: Default::default(),
            arr_clip: vec![],
            arr_focus: false,
            mods: Default::default(),
            key_log: None,
            show_auto: true,
            auto_pencil: false,
            auto_shape: crate::automation::Shape::Linear,
            auto_lane: 0,
            auto_sel: None,
            auto_drag: None,
            auto_pencil_lane: None,
            wheel_pat: 0.0,
            ch_view: 0,
            ch_bar: egui::Rect::NOTHING,
            mix_scroll: None,
            mix_bar: egui::Rect::NOTHING,
            wheel_arr: 0.0,
            arr_scroll: 0.0,
            arr_scroll_set: None,
            arr_area: Rect::NOTHING,
            cue: (0, 0),
            arr_ruler: Rect::NOTHING,
            arr_x0: 0.0,
            ruler_drag: false,
            zoom: crate::settings::load().zoom,
            buffer: crate::settings::load().buffer,
            win_max: crate::settings::load().maximized,
            win_rect: crate::settings::load().window,
            max_zoom: 3.0,
            zoom_checked: false,
            themed: false,
            theme: crate::settings::load().theme(),
            show_help: false,
            show_about: false,
            about: Default::default(),
            pending: vec![],
            deferred_keys: vec![],
            mouse_since_commit: false,
            instr_delete: None,
            len_confirm: None,
            midi,
            fx_clip: None,
            instr_shown: 0,
            plug: Default::default(),
            tlv: Default::default(),
            pr: Default::default(),
            noterec: Default::default(),
            too_new: None,
        };
        let connect = app.extra.midi.auto_connect && !app.extra.midi.device.is_empty();
        app.midi_apply(connect);
        if cfg!(test) {
            // tests open the Browser themselves (layouts stay as they were)
            app.extra.browser_open = false;
        } else if !app.extra.plugin_setup_done {
            // first run, or once for users from before the Browser
            app.open_plugin_setup(true);
        }
        app
    }
}

impl App {
    fn with<R>(&self, f: impl FnOnce(&mut Shared) -> R) -> R {
        let mut s = audio::lock(&self.shared);
        f(&mut s)
    }

    /// Call before a discrete edit: commits pending changes so the edit becomes
    /// its own undo step.
    fn snapshot(&mut self) {
        if self.undo_hold == 0 {
            let shared = self.shared.clone();
            let s = audio::lock(&shared);
            self.hist.boundary(&s.song, std::time::Instant::now());
        }
        self.dirty = true;
    }

    /// Record changes made this frame (slider drags wait for the mouse release).
    fn undo_commit(&mut self, ctx: &egui::Context) {
        if self.undo_hold > 0 || ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        // keyboard-driven changes may merge; any click/drag is its own step
        let mouse = ctx.input(|i| {
            i.pointer.any_released() || i.pointer.any_pressed() || i.pointer.any_click()
        }) || self.mouse_since_commit;
        let shared = self.shared.clone();
        let s = audio::lock(&shared);
        if self.hist.commit(&s.song, std::time::Instant::now(), !mouse) {
            self.dirty = true;
            self.mouse_since_commit = false;
        } else if mouse {
            // e.g. a click that changes the song one frame later
            self.mouse_since_commit = true;
        }
    }

    /// start over (new or opened song)
    fn undo_reset(&mut self) {
        let shared = self.shared.clone();
        let s = audio::lock(&shared);
        self.hist.reset(&s.song);
    }

    fn undo(&mut self, redo: bool) {
        let done = {
            let shared = self.shared.clone();
            let mut s = audio::lock(&shared);
            let before = (s.song.bpm, s.song.speed);
            let old_instr = s.song.instruments.clone();
            let done = self.hist.step(&mut s.song, redo, std::time::Instant::now());
            // an instrument came back or went away: keep the playing voices
            // and FX chains on their instruments
            if let Some(f) = instrument_shift(&old_instr, &s.song.instruments) {
                s.engine.remap_instruments(&f);
            }
            // the engine keeps its own running tempo (set_tempo / Fxx): follow
            // a tempo or speed change that undo/redo brought back
            if (s.song.bpm, s.song.speed) != before {
                let (bpm, speed) = (s.song.bpm, s.song.speed);
                s.engine.set_tempo(bpm, speed);
            }
            done
        };
        if done {
            self.dirty = true;
            self.status = if redo { "Redo".into() } else { "Undo".into() };
        } else {
            self.status = if redo {
                "Nothing to redo".into()
            } else {
                "Nothing to undo".into()
            };
        }
        let (n, ch, ni) = self.with(|s| {
            (
                s.song.patterns.len(),
                s.song.channels,
                s.song.instruments.len(),
            )
        });
        self.pattern = self.pattern.min(n.saturating_sub(1));
        self.row = self.row.min(self.rows().saturating_sub(1));
        self.ch = self.ch.min(ch.saturating_sub(1));
        self.instr = self.instr.min(ni.min(255) as u8);
        self.arr_sel = self.arr_sel.min(self.order_len().saturating_sub(1));
        self.arr_multi.clear();
        self.sel = None;
    }

    fn rows(&self) -> usize {
        self.with(|s| {
            s.song
                .patterns
                .get(self.pattern)
                .map(|p| p.rows)
                .unwrap_or(ROWS)
        })
    }

    fn edit_cell(&mut self, f: impl FnOnce(&mut Cell)) {
        self.snapshot();
        let (p, r, c) = (self.pattern, self.row, self.ch);
        self.with(|s| {
            if let Some(cell) = s
                .song
                .patterns
                .get_mut(p)
                .and_then(|p| p.cells.get_mut(r))
                .and_then(|r| r.get_mut(c))
            {
                f(cell)
            }
        });
    }

    fn move_row(&mut self, d: isize) {
        let n = self.rows() as isize;
        self.row = (self.row as isize + d).rem_euclid(n) as usize;
    }

    fn play(&mut self, mode: u8) {
        let (pattern, row) = (self.pattern, self.row);
        let cue = self.cue;
        let tl_cue = (self.tlv.cue.max(0) / crate::timeline::ROW) as usize;
        if let Some(f) = &mut self.fill {
            f.previewing = false;
        }
        self.with(|s| {
            if s.preview.take().is_some() {
                s.engine.stop();
            }
            let Shared { song, engine, .. } = s;
            if engine.playing && mode <= 1 {
                engine.stop();
                return;
            }
            match mode {
                0 if song.timeline.is_some() => {
                    // timeline mode: from the timeline's play marker
                    engine.start(song, 0, tl_cue, false, 0)
                }
                0 => {
                    // song: from the arranger play marker (bar 1 unless moved)
                    let op = cue.0.min(song.order.len().saturating_sub(1));
                    let rows = song
                        .order
                        .get(op)
                        .and_then(|&b| song.patterns.get(b))
                        .map(|p| p.rows)
                        .unwrap_or(ROWS);
                    engine.start(song, op, cue.1.min(rows.saturating_sub(1)), false, 0)
                }
                1 | 3 => engine.start(song, 0, 0, true, pattern), // block loop
                _ => engine.start(song, 0, row, true, pattern),   // from row
            }
        });
    }

    /// PANIC: stop playback and every sound now (toolbar button, Ctrl+.).
    /// Works while stopped too: held MIDI notes, plugin voices and FX tails
    /// are cleared by the audio thread after a 3 ms fade.
    pub(super) fn panic(&mut self) {
        if let Some(f) = &mut self.fill {
            f.previewing = false;
        }
        self.with(|s| {
            s.preview = None;
            s.engine.stop();
            s.engine.panic();
        });
        // (ChipWave has no MIDI output; held MIDI-input notes and the
        // sustain pedal state are cleared in the engine)
        self.status = "Panic: all sound stopped".into();
    }

    fn help(&mut self, ctx: &egui::Context) {
        let mut open = self.show_help;
        egui::Window::new("Keys").open(&mut open).resizable(false).show(ctx, |ui| {
            egui::Grid::new("keys").striped(true).show(ui, |ui| {
                for (k, v) in [
                    ("Up/Down, PgUp/PgDn", "row / 16 rows"), ("Left/Right", "column (wraps channels)"), ("Tab / Shift+Tab", "next / previous channel"),
                    ("Home / End", "first / last row"), ("Ctrl+Home / End", "first / last block"),
                    ("Z S X D C V G B H N J M", "notes C..B (base octave)"), ("Q 2 W 3 E R 5 T 6 Y 7 U I 9 O 0 P", "notes, octave +1"),
                    ("1", "note off"), ("F1 - F8", "base octave 1-8"), ("0-9 A-F", "hex in instr/vol/effect columns"),
                    ("Delete", "clear field"), ("Backspace / Insert", "pull up / push down rows"),
                    ("Space", "play / stop current block (loop)"), ("Shift+Space", "play / stop song (from the arranger play marker)"), ("Enter / Ctrl+Enter", "restart block / play from row"), ("Ctrl+= / Ctrl+- / Ctrl+0", "UI scale up / down / 100 %"), ("Esc", "edit mode on/off"),
                    ("Ctrl+Z / Ctrl+Y", "undo / redo"), ("Ctrl+Shift+F", "fill to length (auto arrange)"), ("1-7 / 0 in arranger", "tag block role / clear"), ("Alt +/- (Shift = octave)", "transpose selection or channel (also the Block bar -12 -1 +1 +12 buttons, Edit > Transpose)"), ("F9 / F10", "mute / solo channel"),
                    ("Numpad - / +", "base octave down / up"), ("Ctrl+Shift+PgDn / PgUp", "double / halve the block length (Length field in the Block bar; Song > Block: set length, stretch, squash)"), ("Row numbers", "decimal 00-63 by default; View > Row numbers or Settings > Look switches to hex 00-3F. Effect values stay hex"), ("Shift+arrows, PgUp/PgDn, mouse drag", "select"), ("Ctrl+A", "select channel, again = block"),
                    ("Ctrl+C / X / V", "copy / cut / paste at cursor"), ("Delete (with selection)", "clear selection"),
                    ("Ctrl+Tab, Ctrl+1..5", "views: pattern / arranger / mixer / FX / sample"), ("F12", "this key list"), ("F11", "full-height pattern (hide / show the lower panel)"), ("Ctrl+I", "instrument editor in the lower panel"), ("Ctrl+F", "search instrument parameters"), ("knob: Ctrl-drag / double-click", "fine adjust / default"), ("Ctrl+Q", "quit"), ("FX view: Up / Down", "previous / next instrument"), ("Ctrl+D / Ctrl+Shift+D", "arranger: duplicate clip / as new block (when visible)"),
                    ("Arranger (click it to focus)", "click / Shift+click / Ctrl+click select; Ctrl+X/C/V cut/copy/paste after selection; Ctrl+Shift+V paste as new blocks; Del; Ctrl+A; right-click menu"), ("Ctrl+Shift+K", "debug: show last 10 input events"),
                    ("Arranger: Left/Right, Del, Ins, Ctrl+D, +/-", "select clip, delete, insert, duplicate, zoom"),
                    ("Arranger ruler (bar numbers)", "click = move the play marker to that bar (jumps there while playing), drag = scrub, Alt = exact row; Play song starts from the marker"),
                    ("Ctrl+N / O / S / Shift+S", "new / open / save / save as (.cwt)"), ("Ctrl+. (Panic button, far right of the toolbar)", "PANIC: stop all sound now: playback, held / MIDI notes, sustain, plugin voices, delay and reverb tails, limiter (3 ms fade, also while stopped)"), ("Ctrl+E", "export WAV"),
                    ("MIDI keyboard", "plays the current instrument (velocity, sustain pedal, pitch bend); Settings > MIDI picks the device"),
                    ("MIDI + edit mode", "records the note at the cursor (instrument + velocity), moves by Step; while playing: at the playing row"),
                    ("Right-click a fader / knob", "MIDI learn: then move a knob or the mod wheel"),
                    ("New > Plugin instrument…", "a CLAP or VST3 instrument plugin (Surge XT …): Configure… opens its editor, 43xy / lanes drive its 8 macros (docs/PLUGINS.md)"),
                    ("Help > Check for updates…", "ask GitHub for a newer version (Settings > Files & Safety: check at startup)"),
                ] { ui.label(egui::RichText::new(k).monospace().color(th().value)); ui.label(v); ui.end_row(); }
            });
            ui.label(
                "Effects: 01/02 slide, 08 note decay (xx*10 ms), 09 speed, 0C volume, 0D vol slide, 0F tempo (0F00 = break)",
            );
            ui.label(
                "Voice / glitch: 30xy vowel x (0 A 1 E 2 I 3 O 4 U 5 ER 6 AE), glide y*25 ms · 31xx sequence step · \
                 32xx formant shift (80 = x1) · 33xy stutter: length x (1 1/64 .. 3 1/16 .. 6 1/2 note), y repeats (any instrument) · \
                 34xy stutter pitch +x/-y st · 35xx breath (00-40) · 36xy babble rate x, amount y · 37xy chop rate x, open y/16 (0 = off)",
            );
            ui.label(
                "Filter / acid (synth + voice): 38xx cutoff (00 = instrument, 01 20 Hz .. 80 ~630 Hz .. FF 20 kHz) · \
                 39xx resonance (00 = instrument, 01-FF 0-100 %) · 3Axx / 3Bxx cutoff slide up / down xx/96 octave per tick · \
                 3Cxx glide to this note (00 off, 01 instrument time, 02-FF xx*5 ms) · 3Dxx accent (00 instrument amount, 01-40)",
            );
            ui.label(
                "Channels: 1-32 per song (Channels box in the block bar or Song menu). Tab / Shift+Tab walk them; \
                 the bar under the pattern and under the mixer strips shows which are in view - click or drag it to scroll.",
            );
            ui.label(
                "Voice words: each note speaks the next word of the Voice's Words list (ka, ta, sha, ...); \
                 3Exx speaks word xx (on a held note: says it again). Tract: Throat = a simulated vocal tract \
                 (inspired by Pink Trombone by Neil Thapen).",
            );
            ui.label(
                "Instruments: double-click a name to rename; right-click for Move up / down, Use this instead of (swap the notes), \
                 Save to library, Export .cwi; Up / Down move the selected one (notes follow). File > Instrument library… browses \
                 the song, your library folder (.cwi files, sub-folders = categories) and the factory sounds: › auditions, + adds, \
                 right-click replaces the selected slot's sound. Drop a .cwi file on the window to import it.",
            );
            ui.label(
                "Chords: Song > Chord helper… or Chords… in the block bar. Pick or detect the song key, click pads to build a \
                 progression (or pick a preset), then Write into block as chords (one tone per channel from the cursor channel), \
                 an arpeggio or a bass line. Scale lock snaps typed and MIDI notes to the key.",
            );
            ui.label(
                "Generators: 43xy sets macro x (1-8) to y (0-F) on that channel, 4300 = instrument. Dice rolls the \
                 unlocked knobs, Mutate nudges them, < > walk the last 16 rolls, Bake to sample renders C-4 to a sample. \
                 Chip FX (sfxr): Pickup / Laser / Explosion / Power-up / Hit / Jump / Blip roll a sound, Randomize anything. \
                 Analog / FM-4 / Noise (0.2.3.14) play their own envelopes; the note-off releases them.",
            );
            ui.label(
                "Voice Transform: 3Fxx blend · 40xx metallic resonance · 41xx digital distortion · 42xx sub level \
                 (00 = instrument, 01-FF = 0-100 %; 40-42 also set the Metal resonator / Digital distortion / Sub bass \
                 devices' mix on that channel).",
            );
            ui.label(
                "Slicing (Sample editor): Transients cuts the sample at its hits (sens = sensitivity, gap = shortest slice), \
                 Grid cuts it into 2-64 equal parts. Click a slice to select it (Prev / Next step through them); Normalise, \
                 Reverse, Fade, Gain then work on that slice. Alt+click adds a slice, right-click a marker removes it. \
                 To instruments makes one sample instrument per slice; To notes also writes them into the block on the \
                 cursor channel at the song tempo. One undo step.",
            );
            ui.label(
                "Master: Mixer > Limiter & loudness (true-peak limiter, LUFS meter); exports use the same limiter.",
            );
        });
        self.show_help = open;
    }
}

impl eframe::App for App {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // let a background save finish before the process ends
        if let Some((path, rx)) = self.save_job.take() {
            let r = rx.recv().unwrap_or_else(|_| Err("save thread died".into()));
            self.finish_save(path, r);
        }
        // clean exit: the recovery folder of this session is not needed
        if let Some(s) = &self.session {
            s.wait_idle(std::time::Duration::from_secs(10));
            if crate::prompt::may_delete_recovery(self.dirty, self.discarded) {
                s.close();
            } else {
                // unsaved work and no explicit discard: keep a fresh recovery copy
                let song = self.with(|s| s.song.clone());
                let _ = crate::recovery::Session::autosave_blocking(&s.dir, &song);
                std::fs::write(s.dir.join("heartbeat"), "0").ok();
            }
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.shot_input(raw);
        // While no text field is being edited, the pattern editor owns the keyboard:
        // take key events away from egui (so Tab/arrows don't move widget focus).
        if let Some(log) = &mut self.key_log {
            for e in &raw.events {
                if let Some(d) = input::describe(e) {
                    log.push_back(d);
                    while log.len() > 10 {
                        log.pop_front();
                    }
                }
            }
        }

        // undo/redo held back from the previous frame (see below)
        self.pending.append(&mut self.deferred_keys);

        if ctx.text_edit_focused() {
            // Typing in a text field (song title, a DragValue being typed into,
            // ...): the field keeps its keys, except song undo/redo. Those
            // close the field first and run next frame, after its value has
            // been applied and recorded, so Ctrl+Z undoes it like any edit.
            let before = raw.events.len();
            let mut keep = Vec::with_capacity(before);
            for e in raw.events.drain(..) {
                if input::is_undo_key(&e) {
                    self.deferred_keys.push(e);
                } else {
                    keep.push(e);
                }
            }
            raw.events = keep;
            if raw.events.len() != before {
                ctx.memory_mut(|m| {
                    if let Some(id) = m.focused() {
                        m.surrender_focus(id)
                    }
                });
                ctx.request_repaint();
            }
            return;
        }
        // Any other focused widget (a clicked DragValue in button mode, a
        // keyboard-focused slider, ...) does not keep the keyboard.
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id)
            }
        });
        let mut keep = Vec::with_capacity(raw.events.len());
        for e in raw.events.drain(..) {
            match e {
                egui::Event::Key { .. }
                | egui::Event::Copy
                | egui::Event::Cut
                | egui::Event::Paste(_) => self.pending.push(e),
                egui::Event::Text(_) => {}
                other => keep.push(other),
            }
        }
        raw.events = keep;
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shot_frame(ui.ctx());
        self.poll_save();
        self.load_warnings_window(ui.ctx());
        self.too_new_window(ui.ctx());
        self.timeline_prompts(ui.ctx());
        self.instr_delete_dialog(ui.ctx());
        self.block_len_dialog(ui.ctx());
        self.chord_window(ui.ctx());
        self.pr_release(!self.pr.open);
        self.rec_frame();
        if !self.pr.open {
            self.pr.focus = false;
        }
        self.library_window(ui.ctx());
        if self.save_job.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        let ctx = ui.ctx().clone();
        if !self.themed {
            self.apply_theme(&ctx);
            ctx.options_mut(|o| o.zoom_with_keyboard = false);
            self.themed = true;
        }
        // Largest scale at which the UI still fits (needs ~1000 x 560 points).
        let win = ui.max_rect().size() * ctx.zoom_factor();
        self.max_zoom = (win.x / 1000.0).min(win.y / 560.0).clamp(0.5, 3.0);
        if !self.zoom_checked && win.x > 100.0 {
            self.zoom_checked = true;
            if self.zoom > self.max_zoom {
                self.zoom = 1.0f32.min(self.max_zoom);
                self.status = "Saved scale did not fit this window; reset to 100 %".into();
            }
        }
        if self.zoom > self.max_zoom + 0.001 {
            self.zoom = self.max_zoom;
        }
        if (ctx.zoom_factor() - self.zoom).abs() > 0.001 {
            ctx.set_zoom_factor(self.zoom);
        }
        // remember window state (saved only when it changes)
        let (maxed, outer, inner) = ctx.input(|i| {
            let v = i.viewport();
            (v.maximized, v.outer_rect, v.inner_rect)
        });
        // ignore the first second: the OS may report un-maximised before it applies
        let settled = ctx.input(|i| i.time) > 1.5;
        if let (Some(m), true) = (maxed, settled) {
            let mut changed = m != self.win_max;
            self.win_max = m;
            if !m {
                if let (Some(o), Some(r)) = (outer, inner) {
                    let new = [o.left(), o.top(), r.width(), r.height()];
                    let differs = self
                        .win_rect
                        .map(|w| w.iter().zip(new).any(|(a, b)| (a - b).abs() > 1.0))
                        .unwrap_or(true);
                    if differs && !ctx.input(|i| i.pointer.any_down()) {
                        self.win_rect = Some(new);
                        changed = true;
                    }
                }
            }
            if changed {
                self.save_settings();
            }
        }
        let mut dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        // audio files dropped on the Timeline view: audio clips where the
        // mouse is (else on the current track at the play marker)
        if self.view == View::Timeline && dropped.iter().any(|p| crate::audiofile::is_audio(p)) {
            let (audio, rest): (Vec<PathBuf>, Vec<PathBuf>) = dropped
                .into_iter()
                .partition(|p| crate::audiofile::is_audio(p));
            let (tr, at) = self.tlv_drop_target();
            self.import_audio_files(audio, tr, at);
            dropped = rest;
        }
        self.poll_audio_imports(&ctx);
        for p in dropped {
            if p.extension()
                .map(|e| e.eq_ignore_ascii_case("cwt"))
                .unwrap_or(false)
            {
                self.request(crate::prompt::Pending::OpenPath(p), &ctx);
            } else if p
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(crate::instfile::EXT))
            {
                self.import_cwi(&p);
            } else {
                self.import_sample(p);
            }
        }
        self.midi_frame(&ctx);
        self.br_poll(&ctx);
        self.br.slot_rects.clear();
        self.handle_keys(&ctx);
        self.tick_safety();
        self.tick_derived();
        self.plugin_frame(&ctx);

        let full = ui.max_rect();
        theme::gradient(ui.painter(), full, th().bg_top, th().bg_bot);

        egui::Panel::top("menubar")
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(th().hdr_top)
                    .inner_margin(egui::Margin::symmetric(0, 3)),
            )
            .show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("toolbar")
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(th().hdr_bot)
                    .inner_margin(egui::Margin::symmetric(0, 5)),
            )
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").exact_size(26.0).frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(12, 4))).show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(log) = &self.key_log {
                    let v: Vec<&str> = log.iter().map(|s| s.as_str()).collect();
                    ui.label(
                        egui::RichText::new(format!("keys: {}", v.join("  ")))
                            .monospace()
                            .color(th().fx),
                    );
                } else {
                    ui.label(egui::RichText::new(&self.status).color(Color32::WHITE));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (peak, len) = self.with(|s| (s.engine.peak, s.song.length_seconds()));
                    ui.label(egui::RichText::new(format!("Row {}/{} · Instr {:02X} · Oct {} · Step {} · length {}:{:04.1} · peak {:.0} %", self.row_text(self.row, self.rows()), self.rows(), self.instr, self.octave, self.step, (len / 60.0) as u32, len % 60.0, peak * 100.0)).color(Color32::WHITE).monospace());
                });
            });
        });
        egui::Panel::left("side")
            .exact_size(330.0)
            .resizable(false)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: 8,
                right: 4,
                top: 6,
                bottom: 6,
            }))
            .show(ui, |ui| {
                theme::cartridge(ui.painter(), ui.max_rect(), th().slate);
                egui::ScrollArea::vertical()
                    .id_salt("side_scroll")
                    .show(ui, |ui| self.side_panel(ui));
            });
        if self.extra.browser_open {
            self.browser_side(ui);
        } else {
            self.br.rect = Rect::NOTHING;
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: 4,
                right: 8,
                top: 6,
                bottom: 6,
            }))
            .show(ui, |ui| {
                let r = ui.max_rect();
                self.br.central = r;
                theme::cartridge(ui.painter(), r, th().ochre);
                let inner = r.shrink2(egui::vec2(12.0, 8.0));
                let inner =
                    Rect::from_min_max(Pos2::new(inner.left() + 4.0, inner.top()), inner.max);
                match self.view {
                    View::Pattern if self.lower_visible() => {
                        self.pattern_and_lower(ui, inner);
                    }
                    View::Pattern => {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            self.pattern_view(ui)
                        });
                    }
                    View::Arranger => {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            self.arranger_view(ui)
                        });
                    }
                    View::Timeline if self.lower_visible() => {
                        self.pattern_and_lower(ui, inner);
                    }
                    View::Timeline => {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            self.timeline_view(ui)
                        });
                    }
                    View::Mixer => {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            self.mixer_view(ui)
                        });
                    }
                    View::Fx => {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            self.fx_view(ui)
                        });
                    }
                    View::Sample => {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            self.sample_view(ui)
                        });
                    }
                    View::Settings => {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            self.settings_view(ui)
                        });
                    }
                }
            });
        if self.show_help {
            self.help(&ctx);
        }
        self.poll_updates(&ctx);
        self.dialogs(&ctx);
        self.update_window(&ctx);
        self.modal_dialogs(&ctx);
        self.fill_dialog(&ctx);
        self.plugin_browser(&ctx);
        self.plugin_setup_window(&ctx);
        self.browser_drag(&ctx);
        // window close button: ask first when there are unsaved changes
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.dirty {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.prompt = Some(crate::prompt::Pending::Quit);
            } else {
                self.allow_close = true;
            }
        }
        // record this frame's edits as an undo step (drags: on release)
        self.undo_commit(ui.ctx());
        // timeline mode: the engine's playback plan follows the song (cheap
        // when nothing changed; edits rebuild it at once themselves)
        self.with(|s| {
            let Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
        });
        if self.with(|s| s.engine.playing || s.engine.peak > 0.001) {
            ctx.request_repaint();
        }
    }
}

/// Index map after one instrument was inserted into or removed from `old`
/// (None when the count did not change by exactly one).
fn instrument_shift(
    old: &[crate::model::Instrument],
    new: &[crate::model::Instrument],
) -> Option<impl Fn(usize) -> Option<usize>> {
    if old.len().abs_diff(new.len()) != 1 {
        return None;
    }
    let k = old
        .iter()
        .zip(new)
        .position(|(a, b)| a != b)
        .unwrap_or(old.len().min(new.len()));
    let removed = new.len() < old.len();
    Some(move |i: usize| {
        if i < k {
            Some(i)
        } else if removed {
            (i != k).then(|| i - 1)
        } else {
            Some(i + 1)
        }
    })
}
