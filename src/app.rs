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

mod dialogs;
mod fillview;
mod fxview;
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

fn note_key(k: Key) -> Option<i32> {
    use Key::*;
    Some(match k {
        Z => 0,
        S => 1,
        X => 2,
        D => 3,
        C => 4,
        V => 5,
        G => 6,
        B => 7,
        H => 8,
        N => 9,
        J => 10,
        M => 11,
        Comma => 12,
        L => 13,
        Period => 14,
        Semicolon => 15,
        Slash => 16,
        Q => 12,
        Num2 => 13,
        W => 14,
        Num3 => 15,
        E => 16,
        R => 17,
        Num5 => 18,
        T => 19,
        Num6 => 20,
        Y => 21,
        Num7 => 22,
        U => 23,
        I => 24,
        Num9 => 25,
        O => 26,
        Num0 => 27,
        P => 28,
        _ => return None,
    })
}

fn hex_key(k: Key) -> Option<u8> {
    use Key::*;
    Some(match k {
        Num0 => 0,
        Num1 => 1,
        Num2 => 2,
        Num3 => 3,
        Num4 => 4,
        Num5 => 5,
        Num6 => 6,
        Num7 => 7,
        Num8 => 8,
        Num9 => 9,
        A => 10,
        B => 11,
        C => 12,
        D => 13,
        E => 14,
        F => 15,
        _ => return None,
    })
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

    fn save(&mut self, as_new: bool) {
        self.save_inner(as_new, false);
    }

    /// save and block until done (used by "Save" in the quit/new prompt)
    pub(super) fn save_and_wait(&mut self) -> bool {
        self.save_inner(false, true)
    }

    fn save_inner(&mut self, as_new: bool, wait: bool) -> bool {
        if as_new || self.path.is_none() {
            let mut d = rfd::FileDialog::new()
                .add_filter("ChipWave song", &["cwt"])
                .set_file_name(format!(
                    "{}.cwt",
                    sanitize(&self.with(|s| s.song.title.clone()))
                ));
            if !self.extra.song_dir.is_empty() {
                d = d.set_directory(&self.extra.song_dir);
            }
            let Some(p) = d.save_file() else {
                return false;
            };
            self.path = Some(if p.extension().is_none() {
                p.with_extension("cwt")
            } else {
                p
            });
        }
        let Some(path) = self.path.clone() else {
            return false;
        };
        self.start_save(path, wait)
    }

    /// Save a snapshot of the song on a worker thread (the audio lock is only
    /// held for the clone). `wait` blocks the UI (not the audio) until done and
    /// returns whether the save succeeded.
    fn start_save(&mut self, path: PathBuf, wait: bool) -> bool {
        if let Some(job) = self.save_job.take() {
            // finish the previous save first so saves never overlap
            let r = job
                .1
                .recv()
                .unwrap_or_else(|_| Err("save thread died".into()));
            self.finish_save(job.0, r);
        }
        let nb = self.extra.backups.min(10);
        let song = self.with(|s| s.song.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        let p2 = path.clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::recovery::atomic_save(&song, &p2, nb));
        });
        // edits made while saving mark the song dirty again
        let was_dirty = self.dirty;
        self.dirty = false;
        self.save_was_dirty = was_dirty;
        self.status = format!("Saving {}…", path.display());
        if wait {
            let r = rx.recv().unwrap_or_else(|_| Err("save thread died".into()));
            self.finish_save(path, r)
        } else {
            self.save_job = Some((path, rx));
            false
        }
    }

    fn load_warnings_window(&mut self, ctx: &egui::Context) {
        if self.load_warnings.is_empty() {
            return;
        }
        let mut ok = false;
        egui::Window::new("Song opened with warnings")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Some parts of this song could not be loaded:");
                for w in &self.load_warnings {
                    ui.label(format!("• {w}"));
                }
                ui.add_space(6.0);
                ui.label("Saving now would drop them. Use Save As to keep the original file.");
                ok = ui.button("OK").clicked();
            });
        if ok || ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape)) {
            self.load_warnings.clear();
        }
    }

    /// poll the background save (called every frame)
    fn poll_save(&mut self) {
        let Some((_, rx)) = &self.save_job else {
            return;
        };
        let r = match rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(_) => Err("save thread died".into()),
        };
        if let Some((path, _)) = self.save_job.take() {
            self.finish_save(path, r);
        }
    }

    fn finish_save(&mut self, path: PathBuf, r: Result<(), String>) -> bool {
        match r {
            Ok(()) => {
                self.add_recent(&path);
                self.status = format!("Saved {}", path.display());
                true
            }
            Err(e) => {
                self.dirty |= self.save_was_dirty;
                self.status = format!("Save failed: {e} (old file kept; try Save As)");
                false
            }
        }
    }

    fn new_song(&mut self) {
        let ch = self.with(|s| s.song.channels);
        self.with(|s| {
            s.engine.stop();
            let ins = s.song.instruments.clone();
            s.song = Song::empty();
            s.song.channels = ch;
            s.song.channel_names = (1..=ch).map(|i| format!("Ch {i}")).collect();
            s.song.patterns = vec![Pattern::new("Block 00", ROWS, ch)];
            s.song.instruments = ins;
        });
        self.path = None;
        self.reset_cursor();
        self.status = "New song (instruments kept)".into();
    }

    fn add_block(&mut self) {
        let ch = self.with(|s| s.song.channels);
        let n = self.with(|s| {
            s.song.patterns.push(Pattern::new(
                &format!("Block {:02}", s.song.patterns.len()),
                ROWS,
                ch,
            ));
            s.song.order.push(s.song.patterns.len() - 1);
            s.song.patterns.len() - 1
        });
        self.pattern = n;
        self.row = 0;
        self.dirty = true;
        self.status = format!("Added block {n:02}");
    }

    fn set_channels(&mut self, ch: usize) {
        let ch = ch.clamp(1, MAX_CHANNELS);
        self.with(|s| {
            s.song.channels = ch;
            s.song.channel_names.resize(ch, "Ch".into());
            for p in &mut s.song.patterns {
                for r in &mut p.cells {
                    r.resize(ch, Cell::default());
                }
            }
        });
        if self.ch >= ch {
            self.ch = ch - 1;
        }
        self.dirty = true;
    }

    fn add_recent(&mut self, p: &std::path::Path) {
        let full = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let mut txt = full.display().to_string();
        if let Some(t) = txt.strip_prefix(r"\\?\") {
            txt = t.to_string();
        }
        crate::settings::push_recent(&mut self.recent, &txt);
        self.save_settings();
        let title = self.with(|s| s.song.title.clone());
        if let Some(sess) = &self.session {
            sess.write_info(&title, Some(p));
        }
    }

    /// open a .cwt by path (File > Recent, drag and drop)
    fn open_path(&mut self, p: PathBuf) {
        match fileio::load_cwt_warn(&p) {
            Ok((song, warnings)) => {
                self.with(|s| {
                    s.engine.stop();
                    s.song = song;
                });
                self.path = Some(p.clone());
                self.reset_cursor();
                self.add_recent(&p);
                self.status = if warnings.is_empty() {
                    format!("Opened {}", p.display())
                } else {
                    format!("Opened {} with {} warning(s)", p.display(), warnings.len())
                };
                self.load_warnings = warnings;
            }
            Err(e) => self.status = format!("Open failed: {e}"),
        }
    }

    /// periodic autosave + session heartbeat (called every frame, cheap)
    fn tick_safety(&mut self) {
        let Some(sess) = &self.session else { return };
        if self.last_heartbeat.elapsed() >= crate::recovery::HEARTBEAT_EVERY {
            self.last_heartbeat = std::time::Instant::now();
            sess.heartbeat();
        }
        let every = std::time::Duration::from_secs(self.extra.autosave_mins as u64 * 60);
        if self.extra.autosave_mins > 0 && self.dirty && self.last_autosave.elapsed() >= every {
            let song = self.with(|s| s.song.clone());
            // a still-running autosave means we simply try again next frame
            if sess.autosave(song) {
                self.last_autosave = std::time::Instant::now();
                self.status = "Autosaved (recovery copy)".into();
            }
        }
    }

    fn open(&mut self) {
        let mut d = rfd::FileDialog::new().add_filter("ChipWave song", &["cwt"]);
        if !self.extra.song_dir.is_empty() {
            d = d.set_directory(&self.extra.song_dir);
        }
        let Some(p) = d.pick_file() else {
            return;
        };
        self.open_path(p);
    }

    fn reset_cursor(&mut self) {
        self.row = 0;
        self.ch = 0;
        self.col = 0;
        self.pattern = 0;
        self.undo.clear();
        self.redo.clear();
        self.sel = None;
        self.arr_sel = 0;
        self.arr_multi.clear();
        self.dirty = false;
    }

    /// Quick WAV export: pick a file, then render on the export worker thread
    /// (the export dialog shows progress and can cancel).
    fn export_wav(&mut self) {
        if self.export_dlg.as_ref().is_some_and(|d| d.job.is_some()) {
            return;
        }
        let Some(p) = rfd::FileDialog::new()
            .add_filter("WAV audio", &["wav"])
            .set_file_name("chipwave-export.wav")
            .save_file()
        else {
            return;
        };
        self.export_dlg = None;
        self.open_export();
        if let Some(d) = &mut self.export_dlg {
            d.opts.format = crate::export::Format::Wav;
            d.path = p.display().to_string();
        }
        self.start_export();
    }

    fn import_sample(&mut self, path: PathBuf) {
        match fileio::import_wav(&path) {
            Ok(inst) => {
                let name = inst.name.clone();
                let n = self.with(|s| {
                    s.song.instruments.push(inst);
                    s.song.instruments.len()
                });
                self.instr = n as u8;
                self.dirty = true;
                self.status = format!("Imported '{name}' into slot {:02X}", n);
            }
            Err(e) => self.status = format!("Import failed: {e}"),
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let mut events = std::mem::take(&mut self.pending);
        // settings > keys: the next key press becomes the new binding
        if let Some(a) = self.rebinding {
            if let Some(c) = events.iter().find_map(crate::keymap::Combo::from_event) {
                self.finish_rebind(a, c);
            }
            return;
        }
        events = events
            .iter()
            .filter_map(|e| self.keymap.translate(e))
            .collect();
        let channels = self.with(|s| s.song.channels);
        let arr_focus = self.arr_focused();
        self.mods = ctx.input(|i| i.modifiers);
        for a in input::clipboard_actions(&events) {
            match input::route_clip(a, arr_focus, self.mods.shift) {
                input::Routed::Pattern(input::ClipAction::Copy) => self.copy(false, ctx),
                input::Routed::Pattern(input::ClipAction::Cut) => self.copy(true, ctx),
                input::Routed::Pattern(input::ClipAction::Paste(t)) => self.paste(t),
                input::Routed::Arranger(a, t) => self.arranger_action_text(a, t, ctx),
            }
        }
        let arr_visible =
            self.view == View::Arranger || (self.view == View::Pattern && self.dock_arranger);
        for ev in events {
            // the Fill to length dialog owns plain keys while it is open
            if self.fill.is_some() && self.prompt.is_none() {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers: m,
                    ..
                } = &ev
                {
                    if self.fill_key(*key, *m) {
                        continue;
                    }
                }
            }
            if input::is_clipboard_key(&ev) {
                continue;
            }
            if arr_visible {
                if let Some(a) = input::arranger_shortcut(&ev, arr_focus) {
                    self.arranger_action(a, ctx);
                    continue;
                }
            }
            // with the arranger focused, plain keys must not type notes into the pattern
            if arr_focus {
                if let egui::Event::Key {
                    key, modifiers: m, ..
                } = &ev
                {
                    let allowed = m.command
                        || matches!(
                            key,
                            Key::Space
                                | Key::Enter
                                | Key::Escape
                                | Key::F1
                                | Key::F2
                                | Key::F3
                                | Key::F4
                                | Key::F5
                                | Key::F6
                                | Key::F7
                                | Key::F8
                                | Key::F9
                                | Key::F10
                        );
                    if !allowed {
                        continue;
                    }
                    if *key == Key::Escape && self.view == View::Pattern {
                        self.arr_focus = false;
                        continue;
                    }
                }
            }
            if let egui::Event::Key {
                key: Key::K,
                pressed: true,
                modifiers: m,
                ..
            } = &ev
            {
                if m.command && m.shift {
                    self.key_log = if self.key_log.is_some() {
                        None
                    } else {
                        Some(std::collections::VecDeque::new())
                    };
                    self.status = format!(
                        "Key log {}",
                        if self.key_log.is_some() {
                            "on (Ctrl+Shift+K = off)"
                        } else {
                            "off"
                        }
                    );
                    continue;
                }
            }
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers: m,
                repeat,
                ..
            } = ev
            else {
                continue;
            };
            let cmd = m.command;
            if self.modal_key(key, m, ctx) {
                continue;
            }
            let nav_sel = m.shift
                && !cmd
                && matches!(
                    key,
                    Key::ArrowUp
                        | Key::ArrowDown
                        | Key::PageUp
                        | Key::PageDown
                        | Key::ArrowLeft
                        | Key::ArrowRight
                        | Key::Home
                        | Key::End
                );
            if nav_sel {
                let before = (self.row, self.ch);
                let last = self.rows() as isize - 1;
                let mv = |r: usize, d: isize| (r as isize + d).clamp(0, last) as usize;
                match key {
                    Key::ArrowUp => self.row = mv(self.row, -1),
                    Key::ArrowDown => self.row = mv(self.row, 1),
                    Key::PageUp => self.row = mv(self.row, -16),
                    Key::PageDown => self.row = mv(self.row, 16),
                    Key::Home => self.row = 0,
                    Key::End => self.row = last as usize,
                    Key::ArrowLeft => self.ch = self.ch.saturating_sub(1),
                    _ => self.ch = (self.ch + 1).min(channels - 1),
                }
                let a = self.sel.map(|s| s.0).unwrap_or(before);
                self.sel = Some((a, (self.row, self.ch)));
                continue;
            }
            if matches!(
                key,
                Key::ArrowUp
                    | Key::ArrowDown
                    | Key::PageUp
                    | Key::PageDown
                    | Key::ArrowLeft
                    | Key::ArrowRight
                    | Key::Home
                    | Key::End
                    | Key::Escape
            ) {
                self.sel = None;
            }
            match key {
                Key::Tab if cmd => {
                    self.view = match self.view {
                        View::Pattern => View::Arranger,
                        View::Arranger => View::Mixer,
                        View::Mixer => View::Fx,
                        View::Fx => View::Sample,
                        View::Sample | View::Settings => View::Pattern,
                    }
                }
                Key::Num1 if cmd => {
                    self.view = View::Pattern;
                    self.arr_focus = false;
                }
                Key::Num2 if cmd => self.view = View::Arranger,
                Key::Num3 if cmd => self.view = View::Mixer,
                Key::Num4 if cmd => self.view = View::Fx,
                Key::Num5 if cmd => self.view = View::Sample,
                Key::A if cmd => {
                    let last = self.rows() - 1;
                    let chsel = Some(((0, self.ch), (last, self.ch)));
                    self.sel = if self.sel == chsel {
                        Some(((0, 0), (last, channels - 1)))
                    } else {
                        chsel
                    };
                }
                Key::ArrowUp | Key::ArrowDown if matches!(self.view, View::Fx | View::Sample) => {
                    let n = self.with(|s| s.song.instruments.len()).max(1) as i32;
                    let d = if key == Key::ArrowUp { -1 } else { 1 };
                    self.instr = ((self.instr as i32 - 1 + d).rem_euclid(n) + 1) as u8;
                }
                Key::ArrowUp => self.move_row(-1),
                Key::ArrowDown => self.move_row(1),
                Key::PageUp => self.move_row(-16),
                Key::PageDown => self.move_row(16),
                Key::Home if cmd => {
                    self.pattern = 0;
                    self.row = 0;
                }
                Key::End if cmd => {
                    self.pattern = self.with(|s| s.song.patterns.len() - 1);
                    self.row = self.rows() - 1;
                }
                Key::Home => self.row = 0,
                Key::End => self.row = self.rows() - 1,
                Key::ArrowLeft => {
                    if self.col == 0 {
                        self.ch = (self.ch + channels - 1) % channels;
                        self.col = SUBCOLS - 1;
                    } else {
                        self.col -= 1;
                    }
                }
                Key::ArrowRight => {
                    if self.col + 1 >= SUBCOLS {
                        self.ch = (self.ch + 1) % channels;
                        self.col = 0;
                    } else {
                        self.col += 1;
                    }
                }
                Key::Tab => {
                    self.ch = if m.shift {
                        (self.ch + channels - 1) % channels
                    } else {
                        (self.ch + 1) % channels
                    };
                    self.col = 0;
                }
                Key::Space if self.view == View::Sample && !m.shift => self.sample_preview_toggle(),
                Key::Space => self.play(if m.shift { 0 } else { 1 }),
                Key::Enter => self.play(if cmd || m.shift { 2 } else { 3 }),
                Key::Plus | Key::Equals if cmd => self.set_zoom(self.zoom + 0.25),
                Key::Minus if cmd => self.set_zoom(self.zoom - 0.25),
                Key::Num0 if cmd => self.set_zoom(1.0),
                Key::Escape => {
                    self.edit = !self.edit;
                    self.status = format!("Edit mode {}", if self.edit { "on" } else { "off" });
                }
                Key::Z if cmd && m.shift => self.do_undo(true),
                Key::Z if cmd => self.do_undo(false),
                Key::Y if cmd => self.do_undo(true),
                Key::Q if cmd => self.request(crate::prompt::Pending::Quit, ctx),
                Key::Comma if cmd => self.toggle_settings(),
                Key::E if cmd && m.shift => self.open_export(),
                Key::F if cmd && m.shift => self.open_fill(),
                Key::F12 => self.show_help = !self.show_help,
                Key::S if cmd => self.save(m.shift),
                Key::O if cmd => self.request(crate::prompt::Pending::Open, ctx),
                Key::E if cmd => self.export_wav(),
                Key::N if cmd => self.request(crate::prompt::Pending::New, ctx),
                Key::Plus | Key::Equals if m.alt => self.transpose(if m.shift { 12 } else { 1 }),
                Key::Minus if m.alt => self.transpose(if m.shift { -12 } else { -1 }),
                // numpad -/+ (and the main-row -/+): base octave down / up
                Key::Minus if !cmd => self.set_octave(self.octave as i32 - 1),
                Key::Plus if !cmd => self.set_octave(self.octave as i32 + 1),
                Key::F1 | Key::F2 | Key::F3 | Key::F4 | Key::F5 | Key::F6 | Key::F7 | Key::F8 => {
                    self.octave = [
                        Key::F1,
                        Key::F2,
                        Key::F3,
                        Key::F4,
                        Key::F5,
                        Key::F6,
                        Key::F7,
                        Key::F8,
                    ]
                    .iter()
                    .position(|k| *k == key)
                    .unwrap() as u8
                        + 1;
                }
                Key::F9 => {
                    let ch = self.ch;
                    self.with(|s| s.song.mixer.channels[ch].mute ^= true);
                    self.dirty = true;
                }
                Key::F10 => {
                    let ch = self.ch;
                    self.with(|s| s.song.mixer.channels[ch].solo ^= true);
                    self.dirty = true;
                }
                Key::Delete if self.edit && self.sel.is_some() => self.clear_sel(),
                Key::Delete if self.edit => {
                    let col = self.col;
                    self.edit_cell(|c| match col {
                        0 => {
                            c.note = 0;
                            c.instr = 0;
                        }
                        1 | 2 => c.instr = 0,
                        3 | 4 => c.vol = 0,
                        _ => {
                            c.fx = 0;
                            c.fxp = 0;
                        }
                    });
                    self.move_row(self.step as isize);
                }
                Key::Backspace if self.edit => self.shift_rows(false),
                Key::Insert if self.edit => self.shift_rows(true),
                _ if cmd || m.alt => {}
                _ if self.col == 0 => {
                    if key == Key::Num1 && self.edit {
                        self.edit_cell(|c| {
                            c.note = NOTE_OFF;
                            c.instr = 0;
                        });
                        self.move_row(self.step as isize);
                    } else if let Some(off) = note_key(key) {
                        if repeat {
                            continue;
                        }
                        let note = (self.octave as i32 * 12 + off + 1).clamp(1, 120) as u8;
                        let (ch, ins) = (self.ch, self.instr);
                        self.with(|s| {
                            let Shared { song, engine, .. } = s;
                            engine.preview(song, ch, note, ins);
                        });
                        if self.edit {
                            self.edit_cell(|c| {
                                c.note = note;
                                c.instr = ins;
                            });
                            self.move_row(self.step as isize);
                        }
                    }
                }
                _ if self.edit => {
                    if let Some(h) = hex_key(key) {
                        let col = self.col;
                        self.edit_cell(|c| {
                            let set = |v: u8, hi: bool| {
                                if hi {
                                    (v & 0x0F) | (h << 4)
                                } else {
                                    (v & 0xF0) | h
                                }
                            };
                            match col {
                                1 | 2 => c.instr = set(c.instr, col == 1),
                                3 | 4 => {
                                    let v = if c.vol == 0 { 0 } else { c.vol - 1 };
                                    c.vol = set(v, col == 3).min(0x40) + 1;
                                }
                                5 | 6 => c.fx = set(c.fx, col == 5),
                                _ => c.fxp = set(c.fxp, col == 7),
                            }
                        });
                        self.move_row(self.step as isize);
                    }
                }
                _ => {}
            }
        }
    }

    fn set_zoom(&mut self, z: f32) {
        let z = (z * 4.0).round() / 4.0;
        self.zoom = z.clamp(0.5, self.max_zoom.max(0.5));
        if z > self.max_zoom + 0.001 {
            self.status = format!(
                "Scale limited to {:.0} % so everything fits the window",
                self.zoom * 100.0
            );
        }
        self.save_settings();
    }

    fn restart_audio(&mut self) {
        self.audio = None; // stops the old stream + engine thread
        match audio::start_with(
            self.shared.clone(),
            self.buffer,
            &self.extra.device,
            self.extra.rate,
        ) {
            Ok(a) => {
                self.status = format!(
                    "Audio: {} @ {} Hz, buffer {}",
                    a.device_name, a.sample_rate, a.buffer
                );
                self.audio = Some(a);
                self.audio_err = None;
            }
            Err(e) => {
                self.status = format!("Audio restart failed: {e}");
                self.audio_err = Some(e);
            }
        }
    }

    fn save_settings(&self) {
        crate::settings::save(&crate::settings::Settings {
            zoom: self.zoom,
            theme: self.theme.name.to_string(),
            buffer: self.buffer,
            maximized: self.win_max,
            window: self.win_rect,
            recent: self.recent.clone(),
            extra: self.extra.clone(),
        });
    }

    fn set_octave(&mut self, o: i32) {
        self.octave = o.clamp(0, 8) as u8;
        self.status = format!("Base octave {}", self.octave);
    }

    /// selection as (row0, row1, ch0, ch1), inclusive and clamped to the block
    fn sel_rect(&self) -> Option<(usize, usize, usize, usize)> {
        let ((r0, c0), (r1, c1)) = self.sel?;
        let rows = self.rows();
        let chans = self.with(|s| s.song.channels);
        Some((
            r0.min(r1).min(rows - 1),
            r0.max(r1).min(rows - 1),
            c0.min(c1).min(chans - 1),
            c0.max(c1).min(chans - 1),
        ))
    }

    fn for_sel(&mut self, mut f: impl FnMut(&mut Cell)) -> bool {
        let Some((r0, r1, c0, c1)) = self.sel_rect() else {
            return false;
        };
        self.snapshot();
        let p = self.pattern;
        self.with(|s| {
            if let Some(p) = s.song.patterns.get_mut(p) {
                for row in &mut p.cells[r0..=r1] {
                    for c in &mut row[c0..=c1] {
                        f(c);
                    }
                }
            }
        });
        true
    }

    fn copy(&mut self, cut: bool, ctx: &egui::Context) {
        let rect = self
            .sel_rect()
            .unwrap_or((self.row, self.row, self.ch, self.ch));
        let (r0, r1, c0, c1) = rect;
        let p = self.pattern;
        self.clip = self.with(|s| {
            s.song.patterns[p].cells[r0..=r1]
                .iter()
                .map(|r| r[c0..=c1].to_vec())
                .collect()
        });
        // mirror to the system clipboard; this also makes egui deliver
        // Event::Paste on Ctrl+V (it sends nothing when the clipboard is empty)
        ctx.copy_text(input::clip_to_text(&self.clip));
        if cut {
            if self.sel.is_none() {
                self.sel = Some(((r0, c0), (r1, c1)));
                self.clear_sel();
                self.sel = None;
            } else {
                self.clear_sel();
            }
        }
        self.status = format!(
            "{} {} rows x {} channels",
            if cut { "Cut" } else { "Copied" },
            r1 - r0 + 1,
            c1 - c0 + 1
        );
    }

    fn paste(&mut self, text: Option<String>) {
        if let Some(c) = text.as_deref().and_then(input::clip_from_text) {
            self.clip = c;
        }
        if self.clip.is_empty() {
            self.status = "Nothing to paste (copy cells with Ctrl+C first)".into();
            return;
        }
        if !self.edit {
            self.status = "Edit mode is off (Esc) - paste ignored".into();
            return;
        }
        self.snapshot();
        let (p, r0, c0) = (self.pattern, self.row, self.ch);
        let clip = self.clip.clone();
        self.with(|s| {
            let chans = s.song.channels;
            if let Some(p) = s.song.patterns.get_mut(p) {
                for (dr, row) in clip.iter().enumerate() {
                    let Some(dst) = p.cells.get_mut(r0 + dr) else {
                        break;
                    };
                    for (dc, cell) in row.iter().enumerate() {
                        if c0 + dc < chans {
                            dst[c0 + dc] = *cell;
                        }
                    }
                }
            }
        });
        let h = clip.len();
        let w = clip.first().map(|r| r.len()).unwrap_or(1);
        let last_r = (r0 + h - 1).min(self.rows() - 1);
        let last_c = (c0 + w - 1).min(self.with(|s| s.song.channels) - 1);
        self.sel = Some(((r0, c0), (last_r, last_c)));
        self.status = format!("Pasted {h} rows x {w} channels");
    }

    fn clear_sel(&mut self) {
        if self.for_sel(|c| *c = Cell::default()) {
            self.status = "Cleared selection".into();
        }
    }

    fn transpose(&mut self, semis: i32) {
        let tr = |c: &mut Cell| {
            if c.note > 0 && c.note != NOTE_OFF {
                c.note = (c.note as i32 + semis).clamp(1, 120) as u8;
            }
        };
        if self.for_sel(tr) {
            self.status = format!("Transposed selection by {semis:+} semitones");
            return;
        }
        self.snapshot();
        let (p, ch) = (self.pattern, self.ch);
        self.with(|s| {
            if let Some(p) = s.song.patterns.get_mut(p) {
                for row in &mut p.cells {
                    tr(&mut row[ch]);
                }
            }
        });
        self.status = format!("Transposed channel {} by {semis:+} semitones", self.ch + 1);
    }

    fn shift_rows(&mut self, insert: bool) {
        self.snapshot();
        let (p, ch, row) = (self.pattern, self.ch, self.row);
        self.with(|s| {
            if let Some(p) = s.song.patterns.get_mut(p) {
                let n = p.rows;
                if insert {
                    for r in (row + 1..n).rev() {
                        p.cells[r][ch] = p.cells[r - 1][ch];
                    }
                    p.cells[row][ch] = Cell::default();
                } else if row > 0 {
                    for r in row - 1..n - 1 {
                        p.cells[r][ch] = p.cells[r + 1][ch];
                    }
                    p.cells[n - 1][ch] = Cell::default();
                }
            }
        });
        if !insert && self.row > 0 {
            self.row -= 1;
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Instruments").strong().size(15.0));
        });
        let names: Vec<(String, bool)> = self.with(|s| {
            s.song
                .instruments
                .iter()
                .map(|i| {
                    (
                        i.name.clone(),
                        matches!(i.kind, InstrumentKind::Sample { .. }),
                    )
                })
                .collect()
        });
        egui::ScrollArea::vertical()
            .id_salt("inst")
            .max_height(260.0)
            .show(ui, |ui| {
                for (i, (n, is_sample)) in names.iter().enumerate() {
                    let slot = (i + 1) as u8;
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        let txt = egui::RichText::new(format!(
                            "{:02X}  {}{}",
                            slot,
                            n,
                            if *is_sample { "  (sample)" } else { "" }
                        ))
                        .monospace();
                        if ui.selectable_label(self.instr == slot, txt).clicked() {
                            self.instr = slot;
                        }
                    });
                }
            });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            if ui.button("Import WAV…").clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("WAV", &["wav"])
                    .pick_file()
                {
                    self.import_sample(p);
                }
            }
            ui.label(
                egui::RichText::new("or drop .wav files")
                    .small()
                    .color(th().ink2),
            );
        });
        ui.separator();
        // patch editor for the selected synth instrument
        let idx = self.instr as usize;
        let mut changed = false;
        let Some((name, mut volume, mut transpose, mut patch)) = self.with(|s| {
            s.song.instruments.get(idx.wrapping_sub(1)).map(|i| {
                let patch = match &i.kind {
                    InstrumentKind::Synth(p) => Some(p.clone()),
                    _ => None,
                };
                (i.name.clone(), i.volume, i.transpose, patch)
            })
        }) else {
            return;
        };
        // edit a copy without holding the audio lock, commit only on change
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new(format!("Sound · {}", name)).strong());
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label("Volume");
            changed |= ui.add(egui::Slider::new(&mut volume, 0.0..=1.0)).changed();
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label("Transpose");
            for (txt, d) in [("-12", -12), (" - ", -1)] {
                if ui.button(txt).clicked() {
                    transpose = (transpose as i32 + d).clamp(-48, 48) as i8;
                    changed = true;
                }
            }
            ui.label(
                egui::RichText::new(format!("{:+} st", transpose))
                    .monospace()
                    .color(th().value),
            );
            for (txt, d) in [(" + ", 1), ("+12", 12)] {
                if ui.button(txt).clicked() {
                    transpose = (transpose as i32 + d).clamp(-48, 48) as i8;
                    changed = true;
                }
            }
        });
        if let Some(p) = &mut patch {
            egui::Grid::new("patch")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label("   Osc");
                    egui::ComboBox::from_id_salt("wave")
                        .selected_text(format!("{:?}", p.osc1))
                        .show_ui(ui, |ui| {
                            for w in [
                                Wave::Saw,
                                Wave::Square,
                                Wave::Pulse,
                                Wave::Triangle,
                                Wave::Sine,
                            ] {
                                changed |= ui
                                    .selectable_value(&mut p.osc1, w, format!("{w:?}"))
                                    .changed();
                            }
                        });
                    ui.end_row();
                    ui.label("   Octave");
                    changed |= ui.add(egui::Slider::new(&mut p.osc1_oct, -3..=3)).changed();
                    ui.end_row();
                    ui.label("   Pulse width");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.pulse_width, 0.05..=0.95))
                        .changed();
                    ui.end_row();
                    ui.label("   Unison");
                    changed |= ui.add(egui::Slider::new(&mut p.unison, 1..=8)).changed();
                    ui.end_row();
                    ui.label("   Detune ct");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.detune_cents, 0.0..=50.0))
                        .changed();
                    ui.end_row();
                    ui.label("   Filter");
                    egui::ComboBox::from_id_salt("fmode")
                        .selected_text(format!("{:?}", p.filter))
                        .show_ui(ui, |ui| {
                            for m in [FilterMode::Lp, FilterMode::Hp, FilterMode::Bp] {
                                changed |= ui
                                    .selectable_value(&mut p.filter, m, format!("{m:?}"))
                                    .changed();
                            }
                        });
                    ui.end_row();
                    ui.label("   Cutoff Hz");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.cutoff, 20.0..=18000.0).logarithmic(true))
                        .changed();
                    ui.end_row();
                    ui.label("   Resonance");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.resonance, 0.0..=0.98))
                        .changed();
                    ui.end_row();
                    ui.label("   Env amount");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.env_amount, -1.0..=1.0))
                        .changed();
                    ui.end_row();
                    ui.label("   Attack s");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.amp.a, 0.0..=2.0).logarithmic(true))
                        .changed();
                    ui.end_row();
                    ui.label("   Decay s");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.amp.d, 0.001..=3.0).logarithmic(true))
                        .changed();
                    ui.end_row();
                    ui.label("   Sustain");
                    changed |= ui.add(egui::Slider::new(&mut p.amp.s, 0.0..=1.0)).changed();
                    ui.end_row();
                    ui.label("   Release s");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.amp.r, 0.001..=4.0).logarithmic(true))
                        .changed();
                    ui.end_row();
                    ui.label("   Filter decay");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.fenv.d, 0.001..=3.0).logarithmic(true))
                        .changed();
                    ui.end_row();
                    ui.label("   Drive");
                    changed |= ui.add(egui::Slider::new(&mut p.drive, 0.0..=1.0)).changed();
                    ui.end_row();
                });
        }
        if changed {
            self.with(|s| {
                if let Some(inst) = s.song.instruments.get_mut(idx.wrapping_sub(1)) {
                    inst.volume = volume;
                    inst.transpose = transpose;
                    if let (Some(p), InstrumentKind::Synth(dst)) = (patch, &mut inst.kind) {
                        *dst = p;
                    }
                }
            });
            self.dirty = true;
        }
    }

    fn pattern_view(&mut self, ui: &mut egui::Ui) {
        let (song_snapshot, play_pat, play_row, playing) = self.with(|s| {
            (
                s.song.patterns.get(self.pattern).cloned(),
                s.engine.pattern,
                s.engine.row,
                s.engine.playing,
            )
        });
        let (channels, names, npat, mixer) = self.with(|s| {
            (
                s.song.channels,
                s.song.channel_names.clone(),
                s.song.patterns.len(),
                s.song.mixer.clone(),
            )
        });
        if playing && self.follow {
            self.pattern = play_pat.min(npat - 1);
            self.row = play_row;
        }
        let Some(pat) = song_snapshot else { return };
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Block").strong());
            if ui.button("<").clicked() && self.pattern > 0 {
                self.pattern -= 1;
            }
            ui.label(
                egui::RichText::new(format!("{:02} {}", self.pattern, pat.name))
                    .monospace()
                    .color(th().value)
                    .strong(),
            );
            if ui.button(">").clicked() && self.pattern + 1 < npat {
                self.pattern += 1;
            }
            if ui
                .button("+ New block")
                .on_hover_text("add an empty block at the end of the order")
                .clicked()
            {
                self.add_block();
            }
            ui.separator();
            let order = self.with(|s| s.song.order.clone());
            ui.label(
                egui::RichText::new(format!(
                    "Order: {}",
                    order
                        .iter()
                        .map(|o| format!("{o:02}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ))
                .monospace(),
            );
            if ui
                .small_button("+cur")
                .on_hover_text("append current block to the order list")
                .clicked()
            {
                let p = self.pattern;
                self.with(|s| s.song.order.push(p));
                self.dirty = true;
            }
            if ui.small_button("-last").clicked() {
                self.with(|s| {
                    if s.song.order.len() > 1 {
                        s.song.order.pop();
                    }
                });
                self.dirty = true;
            }
            ui.separator();
            let mut ch = channels;
            ui.label("Channels");
            if ui
                .add(egui::DragValue::new(&mut ch).range(1..=MAX_CHANNELS))
                .changed()
            {
                self.set_channels(ch);
            }
        });

        let avail = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(avail, Sense::click_and_drag());
        let sel = self.sel_rect();
        let painter = ui.painter_at(avail);
        theme::crt(&painter, avail);
        let font = FontId::monospace(14.0);
        let cw = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
        let rh = 19.0;
        let row_w = cw * 3.0;
        let chan_w = cw * 15.0 + 10.0; // "C-4 01 40 0C20" + gap
        let x0 = avail.left() + 10.0 + row_w + 8.0;
        let header_y = avail.top() + 6.0;
        let top = header_y + 26.0;
        let visible = (((avail.bottom() - top) / rh) as usize).max(1);
        let start = (self.row as isize - visible as isize / 2)
            .clamp(0, (pat.rows as isize - visible as isize).max(0)) as usize;

        // horizontal scroll so the cursor channel is visible
        let max_ch_vis = (((avail.right() - x0) / chan_w) as usize).max(1);
        let first_ch = if self.ch >= max_ch_vis {
            self.ch + 1 - max_ch_vis
        } else {
            0
        };

        for (vi, ch) in (first_ch..channels).enumerate() {
            let x = x0 + vi as f32 * chan_w;
            if x > avail.right() {
                break;
            }
            let hr = Rect::from_min_size(Pos2::new(x, header_y), egui::vec2(chan_w - 8.0, 22.0));
            painter.rect_filled(
                hr,
                6,
                if !mixer.audible(ch) {
                    hex(0x4A525E)
                } else {
                    th().slate
                },
            );
            painter.text(
                hr.left_center() + egui::vec2(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("{} {}", ch + 1, names.get(ch).cloned().unwrap_or_default()),
                FontId::proportional(13.0),
                Color32::WHITE,
            );
            painter.text(
                hr.right_center() - egui::vec2(6.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                if mixer.channels[ch].solo {
                    "SOLO"
                } else if mixer.channels[ch].mute {
                    "MUTED"
                } else {
                    ""
                },
                FontId::proportional(11.0),
                hex(0xFFB4AE),
            );
        }
        for vi in 0..visible {
            let r = start + vi;
            if r >= pat.rows {
                break;
            }
            let y = top + vi as f32 * rh;
            let row_rect = Rect::from_min_size(
                Pos2::new(avail.left() + 4.0, y),
                egui::vec2(avail.width() - 8.0, rh),
            );
            if r.is_multiple_of(16) {
                painter.rect_filled(row_rect, 0, th().crt_grid.gamma_multiply(0.9));
            } else if r.is_multiple_of(4) {
                painter.rect_filled(row_rect, 0, th().crt_grid.gamma_multiply(0.55));
            }
            if playing && play_pat == self.pattern && r == play_row {
                painter.rect_stroke(
                    row_rect,
                    3,
                    Stroke::new(1.5, th().play),
                    egui::StrokeKind::Inside,
                );
            }
            if r == self.row {
                painter.rect_filled(row_rect, 4, th().slate.gamma_multiply(0.7));
            }
            if let Some((r0, r1, c0, c1)) = sel {
                if (r0..=r1).contains(&r) {
                    for (vi_c, ch) in (first_ch..channels).enumerate() {
                        if (c0..=c1).contains(&ch) {
                            let x = x0 + vi_c as f32 * chan_w - 3.0;
                            let sr =
                                Rect::from_min_size(Pos2::new(x, y), egui::vec2(chan_w - 4.0, rh));
                            painter.rect_filled(sr, 0, th().sel);
                            let st = Stroke::new(1.5, th().accent);
                            if r == r0 {
                                painter.hline(sr.x_range(), sr.top(), st);
                            }
                            if r == r1 {
                                painter.hline(sr.x_range(), sr.bottom(), st);
                            }
                            if ch == c0 {
                                painter.vline(sr.left(), sr.y_range(), st);
                            }
                            if ch == c1 {
                                painter.vline(sr.right(), sr.y_range(), st);
                            }
                        }
                    }
                }
            }
            painter.text(
                Pos2::new(avail.left() + 10.0, y + 2.0),
                egui::Align2::LEFT_TOP,
                format!("{:02X}", r),
                font.clone(),
                if r.is_multiple_of(4) {
                    th().fx
                } else {
                    hex(0x7FA6D9)
                },
            );
            for (vi_c, ch) in (first_ch..channels).enumerate() {
                let x = x0 + vi_c as f32 * chan_w;
                if x > avail.right() {
                    break;
                }
                let c = pat.cells[r][ch];
                let parts: [(String, Color32, usize); 4] = [
                    (
                        note_name(c.note),
                        if c.note == 0 { th().empty } else { th().note },
                        0,
                    ),
                    (
                        if c.instr == 0 {
                            "..".into()
                        } else {
                            format!("{:02X}", c.instr)
                        },
                        if c.instr == 0 { th().empty } else { th().instr },
                        4,
                    ),
                    (
                        if c.vol == 0 {
                            "..".into()
                        } else {
                            format!("{:02X}", c.vol - 1)
                        },
                        if c.vol == 0 { th().empty } else { th().vol },
                        7,
                    ),
                    (
                        if c.fx == 0 && c.fxp == 0 {
                            "....".into()
                        } else {
                            format!("{:02X}{:02X}", c.fx, c.fxp)
                        },
                        if c.fx == 0 && c.fxp == 0 {
                            th().empty
                        } else {
                            th().fx
                        },
                        10,
                    ),
                ];
                for (t, col, off) in parts {
                    painter.text(
                        Pos2::new(x + off as f32 * cw, y + 2.0),
                        egui::Align2::LEFT_TOP,
                        t,
                        font.clone(),
                        col,
                    );
                }
                if r == self.row && ch == self.ch {
                    let (off, w) = match self.col {
                        0 => (0.0, 3.0),
                        1 => (4.0, 1.0),
                        2 => (5.0, 1.0),
                        3 => (7.0, 1.0),
                        4 => (8.0, 1.0),
                        k => (10.0 + (k - 5) as f32, 1.0),
                    };
                    let cr = Rect::from_min_size(
                        Pos2::new(x + off * cw - 1.0, y),
                        egui::vec2(w * cw + 2.0, rh),
                    );
                    painter.rect_stroke(
                        cr,
                        3,
                        Stroke::new(2.0, if self.edit { th().fx } else { Color32::WHITE }),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }
        let cell_at = |p: Pos2| -> Pos {
            let vr = ((p.y - top) / rh).floor().max(0.0);
            let vc = ((p.x - x0) / chan_w).floor().max(0.0);
            (
                (start + vr as usize).min(pat.rows - 1),
                (first_ch + vc as usize).min(channels - 1),
            )
        };
        if resp.clicked() || resp.drag_started() {
            self.arr_focus = false;
        }
        // mouse wheel: 1 row per notch (Shift = 16 rows); line and pixel deltas
        if resp.hovered() {
            let ev = ui.input(|i| i.events.clone());
            let n = input::take_notches(&mut self.wheel_pat, input::wheel_lines(&ev));
            if n != 0 {
                let per = if ui.input(|i| i.modifiers.shift) {
                    16
                } else {
                    1
                };
                let last = pat.rows as isize - 1;
                self.row = (self.row as isize - n as isize * per).clamp(0, last) as usize;
                self.sel = None;
            }
        }
        if self.view == View::Pattern && self.dock_arranger && !self.arr_focus {
            focus_ring(&ui.painter_at(avail.expand(6.0)), avail.expand(2.0));
        }
        if resp.drag_started() {
            if let Some(o) = ui.input(|i| i.pointer.press_origin()) {
                self.drag_anchor = Some(cell_at(o));
            }
        }
        if resp.dragged() {
            if let (Some(a), Some(p)) = (self.drag_anchor, resp.interact_pointer_pos()) {
                let c = cell_at(p);
                self.row = c.0;
                self.ch = c.1;
                self.sel = Some((a, c));
                // auto-scroll when dragging past the edges
                if p.y > avail.bottom() - rh {
                    self.row = (self.row + 1).min(pat.rows - 1);
                    self.sel = Some((a, (self.row, self.ch)));
                }
            }
        }
        if resp.drag_stopped() {
            self.drag_anchor = None;
        }
        if resp.clicked() {
            self.sel = None;
            if let Some(p) = resp.interact_pointer_pos() {
                let vr = ((p.y - top) / rh).floor();
                if vr >= 0.0 {
                    self.row = (start + vr as usize).min(pat.rows - 1);
                }
                let vc = ((p.x - x0) / chan_w).floor();
                if vc >= 0.0 {
                    self.ch = (first_ch + vc as usize).min(channels - 1);
                    let within = (p.x - x0 - vc * chan_w) / cw;
                    self.col = match within as usize {
                        0..=3 => 0,
                        4 => 1,
                        5 | 6 => 2,
                        7 => 3,
                        8 | 9 => 4,
                        10 => 5,
                        11 => 6,
                        12 => 7,
                        _ => 8,
                    };
                }
            }
            ui.memory_mut(|m| m.surrender_focus(m.focused().unwrap_or(egui::Id::NULL)));
        }
    }

    // ---------------------------------------------------------------- arranger

    fn arr_focused(&self) -> bool {
        self.view == View::Arranger
            || (self.view == View::Pattern && self.dock_arranger && self.arr_focus)
    }

    fn order_len(&self) -> usize {
        self.with(|s| s.song.order.len())
    }

    /// selected song-order indices, sorted (never empty)
    fn arr_sel_list(&self) -> Vec<usize> {
        let len = self.order_len();
        let mut v: Vec<usize> = self
            .arr_multi
            .iter()
            .copied()
            .filter(|&i| i < len)
            .collect();
        if v.is_empty() {
            v.push(self.arr_sel.min(len - 1));
        }
        v
    }

    fn arr_select(&mut self, idx: impl IntoIterator<Item = usize>, anchor: usize) {
        let len = self.order_len();
        self.arr_multi = idx.into_iter().filter(|&i| i < len).collect();
        self.arr_sel = anchor.min(len - 1);
        self.arr_multi.insert(self.arr_sel);
        self.pattern = self.with(|s| s.song.order[self.arr_sel]);
        self.row = self.row.min(self.rows() - 1);
    }

    fn arr_flash_now(&mut self, idx: Vec<usize>, ctx: &egui::Context) {
        self.arr_flash = Some((idx, ctx.input(|i| i.time)));
    }

    /// insert block ids after the last selected entry (optionally as new copies)
    fn arr_insert_blocks(&mut self, blocks: &[usize], unique: bool, ctx: &egui::Context) -> usize {
        let npat = self.with(|s| s.song.patterns.len());
        let blocks: Vec<usize> = blocks.iter().copied().filter(|&b| b < npat).collect();
        if blocks.is_empty() {
            return 0;
        }
        self.snapshot();
        let at = self.arr_sel_list().last().copied().unwrap_or(0) + 1;
        self.with(|s| {
            let song = &mut s.song;
            let at = at.min(song.order.len());
            for (k, &b) in blocks.iter().enumerate() {
                let b = if unique {
                    let mut p = song.patterns[b].clone();
                    p.name = format!("{} copy", p.name);
                    song.patterns.push(p);
                    song.patterns.len() - 1
                } else {
                    b
                };
                song.order.insert(at + k, b);
            }
        });
        let range: Vec<usize> = (at..at + blocks.len()).collect();
        self.arr_select(range.clone(), at + blocks.len() - 1);
        self.arr_flash_now(range, ctx);
        blocks.len()
    }

    fn arr_insert(&mut self, ctx: &egui::Context) {
        self.snapshot();
        let n = self.with(|s| {
            let n = s.song.patterns.len();
            let ch = s.song.channels;
            s.song
                .patterns
                .push(Pattern::new(&format!("Block {n:02}"), ROWS, ch));
            n
        });
        // the snapshot above already covers this; don't push a second one
        let undo = self.undo.pop();
        self.arr_insert_blocks(&[n], false, ctx);
        if let (Some(u), Some(last)) = (undo, self.undo.last_mut()) {
            *last = u;
        }
        self.status = format!("Inserted new empty block {n:02} at #{}", self.arr_sel + 1);
    }

    fn arr_duplicate(&mut self, unique: bool, ctx: &egui::Context) {
        let order = self.with(|s| s.song.order.clone());
        let blocks: Vec<usize> = self.arr_sel_list().iter().map(|&i| order[i]).collect();
        let n = self.arr_insert_blocks(&blocks, unique, ctx);
        self.status = format!(
            "{} {n} clip(s) -> #{}",
            if unique {
                "Duplicated as new block copies:"
            } else {
                "Duplicated (same blocks):"
            },
            self.arr_sel + 2 - n
        );
    }

    fn arr_copy(&mut self, cut: bool, ctx: &egui::Context) {
        let order = self.with(|s| s.song.order.clone());
        let idx = self.arr_sel_list();
        self.arr_clip = idx.iter().map(|&i| order[i]).collect();
        // system clipboard text so Ctrl+V delivers Event::Paste on Windows
        ctx.copy_text(input::arr_clip_to_text(&self.arr_clip));
        let names: Vec<String> = self.arr_clip.iter().map(|b| format!("{b:02}")).collect();
        if cut {
            if idx.len() >= order.len() {
                self.status = format!(
                    "Copied {} clip(s) [{}] - can't cut every clip, the song needs one entry",
                    idx.len(),
                    names.join(" ")
                );
                return;
            }
            self.arr_remove(&idx);
            self.status = format!("Cut {} clip(s) [{}]", idx.len(), names.join(" "));
        } else {
            self.arr_flash_now(idx.clone(), ctx);
            self.status = format!("Copied {} clip(s) [{}]", idx.len(), names.join(" "));
        }
    }

    fn arr_paste(&mut self, text: Option<String>, unique: bool, ctx: &egui::Context) {
        if let Some(c) = text.as_deref().and_then(input::arr_clip_from_text) {
            self.arr_clip = c;
        }
        if self.arr_clip.is_empty() {
            self.status = "Arranger clipboard is empty (select clips, Ctrl+C)".into();
            return;
        }
        let clip = self.arr_clip.clone();
        let n = self.arr_insert_blocks(&clip, unique, ctx);
        self.status = if n == 0 {
            "Nothing pasted (blocks not in this song)".into()
        } else {
            format!(
                "Pasted {n} clip(s){} at #{}",
                if unique { " as new block copies" } else { "" },
                self.arr_sel + 2 - n
            )
        };
    }

    fn arr_remove(&mut self, idx: &[usize]) {
        self.snapshot();
        let first = idx.first().copied().unwrap_or(0);
        self.with(|s| {
            for &i in idx.iter().rev() {
                if i < s.song.order.len() {
                    s.song.order.remove(i);
                }
            }
        });
        let len = self.order_len();
        let a = first.min(len - 1);
        self.arr_select([a], a);
    }

    fn arr_delete(&mut self) {
        let idx = self.arr_sel_list();
        if idx.len() >= self.order_len() {
            self.status = "The song needs at least one entry".into();
            return;
        }
        self.arr_remove(&idx);
        self.status = format!("Removed {} clip(s) (blocks stay in the pool)", idx.len());
    }

    fn arr_move(&mut self, from: usize, to: usize) {
        if from == to || to == from + 1 {
            return;
        }
        self.snapshot();
        let to = self.with(|s| {
            let b = s.song.order.remove(from);
            let to = if to > from { to - 1 } else { to };
            let to = to.min(s.song.order.len());
            s.song.order.insert(to, b);
            to
        });
        self.arr_select([to], to);
        self.status = "Moved clip".into();
    }

    fn arr_zoom(&mut self, f: f32) {
        self.with(|s| s.song.arranger_zoom = (s.song.arranger_zoom * f).clamp(0.25, 16.0));
    }

    fn arranger_action(&mut self, a: input::ArrAction, ctx: &egui::Context) {
        self.arranger_action_text(a, None, ctx)
    }

    fn arranger_action_text(
        &mut self,
        a: input::ArrAction,
        text: Option<String>,
        ctx: &egui::Context,
    ) {
        use input::ArrAction::*;
        let len = self.order_len();
        let cur = self.arr_sel.min(len - 1);
        let ends = self.arr_sel_list();
        match a {
            Prev => self.arr_select([cur.saturating_sub(1)], cur.saturating_sub(1)),
            Next => self.arr_select([(cur + 1).min(len - 1)], (cur + 1).min(len - 1)),
            ExtendPrev | ExtendNext => {
                let (lo, hi) = (ends[0], *ends.last().unwrap());
                let (lo, hi) = if a == ExtendPrev {
                    (lo.saturating_sub(1), hi)
                } else {
                    (lo, (hi + 1).min(len - 1))
                };
                self.arr_select(lo..=hi, if a == ExtendPrev { lo } else { hi });
            }
            SelectAll => {
                self.arr_select(0..len, cur);
                self.status = format!("Selected all {len} clips");
            }
            Delete => self.arr_delete(),
            Insert => self.arr_insert(ctx),
            Duplicate => self.arr_duplicate(false, ctx),
            DuplicateUnique => self.arr_duplicate(true, ctx),
            ZoomIn => self.arr_zoom(1.25),
            ZoomOut => self.arr_zoom(0.8),
            Copy => self.arr_copy(false, ctx),
            Cut => self.arr_copy(true, ctx),
            Paste => self.arr_paste(text, false, ctx),
            PasteUnique => self.arr_paste(text, true, ctx),
            Role(r) => self.set_role(r),
        }
        if matches!(a, Prev | Next) {
            self.row = 0;
        }
    }

    #[allow(clippy::type_complexity)]
    fn arranger_view(&mut self, ui: &mut egui::Ui) {
        let (order, blocks, zoom, playing, loop_pat, opos, prow, ppat) = self.with(|s| {
            #[allow(clippy::type_complexity)]
            let blocks: Vec<(
                String,
                usize,
                Vec<(usize, usize)>,
                Option<crate::arrange::Role>,
                bool,
            )> = s
                .song
                .patterns
                .iter()
                .map(|p| {
                    let mut ticks = vec![];
                    for (r, row) in p.cells.iter().enumerate() {
                        for (c, cell) in row.iter().enumerate().take(s.song.channels) {
                            if cell.note > 0 && cell.note != NOTE_OFF {
                                ticks.push((r, c));
                            }
                        }
                    }
                    (p.name.clone(), p.rows, ticks, p.role, p.derived.is_some())
                })
                .collect();
            (
                s.song.order.clone(),
                blocks,
                s.song.arranger_zoom,
                s.engine.playing,
                s.engine.loop_pattern,
                s.engine.order_pos,
                s.engine.row,
                s.engine.pattern,
            )
        });
        let channels = self.with(|s| s.song.channels).max(1);
        self.arr_sel = self.arr_sel.min(order.len() - 1);
        if self.arr_focused() {
            focus_ring(ui.painter(), ui.max_rect().expand(3.0));
        }
        ui.horizontal_wrapped(|ui| {
            if self.arr_focused() {
                ui.label(
                    egui::RichText::new(" ARRANGER ")
                        .strong()
                        .monospace()
                        .color(Color32::from_rgb(0x1A, 0x1C, 0x1F))
                        .background_color(th().focus_ring),
                )
                .on_hover_text("keyboard focus: Ctrl+X/C/V, Del, arrows act on clips");
            } else {
                ui.label(egui::RichText::new("Arranger").strong().color(th().ink2))
                    .on_hover_text("click to give the arranger keyboard focus");
            }
            ui.label(egui::RichText::new("zoom").color(th().ink2));
            if ui.small_button("-").clicked() {
                self.arr_zoom(0.8);
            }
            if ui.small_button("+").clicked() {
                self.arr_zoom(1.25);
            }
            ui.separator();
            if ui.button("Insert block").on_hover_text("Ins").clicked() {
                self.arranger_action(input::ArrAction::Insert, &ui.ctx().clone());
            }
            if ui.button("Duplicate").on_hover_text("Ctrl+D").clicked() {
                self.arranger_action(input::ArrAction::Duplicate, &ui.ctx().clone());
            }
            if ui
                .button("Copy as new")
                .on_hover_text("Ctrl+Shift+D: unique copy of the block")
                .clicked()
            {
                self.arranger_action(input::ArrAction::DuplicateUnique, &ui.ctx().clone());
            }
            if ui.button("Delete").on_hover_text("Del").clicked() {
                self.arr_delete();
            }
            ui.separator();
            for (t, h, a) in [
                ("Cut", "Ctrl+X", input::ArrAction::Cut),
                ("Copy", "Ctrl+C", input::ArrAction::Copy),
                (
                    "Paste",
                    "Ctrl+V (Ctrl+Shift+V = as new blocks)",
                    input::ArrAction::Paste,
                ),
            ] {
                if ui.small_button(t).on_hover_text(h).clicked() {
                    self.arr_focus = true;
                    self.arranger_action(a, &ui.ctx().clone());
                }
            }
            ui.checkbox(&mut self.follow, "Follow");
            ui.label(
                egui::RichText::new(format!(
                    "{} entries · drag clips to reorder · click = open in editor",
                    order.len()
                ))
                .small()
                .color(th().ink2),
            );
        });
        self.automation_bar(ui);
        // clip x positions (in rows)
        let mut starts = Vec::with_capacity(order.len() + 1);
        let mut acc = 0usize;
        for &b in &order {
            starts.push(acc);
            acc += blocks.get(b).map(|x| x.1).unwrap_or(ROWS);
        }
        starts.push(acc);
        let pad = 8.0;
        let avail_h = ui.available_height().max(80.0);
        let follow = self.follow && playing;
        let (lanes, song_pos, fxk) = self.with(|s| {
            (
                s.song.automation.clone(),
                s.engine.song_pos,
                s.song
                    .instruments
                    .iter()
                    .map(|i| i.fx.iter().map(|d| d.kind).collect::<Vec<_>>())
                    .collect::<Vec<_>>(),
            )
        });
        let show_auto = self.show_auto;
        let n_lanes = if show_auto { lanes.len() } else { 0 };
        let lanes_h = n_lanes as f32 * (LANE_H + 4.0) + if n_lanes > 0 { 4.0 } else { 0.0 };
        let clip_h = (avail_h - 14.0 - lanes_h).max(70.0);
        // mouse wheel: scroll horizontally; Ctrl+wheel zooms around the pointer
        let hover_x = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| self.arr_area.contains(*p))
            .map(|p| p.x);
        if let Some(px) = hover_x {
            let ev = ui.input(|i| i.events.clone());
            let lines = input::wheel_lines(&ev);
            if lines != 0.0 {
                let ctrl = ui.input(|i| i.modifiers.command);
                if ctrl {
                    let n = input::take_notches(&mut self.wheel_arr, lines);
                    if n != 0 {
                        let z0 = zoom;
                        self.arr_zoom(1.15f32.powi(n));
                        let z1 = self.with(|s| s.song.arranger_zoom);
                        let rel = px - self.arr_area.left();
                        let off = ((self.arr_scroll + rel - pad) * z1 / z0 + pad - rel).max(0.0);
                        self.arr_scroll_set = Some(off);
                    }
                } else {
                    // smooth: pixels follow the wheel directly (40 px per line)
                    let off = (self.arr_scroll - lines * 40.0).max(0.0);
                    self.arr_scroll_set = Some(off);
                }
            }
        }
        let zoom = self.with(|s| s.song.arranger_zoom);
        let ghost_rows = self
            .fill
            .as_ref()
            .and_then(|f| f.prop.as_ref())
            .map(|p| p.rows);
        let total_w = acc.max(ghost_rows.unwrap_or(0)) as f32 * zoom + pad * 2.0;
        let mut lane_ops: Vec<LaneOp> = vec![];
        let mut sa = egui::ScrollArea::both()
            .id_salt("arranger_scroll")
            .auto_shrink([false, false])
            .scroll_source(egui::containers::scroll_area::ScrollSource {
                mouse_wheel: false,
                ..Default::default()
            });
        if let Some(off) = self.arr_scroll_set.take() {
            sa = sa.horizontal_scroll_offset(off);
        }
        let out = sa.show(ui, |ui| {
            let w = total_w.max(ui.available_width());
            let (rect, bg) = ui.allocate_exact_size(egui::vec2(w, clip_h), Sense::click());
            if bg.clicked() || bg.secondary_clicked() {
                self.arr_focus = true;
            }
            let painter = ui.painter_at(rect);
            theme::crt(&painter, rect);
            let ruler_h = 16.0;
            let full_lane = Rect::from_min_max(
                Pos2::new(rect.left(), rect.top() + ruler_h + 4.0),
                Pos2::new(rect.right(), rect.bottom() - 6.0),
            );
            // with a Fill-to-length proposal: real clips on top, ghost clips below
            let (lane, ghost) = if ghost_rows.is_some() {
                let h = full_lane.height();
                (
                    Rect::from_min_max(
                        full_lane.min,
                        Pos2::new(full_lane.right(), full_lane.top() + h * 0.5),
                    ),
                    Some(Rect::from_min_max(
                        Pos2::new(full_lane.left(), full_lane.top() + h * 0.5 + 14.0),
                        full_lane.max,
                    )),
                )
            } else {
                (full_lane, None)
            };
            if let Some(g) = ghost {
                let names: Vec<String> = blocks.iter().map(|b| b.0.clone()).collect();
                self.ghost_lane(&painter, g, rect.left() + pad, zoom, &names);
            }
            let xof = |rows: usize| rect.left() + pad + rows as f32 * zoom;
            // ruler: a tick every 16 rows, labels every 64
            let step = if zoom < 1.0 { 64 } else { 16 };
            let mut r = 0;
            while r <= acc {
                let x = xof(r);
                let major = r % 64 == 0;
                painter.vline(
                    x,
                    rect.top() + if major { 2.0 } else { 9.0 }..=rect.top() + ruler_h,
                    Stroke::new(1.0, th().ink2.gamma_multiply(0.6)),
                );
                if major && zoom * 64.0 > 28.0 {
                    painter.text(
                        Pos2::new(x + 3.0, rect.top() + 1.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", r / 16 + 1),
                        FontId::monospace(10.0),
                        th().ink2,
                    );
                }
                r += step;
            }
            let pointer = ui.input(|i| i.pointer.interact_pos());
            let mods = ui.input(|i| i.modifiers);
            let selected = self.arr_sel_list();
            let mut menu_act: Option<input::ArrAction> = None;
            let mut bake = false;
            let released = ui.input(|i| i.pointer.any_released());
            for (i, &b) in order.iter().enumerate() {
                let cr = Rect::from_min_max(
                    Pos2::new(xof(starts[i]) + 1.0, lane.top()),
                    Pos2::new(xof(starts[i + 1]) - 1.0, lane.bottom()),
                );
                let resp =
                    ui.interact(cr, ui.scope_id().with(("clip", i)), Sense::click_and_drag());
                let col = hex(CLIP_COLOURS[b % CLIP_COLOURS.len()]);
                let col = if resp.hovered() {
                    col.gamma_multiply(1.15)
                } else {
                    col
                };
                let is_sel = selected.contains(&i);
                let col = if is_sel { col.gamma_multiply(1.3) } else { col };
                painter.rect_filled(cr, 6, col);
                if let Some((fi, t0)) = &self.arr_flash {
                    let age = ui.input(|inp| inp.time) - t0;
                    if fi.contains(&i) && age < 0.8 {
                        painter.rect_filled(
                            cr,
                            6,
                            Color32::from_white_alpha((90.0 * (1.0 - age / 0.8)) as u8),
                        );
                        ui.ctx().request_repaint();
                    }
                }
                if is_sel {
                    // inner shadow, then a thick light outline
                    painter.rect_stroke(
                        cr.shrink(3.0),
                        4,
                        Stroke::new(2.0, Color32::from_black_alpha(90)),
                        egui::StrokeKind::Inside,
                    );
                    painter.rect_stroke(
                        cr,
                        6,
                        Stroke::new(3.0, th().selection_outline),
                        egui::StrokeKind::Inside,
                    );
                }
                // mini pattern preview: note ticks per channel
                if let Some((name, rows, ticks, role, der)) = blocks.get(b) {
                    let body = Rect::from_min_max(
                        Pos2::new(cr.left() + 4.0, cr.top() + 20.0),
                        Pos2::new(cr.right() - 4.0, cr.bottom() - 4.0),
                    );
                    let lh = (body.height() / channels as f32).max(1.0);
                    let tick = Color32::from_white_alpha(110);
                    for &(r, c) in ticks {
                        let x = body.left() + r as f32 / *rows as f32 * body.width();
                        let y = body.top() + c as f32 * lh + lh * 0.5;
                        painter.hline(
                            x..=x + (zoom * 0.8).max(1.0),
                            y,
                            Stroke::new((lh * 0.5).clamp(1.0, 3.0), tick),
                        );
                    }
                    let tp = ui.painter_at(cr.shrink(2.0));
                    tp.text(
                        cr.left_top() + egui::vec2(6.0, 3.0),
                        egui::Align2::LEFT_TOP,
                        format!("{b:02} {name}"),
                        FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                    // role badge (muted palette), top-right
                    if let Some(r) = role {
                        let txt = if cr.width() > 70.0 {
                            r.name()
                        } else {
                            r.short()
                        };
                        let w = txt.len() as f32 * 6.5 + 8.0;
                        let br = Rect::from_min_size(
                            Pos2::new(cr.right() - w - 4.0, cr.top() + 3.0),
                            egui::vec2(w, 14.0),
                        );
                        if br.left() > cr.left() + 4.0 {
                            tp.rect_filled(br, 7, fillview::role_col(*r));
                            tp.text(
                                br.center(),
                                egui::Align2::CENTER_CENTER,
                                txt,
                                FontId::proportional(10.0),
                                Color32::from_rgb(0x16, 0x18, 0x1b),
                            );
                        }
                    }
                    if *der {
                        tp.text(
                            cr.left_bottom() + egui::vec2(6.0, -3.0),
                            egui::Align2::LEFT_BOTTOM,
                            "derived'",
                            FontId::proportional(10.0),
                            Color32::from_white_alpha(200),
                        );
                    }
                }
                if resp.clicked() {
                    self.arr_focus = true;
                    if mods.shift {
                        let a = self.arr_sel.min(order.len() - 1);
                        self.arr_multi = (a.min(i)..=a.max(i)).collect();
                    } else if mods.command {
                        if !self.arr_multi.remove(&i) {
                            self.arr_multi.insert(i);
                            self.arr_sel = i;
                        }
                    } else {
                        self.arr_select([i], i);
                        self.row = 0;
                    }
                    self.pattern = b;
                    self.sel = None;
                }
                if resp.secondary_clicked() {
                    self.arr_focus = true;
                    if !selected.contains(&i) {
                        self.arr_select([i], i);
                    }
                }
                let resp = resp.on_hover_text(format!(
                    "#{} · block {:02} · {} rows",
                    i + 1,
                    b,
                    blocks.get(b).map(|x| x.1).unwrap_or(0)
                ));
                resp.context_menu(|ui| {
                    use input::ArrAction as A;
                    for (t, a) in [
                        ("Cut            Ctrl+X", A::Cut),
                        ("Copy           Ctrl+C", A::Copy),
                        ("Paste          Ctrl+V", A::Paste),
                        ("Paste as new   Ctrl+Shift+V", A::PasteUnique),
                        ("Duplicate      Ctrl+D", A::Duplicate),
                        ("Delete         Del", A::Delete),
                        ("Insert block   Ins", A::Insert),
                    ] {
                        if ui.button(egui::RichText::new(t).monospace()).clicked() {
                            menu_act = Some(a);
                            ui.close();
                        }
                    }
                    ui.separator();
                    ui.menu_button("Role", |ui| {
                        for (k, r) in crate::arrange::ROLES.iter().enumerate() {
                            let cur = blocks.get(b).and_then(|x| x.3) == Some(*r);
                            if ui
                                .selectable_label(
                                    cur,
                                    egui::RichText::new(format!("{}  {}", k + 1, r.name()))
                                        .monospace()
                                        .color(fillview::role_col(*r)),
                                )
                                .clicked()
                            {
                                menu_act = Some(A::Role(Some(*r)));
                                ui.close();
                            }
                        }
                        if ui
                            .button(egui::RichText::new("0  none").monospace())
                            .clicked()
                        {
                            menu_act = Some(A::Role(None));
                            ui.close();
                        }
                    });
                    if blocks.get(b).is_some_and(|x| x.4)
                        && ui
                            .button("Bake derived block")
                            .on_hover_text(
                                "make it an ordinary block that no longer follows its source",
                            )
                            .clicked()
                    {
                        bake = true;
                        ui.close();
                    }
                });
                if resp.double_clicked() {
                    self.view = View::Pattern;
                }
                if resp.drag_started() {
                    self.arr_focus = true;
                    self.arr_drag = Some(i);
                    self.arr_select([i], i);
                }
            }
            if let Some(a) = menu_act {
                self.arranger_action(a, &ui.ctx().clone());
            }
            if bake {
                self.bake_selected();
            }
            // drag to reorder: insertion marker + drop
            if let (Some(from), Some(p)) = (self.arr_drag, pointer) {
                let to = (0..order.len())
                    .find(|&i| p.x < (xof(starts[i]) + xof(starts[i + 1])) * 0.5)
                    .unwrap_or(order.len());
                let x = xof(starts[to]);
                painter.vline(x, lane.y_range(), Stroke::new(3.0, th().accent));
                if released {
                    self.arr_drag = None;
                    self.arr_move(from, to);
                }
            } else if released {
                self.arr_drag = None;
            }
            // playhead
            if playing {
                let idx = if loop_pat {
                    if order.get(self.arr_sel) == Some(&ppat) {
                        Some(self.arr_sel)
                    } else {
                        order.iter().position(|&b| b == ppat)
                    }
                } else {
                    Some(opos.min(order.len() - 1))
                };
                if let Some(i) = idx {
                    let x = xof(starts[i] + prow);
                    painter.vline(x, rect.y_range(), Stroke::new(2.0, th().play));
                    if follow {
                        ui.scroll_to_rect(
                            Rect::from_center_size(
                                Pos2::new(x, rect.center().y),
                                egui::vec2(zoom * 64.0, 10.0),
                            ),
                            None,
                        );
                    }
                }
            }
            // ------------------------------------------------ automation lanes
            if show_auto && n_lanes > 0 {
                let (lrect, _) = ui.allocate_exact_size(egui::vec2(w, lanes_h), Sense::hover());
                let shift = ui.input(|i| i.modifiers.shift);
                let clip_x = ui.clip_rect().x_range();
                let to_x = |pos: f32| rect.left() + pad + pos * zoom;
                let to_pos = |x: f32| ((x - rect.left() - pad) / zoom).max(0.0);
                for (k, lane) in lanes.iter().enumerate() {
                    let lr = Rect::from_min_size(
                        Pos2::new(lrect.left(), lrect.top() + 4.0 + k as f32 * (LANE_H + 4.0)),
                        egui::vec2(w, LANE_H),
                    );
                    let body = lr.shrink2(egui::vec2(0.0, 6.0));
                    let to_y = |n: f32| body.bottom() - n.clamp(0.0, 1.0) * body.height();
                    let to_n = |y: f32| ((body.bottom() - y) / body.height()).clamp(0.0, 1.0);
                    let p = ui.painter_at(lr);
                    p.rect_filled(lr, 6, th().crt_bot);
                    for f in [0.25, 0.5, 0.75] {
                        p.hline(lr.x_range(), to_y(f), Stroke::new(1.0, th().crt_grid));
                    }
                    let mut r = 0;
                    while r <= acc {
                        p.vline(
                            to_x(r as f32),
                            lr.y_range(),
                            Stroke::new(1.0, th().crt_grid),
                        );
                        r += 16;
                    }
                    let target = crate::automation::Target::parse(&lane.target);
                    let col = hex(LANE_COLOURS[k % LANE_COLOURS.len()]);
                    // curve, sampled every 2 px across the visible range
                    let (x0, x1) = (clip_x.min.max(lr.left()), clip_x.max.min(lr.right()));
                    let mut pts = vec![];
                    let mut x = x0;
                    while x <= x1 {
                        if let Some(n) = crate::automation::value_at(&lane.points, to_pos(x)) {
                            pts.push(Pos2::new(x, to_y(n)));
                        }
                        x += 2.0;
                    }
                    if pts.len() > 1 {
                        // soft glow, then the line itself (muted, not neon)
                        for (wd, a) in [(7.0, 0.07), (4.0, 0.16)] {
                            p.add(egui::Shape::line(
                                pts.clone(),
                                Stroke::new(wd, col.gamma_multiply(a)),
                            ));
                        }
                        p.add(egui::Shape::line(
                            pts,
                            Stroke::new(1.8, col.gamma_multiply(0.9)),
                        ));
                    }
                    for (i, pt) in lane.points.iter().enumerate() {
                        let c = Pos2::new(to_x(pt.pos), to_y(pt.value));
                        let sel = self.auto_sel == Some((k, i));
                        p.circle_filled(c, if sel { 5.0 } else { 3.5 }, col);
                        if sel {
                            p.circle_stroke(c, 7.0, Stroke::new(1.5, th().selection_outline));
                        }
                    }
                    // playhead
                    if playing {
                        p.vline(to_x(song_pos), lr.y_range(), Stroke::new(1.5, th().play));
                    }
                    let resp =
                        ui.interact(lr, ui.scope_id().with(("lane", k)), Sense::click_and_drag());
                    let hit = |pp: Pos2| {
                        lane.points.iter().position(|pt| {
                            Pos2::new(to_x(pt.pos), to_y(pt.value)).distance(pp) < 8.0
                        })
                    };
                    // label: name and value (at the playhead, or under the mouse)
                    let readout = if let Some(hp) = resp.hover_pos() {
                        let pos = to_pos(hp.x);
                        format!(
                            "line {:.0} · {}",
                            pos,
                            target
                                .map(|t| t.format_in(to_n(hp.y), &fxk))
                                .unwrap_or_default()
                        )
                    } else {
                        let pos = if playing { song_pos } else { 0.0 };
                        crate::automation::value_at(&lane.points, pos)
                            .and_then(|n| target.map(|t| t.format_in(n, &fxk)))
                            .unwrap_or_default()
                    };
                    let lp = Pos2::new(x0 + 8.0, lr.top() + 4.0);
                    p.text(
                        lp,
                        egui::Align2::LEFT_TOP,
                        format!(
                            "{}{}   {}",
                            if self.auto_lane == k { "> " } else { "" },
                            lane.label,
                            readout
                        ),
                        FontId::proportional(11.5),
                        col.gamma_multiply(1.1),
                    );
                    if resp.clicked() || resp.drag_started() || resp.secondary_clicked() {
                        self.auto_lane = k;
                        self.arr_focus = true;
                    }
                    let snap = |pos: f32| if shift { pos } else { pos.round() };
                    if resp.clicked() {
                        if let Some(pp) = resp.interact_pointer_pos() {
                            if let Some(i) = hit(pp) {
                                self.auto_sel = Some((k, i));
                            } else {
                                lane_ops.push(LaneOp::Add(k, snap(to_pos(pp.x)), to_n(pp.y)));
                            }
                        }
                    }
                    if resp.secondary_clicked() {
                        if let Some(i) = resp.interact_pointer_pos().and_then(hit) {
                            lane_ops.push(LaneOp::Delete(k, i));
                        }
                    }
                    if resp.drag_started() {
                        let o = ui.input(|i| i.pointer.press_origin());
                        if let Some(i) = o.and_then(hit) {
                            self.auto_drag = Some((k, i));
                            self.auto_sel = Some((k, i));
                            lane_ops.push(LaneOp::Snapshot);
                        } else if self.auto_pencil {
                            self.auto_pencil_lane = Some(k);
                            lane_ops.push(LaneOp::Snapshot);
                        }
                    }
                    if resp.dragged() {
                        if let Some(pp) = resp.interact_pointer_pos() {
                            if let Some((dk, i)) = self.auto_drag {
                                if dk == k {
                                    lane_ops.push(LaneOp::Move(
                                        k,
                                        i,
                                        snap(to_pos(pp.x)),
                                        to_n(pp.y),
                                    ));
                                }
                            } else if self.auto_pencil_lane == Some(k) {
                                lane_ops.push(LaneOp::Pencil(k, to_pos(pp.x).round(), to_n(pp.y)));
                            }
                        }
                    }
                    if resp.drag_stopped() {
                        self.auto_drag = None;
                        self.auto_pencil_lane = None;
                    }
                }
            }
        });
        self.arr_scroll = out.state.clamped_offset().x;
        self.arr_area = out.inner_rect;
        self.apply_lane_ops(lane_ops);
    }

    fn apply_lane_ops(&mut self, ops: Vec<LaneOp>) {
        use crate::automation::Point;
        let shape = self.auto_shape;
        for op in ops {
            match op {
                LaneOp::Snapshot => self.snapshot(),
                LaneOp::Add(k, pos, value) => {
                    self.snapshot();
                    let idx = self.with(|s| {
                        let lane = &mut s.song.automation[k];
                        lane.points.push(Point { pos, value, shape });
                        lane.sort();
                        lane.points
                            .iter()
                            .position(|p| p.pos == pos && p.value == value)
                    });
                    self.auto_sel = idx.map(|i| (k, i));
                    self.status =
                        "Added automation point (drag to move, right-click to delete)".into();
                }
                LaneOp::Delete(k, i) => {
                    self.snapshot();
                    self.with(|s| {
                        if i < s.song.automation[k].points.len() {
                            s.song.automation[k].points.remove(i);
                        }
                    });
                    self.auto_sel = None;
                    self.status = "Deleted automation point".into();
                }
                LaneOp::Move(k, i, pos, value) => {
                    self.with(|s| {
                        let pts = &mut s.song.automation[k].points;
                        if i < pts.len() {
                            let lo = if i > 0 { pts[i - 1].pos } else { 0.0 };
                            let hi = pts.get(i + 1).map(|p| p.pos).unwrap_or(f32::MAX);
                            pts[i].pos = pos.clamp(lo, hi);
                            pts[i].value = value;
                        }
                    });
                }
                LaneOp::Pencil(k, pos, value) => {
                    self.with(|s| {
                        let lane = &mut s.song.automation[k];
                        if let Some(p) = lane.points.iter_mut().find(|p| (p.pos - pos).abs() < 0.5)
                        {
                            p.value = value;
                        } else {
                            lane.points.push(Point { pos, value, shape });
                            lane.sort();
                        }
                    });
                }
            }
            self.dirty = true;
        }
    }

    fn add_lane(&mut self, t: crate::automation::Target) {
        use crate::automation::{Lane, Target};
        let label = self.with(|s| {
            let iname = |i: usize| {
                s.song
                    .instruments
                    .get(i)
                    .map(|x| x.name.clone())
                    .unwrap_or_default()
            };
            let cname = |c: usize| s.song.channel_names.get(c).cloned().unwrap_or_default();
            match t {
                Target::Cutoff(i) | Target::Resonance(i) | Target::Drive(i) | Target::Level(i) => {
                    format!("{} - {}", iname(i), t.param_name())
                }
                Target::ChanVolume(c) | Target::ChanPan(c) => {
                    format!("Ch {} {} - {}", c + 1, cname(c), t.param_name())
                }
                Target::Fx(i, d, p) => {
                    let pn = s
                        .song
                        .instruments
                        .get(i)
                        .and_then(|x| x.fx.get(d))
                        .map(|dev| {
                            format!(
                                "{} {}",
                                dev.kind.name(),
                                crate::fx::params(dev.kind)
                                    .get(p)
                                    .map(|x| x.name)
                                    .unwrap_or("")
                            )
                        });
                    format!("{} - {}", iname(i), pn.unwrap_or_default())
                }
                Target::Ret(r, p) => {
                    let k = if r == 0 {
                        crate::fx::FxKind::Delay
                    } else {
                        crate::fx::FxKind::Reverb
                    };
                    format!(
                        "Return {} - {}",
                        if r == 0 { "A" } else { "B" },
                        crate::fx::params(k).get(p).map(|x| x.name).unwrap_or("")
                    )
                }
                Target::Send(c, r) => {
                    format!(
                        "Ch {} {} - Send {}",
                        c + 1,
                        cname(c),
                        if r == 0 { "A" } else { "B" }
                    )
                }
                _ => t.param_name().to_string(),
            }
        });
        self.snapshot();
        let n = self.with(|s| {
            // start flat at the current value
            let cur = match t {
                Target::Cutoff(i) | Target::Resonance(i) | Target::Drive(i) => {
                    match s.song.instruments.get(i).map(|x| &x.kind) {
                        Some(InstrumentKind::Synth(p)) => t.to_norm(match t {
                            Target::Cutoff(_) => p.cutoff,
                            Target::Resonance(_) => p.resonance,
                            _ => p.drive,
                        }),
                        _ => 0.5,
                    }
                }
                Target::Level(_) => t.to_norm(1.0),
                Target::ChanVolume(c) => t.to_norm(s.song.mixer.channels[c].volume),
                Target::ChanPan(c) => t.to_norm(s.song.mixer.channels[c].pan),
                Target::Master => t.to_norm(s.song.mixer.master),
                Target::Tempo => t.to_norm(s.song.bpm),
                Target::Fx(i, d, p) => s
                    .song
                    .instruments
                    .get(i)
                    .and_then(|x| x.fx.get(d))
                    .and_then(|dev| dev.params.get(p).copied())
                    .unwrap_or(0.5),
                Target::Ret(r, p) => s
                    .song
                    .mixer
                    .returns
                    .get(r)
                    .and_then(|x| x.device.params.get(p).copied())
                    .unwrap_or(0.5),
                Target::Send(c, r) => s
                    .song
                    .mixer
                    .channels
                    .get(c)
                    .map(|m| m.send[r])
                    .unwrap_or(0.0),
            };
            let mut lane = Lane::new(t, label.clone());
            lane.points.push(crate::automation::Point {
                pos: 0.0,
                value: cur,
                shape: crate::automation::Shape::Linear,
            });
            s.song.automation.push(lane);
            s.song.automation.len() - 1
        });
        self.auto_lane = n;
        self.show_auto = true;
        self.status = format!("Added lane '{label}' (click to add points, Pencil to draw)");
    }

    fn automation_bar(&mut self, ui: &mut egui::Ui) {
        use crate::automation::{Shape, Target};
        let fxdevs = self.with(|s| {
            s.song
                .instruments
                .iter()
                .map(|i| i.fx.iter().map(|d| d.kind).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        });
        let (ninst, synths, nch, nl) = self.with(|s| {
            (
                s.song.instruments.len(),
                s.song
                    .instruments
                    .iter()
                    .map(|i| (i.name.clone(), matches!(i.kind, InstrumentKind::Synth(_))))
                    .collect::<Vec<_>>(),
                s.song.channels,
                s.song.automation.len(),
            )
        });
        let _ = ninst;
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(
                &mut self.show_auto,
                egui::RichText::new("Automation").strong(),
            );
            if !self.show_auto {
                return;
            }
            let mut add = None;
            ui.menu_button("+ Lane", |ui| {
                ui.set_max_height(320.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, (n, synth)) in synths.iter().enumerate() {
                        let mut ts = vec![Target::Level(i)];
                        if *synth {
                            ts = vec![
                                Target::Cutoff(i),
                                Target::Resonance(i),
                                Target::Drive(i),
                                Target::Level(i),
                            ];
                        }
                        for t in ts {
                            if ui
                                .button(format!("{:02X} {n} - {}", i + 1, t.param_name()))
                                .clicked()
                            {
                                add = Some(t);
                                ui.close();
                            }
                        }
                        for (d, k) in fxdevs.get(i).into_iter().flatten().enumerate() {
                            ui.menu_button(
                                format!("{:02X} {n} - FX {} {}", i + 1, d + 1, k.name()),
                                |ui| {
                                    for (p, info) in crate::fx::params(*k).iter().enumerate() {
                                        if ui.button(info.name).clicked() {
                                            add = Some(Target::Fx(i, d, p));
                                            ui.close();
                                        }
                                    }
                                },
                            );
                        }
                    }
                    ui.separator();
                    for c in 0..nch {
                        for t in [Target::ChanVolume(c), Target::ChanPan(c)] {
                            if ui
                                .button(format!("Mixer ch {} - {}", c + 1, t.param_name()))
                                .clicked()
                            {
                                add = Some(t);
                                ui.close();
                            }
                        }
                    }
                    for c in 0..nch {
                        for r in 0..2 {
                            let t = Target::Send(c, r);
                            if ui
                                .button(format!(
                                    "Mixer ch {} - Send {}",
                                    c + 1,
                                    if r == 0 { "A" } else { "B" }
                                ))
                                .clicked()
                            {
                                add = Some(t);
                                ui.close();
                            }
                        }
                    }
                    for (r, k) in [crate::fx::FxKind::Delay, crate::fx::FxKind::Reverb]
                        .into_iter()
                        .enumerate()
                    {
                        ui.menu_button(
                            format!("Return {} ({})", if r == 0 { "A" } else { "B" }, k.name()),
                            |ui| {
                                for (p, info) in crate::fx::params(k).iter().enumerate() {
                                    if ui.button(info.name).clicked() {
                                        add = Some(Target::Ret(r, p));
                                        ui.close();
                                    }
                                }
                            },
                        );
                    }
                    for t in [Target::Master, Target::Tempo] {
                        if ui.button(t.param_name()).clicked() {
                            add = Some(t);
                            ui.close();
                        }
                    }
                });
            });
            if let Some(t) = add {
                self.add_lane(t);
            }
            ui.separator();
            ui.selectable_value(&mut self.auto_pencil, false, "Select")
                .on_hover_text("click = add point, drag point = move, right-click = delete");
            ui.selectable_value(&mut self.auto_pencil, true, "Pencil")
                .on_hover_text("drag on a lane to draw (one point per line)");
            ui.label("Shape");
            let mut shape = self
                .auto_sel
                .and_then(|(k, i)| {
                    self.with(|s| {
                        s.song
                            .automation
                            .get(k)
                            .and_then(|l| l.points.get(i))
                            .map(|p| p.shape)
                    })
                })
                .unwrap_or(self.auto_shape);
            let before = shape;
            egui::ComboBox::from_id_salt("auto_shape")
                .selected_text(shape.name())
                .width(70.0)
                .show_ui(ui, |ui| {
                    for sh in Shape::ALL {
                        ui.selectable_value(&mut shape, sh, sh.name());
                    }
                })
                .response
                .on_hover_text("segment shape after the selected point / for new points");
            if shape != before {
                self.auto_shape = shape;
                if let Some((k, i)) = self.auto_sel {
                    self.snapshot();
                    self.with(|s| {
                        if let Some(p) = s
                            .song
                            .automation
                            .get_mut(k)
                            .and_then(|l| l.points.get_mut(i))
                        {
                            p.shape = shape;
                        }
                    });
                }
            }
            if nl > 0 {
                let k = self.auto_lane.min(nl - 1);
                let (mut en, label) = self.with(|s| {
                    (
                        s.song.automation[k].enabled,
                        s.song.automation[k].label.clone(),
                    )
                });
                ui.separator();
                ui.label(
                    egui::RichText::new(format!("Lane: {label}"))
                        .small()
                        .color(th().ink2),
                );
                if ui.checkbox(&mut en, "on").changed() {
                    self.with(|s| s.song.automation[k].enabled = en);
                    self.dirty = true;
                }
                if ui.small_button("Delete lane").clicked() {
                    self.snapshot();
                    self.with(|s| {
                        s.song.automation.remove(k);
                    });
                    self.auto_sel = None;
                    self.status = format!("Deleted lane '{label}'");
                }
            }
        });
    }

    // ---------------------------------------------------------------- mixer

    fn meter(ui: &mut egui::Ui, level: f32, h: f32) {
        let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, h), Sense::hover());
        let p = ui.painter();
        p.rect_filled(r, 3, th().crt_bot);
        let db = if level > 1e-5 {
            20.0 * level.log10()
        } else {
            -90.0
        };
        let frac = ((db + 60.0) / 66.0).clamp(0.0, 1.0);
        if frac > 0.0 {
            let top = r.bottom() - frac * r.height();
            let col = if db > 0.0 {
                hex(0xD9605A)
            } else if db > -6.0 {
                hex(0xE8C35A)
            } else {
                hex(0x5CC07A)
            };
            p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), top), r.max), 3, col);
        }
        // 0 dB line
        let y0 = r.bottom() - 60.0 / 66.0 * r.height();
        p.hline(r.x_range(), y0, Stroke::new(1.0, th().ink2));
    }

    #[allow(clippy::needless_range_loop)]
    fn mixer_view(&mut self, ui: &mut egui::Ui) {
        let (mut mix, names, nch, cpk, plr) = self.with(|s| {
            (
                s.song.mixer.clone(),
                s.song.channel_names.clone(),
                s.song.channels,
                s.engine.chan_peak,
                s.engine.peak_lr,
            )
        });
        let before = mix.clone();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Mixer").strong());
            ui.label(
                egui::RichText::new(
                    "fader 0 dB = double-click · pan C = double-click · M mute · S solo (F9 / F10)",
                )
                .small()
                .color(th().ink2),
            );
        });
        ui.add_space(6.0);
        let fader_h = (ui.available_height() - 150.0).clamp(80.0, 320.0);
        egui::ScrollArea::horizontal()
            .id_salt("mixer_scroll")
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let strip =
                        |ui: &mut egui::Ui,
                         title: String,
                         spine: Color32,
                         vol: &mut f32,
                         levels: &[f32],
                         extra: &mut dyn FnMut(&mut egui::Ui)| {
                            egui::Frame::new()
                                .fill(th().cart_top)
                                .stroke(Stroke::new(1.0, spine))
                                .corner_radius(8)
                                .inner_margin(8)
                                .show(ui, |ui| {
                                    ui.set_width(92.0);
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            egui::RichText::new(title).strong().color(th().ink),
                                        );
                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().slider_width = fader_h;
                                            ui.add_space(8.0);
                                            let r = ui.add(
                                                egui::Slider::new(vol, 0.0..=2.0)
                                                    .vertical()
                                                    .trailing_fill(true)
                                                    .show_value(false),
                                            );
                                            if r.double_clicked() {
                                                *vol = 1.0;
                                            }
                                            for l in levels {
                                                Self::meter(ui, *l, fader_h);
                                            }
                                        });
                                        ui.label(
                                            egui::RichText::new(db_text(*vol))
                                                .monospace()
                                                .small()
                                                .color(th().value),
                                        );
                                        extra(ui);
                                    });
                                });
                        };
                    for ch in 0..nch.min(MAX_CHANNELS) {
                        let c = &mut mix.channels[ch];
                        let mut vol = c.volume;
                        let (mut pan, mut mute, mut solo) = (c.pan, c.mute, c.solo);
                        let mut send = c.send;
                        strip(
                            ui,
                            format!("{} {}", ch + 1, names.get(ch).cloned().unwrap_or_default()),
                            th().slate,
                            &mut vol,
                            &[cpk[ch]],
                            &mut |ui| {
                                ui.spacing_mut().slider_width = 76.0;
                                let r = ui
                                    .add(egui::Slider::new(&mut pan, -1.0..=1.0).show_value(false));
                                if r.double_clicked() {
                                    pan = 0.0;
                                }
                                let pt = if pan.abs() < 0.01 {
                                    "C".to_string()
                                } else if pan < 0.0 {
                                    format!("L{:.0}", -pan * 100.0)
                                } else {
                                    format!("R{:.0}", pan * 100.0)
                                };
                                ui.label(egui::RichText::new(pt).monospace().small());
                                ui.horizontal(|ui| {
                                    ui.add_space(14.0);
                                    ui.toggle_value(&mut mute, egui::RichText::new(" M ").strong())
                                        .on_hover_text("mute (F9)");
                                    ui.toggle_value(&mut solo, egui::RichText::new(" S ").strong())
                                        .on_hover_text("solo (F10)");
                                });
                                ui.spacing_mut().slider_width = 56.0;
                                for (b, nm) in ["A", "B"].iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(*nm)
                                                .small()
                                                .monospace()
                                                .color(th().ink2),
                                        );
                                        let r = ui
                                            .add(
                                                egui::Slider::new(&mut send[b], 0.0..=1.0)
                                                    .show_value(false),
                                            )
                                            .on_hover_text(format!(
                                                "send {} ({}): {:.0} %",
                                                nm,
                                                if b == 0 { "delay" } else { "reverb" },
                                                send[b] * 100.0
                                            ));
                                        if r.double_clicked() {
                                            send[b] = 0.0;
                                        }
                                    });
                                }
                            },
                        );
                        let c = &mut mix.channels[ch];
                        c.send = send;
                        c.volume = vol;
                        c.pan = pan;
                        c.mute = mute;
                        c.solo = solo;
                    }
                    ui.add_space(12.0);
                    // return buses (delay / reverb), 100 % wet
                    for b in 0..mix.returns.len() {
                        let mut lvl = mix.returns[b].level;
                        let mut dev = mix.returns[b].device.clone();
                        strip(
                            ui,
                            format!("Ret {}", mix.returns[b].name),
                            fxview::strip_fill(dev.kind, dev.enabled),
                            &mut lvl,
                            &[],
                            &mut |ui| {
                                ui.spacing_mut().slider_width = 76.0;
                                let keys: &[&str] = if b == 0 {
                                    &["sync", "feedback", "lp"]
                                } else {
                                    &["size", "damping", "predelay"]
                                };
                                for key in keys {
                                    let Some(k) = crate::fx::Device::param_index(dev.kind, key)
                                    else {
                                        continue;
                                    };
                                    let info = &crate::fx::params(dev.kind)[k];
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} {}",
                                            info.name,
                                            info.format(dev.params[k])
                                        ))
                                        .small()
                                        .color(th().ink2),
                                    );
                                    ui.add(
                                        egui::Slider::new(&mut dev.params[k], 0.0..=1.0)
                                            .show_value(false),
                                    );
                                }
                                ui.toggle_value(&mut dev.enabled, "On");
                            },
                        );
                        mix.returns[b].level = lvl;
                        mix.returns[b].device = dev;
                    }
                    ui.add_space(12.0);
                    let mut master = mix.master;
                    strip(
                        ui,
                        "Master".into(),
                        th().accent,
                        &mut master,
                        &plr,
                        &mut |ui| {
                            ui.label(egui::RichText::new("soft limiter").small().color(th().ink2));
                        },
                    );
                    mix.master = master;
                });
            });
        if mix != before {
            self.with(|s| s.song.mixer = mix);
            self.dirty = true;
        }
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
