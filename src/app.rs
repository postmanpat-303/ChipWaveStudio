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

mod arranger;
mod dialogs;
mod edit;
mod file;
mod fillview;
mod fxview;
mod instruments;
mod keys;
mod lanes;
mod mixer;
mod pattern;
mod sampleview;
mod settingsview;
mod topbar;

/// cursor sub-columns: 0 note, 1-2 instr, 3-4 vol, 5-8 effect
const SUBCOLS: usize = 9;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Pattern,
    Arranger,
    Mixer,
    Fx,
    Sample,
    Settings,
}

/// muted clip palette for the arranger (block id -> colour)
const CLIP_COLOURS: [u32; 8] = [
    // no greens: the theme accent is green, so clips must never look "selected"
    0x516D96, 0x8A6D3B, 0x7A4E5C, 0x5E6B7A, 0x6B5E8A, 0x7A6A4E, 0x8A5A4E, 0x4E6A86,
];

type Undo = (
    Vec<Pattern>,
    Vec<usize>,
    Vec<crate::automation::Lane>,
    Vec<Vec<crate::fx::Device>>,
    Vec<crate::model::Return>,
);

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
    undo: Vec<Undo>,
    redo: Vec<Undo>,
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
    keymap: crate::keymap::Keymap,
    /// settings screen: focused card, expanded card, key being rebound
    set_card: usize,
    set_open: Option<usize>,
    rebinding: Option<crate::keymap::Action>,
    bind_msg: String,
    export_dlg: Option<dialogs::ExportDlg>,
    /// Song > Fill to length dialog state
    fill: Option<fillview::FillState>,
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
    wheel_arr: f32,
    arr_scroll: f32,
    arr_scroll_set: Option<f32>,
    arr_area: Rect,
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
    /// key events taken from egui before it can use them for focus navigation
    pending: Vec<egui::Event>,
}

pub fn run(open: Option<PathBuf>) -> Result<(), String> {
    let song = match &open {
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
        .map(|r| crate::recovery::find_orphans(r, None))
        .unwrap_or_default();
    let session = rroot.as_deref().and_then(|r| {
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
    let (audio, audio_err) = match audio::start_with(
        shared.clone(),
        st0.buffer,
        &st0.extra.device,
        st0.extra.rate,
    ) {
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
    let app = App {
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
        undo: vec![],
        redo: vec![],
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
        keymap: crate::keymap::Keymap::load(),
        set_card: 0,
        set_open: Some(0),
        rebinding: None,
        bind_msg: String::new(),
        export_dlg: None,
        fill: None,
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
        wheel_arr: 0.0,
        arr_scroll: 0.0,
        arr_scroll_set: None,
        arr_area: Rect::NOTHING,
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
        pending: vec![],
    };
    let set = crate::settings::load();
    let mut vb = egui::ViewportBuilder::default().with_inner_size([1360.0, 800.0]);
    if set.maximized {
        vb = vb.with_maximized(true);
    } else if let Some([x, y, w, h]) = set.window {
        vb = vb.with_inner_size([w.max(800.0), h.max(500.0)]);
        if x > -200.0 && y > -50.0 {
            vb = vb.with_position([x, y]);
        }
    }
    let opts = eframe::NativeOptions {
        viewport: vb
            .with_title("ChipWave Tracker")
            .with_min_inner_size([800.0, 500.0])
            .with_drag_and_drop(true)
            .with_icon(std::sync::Arc::new(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../assets/icon/chipwave_tracker_256.png"
                ))
                .unwrap_or_default(),
            )),
        ..Default::default()
    };
    eframe::run_native("ChipWave Tracker", opts, Box::new(|_cc| Ok(Box::new(app))))
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
    fn with<R>(&self, f: impl FnOnce(&mut Shared) -> R) -> R {
        let mut s = audio::lock(&self.shared);
        f(&mut s)
    }

    fn snapshot(&mut self) {
        let st = self.with(|s| {
            (
                s.song.patterns.clone(),
                s.song.order.clone(),
                s.song.automation.clone(),
                s.song.instruments.iter().map(|i| i.fx.clone()).collect(),
                s.song.mixer.returns.clone(),
            )
        });
        self.undo.push(st);
        if self.undo.len() > 500 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    fn undo(&mut self, redo: bool) {
        let (from, to) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        if let Some(p) = from.pop() {
            let cur = {
                let mut s = audio::lock(&self.shared);
                (
                    std::mem::replace(&mut s.song.patterns, p.0),
                    std::mem::replace(&mut s.song.order, p.1),
                    std::mem::replace(&mut s.song.automation, p.2),
                    s.song
                        .instruments
                        .iter_mut()
                        .zip(p.3.into_iter().chain(std::iter::repeat_with(Vec::new)))
                        .map(|(i, f)| std::mem::replace(&mut i.fx, f))
                        .collect(),
                    std::mem::replace(&mut s.song.mixer.returns, p.4),
                )
            };

            to.push(cur);
            self.status = if redo { "Redo".into() } else { "Undo".into() };
        }
        let n = self.with(|s| s.song.patterns.len());
        self.pattern = self.pattern.min(n - 1);
        self.row = self.row.min(self.rows() - 1);
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
                0 => engine.start(song, 0, 0, false, 0),          // song
                1 | 3 => engine.start(song, 0, 0, true, pattern), // block loop
                _ => engine.start(song, 0, row, true, pattern),   // from row
            }
        });
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
                    ("Space", "play / stop current block (loop)"), ("Shift+Space", "play / stop song"), ("Enter / Ctrl+Enter", "restart block / play from row"), ("Ctrl+= / Ctrl+- / Ctrl+0", "UI scale up / down / 100 %"), ("Esc", "edit mode on/off"),
                    ("Ctrl+Z / Ctrl+Y", "undo / redo"), ("Ctrl+Shift+F", "fill to length (auto arrange)"), ("1-7 / 0 in arranger", "tag block role / clear"), ("Alt +/- (Shift = octave)", "transpose selection or channel"), ("F9 / F10", "mute / solo channel"),
                    ("Numpad - / +", "base octave down / up"), ("Shift+arrows, PgUp/PgDn, mouse drag", "select"), ("Ctrl+A", "select channel, again = block"),
                    ("Ctrl+C / X / V", "copy / cut / paste at cursor"), ("Delete (with selection)", "clear selection"),
                    ("Ctrl+Tab, Ctrl+1..5", "views: pattern / arranger / mixer / FX / sample"), ("F12", "this key list"), ("Ctrl+Q", "quit"), ("FX view: Up / Down", "previous / next instrument"), ("Ctrl+D / Ctrl+Shift+D", "arranger: duplicate clip / as new block (when visible)"),
                    ("Arranger (click it to focus)", "click / Shift+click / Ctrl+click select; Ctrl+X/C/V cut/copy/paste after selection; Ctrl+Shift+V paste as new blocks; Del; Ctrl+A; right-click menu"), ("Ctrl+Shift+K", "debug: show last 10 input events"),
                    ("Arranger: Left/Right, Del, Ins, Ctrl+D, +/-", "select clip, delete, insert, duplicate, zoom"),
                    ("Ctrl+N / O / S / Shift+S", "new / open / save / save as (.cwt)"), ("Ctrl+E", "export WAV"),
                ] { ui.label(egui::RichText::new(k).monospace().color(th().value)); ui.label(v); ui.end_row(); }
            });
            ui.label("Effects: 01/02 slide, 09 speed, 0C volume, 0D vol slide, 0F tempo (0F00 = break)");
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

        if ctx.egui_wants_keyboard_input() {
            return;
        }
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
        self.poll_save();
        self.load_warnings_window(ui.ctx());
        if self.save_job.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        let ctx = ui.ctx().clone();
        if !self.themed {
            theme::apply(&ctx, theme::with_accent(self.theme, &self.extra.accent));
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
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        for p in dropped {
            if p.extension()
                .map(|e| e.eq_ignore_ascii_case("cwt"))
                .unwrap_or(false)
            {
                self.request(crate::prompt::Pending::OpenPath(p), &ctx);
            } else {
                self.import_sample(p);
            }
        }
        self.handle_keys(&ctx);
        self.tick_safety();
        self.tick_derived();

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
                    ui.label(egui::RichText::new(format!("Instr {:02X} · Oct {} · Step {} · length {}:{:04.1} · peak {:.0} %", self.instr, self.octave, self.step, (len / 60.0) as u32, len % 60.0, peak * 100.0)).color(Color32::WHITE).monospace());
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
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: 4,
                right: 8,
                top: 6,
                bottom: 6,
            }))
            .show(ui, |ui| {
                let r = ui.max_rect();
                theme::cartridge(ui.painter(), r, th().ochre);
                let inner = r.shrink2(egui::vec2(12.0, 8.0));
                let inner =
                    Rect::from_min_max(Pos2::new(inner.left() + 4.0, inner.top()), inner.max);
                match self.view {
                    View::Pattern if self.dock_arranger => {
                        let nl = self.with(|s| s.song.automation.len());
                        let h = if self.show_auto && nl > 0 {
                            (inner.height() * 0.5).clamp(190.0, 120.0 + nl as f32 * 68.0 + 60.0)
                        } else {
                            (inner.height() * 0.3).clamp(120.0, 190.0)
                        };
                        let top = Rect::from_min_max(
                            inner.min,
                            Pos2::new(inner.right(), inner.bottom() - h - 6.0),
                        );
                        let bot = Rect::from_min_max(
                            Pos2::new(inner.left(), inner.bottom() - h),
                            inner.max,
                        );
                        ui.scope_builder(egui::UiBuilder::new().max_rect(top), |ui| {
                            self.pattern_view(ui)
                        });
                        ui.painter().hline(
                            bot.x_range(),
                            bot.top() - 3.0,
                            Stroke::new(1.0, th().border.gamma_multiply(0.6)),
                        );
                        ui.scope_builder(egui::UiBuilder::new().max_rect(bot), |ui| {
                            self.arranger_view(ui)
                        });
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
        self.dialogs(&ctx);
        self.modal_dialogs(&ctx);
        self.fill_dialog(&ctx);
        // window close button: ask first when there are unsaved changes
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.dirty {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.prompt = Some(crate::prompt::Pending::Quit);
            } else {
                self.allow_close = true;
            }
        }
        if self.with(|s| s.engine.playing || s.engine.peak > 0.001) {
            ctx.request_repaint();
        }
    }
}
