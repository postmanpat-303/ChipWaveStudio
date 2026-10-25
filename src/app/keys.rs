//! Global keyboard handling for the editor views, UI zoom, audio restart, settings save.
use super::*;

/// status line for Ctrl+C / Ctrl+X / Ctrl+V outside the Tracker views
pub(super) const CLIPBOARD_ELSEWHERE: &str =
    "Cut, copy and paste work on the Timeline (clips), in the piano roll (click into it), the Tracker and the Order list";

impl App {
    pub(super) fn handle_keys(&mut self, ctx: &egui::Context) {
        let mut events = std::mem::take(&mut self.pending);
        // settings > keys: the next key press becomes the new binding
        if let Some(a) = self.rebinding {
            if let Some(c) = events.iter().find_map(crate::keymap::Combo::from_event) {
                self.finish_rebind(a, c);
            }
            return;
        }
        // musical typing (Ctrl+K): its keys play notes, not shortcuts
        events = self.typing_keys(events);
        events = events
            .iter()
            .filter_map(|e| self.keymap.translate(e))
            .collect();
        // at least 1: a timeline song can have no tracks (cursor maths below)
        let channels = self.with(|s| s.song.channels).max(1);
        let arr_focus = self.arr_focused();
        self.mods = ctx.input(|i| i.modifiers);
        // cut / copy / paste edit Tracker cells and Order list entries, so
        // they only act while one of those views shows: elsewhere they
        // would change a block the user can't see (or, with no tracks,
        // index nothing)
        let tracker_view = matches!(self.view, View::Pattern | View::Arranger);
        for a in input::clipboard_actions(&events) {
            // the piano roll has the keyboard (clicked into last): its notes
            if self.pr_active() {
                match a {
                    input::ClipAction::Copy => self.pr_copy(false, ctx),
                    input::ClipAction::Cut => self.pr_copy(true, ctx),
                    input::ClipAction::Paste(t) => self.pr_paste(t),
                }
                continue;
            }
            // the Timeline: its clips (Ctrl+Shift+V pastes linked copies)
            if self.view == View::Timeline {
                match a {
                    input::ClipAction::Copy => self.tl_copy(false),
                    input::ClipAction::Cut => self.tl_copy(true),
                    input::ClipAction::Paste(_) => self.tl_paste(None, self.mods.shift),
                }
                continue;
            }
            if !tracker_view {
                self.status = CLIPBOARD_ELSEWHERE.into();
                continue;
            }
            match input::route_clip(a, arr_focus, self.mods.shift) {
                input::Routed::Pattern(input::ClipAction::Copy) => self.copy(false, ctx),
                input::Routed::Pattern(input::ClipAction::Cut) => self.copy(true, ctx),
                input::Routed::Pattern(input::ClipAction::Paste(t)) => self.paste(t),
                input::Routed::Arranger(a, t) => self.arranger_action_text(a, t, ctx),
            }
        }
        let arr_visible =
            self.view == View::Arranger || (self.view == View::Pattern && self.arr_docked());
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
            // Ctrl+D in the piano roll duplicates notes, not Order list clips
            let roll_dup = self.pr_active()
                && matches!(&ev, egui::Event::Key { key: Key::D, pressed: true, modifiers: m, .. }
                    if m.command && !m.shift && !m.alt);
            // the Rack (clicked into last) owns Delete, Ctrl+D and 0
            let rack_owns = self.rack_focus()
                && matches!(&ev, egui::Event::Key { key, pressed: true, modifiers: m, .. }
                    if matches!(key, Key::Delete | Key::Backspace | Key::Num0)
                        || (*key == Key::D && m.command));
            if arr_visible && !roll_dup && !rack_owns {
                if let Some(a) = input::arranger_shortcut(&ev, arr_focus) {
                    self.arranger_action(a, ctx);
                    continue;
                }
            }
            // with the arranger focused, plain keys must not type notes into the pattern
            if arr_focus && !rack_owns {
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
                                | Key::F11
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
            // R: Record (live, into the Timeline). The Tracker types notes
            // with R.
            if key == Key::R
                && !cmd
                && !m.alt
                && !m.shift
                && !repeat
                && self.view != View::Pattern
                && self.prompt.is_none()
            {
                self.rec_toggle();
                continue;
            }
            // the Rack has the keyboard (clicked into last)
            if self.rack_key(key, m) {
                continue;
            }
            // the piano roll has the keyboard (clicked into last)
            if self.pr_active() && self.pr_key(key, m) {
                continue;
            }
            // the Browser has the keyboard (clicked into last)
            if self.br_active() && self.br_key(key, m, ctx) {
                continue;
            }
            // I: the Inspector (no text field has focus here: those keep
            // their keys). The pattern editor types a note with I, so there
            // it is Alt+I (which works everywhere).
            if key == Key::I && !cmd && !m.shift && !repeat && (m.alt || self.view != View::Pattern)
            {
                self.toggle_inspector();
                continue;
            }
            // Timeline view: its own keys; plain keys never type into the pattern
            if self.view == View::Timeline {
                if self.timeline_key(key, m) {
                    continue;
                }
                let allowed = cmd
                    || matches!(
                        key,
                        Key::Space
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
                            | Key::F11
                            | Key::F12
                    );
                if !allowed {
                    continue;
                }
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
                Key::Tab if cmd && !self.tracker_on() => {
                    self.view = match self.view {
                        View::Timeline => View::Mixer,
                        View::Mixer => View::Fx,
                        View::Fx => View::Sample,
                        _ => View::Timeline,
                    }
                }
                Key::Tab if cmd => {
                    self.view = match self.view {
                        View::Pattern => View::Arranger,
                        View::Arranger => View::Timeline,
                        View::Timeline => View::Mixer,
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
                Key::T if cmd => self.view = View::Timeline,
                Key::Num3 if cmd => self.view = View::Mixer,
                Key::Num4 if cmd => self.view = View::Fx,
                Key::Num5 if cmd => self.view = View::Sample,
                Key::Num6 if cmd => self.reset_layout(),
                Key::A if cmd && m.shift => self.toggle_adv_edit(),
                Key::Num7 if cmd => self.vtab_select(super::viewbar::VTab::Devices),
                Key::Num8 if cmd && m.shift => self.toggle_side_panel(),
                Key::Num8 if cmd => self.toggle_clip_arranger(),
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
                // block length keys change the Tracker's current block (every
                // clip that plays it): only where that block is in view
                Key::PageDown | Key::PageUp if cmd && m.shift && !tracker_view => {}
                Key::PageDown if cmd && m.shift => {
                    let n = self.rows() * 2;
                    self.block_length(blocklen::LenOp::Set(n));
                }
                Key::PageUp if cmd && m.shift => {
                    let n = self.rows().div_ceil(2);
                    self.block_length(blocklen::LenOp::Set(n));
                }
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
                // the Timeline plays the song from its play marker
                Key::Space if self.view == View::Timeline => self.play(0),
                Key::Space => self.play(if m.shift { 0 } else { 1 }),
                Key::Enter => self.play(if cmd || m.shift { 2 } else { 3 }),
                Key::Plus | Key::Equals if cmd => self.set_zoom(self.zoom + 0.1),
                Key::Minus if cmd => self.set_zoom(self.zoom - 0.1),
                Key::Num0 if cmd => self.set_zoom(1.0),
                // Esc is the Tracker's Edit mode switch only where the
                // Tracker shows; elsewhere it must not change how notes and
                // MIDI are recorded behind the user's back
                Key::Escape if tracker_view => {
                    self.edit = !self.edit;
                    self.status = format!("Edit mode {}", if self.edit { "on" } else { "off" });
                }
                Key::Escape => {}
                Key::Z if cmd && m.shift => self.do_undo(true),
                Key::Z if cmd => self.do_undo(false),
                Key::Y if cmd => self.do_undo(true),
                Key::Q if cmd => self.request(crate::prompt::Pending::Quit, ctx),
                Key::Comma if cmd => self.toggle_settings(),
                Key::Period if cmd => self.panic(),
                Key::E if cmd && m.shift => self.open_export(),
                Key::F if cmd && m.shift => self.open_fill(),
                Key::F if cmd => {
                    if !(self.view == View::Pattern
                        && self.lower_visible()
                        && self.extra.lower_inst)
                    {
                        self.show_instrument_panel();
                    }
                    self.ip.focus_search = true;
                }
                Key::I if cmd => self.toggle_instrument_panel(),
                Key::B if cmd && !m.shift => self.toggle_browser(),
                Key::F11 if m.shift && self.sw_panel_shown(ctx) => self.toggle_sw_full(),
                Key::F11 => self.toggle_full_height(),
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
                        let mut note = (self.octave as i32 * 12 + off + 1).clamp(1, 120) as u8;
                        if let Some(k) = self.scale_lock_key() {
                            note = k.snap(note);
                        }
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

    /// the largest UI scale (5 % steps) at which the workspace fits the
    /// window, at least 90 %
    pub(super) fn zoom_fit(&self) -> f32 {
        let m = (self.max_zoom / crate::settings::SCALE_STEP + 0.001).floor()
            * crate::settings::SCALE_STEP;
        m.clamp(crate::settings::ZOOM_MIN, crate::settings::ZOOM_MAX)
    }

    pub(super) fn set_zoom(&mut self, z: f32) {
        let z = crate::settings::snap_zoom(z);
        self.zoom = z.min(self.zoom_fit());
        if z > self.zoom_fit() + 0.001 {
            self.status = format!(
                "Scale limited to {:.0} % so everything fits the window",
                self.zoom * 100.0
            );
        }
        self.save_settings();
    }

    pub(super) fn restart_audio(&mut self) {
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

    pub(super) fn save_settings(&self) {
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

    pub(super) fn set_octave(&mut self, o: i32) {
        self.octave = o.clamp(0, 8) as u8;
        self.status = format!("Base octave {}", self.octave);
    }
}

pub(super) fn note_key(k: Key) -> Option<i32> {
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

pub(super) fn hex_key(k: Key) -> Option<u8> {
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
