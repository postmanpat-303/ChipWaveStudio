//! MIDI keyboard input on the UI side: settings card, recording into the
//! pattern, MIDI learn, the toolbar activity light (docs/MIDI.md).
use super::*;
use crate::automation::Target;
use crate::midi::{self, Curve, UiEv};
use std::sync::atomic::Ordering::Relaxed;

/// MIDI state kept by the app
pub(super) struct MidiUi {
    pub hub: midi::Hub,
    /// keys held down right now
    pub(super) held: Vec<u8>,
    /// keys of the chord being recorded (chord spread)
    chord: Vec<u8>,
    /// the chord was step-recorded: move the cursor when it is released
    chord_step: bool,
    /// waiting for a CC to bind to this target id
    pub learn: Option<String>,
    /// activity counter seen and when it last changed (egui time)
    seen: u32,
    lit_at: f64,
    /// the latest key pressure 0..1 and its key (midi::ALL_KEYS: channel
    /// pressure), for the MIDI light's meter; `press_seen` once any came
    pub(super) press: f32,
    pub(super) press_key: u8,
    pub(super) press_seen: bool,
}

impl MidiUi {
    /// The UI half and the engine's end of the live queue.
    pub fn new() -> (MidiUi, rtrb::Consumer<midi::LiveEv>) {
        let (hub, rx) = midi::Hub::new();
        (
            MidiUi {
                hub,
                held: vec![],
                chord: vec![],
                chord_step: false,
                learn: None,
                seen: 0,
                lit_at: -10.0,
                press: 0.0,
                press_key: 0,
                press_seen: false,
            },
            rx,
        )
    }
}

/// human name of a learn target
pub(super) fn target_text(t: &str) -> String {
    match Target::parse(t) {
        Some(
            x @ (Target::Cutoff(i) | Target::Resonance(i) | Target::Drive(i) | Target::Level(i)),
        ) => {
            format!("instrument {:02X} {}", i + 1, x.param_name().to_lowercase())
        }
        Some(Target::ChanVolume(c)) => format!("channel {} volume", c + 1),
        Some(Target::ChanPan(c)) => format!("channel {} pan", c + 1),
        Some(Target::Send(c, b)) => {
            format!("channel {} send {}", c + 1, if b == 0 { "A" } else { "B" })
        }
        Some(Target::Master) => "master volume".into(),
        Some(Target::Tempo) => "tempo".into(),
        _ => t.to_string(),
    }
}

/// Write a learned CC value (0..1) into the song. Returns true if it applied.
/// the knob a CC moves on StringWave instrument `instr` (1-based) when it
/// isn't learned (`strings::DEFAULT_CC`)
pub(super) fn default_cc_target(song: &crate::model::Song, instr: usize, cc: u8) -> Option<String> {
    let i = instr.wrapping_sub(1);
    crate::strings::rack::patch(song, i)?;
    Some(Target::Gen(i, crate::strings::default_cc(cc)?).id())
}

pub(super) fn apply_learned(s: &mut Shared, target: &str, n: f32) -> bool {
    let Some(t) = Target::parse_in(target, &s.song) else {
        return false;
    };
    let n = n.clamp(0.0, 1.0);
    let real = t.to_real(n);
    let song = &mut s.song;
    match t {
        Target::Cutoff(i) | Target::Resonance(i) | Target::Drive(i) => {
            let Some(InstrumentKind::Synth(p)) = song.instruments.get_mut(i).map(|x| &mut x.kind)
            else {
                return false;
            };
            match t {
                Target::Cutoff(_) => p.cutoff = real,
                Target::Resonance(_) => p.resonance = real,
                _ => p.drive = real,
            }
        }
        Target::Level(i) => match song.instruments.get_mut(i) {
            Some(x) => x.volume = n,
            None => return false,
        },
        Target::ChanVolume(c) => match song.mixer.channels.get_mut(c) {
            Some(m) => m.volume = real,
            None => return false,
        },
        Target::ChanPan(c) => match song.mixer.channels.get_mut(c) {
            Some(m) => m.pan = real,
            None => return false,
        },
        Target::Send(c, b) => match song.mixer.channels.get_mut(c) {
            Some(m) if b < 2 => m.send[b] = n,
            _ => return false,
        },
        Target::Macro(i, k) => return crate::plugin::set_macro_norm(s, i, k, n),
        Target::Gen(i, k) => match song.instruments.get_mut(i).map(|x| &mut x.kind) {
            Some(InstrumentKind::Gen(g)) => return crate::gen::set_gen_knob_norm(g, k, n),
            _ => return false,
        },
        Target::Master => song.mixer.master = real,
        Target::Tempo => {
            song.bpm = real.round();
            let (b, sp) = (song.bpm, song.speed);
            s.engine.set_tempo(b, sp);
        }
        Target::Fx(i, d, p) => {
            match song
                .instruments
                .get_mut(i)
                .and_then(|x| x.fx.get_mut(d))
                .and_then(|dv| dv.params.get_mut(p))
            {
                Some(v) => *v = n,
                None => return false,
            }
        }
        Target::Ret(b, p) => {
            match song
                .mixer
                .returns
                .get_mut(b)
                .and_then(|r| r.device.params.get_mut(p))
            {
                Some(v) => *v = n,
                None => return false,
            }
        }
        Target::Patch(i, k) => {
            let (Some(p), Some(pp)) = (
                song.instruments.get_mut(i).and_then(|x| x.kind.patch_mut()),
                crate::modulation::PATCH_PARAMS.get(k),
            ) else {
                return false;
            };
            (pp.set)(p, real);
        }
        Target::ModAmount(k) => match song.modulation.routes.get_mut(k) {
            Some(r) => r.amount = real,
            None => return false,
        },
    }
    true
}

impl App {
    /// Once per frame, before the keyboard: settings to the MIDI thread,
    /// hot-plug rescan, then the queued notes / CCs.
    pub(super) fn midi_frame(&mut self, ctx: &egui::Context) {
        self.midi.hub.set_ctx(ctx);
        let cfg = self.midi.hub.cfg.clone();
        cfg.instr.store(self.instr, Relaxed);
        cfg.mon_ch.store(self.ch as u8, Relaxed);
        cfg.scale
            .store(crate::chords::pack(self.scale_lock_key()), Relaxed);
        if self.extra.midi.auto_connect {
            self.midi.hub.poll();
        }
        self.typing_follow_midi();
        let n = cfg.activity.load(Relaxed);
        if n != self.midi.seen {
            self.midi.seen = n;
            self.midi.lit_at = ctx.input(|i| i.time);
        }
        while let Ok(ev) = self.midi.hub.ui_rx.pop() {
            match ev {
                UiEv::On { key, note, vel, at } => self.midi_note_on(key, note, vel, at),
                UiEv::Off { key } => self.midi_note_off(key),
                UiEv::Cc { cc, val } => self.midi_cc(cc, val),
                UiEv::Pressure { key, val } => self.midi_pressure(key, val),
            }
        }
    }

    /// the MIDI light's pressure meter line: None until the keyboard sent
    /// key or channel pressure
    pub(super) fn midi_press_meter(&self) -> Option<(f32, String)> {
        if !self.midi.press_seen {
            return None;
        }
        let m = &self.midi;
        let what = if m.press_key == midi::ALL_KEYS {
            "Channel pressure".to_string()
        } else {
            format!("Key pressure (key {})", m.press_key)
        };
        Some((m.press, format!("{what} {:.0} %", m.press * 100.0)))
    }

    /// (lit, tooltip) for the toolbar light
    pub(super) fn midi_light(&self, now: f64) -> (bool, String) {
        let names = self.midi.hub.connected();
        let tip = if names.is_empty() {
            if self.extra.midi.device.is_empty() {
                "MIDI: no input selected (Settings > MIDI)".to_string()
            } else {
                format!(
                    "MIDI: waiting for \"{}\" (plug it in)",
                    self.extra.midi.device
                )
            }
        } else {
            format!("MIDI: {}", names.join(", "))
        };
        (now - self.midi.lit_at < 0.15, tip)
    }

    fn set_cell_at(&mut self, p: usize, r: usize, c: usize, f: impl FnOnce(&mut Cell)) {
        // one undo step per recorded note
        self.snapshot();
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

    pub(super) fn midi_note_on(&mut self, key: u8, note: u8, vel: f32, at: std::time::Instant) {
        if !self.midi.held.contains(&key) {
            self.midi.held.push(key);
        }
        self.rec_remember_on(key, note, vel, at);
        if self.rec_note_on(key, note, vel, at) {
            return;
        }
        if self.step_note_on(note, vel) {
            return;
        }
        if !self.edit || self.view != View::Pattern {
            return; // play only
        }
        let ms = self.extra.midi.clone();
        let (playing, epat, erow, phase, row_secs, looping, nch) = self.with(|s| {
            (
                s.engine.playing && s.preview.is_none(),
                s.engine.pattern,
                s.engine.row,
                s.engine.row_phase(),
                s.engine.row_secs(),
                s.engine.loop_pattern,
                s.song.channels.max(1),
            )
        });
        let k = if ms.chord_spread {
            self.midi.chord.len()
        } else {
            0
        };
        let ch = (self.ch + k) % nch;
        let ins = self.instr;
        let vol = if ms.velocity {
            midi::vol_column(vel)
        } else {
            0
        };
        let put = |c: &mut Cell| {
            c.note = note;
            c.instr = ins;
            if vol > 0 {
                c.vol = vol;
            }
        };
        let live = playing && ms.live_record;
        if live {
            // the row that was audible when the key went down: the engine
            // runs ahead of the speakers by the output latency
            let lat = self.audio.as_ref().map_or(0.0, |a| a.latency_ms() / 1000.0);
            let back = (at.elapsed().as_secs_f32() + lat) / row_secs.max(1e-4);
            let rows = self.with(|s| s.song.patterns.get(epat).map_or(1, |p| p.rows.max(1)));
            let row = live_row(erow, phase, back, rows, looping);
            self.set_cell_at(epat, row, ch, put);
            self.status = format!(
                "MIDI rec {} row {} ch {}",
                note_name(note),
                self.row_text(row, rows),
                ch + 1
            );
        } else {
            let (p, r) = (self.pattern, self.row);
            self.set_cell_at(p, r, ch, put);
            if !ms.chord_spread {
                self.move_row(self.step as isize);
            }
        }
        if ms.chord_spread {
            if self.midi.chord.is_empty() {
                self.midi.chord_step = !live;
            }
            self.midi.chord.push(key);
        }
    }

    pub(super) fn midi_note_off(&mut self, key: u8) {
        self.midi.held.retain(|k| *k != key);
        self.rec_remember_off(key);
        self.rec_note_off(key);
        self.step_note_off();
        if self.midi.held.is_empty() && !self.midi.chord.is_empty() {
            self.midi.chord.clear();
            if self.midi.chord_step {
                self.move_row(self.step as isize);
            }
        }
    }

    /// key pressure (aftertouch): the engine already plays it (Pressure
    /// modulators); here it is metered for the MIDI light and recorded into
    /// the held notes of a take
    pub(super) fn midi_pressure(&mut self, key: u8, val: f32) {
        self.midi.press = val.clamp(0.0, 1.0);
        self.midi.press_key = key;
        self.midi.press_seen = true;
        self.rec_pressure(key, val);
    }

    pub(super) fn midi_cc(&mut self, cc: u8, val: u8) {
        if cc == 1 {
            // the mod wheel feeds "Mod wheel" modulators
            self.with(|s| s.engine.mods.feed.wheel = val as f32 / 127.0);
        }
        if let Some(t) = self.midi.learn.take() {
            let l = &mut self.extra.midi.learn;
            l.retain(|(c, x)| *c != cc && *x != t);
            l.push((cc, t.clone()));
            self.save_settings();
            self.status = format!("MIDI learn: CC {cc} now controls {}", target_text(&t));
            return;
        }
        let targets: Vec<String> = self
            .extra
            .midi
            .learn
            .iter()
            .filter(|(c, _)| *c == cc)
            .map(|(_, t)| t.clone())
            .collect();
        let mut targets = targets;
        // StringWave's default map (a learned CC wins): the MIDI instrument
        if targets.is_empty() {
            let instr = self.instr as usize;
            if let Some(t) = self.with(|s| default_cc_target(&s.song, instr, cc)) {
                targets.push(t);
            }
        }
        for t in targets {
            if self.with(|s| apply_learned(s, &t, val as f32 / 127.0)) {
                self.dirty = true;
            }
        }
    }

    /// Right-click menu on a knob / slider: MIDI learn / forget.
    pub(super) fn learn_menu(&mut self, r: &egui::Response, target: Target) {
        let id = target.id();
        let waiting = self.midi.learn.as_deref() == Some(id.as_str());
        // sliders only sense drags: add clicks so the right-click menu opens
        let r = r.interact(egui::Sense::click());
        r.context_menu(|ui| self.learn_items(ui, &id, waiting));
        if waiting {
            ui_ring(&r.ctx, r.rect);
        }
    }

    /// the MIDI learn / forget items of a control's right-click menu
    pub(super) fn learn_items(&mut self, ui: &mut egui::Ui, id: &str, waiting: bool) {
        let id = id.to_string();
        let bound: Vec<u8> = self
            .extra
            .midi
            .learn
            .iter()
            .filter(|(_, t)| *t == id)
            .map(|(c, _)| *c)
            .collect();
        if waiting {
            if ui.button("Cancel MIDI learn").clicked() {
                self.midi.learn = None;
                ui.close();
            }
        } else if ui
            .button("MIDI learn")
            .on_hover_text("then move a knob, fader or the mod wheel on the MIDI keyboard")
            .clicked()
        {
            self.midi.learn = Some(id.clone());
            self.status = format!(
                "MIDI learn: move a knob or the mod wheel to control {}",
                target_text(&id)
            );
            ui.close();
        }
        for cc in &bound {
            if ui.button(format!("Forget MIDI CC {cc}")).clicked() {
                self.extra
                    .midi
                    .learn
                    .retain(|(c, t)| !(c == cc && *t == id));
                self.save_settings();
                ui.close();
            }
        }
    }

    /// apply the MIDI settings to the hub (after a change or at start)
    pub(super) fn midi_apply(&mut self, reconnect: bool) {
        self.midi.hub.cfg.apply(&self.extra.midi);
        if reconnect {
            let d = self.extra.midi.device.clone();
            self.midi.hub.connect(&d);
        }
    }

    pub(super) fn midi_summary(&self) -> (Vec<String>, Option<String>) {
        let m = &self.extra.midi;
        let names = self.midi.hub.connected();
        let dev = match m.device.as_str() {
            "" => "no input".to_string(),
            "*" => format!("all devices ({} connected)", names.len()),
            d if names.is_empty() => format!("{d} (not connected)"),
            d => d.to_string(),
        };
        (
            vec![
                dev,
                format!(
                    "ch {} · velocity {} · octave {:+}",
                    if m.channel == 0 {
                        "all".to_string()
                    } else {
                        m.channel.to_string()
                    },
                    if m.velocity {
                        m.curve.name().split(' ').next().unwrap_or("")
                    } else {
                        "off"
                    },
                    m.octave
                ),
                format!("{} learned control(s)", m.learn.len()),
            ],
            self.midi.hub.error.clone(),
        )
    }

    pub(super) fn midi_details(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        let mut reconnect = false;
        let m = &mut self.extra.midi;
        egui::Grid::new("midi")
            .num_columns(2)
            .spacing([14.0, 8.0])
            .show(ui, |ui| {
                ui.label("Input device");
                ui.horizontal(|ui| {
                    let label = |d: &str| match d {
                        "" => "None".to_string(),
                        "*" => "All devices".to_string(),
                        d => d.to_string(),
                    };
                    egui::ComboBox::from_id_salt("midi_dev")
                        .selected_text(label(&m.device))
                        .width(300.0)
                        .show_ui(ui, |ui| {
                            let mut opts = vec![String::new(), "*".to_string()];
                            opts.extend(self.midi.hub.ports.iter().cloned());
                            if !m.device.is_empty() && !opts.contains(&m.device) {
                                opts.push(m.device.clone());
                            }
                            for d in opts {
                                if ui.selectable_label(m.device == d, label(&d)).clicked() {
                                    m.device = d;
                                    reconnect = true;
                                }
                            }
                        });
                    if ui
                        .button("Rescan")
                        .on_hover_text("look for newly plugged-in devices")
                        .clicked()
                    {
                        self.midi.hub.rescan_now();
                    }
                });
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut m.auto_connect, "Connect automatically (at start and when plugged in)")
                    .changed()
                    .then(|| changed = true);
                ui.end_row();
                ui.label("Status");
                let names = self.midi.hub.connected();
                match (&self.midi.hub.error, names.is_empty()) {
                    (Some(e), _) => ui.label(egui::RichText::new(e).color(th().brick)),
                    (None, true) => ui.label(egui::RichText::new("not connected").color(th().ink2)),
                    (None, false) => ui.label(
                        egui::RichText::new(format!("connected: {}", names.join(", "))).color(th().value),
                    ),
                };
                ui.end_row();
                ui.label("MIDI channel");
                egui::ComboBox::from_id_salt("midi_ch")
                    .selected_text(if m.channel == 0 { "All".to_string() } else { m.channel.to_string() })
                    .show_ui(ui, |ui| {
                        for c in 0..=16u8 {
                            let t = if c == 0 { "All".to_string() } else { c.to_string() };
                            if ui.selectable_label(m.channel == c, t).clicked() {
                                m.channel = c;
                                changed = true;
                            }
                        }
                    });
                ui.end_row();
                ui.label("Velocity");
                ui.horizontal(|ui| {
                    changed |= ui.checkbox(&mut m.velocity, "use velocity").changed();
                    ui.add_enabled_ui(m.velocity, |ui| {
                        egui::ComboBox::from_id_salt("midi_curve")
                            .selected_text(m.curve.name())
                            .show_ui(ui, |ui| {
                                for c in Curve::ALL {
                                    if ui.selectable_label(m.curve == c, c.name()).clicked() {
                                        m.curve = c;
                                        changed = true;
                                    }
                                }
                            });
                    });
                    ui.label(
                        egui::RichText::new("recorded into the volume column")
                            .small()
                            .color(th().ink2),
                    );
                });
                ui.end_row();
                ui.label("MIDI octave");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(egui::Slider::new(&mut m.octave, -4..=4).suffix(" oct"))
                        .changed();
                    ui.label(
                        egui::RichText::new(format!(
                            "middle C plays {} (computer keyboard octave is separate)",
                            note_name(midi::tracker_note(60, m.octave as i32).unwrap_or(49))
                        ))
                        .small()
                        .color(th().ink2),
                    );
                });
                ui.end_row();
                ui.label("Pitch bend range");
                changed |= ui
                    .add(egui::Slider::new(&mut m.bend_range, 1..=24).suffix(" semitones"))
                    .changed();
                ui.end_row();
                ui.label("Recording");
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Edit mode on + Tracker: notes are written at the cursor and the cursor moves by Step.")
                            .small()
                            .color(th().ink2),
                    );
                    changed |= ui
                        .checkbox(&mut m.live_record, "While playing: record at the playing row (quantised), cursor stays")
                        .changed();
                    changed |= ui
                        .checkbox(&mut m.chord_spread, "Chords: spread over the cursor channel and the channels to its right")
                        .changed();
                });
                ui.end_row();
                ui.label("MIDI learn");
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Right-click a fader, pan, send, the instrument Volume / Cutoff / Resonance / Drive slider or the tempo > MIDI learn, then move a knob or the mod wheel.")
                            .small()
                            .color(th().ink2),
                    );
                    let mut forget = None;
                    for (k, (cc, t)) in m.learn.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("CC {cc:3} -> {}", target_text(t))).monospace());
                            if ui.small_button("Forget").clicked() {
                                forget = Some(k);
                            }
                        });
                    }
                    if let Some(k) = forget {
                        m.learn.remove(k);
                        changed = true;
                    }
                });
                ui.end_row();
            });
        if changed || reconnect {
            self.save_settings();
            self.midi_apply(reconnect);
        }
    }
}

/// pulsing outline around a control that waits for MIDI learn
pub(super) fn ui_ring(ctx: &egui::Context, r: Rect) {
    let p = ctx.layer_painter(egui::LayerId::debug());
    p.rect_stroke(
        r.expand(3.0),
        4,
        Stroke::new(2.0, th().fx),
        egui::StrokeKind::Outside,
    );
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}

/// Row to record a live note into: the engine row + phase minus how far the
/// audible position lags behind (`back`, in rows), rounded to the nearest row.
pub(super) fn live_row(row: usize, phase: f32, back: f32, rows: usize, looping: bool) -> usize {
    let pos = (row as f32 + phase - back).round() as isize;
    let rows = rows.max(1) as isize;
    if pos < 0 && !looping {
        0
    } else {
        pos.rem_euclid(rows) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_row_quantises_and_compensates() {
        assert_eq!(live_row(10, 0.2, 0.0, 64, true), 10);
        assert_eq!(live_row(10, 0.6, 0.0, 64, true), 11);
        // 40 ms latency at 125 ms rows: a key heard at 10.3 lands on row 10
        assert_eq!(live_row(10, 0.62, 0.32, 64, true), 10);
        // wraps in a looping block, clamps at the song start otherwise
        assert_eq!(live_row(0, 0.0, 1.2, 64, true), 63);
        assert_eq!(live_row(0, 0.0, 1.2, 64, false), 0);
        assert_eq!(live_row(63, 0.9, 0.0, 64, true), 0);
    }

    #[test]
    fn learned_cc_writes_song_values() {
        let mut s = Shared {
            song: crate::fileio::demo_song(),
            engine: Engine::new(48000.0),
            preview: None,
        };
        assert!(apply_learned(&mut s, "mixer.ch.2.volume", 0.5));
        assert_eq!(s.song.mixer.channels[1].volume, 1.0);
        assert!(apply_learned(&mut s, "mixer.ch.1.pan", 0.0));
        assert_eq!(s.song.mixer.channels[0].pan, -1.0);
        assert!(apply_learned(&mut s, "instrument.1.level", 0.25));
        assert_eq!(s.song.instruments[0].volume, 0.25);
        assert!(apply_learned(&mut s, "tempo", 1.0));
        assert_eq!(s.song.bpm, 300.0);
        assert_eq!(s.engine.bpm(), 300.0);
        let synth = s
            .song
            .instruments
            .iter()
            .position(|i| matches!(i.kind, InstrumentKind::Synth(_)))
            .unwrap();
        assert!(apply_learned(
            &mut s,
            &format!("instrument.{}.cutoff", synth + 1),
            1.0
        ));
        let InstrumentKind::Synth(p) = &s.song.instruments[synth].kind else {
            unreachable!()
        };
        assert!(p.cutoff > 10000.0);
        assert!(!apply_learned(&mut s, "instrument.99.cutoff", 1.0));
        assert!(!apply_learned(&mut s, "nonsense", 1.0));
    }

    #[test]
    fn stringwave_default_ccs_move_knobs() {
        let mut s = Shared {
            song: crate::engine::strings_tests::sw_song(),
            engine: Engine::new(48000.0),
            preview: None,
        };
        use crate::strings::{CUTOFF, MASTER};
        // CC 74 is cutoff, CC 7 master; CC 1/11 play live (no knob)
        let t = default_cc_target(&s.song, 1, 74).unwrap();
        assert!(apply_learned(&mut s, &t, 0.25));
        let p = crate::strings::rack::patch(&s.song, 0).unwrap();
        assert!((p.p[CUTOFF] - 0.25).abs() < 1e-6);
        let t = default_cc_target(&s.song, 1, 7).unwrap();
        assert!(apply_learned(&mut s, &t, 0.5));
        let p = crate::strings::rack::patch(&s.song, 0).unwrap();
        assert!((p.p[MASTER] - 0.5).abs() < 1e-6);
        assert_eq!(default_cc_target(&s.song, 1, 1), None);
        assert_eq!(default_cc_target(&s.song, 1, 11), None);
        // other instruments have no default map
        assert_eq!(default_cc_target(&s.song, 2, 74), None);
        assert_eq!(default_cc_target(&crate::fileio::demo_song(), 1, 74), None);
    }
}
