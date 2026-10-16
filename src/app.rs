//! The egui application: toolbar, instrument panel, pattern editor.
use crate::audio::{self, Shared, SharedRef};
use crate::engine::{self, Engine};
use crate::fileio;
use crate::model::*;
use crate::synth::{FilterMode, Wave};
use crate::theme::{self, hex, th};
use eframe::egui::{self, Color32, FontId, Key, Pos2, Rect, Sense, Stroke};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// cursor sub-columns: 0 note, 1-2 instr, 3-4 vol, 5-8 effect
const SUBCOLS: usize = 9;

pub struct App {
    shared: SharedRef,
    #[allow(dead_code)] // keeps the stream alive
    audio: Option<audio::Audio>,
    #[allow(dead_code)]
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
    undo: Vec<Vec<Pattern>>,
    redo: Vec<Vec<Pattern>>,
    path: Option<PathBuf>,
    status: String,
    dirty: bool,
    mutes: Vec<bool>,
    zoom: f32,
    themed: bool,
    theme: theme::Theme,
    show_help: bool,
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
    }));
    let (audio, audio_err) = match audio::start(shared.clone()) {
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
        mutes: vec![false; MAX_CHANNELS],
        zoom: 1.0,
        themed: false,
        theme: theme::DARK,
        show_help: false,
        pending: vec![],
    };
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ChipWave Tracker")
            .with_inner_size([1360.0, 800.0])
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

impl App {
    fn with<R>(&self, f: impl FnOnce(&mut Shared) -> R) -> R {
        let mut s = self.shared.lock().unwrap();
        f(&mut s)
    }

    fn snapshot(&mut self) {
        let pats = self.with(|s| s.song.patterns.clone());
        self.undo.push(pats);
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
                let mut s = self.shared.lock().unwrap();
                std::mem::replace(&mut s.song.patterns, p)
            };
            to.push(cur);
            self.status = if redo { "Redo".into() } else { "Undo".into() };
        }
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
        let (pattern, row, mutes) = (self.pattern, self.row, self.mutes.clone());
        self.with(|s| {
            let Shared { song, engine } = s;
            if engine.playing && mode == 0 {
                engine.stop();
                return;
            }
            engine.set_mutes(&mutes);
            match mode {
                0 => engine.start(song, 0, 0, false, 0),        // song
                1 => engine.start(song, 0, 0, true, pattern),   // block loop
                _ => engine.start(song, 0, row, true, pattern), // from row
            }
        });
    }

    fn save(&mut self, as_new: bool) {
        if as_new || self.path.is_none() {
            let Some(p) = rfd::FileDialog::new()
                .add_filter("ChipWave song", &["cwt"])
                .set_file_name("song.cwt")
                .save_file()
            else {
                return;
            };
            self.path = Some(if p.extension().is_none() {
                p.with_extension("cwt")
            } else {
                p
            });
        }
        let path = self.path.clone().unwrap();
        let r = self.with(|s| fileio::save_cwt(&s.song, &path));
        self.status = match r {
            Ok(()) => {
                self.dirty = false;
                format!("Saved {}", path.display())
            }
            Err(e) => format!("Save failed: {e}"),
        };
    }

    fn open(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .add_filter("ChipWave song", &["cwt"])
            .pick_file()
        else {
            return;
        };
        match fileio::load_cwt(&p) {
            Ok(song) => {
                self.with(|s| {
                    s.engine.stop();
                    s.song = song;
                });
                self.path = Some(p.clone());
                self.reset_cursor();
                self.status = format!("Opened {}", p.display());
            }
            Err(e) => self.status = format!("Open failed: {e}"),
        }
    }

    fn reset_cursor(&mut self) {
        self.row = 0;
        self.ch = 0;
        self.col = 0;
        self.pattern = 0;
        self.undo.clear();
        self.redo.clear();
        self.dirty = false;
    }

    fn export_wav(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .add_filter("WAV audio", &["wav"])
            .set_file_name("chipwave-export.wav")
            .save_file()
        else {
            return;
        };
        let song = self.with(|s| s.song.clone());
        let data = engine::render_song(&song, 44100);
        self.status = match engine::write_wav(&p, &data, 44100) {
            Ok(()) => format!(
                "Exported {:.1} s to {}",
                data.len() as f32 / 88200.0,
                p.display()
            ),
            Err(e) => format!("Export failed: {e}"),
        };
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

    fn handle_keys(&mut self, _ctx: &egui::Context) {
        let events = std::mem::take(&mut self.pending);
        let channels = self.with(|s| s.song.channels);
        for ev in events {
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
            match key {
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
                Key::Space => self.play(0),
                Key::Enter => self.play(if cmd || m.shift { 2 } else { 1 }),
                Key::Escape => {
                    self.edit = !self.edit;
                    self.status = format!("Edit mode {}", if self.edit { "on" } else { "off" });
                }
                Key::Z if cmd && m.shift => self.undo(true),
                Key::Z if cmd => self.undo(false),
                Key::Y if cmd => self.undo(true),
                Key::S if cmd => self.save(m.shift),
                Key::O if cmd => self.open(),
                Key::E if cmd => self.export_wav(),
                Key::N if cmd => {
                    let ch = channels;
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
                Key::Plus | Key::Equals if cmd => self.transpose(if m.shift { 12 } else { 1 }),
                Key::Minus if cmd => self.transpose(if m.shift { -12 } else { -1 }),
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
                    self.mutes[self.ch] = !self.mutes[self.ch];
                    self.sync_mutes();
                }
                Key::F10 => {
                    let solo = (0..channels).all(|c| c == self.ch || self.mutes[c])
                        && !self.mutes[self.ch];
                    for c in 0..channels {
                        self.mutes[c] = if solo { false } else { c != self.ch };
                    }
                    self.sync_mutes();
                }
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
                            let Shared { song, engine } = s;
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

    fn sync_mutes(&mut self) {
        let m = self.mutes.clone();
        self.with(|s| s.engine.set_mutes(&m));
    }

    fn transpose(&mut self, semis: i32) {
        self.snapshot();
        let (p, ch) = (self.pattern, self.ch);
        self.with(|s| {
            if let Some(p) = s.song.patterns.get_mut(p) {
                for row in &mut p.cells {
                    let c = &mut row[ch];
                    if c.note > 0 && c.note != NOTE_OFF {
                        c.note = (c.note as i32 + semis).clamp(1, 120) as u8;
                    }
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

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let hdr_text = if th().light { Color32::WHITE } else { th().ink };
        let r = ui.max_rect();
        theme::gradient(ui.painter(), r, th().hdr_top, th().hdr_bot);
        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("ChipWave")
                    .size(20.0)
                    .strong()
                    .color(hdr_text),
            );
            let title = self.with(|s| s.song.title.clone());
            ui.label(
                egui::RichText::new(format!("· {}{}", title, if self.dirty { " *" } else { "" }))
                    .color(hdr_text),
            );
            ui.add_space(12.0);
            if ui.button("New").clicked() {
                ui.ctx().input_mut(|i| {
                    i.events.push(egui::Event::Key {
                        key: Key::N,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND,
                    })
                });
            }
            if ui.button("Open").clicked() {
                self.open();
            }
            if ui.button("Save").clicked() {
                self.save(false);
            }
            if ui.button("Export WAV").clicked() {
                self.export_wav();
            }
            ui.separator();
            let playing = self.with(|s| s.engine.playing);
            if ui
                .button(if playing { "Stop" } else { "Play song" })
                .clicked()
            {
                self.play(0);
            }
            if ui.button("Play block").clicked() {
                self.play(1);
            }
            ui.separator();
            let (mut bpm, mut speed) = self.with(|s| (s.song.bpm, s.song.speed));
            ui.label(egui::RichText::new("BPM").color(hdr_text));
            let c1 = ui
                .add(
                    egui::DragValue::new(&mut bpm)
                        .range(30.0..=300.0)
                        .speed(0.5),
                )
                .changed();
            ui.label(egui::RichText::new("Speed").color(hdr_text));
            let c2 = ui
                .add(egui::DragValue::new(&mut speed).range(1..=32))
                .changed();
            if c1 || c2 {
                self.with(|s| {
                    s.song.bpm = bpm;
                    s.song.speed = speed;
                });
                self.dirty = true;
            }
            ui.label(egui::RichText::new("Oct").color(hdr_text));
            ui.add(egui::DragValue::new(&mut self.octave).range(0..=8));
            ui.label(egui::RichText::new("Step").color(hdr_text));
            ui.add(egui::DragValue::new(&mut self.step).range(0..=16));
            ui.checkbox(
                &mut self.follow,
                egui::RichText::new("Follow").color(hdr_text),
            );
            ui.separator();
            ui.label(egui::RichText::new("Scale").color(hdr_text));
            egui::ComboBox::from_id_salt("zoom")
                .selected_text(format!("{:.0} %", self.zoom * 100.0))
                .width(70.0)
                .show_ui(ui, |ui| {
                    for z in [0.75f32, 1.0, 1.25, 1.5, 1.75, 2.0] {
                        ui.selectable_value(&mut self.zoom, z, format!("{:.0} %", z * 100.0));
                    }
                });
            ui.label(egui::RichText::new("Theme").color(hdr_text));
            let mut t = self.theme;
            egui::ComboBox::from_id_salt("theme")
                .selected_text(t.name)
                .show_ui(ui, |ui| {
                    for opt in theme::THEMES {
                        ui.selectable_value(&mut t, opt, opt.name);
                    }
                });
            if t != self.theme {
                self.theme = t;
                theme::apply(ui.ctx(), t);
            }
            if ui.button("? Keys").clicked() {
                self.show_help = !self.show_help;
            }
        });
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        theme::cartridge(ui.painter(), ui.max_rect(), th().slate);
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
        let shared = self.shared.clone();
        let mut s = shared.lock().unwrap();
        if let Some(inst) = s.song.instruments.get_mut(idx.wrapping_sub(1)) {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(egui::RichText::new(format!("Sound · {}", inst.name)).strong());
            });
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label("Volume");
                changed |= ui
                    .add(egui::Slider::new(&mut inst.volume, 0.0..=1.0))
                    .changed();
            });
            if let InstrumentKind::Synth(p) = &mut inst.kind {
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
        }
        drop(s);
        if changed {
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
        let (channels, names, npat) = self.with(|s| {
            (
                s.song.channels,
                s.song.channel_names.clone(),
                s.song.patterns.len(),
            )
        });
        if playing && self.follow {
            self.pattern = play_pat.min(npat - 1);
            self.row = play_row;
        }
        let Some(pat) = song_snapshot else { return };
        ui.horizontal(|ui| {
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
            if ui.button("+ New block").clicked() {
                let ch = channels;
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
            ui.label(
                egui::RichText::new(if self.edit { "EDIT" } else { "edit off (Esc)" })
                    .color(if self.edit { th().brick } else { th().ink2 })
                    .strong(),
            );
        });

        let avail = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(avail, Sense::click());
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
                if self.mutes[ch] {
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
                if self.mutes[ch] { "MUTED" } else { "" },
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
        if resp.clicked() {
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
                    ("Space", "play / stop song"), ("Enter / Ctrl+Enter", "play block / from row"), ("Esc", "edit mode on/off"),
                    ("Ctrl+Z / Ctrl+Y", "undo / redo"), ("Ctrl +/- (Shift = octave)", "transpose channel"), ("F9 / F10", "mute / solo channel"),
                    ("Ctrl+N / O / S / Shift+S", "new / open / save / save as (.cwt)"), ("Ctrl+E", "export WAV"),
                ] { ui.label(egui::RichText::new(k).monospace().color(th().value)); ui.label(v); ui.end_row(); }
            });
            ui.label("Effects: 01/02 slide, 09 speed, 0C volume, 0D vol slide, 0F tempo (0F00 = break)");
        });
        self.show_help = open;
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        // While no text field is being edited, the pattern editor owns the keyboard:
        // take key events away from egui (so Tab/arrows don't move widget focus).
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
                egui::Event::Key { .. } => self.pending.push(e),
                egui::Event::Text(_) => {}
                other => keep.push(other),
            }
        }
        raw.events = keep;
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.themed {
            theme::apply(&ctx, self.theme);
            ctx.options_mut(|o| o.zoom_with_keyboard = false);
            self.themed = true;
        }
        if (ctx.zoom_factor() - self.zoom).abs() > 0.001 {
            ctx.set_zoom_factor(self.zoom);
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
                if let Ok(song) = fileio::load_cwt(&p) {
                    self.with(|s| {
                        s.engine.stop();
                        s.song = song;
                    });
                    self.path = Some(p);
                    self.reset_cursor();
                }
            } else {
                self.import_sample(p);
            }
        }
        self.handle_keys(&ctx);

        let full = ui.max_rect();
        theme::gradient(ui.painter(), full, th().bg_top, th().bg_bot);

        egui::Panel::top("toolbar")
            .exact_size(44.0)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").exact_size(26.0).frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(12, 4))).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&self.status).color(Color32::WHITE));
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
            .show(ui, |ui| self.side_panel(ui));
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
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                    self.pattern_view(ui)
                });
            });
        if self.show_help {
            self.help(&ctx);
        }
        if self.with(|s| s.engine.playing) {
            ctx.request_repaint();
        }
    }
}
