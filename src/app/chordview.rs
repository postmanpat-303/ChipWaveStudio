//! Chord helper window (0.2.3.9): song key (pick or detect), chord pads,
//! a progression, and writing it into the current block as chords, an
//! arpeggio or a bass line. Scale lock snaps typed and MIDI notes to the key.
//! Theory in `crate::chords`.
use super::*;
use crate::chords::{self, ArpPattern, BassRhythm, ChordKind, Key, Scale, Step};
use std::time::{Duration, Instant};

/// keys used for chord previews (MIDI keys 0-127 are real keyboards)
const PREVIEW_KEY: u8 = 200;

pub(super) struct ChordHelper {
    pub open: bool,
    pub kind: ChordKind,
    pub octave: u8,
    pub inversion: usize,
    pub prog: Vec<Step>,
    /// clicking a pad adds it to the progression (else preview only)
    pub click_adds: bool,
    /// 0 chords, 1 arp, 2 bass
    pub style: u8,
    pub arp: ArpPattern,
    pub arp_rate: usize,
    pub arp_oct: u8,
    pub bass: BassRhythm,
    pub fill: bool,
    pub from_cursor: bool,
    pub detected: Vec<(Key, f32)>,
    /// preview notes to release: (when, key)
    pub(super) offs: Vec<(Instant, u8)>,
}

impl Default for ChordHelper {
    fn default() -> Self {
        Self {
            open: false,
            kind: ChordKind::Triad,
            octave: 3,
            inversion: 0,
            prog: vec![],
            click_adds: true,
            style: 0,
            arp: ArpPattern::Up,
            arp_rate: 1,
            arp_oct: 1,
            bass: BassRhythm::Rolling,
            fill: true,
            from_cursor: false,
            detected: vec![],
            offs: vec![],
        }
    }
}

impl App {
    /// the key notes snap to, when scale lock is on and the song has a key
    pub(super) fn scale_lock_key(&self) -> Option<Key> {
        if !self.extra.scale_lock {
            return None;
        }
        self.with(|s| s.song.info.key)
    }

    fn song_key(&self) -> Key {
        self.with(|s| s.song.info.key)
            .unwrap_or(Key::new(9, Scale::Minor))
    }

    fn set_song_key(&mut self, k: Key) {
        if self.with(|s| s.song.info.key) != Some(k) {
            self.with(|s| s.song.info.key = Some(k));
            self.dirty = true;
        }
    }

    /// play `notes` with the current instrument for ~0.7 s
    pub(super) fn preview_chord(&mut self, notes: &[u8]) {
        let (ins, ch) = (self.instr, self.ch as u8);
        self.preview_notes(notes, ins, ch);
    }

    /// play `notes` with instrument `ins` through channel `ch` for ~0.7 s
    /// (the chord helper, the tracker's channel-header Play button)
    pub(super) fn preview_notes(&mut self, notes: &[u8], ins: u8, ch: u8) {
        let now = Instant::now();
        // release a previous preview first
        let old: Vec<u8> = self.chords.offs.drain(..).map(|(_, k)| k).collect();
        self.with(|s| {
            let Shared { song, engine, .. } = s;
            for k in old {
                engine.live_event(song, crate::midi::LiveEv::Off { key: k });
            }
            for (i, &n) in notes.iter().enumerate() {
                engine.live_event(
                    song,
                    crate::midi::LiveEv::On {
                        key: PREVIEW_KEY + i as u8,
                        note: n,
                        vel: 0.8,
                        instr: ins,
                        ch,
                    },
                );
            }
        });
        for i in 0..notes.len() {
            self.chords
                .offs
                .push((now + Duration::from_millis(700), PREVIEW_KEY + i as u8));
        }
    }

    fn release_previews(&mut self, all: bool) {
        let now = Instant::now();
        let due: Vec<u8> = self
            .chords
            .offs
            .iter()
            .filter(|(t, _)| all || *t <= now)
            .map(|(_, k)| *k)
            .collect();
        if due.is_empty() {
            return;
        }
        self.chords.offs.retain(|(t, _)| !all && *t > now);
        self.with(|s| {
            let Shared { song, engine, .. } = s;
            for k in due {
                engine.live_event(song, crate::midi::LiveEv::Off { key: k });
            }
        });
    }

    pub(super) fn detect_key(&mut self) {
        let d = self.with(|s| chords::detect(&s.song));
        self.chords.detected = d.into_iter().take(6).collect();
        match self.chords.detected.first().copied() {
            Some((k, sc)) => {
                self.set_song_key(k);
                self.status = format!("Detected key: {} (score {:.2})", k.name(), sc);
            }
            None => self.status = "No melodic notes to detect a key from".into(),
        }
    }

    /// write the progression into the current block (one undo step)
    pub(super) fn write_progression(&mut self) {
        let key = self.song_key();
        let h = &self.chords;
        let style = match h.style {
            1 => chords::Style::Arp {
                pattern: h.arp,
                rate: h.arp_rate.max(1),
                octaves: h.arp_oct.max(1),
            },
            2 => chords::Style::Bass(h.bass),
            _ => chords::Style::Chords,
        };
        let prog = h.prog.clone();
        let (pat, start) = (self.pattern, if h.from_cursor { self.row } else { 0 });
        let fill = h.fill;
        let octave = h.octave;
        let (ch, ins) = (self.ch, self.instr);
        self.snapshot();
        let r = self.with(|s| {
            let rpb = ROWS_PER_BEAT as usize;
            let p = s.song.patterns.get_mut(pat).ok_or("no block")?;
            chords::write(
                p,
                key,
                &prog,
                &chords::WriteOpts {
                    style,
                    channel: ch,
                    instr: ins,
                    octave,
                    rows_per_beat: rpb,
                    start_row: start,
                    fill,
                    vol: 0,
                    seed: pat as u64 * 31 + start as u64,
                },
            )
            .map_err(|e| e.to_string())
        });
        match r {
            Ok(w) => {
                self.dirty = true;
                self.status = format!(
                    "Wrote {} chord(s), {} note(s) on channel{} {}{} (Ctrl+Z undoes)",
                    w.chords,
                    w.notes,
                    if w.channels > 1 { "s" } else { "" },
                    ch + 1,
                    if w.channels > 1 {
                        format!("–{}", ch + w.channels)
                    } else {
                        String::new()
                    }
                ) + &if w.dropped > 0 {
                    format!(", {} chord tone(s) dropped: not enough channels", w.dropped)
                } else {
                    String::new()
                };
            }
            Err(e) => self.status = format!("Chord helper: {e}"),
        }
    }

    pub(super) fn chord_window(&mut self, ctx: &egui::Context) {
        // previews end on their own timer (also the tracker header Play button,
        // which plays while the chord helper is closed)
        self.release_previews(false);
        if !self.chords.offs.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        if !self.chords.open {
            return;
        }
        let mut open = true;
        let key = self.song_key();
        let has_key = self.with(|s| s.song.info.key.is_some());
        let (pname, inst_name) = self.with(|s| {
            (
                s.song
                    .patterns
                    .get(self.pattern)
                    .map(|p| p.name.clone())
                    .unwrap_or_default(),
                s.song
                    .instruments
                    .get((self.instr as usize).wrapping_sub(1))
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| "(none)".into()),
            )
        });
        let mut new_key = None;
        let mut pad_clicked: Option<usize> = None;
        let mut write = false;
        let mut detect = false;
        let mut lock_changed = false;
        egui::Window::new("Chord helper")
            .open(&mut open)
            .default_width(700.0)
            .resizable(true)
            .show(ctx, |ui| {
                let h = &mut self.chords;
                // ---- key
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Key").strong());
                    let mut root = key.root;
                    egui::ComboBox::from_id_salt("ch_root")
                        .width(56.0)
                        .selected_text(key.note_name(root))
                        .show_ui(ui, |ui| {
                            for p in 0..12u8 {
                                ui.selectable_value(&mut root, p, key.note_name(p));
                            }
                        });
                    let mut scale = key.scale;
                    egui::ComboBox::from_id_salt("ch_scale")
                        .width(150.0)
                        .selected_text(scale.name())
                        .show_ui(ui, |ui| {
                            for s in Scale::ALL {
                                ui.selectable_value(&mut scale, s, s.name());
                            }
                        });
                    if root != key.root || scale != key.scale {
                        new_key = Some(Key::new(root, scale));
                    }
                    if !has_key {
                        ui.label(egui::RichText::new("(not set: pick one or detect)").small().color(th().ink2));
                    }
                    if ui
                        .button("Detect from song")
                        .on_hover_text("Krumhansl–Schmuckler key finding over the melodic notes (drum channels skipped)")
                        .clicked()
                    {
                        detect = true;
                    }
                    let mut lock = self.extra.scale_lock;
                    if ui
                        .checkbox(&mut lock, "Scale lock")
                        .on_hover_text("typed and MIDI notes snap to the nearest note of the key")
                        .changed()
                    {
                        self.extra.scale_lock = lock;
                        lock_changed = true;
                    }
                });
                if !h.detected.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new("detected:").small().color(th().ink2));
                        for (k, sc) in h.detected.clone() {
                            if ui
                                .selectable_label(k == key, format!("{} {:.2}", k.name(), sc))
                                .clicked()
                            {
                                new_key = Some(k);
                            }
                        }
                    });
                }
                let notes_in: String = key
                    .scale
                    .steps()
                    .iter()
                    .map(|s| key.note_name((key.root + s) % 12))
                    .collect::<Vec<_>>()
                    .join(" ");
                ui.label(egui::RichText::new(format!("notes: {notes_in}")).small().color(th().ink2));
                ui.separator();
                // ---- chord options
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Chords").strong());
                    for k in ChordKind::ALL {
                        ui.selectable_value(&mut h.kind, k, k.name());
                    }
                    ui.separator();
                    ui.label("Octave");
                    if ui.small_button("−").clicked() {
                        h.octave = h.octave.saturating_sub(1).max(1);
                    }
                    ui.label(egui::RichText::new(h.octave.to_string()).monospace().color(th().value));
                    if ui.small_button("+").clicked() {
                        h.octave = (h.octave + 1).min(7);
                    }
                    ui.separator();
                    ui.label("Inversion");
                    for i in 0..3 {
                        ui.selectable_value(&mut h.inversion, i, ["root", "1st", "2nd"][i]);
                    }
                    ui.separator();
                    ui.checkbox(&mut h.click_adds, "Click adds to progression");
                });
                // ---- pads
                ui.horizontal_wrapped(|ui| {
                    for d in 0..key.scale.steps().len() {
                        let c = chords::diatonic(key, d, h.kind);
                        let txt = egui::RichText::new(format!(
                            "{}\n{}",
                            chords::roman(key, d, &c),
                            c.name(key)
                        ))
                        .size(15.0)
                        .strong();
                        if ui
                            .add(egui::Button::new(txt).min_size(egui::vec2(82.0, 52.0)))
                            .on_hover_text(format!("chord pad {}", d + 1))
                            .clicked()
                        {
                            pad_clicked = Some(d);
                        }
                    }
                });
                ui.separator();
                // ---- progression
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Progression").strong());
                    let mut remove = None;
                    for (i, st) in h.prog.iter_mut().enumerate() {
                        let c = chords::diatonic(key, st.deg, st.kind);
                        let bars = st.beats / 4.0;
                        let r = ui
                            .button(format!("{}  {}", c.name(key), if bars == 1.0 { "1 bar".to_string() } else { format!("{bars} bars") }))
                            .on_hover_text("click: length 1 → 2 → ½ bar · right-click: remove");
                        if r.clicked() {
                            st.beats = match st.beats as u32 {
                                4 => 8.0,
                                8 => 2.0,
                                _ => 4.0,
                            };
                        }
                        if r.secondary_clicked() {
                            remove = Some(i);
                        }
                    }
                    if let Some(i) = remove {
                        h.prog.remove(i);
                    }
                    if h.prog.is_empty() {
                        ui.label(egui::RichText::new("click pads or pick a preset").small().color(th().ink2));
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    egui::ComboBox::from_id_salt("ch_preset")
                        .width(220.0)
                        .selected_text("Preset progression…")
                        .show_ui(ui, |ui| {
                            for (name, degs, hint) in chords::PRESETS {
                                if ui.button(*name).on_hover_text(*hint).clicked() {
                                    h.prog = degs
                                        .iter()
                                        .map(|&d| Step {
                                            inversion: h.inversion,
                                            ..Step::new(d, h.kind)
                                        })
                                        .collect();
                                }
                            }
                        });
                    if ui.button("Clear").clicked() {
                        h.prog.clear();
                    }
                    if ui.button("Undo last").clicked() {
                        h.prog.pop();
                    }
                });
                ui.separator();
                // ---- write
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Write as").strong());
                    ui.selectable_value(&mut h.style, 0, "Chords");
                    ui.selectable_value(&mut h.style, 1, "Arp");
                    ui.selectable_value(&mut h.style, 2, "Bass");
                    ui.separator();
                    match h.style {
                        1 => {
                            egui::ComboBox::from_id_salt("ch_arp")
                                .width(90.0)
                                .selected_text(h.arp.name())
                                .show_ui(ui, |ui| {
                                    for a in ArpPattern::ALL {
                                        ui.selectable_value(&mut h.arp, a, a.name());
                                    }
                                });
                            ui.label("every");
                            ui.add(egui::DragValue::new(&mut h.arp_rate).range(1..=16).suffix(" row(s)"));
                            ui.label("over");
                            ui.add(egui::DragValue::new(&mut h.arp_oct).range(1..=3).suffix(" oct"));
                        }
                        2 => {
                            egui::ComboBox::from_id_salt("ch_bass")
                                .width(120.0)
                                .selected_text(h.bass.name())
                                .show_ui(ui, |ui| {
                                    for b in BassRhythm::ALL {
                                        ui.selectable_value(&mut h.bass, b, b.name());
                                    }
                                });
                            ui.label(egui::RichText::new("roots, 2 octaves down").small().color(th().ink2));
                        }
                        _ => {
                            ui.label(egui::RichText::new("one chord tone per channel, from the cursor channel right").small().color(th().ink2));
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut h.fill, "Repeat to fill the block");
                    ui.checkbox(&mut h.from_cursor, "Start at the cursor row");
                });
                ui.horizontal_wrapped(|ui| {
                    let tgt = format!(
                        "into block {:02} {pname} · channel {} · instrument {:02X} {inst_name}",
                        self.pattern,
                        self.ch + 1,
                        self.instr
                    );
                    if ui
                        .add_enabled(!h.prog.is_empty(), egui::Button::new(egui::RichText::new("Write into block").strong()))
                        .on_hover_text("replaces the notes on the target channel(s); Ctrl+Z undoes")
                        .clicked()
                    {
                        write = true;
                    }
                    ui.label(egui::RichText::new(tgt).small().color(th().ink2));
                });
            });
        self.chords.open = open;
        if let Some(k) = new_key {
            self.set_song_key(k);
        }
        if detect {
            self.detect_key();
        }
        if lock_changed {
            self.save_settings();
            self.status = if self.extra.scale_lock {
                format!("Scale lock on: notes snap to {}", self.song_key().name())
            } else {
                "Scale lock off".into()
            };
            if self.extra.scale_lock && !has_key {
                self.set_song_key(key);
            }
        }
        if let Some(d) = pad_clicked {
            let key = self.song_key();
            let (kind, oct, inv) = (self.chords.kind, self.chords.octave, self.chords.inversion);
            let c = chords::diatonic(key, d, kind);
            let notes = c.notes(oct, inv);
            self.preview_chord(&notes);
            if self.chords.click_adds {
                self.chords.prog.push(Step {
                    inversion: inv,
                    ..Step::new(d, kind)
                });
            }
            self.status = format!("{} {}", chords::roman(key, d, &c), c.name(key));
        }
        if write {
            self.write_progression();
        }
    }
}
