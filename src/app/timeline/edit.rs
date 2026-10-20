//! Timeline mode editing in the view (M2): the edits behind the mouse
//! gestures, keys and menus, the play marker, and the mode switch prompts.
//! Every edit changes the song under the lock and rebuilds the engine's
//! playback plan right there, so playback never sees a stale plan; the
//! song-wide undo records each gesture as one step.
use super::*;

/// what a drag frame does, re-applied to the timeline as it was when the
/// gesture started (so snapping never drifts)
#[derive(Clone, Copy, Debug)]
pub(super) enum DragEdit {
    /// move the selection by (ticks, tracks)
    Move(Tick, i32),
    /// clip id's right edge to tick
    End(u32, Tick),
    /// clip id's left edge to tick
    Start(u32, Tick),
    /// marker index to tick
    Marker(usize, Tick),
    /// loop region from, to
    Loop(Tick, Tick),
    /// audio clip id's fade in (true) / out handle to tick
    Fade(u32, bool, Tick),
}

impl App {
    pub(super) fn timeline_drag_edit(&mut self, e: DragEdit) {
        let Some(d) = self.tlv.drag.as_mut() else {
            return;
        };
        let mut t = d.origin.clone();
        let sels = self.tlv.sels.clone();
        let mut moved = false;
        let nch = self.with(|s| s.song.channels);
        let what = self.with(|s| {
            match e {
                DragEdit::Move(dt, dtr) => {
                    moved = dt != 0 || dtr != 0;
                    tl::move_clips(&mut t, &sels, dt, dtr, nch);
                }
                DragEdit::End(id, at) => {
                    tl::set_end(&s.song, &mut t, id, at);
                    moved = true;
                }
                DragEdit::Start(id, at) => {
                    tl::set_start(&mut t, id, at);
                    moved = true;
                }
                DragEdit::Marker(i, at) => {
                    if let Some(m) = t.markers.get_mut(i) {
                        moved = m.tick != at;
                        m.tick = at;
                    }
                }
                DragEdit::Loop(a, b) => {
                    tl::set_loop(&mut t, a, b);
                    moved = true;
                }
                DragEdit::Fade(id, inn, at) => {
                    if let Some(c) = t.clips.iter().find(|c| c.id == id) {
                        let (fi, fo) = if inn {
                            (at - c.start, c.fade_out)
                        } else {
                            (c.fade_in, c.end() - at)
                        };
                        tl::set_fades(&mut t, id, fi, fo);
                        moved = true;
                    }
                }
            }
            let what = describe(&t, e);
            s.song.timeline = Some(Box::new(t));
            let Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            what
        });
        if let Some(d) = self.tlv.drag.as_mut() {
            d.moved |= moved;
        }
        self.status = what;
        self.dirty = true;
    }

    /// mouse released: an Alt-copy that never moved is taken back; markers
    /// are put in order again
    pub(super) fn timeline_drag_end(&mut self) {
        let Some(d) = self.tlv.drag.take() else {
            return;
        };
        let undo_copy = matches!(d.kind, DragKind::Move { dup: true }) && !d.moved;
        let marker = matches!(d.kind, DragKind::Marker(_));
        if undo_copy || marker {
            self.with(|s| {
                if undo_copy {
                    if let Some(b) = d.before.clone() {
                        s.song.timeline = Some(Box::new(b));
                    }
                }
                if let Some(t) = s.song.timeline.as_deref_mut() {
                    t.markers.sort_by_key(|m| m.tick);
                }
                let Shared { song, engine, .. } = s;
                engine.ensure_plan(song);
            });
        }
        if undo_copy {
            self.tlv.sels.clear();
            self.tlv.sel = None;
        }
    }

    /// timeline mode: move the play marker to tick `t` (a row); running
    /// playback jumps there with the state chased
    pub(crate) fn timeline_seek(&mut self, t: Tick) {
        let t = t.max(0) / ROW * ROW;
        self.tlv.cue = t;
        let row = (t / ROW) as usize;
        let (bar, beat) = self.with(|s| {
            let Shared {
                song,
                engine,
                preview,
            } = s;
            if engine.playing && preview.is_none() && engine.tl_row().is_some() {
                engine.seek(song, 0, row);
            }
            song.timeline.as_deref().map_or((1, 1), |tl| {
                let p = tl.bar_pos(t);
                (p.bar, p.beat)
            })
        });
        self.status = format!("Play marker: bar {bar}.{beat} (row {row})");
    }

    /// Run a timeline edit. In tracker mode it asks to switch first (the
    /// edit runs after the switch).
    pub(crate) fn tl_apply(&mut self, op: TlOp) {
        let mode = self.with(|s| s.song.timeline.is_some());
        if !mode {
            self.tlv.ask_switch = Some(Some(op));
            return;
        }
        if let TlOp::DrumPattern { pat, track, at } = op {
            self.pat_place_now(pat as usize, track, at);
            return;
        }
        let sels = self.tlv.sels.clone();
        let needs_sel = matches!(
            op,
            TlOp::Duplicate
                | TlOp::Delete
                | TlOp::Mute
                | TlOp::MakeUnique
                | TlOp::Glue
                | TlOp::LoopFromSelection
        );
        if needs_sel && sels.is_empty() {
            self.status =
                "Select a clip first (click it; Ctrl-click or drag a box for more)".into();
            return;
        }
        let hover = self.tlv.hover_at;
        let mut new_sel: Option<Vec<u32>> = None;
        let mut open: Option<(usize, usize)> = None;
        let mut rename: Option<usize> = None;
        let mut open_roll = false;
        let captured = match op {
            TlOp::Capture(..) => self.capture_notes(),
            _ => vec![],
        };
        let ready = if op == TlOp::PlaceAudio {
            std::mem::take(&mut self.tlv.ready_audio)
        } else {
            vec![]
        };
        let msg: String = self.with(|s| {
            let song = &mut s.song;
            let msg = match op {
                TlOp::Split(at) => {
                    let t = song.timeline.as_deref_mut().unwrap();
                    let inside = |c: &tl::Clip| c.start < at && at < c.end();
                    let mut ids: Vec<u32> = t
                        .clips
                        .iter()
                        .filter(|c| sels.contains(&c.id) && inside(c))
                        .map(|c| c.id)
                        .collect();
                    if ids.is_empty() {
                        // nothing selected there: the clip under the mouse
                        if let Some((tr, _)) = hover {
                            ids = t
                                .clips
                                .iter()
                                .filter(|c| c.track as usize == tr && inside(c))
                                .map(|c| c.id)
                                .collect();
                        }
                    }
                    let n = ids.iter().filter(|id| tl::split(t, **id, at).is_some()).count();
                    let p = t.bar_pos(at);
                    if n == 0 {
                        "Nothing to split here: put the mouse (or the play marker) inside a clip".into()
                    } else {
                        format!("Split {n} clip{} at bar {}.{}", plural(n), p.bar, p.beat)
                    }
                }
                TlOp::Duplicate => {
                    let t = song.timeline.as_deref_mut().unwrap();
                    let ids = tl::duplicate(t, &sels);
                    let n = ids.len();
                    new_sel = Some(ids);
                    format!("Duplicated {n} clip{} (linked: same blocks · Ctrl+Shift+D makes one unique)", plural(n))
                }
                TlOp::Delete => {
                    let t = song.timeline.as_deref_mut().unwrap();
                    tl::delete(t, &sels);
                    new_sel = Some(vec![]);
                    format!("Deleted {} clip{} (the blocks stay)", sels.len(), plural(sels.len()))
                }
                TlOp::Mute => {
                    let t = song.timeline.as_deref_mut().unwrap();
                    tl::toggle_mute(t, &sels);
                    let on = t.clips.iter().any(|c| sels.contains(&c.id) && c.muted);
                    format!("{} {} clip{}", if on { "Muted" } else { "Unmuted" }, sels.len(), plural(sels.len()))
                }
                TlOp::MakeUnique if sels.len() == 1 && tl::make_unique_notes(song.timeline.as_deref_mut().unwrap(), sels[0]).is_some() => {
                    "The note clip now has its own notes (edits no longer change its copies)".into()
                }
                TlOp::MakeUnique => {
                    let mut made = vec![];
                    for &id in &sels {
                        if tl::make_unique_notes(song.timeline.as_deref_mut().unwrap(), id).is_some() {
                            continue;
                        }
                        let audio = song.timeline.as_deref().unwrap().clips.iter().any(|c| c.id == id && c.audio().is_some());
                        if audio {
                            continue;
                        }
                        match tl::make_unique(song, id) {
                            Some(b) => made.push(b),
                            None => break,
                        }
                    }
                    match (made.first(), made.len()) {
                        (None, _) => "Make unique: no room for another block".into(),
                        (Some(b), 1) => format!(
                            "The clip now plays its own block {b:02} {} (edits no longer change its copies)",
                            song.patterns[*b].name
                        ),
                        (_, n) => format!("{n} clips now play their own blocks"),
                    }
                }
                TlOp::Glue => match tl::glue(song, &sels) {
                    Ok(id) => {
                        new_sel = Some(vec![id]);
                        let b = song.patterns.len() - 1;
                        format!("Glued into block {b:02} {}", song.patterns[b].name)
                    }
                    Err(e) => format!("Can't glue: {e}"),
                },
                TlOp::LoopFromSelection => {
                    let t = song.timeline.as_deref_mut().unwrap();
                    let sel = t.clips.iter().filter(|c| sels.contains(&c.id));
                    let a = sel.clone().map(|c| c.start).min().unwrap_or(0);
                    let b = sel.map(|c| c.end()).max().unwrap_or(0);
                    tl::set_loop(t, a, b);
                    let (pa, pb) = (t.bar_pos(a), t.bar_pos(b));
                    format!("Loop on: bar {}.{} to {}.{} (L toggles it)", pa.bar, pa.beat, pb.bar, pb.beat)
                }
                TlOp::ToggleLoop => {
                    let t = song.timeline.as_deref_mut().unwrap();
                    if t.loop_range.is_none() {
                        "No loop region yet: Shift-drag the ruler, or select clips and press Ctrl+L".into()
                    } else {
                        t.loop_on = !t.loop_on;
                        format!("Loop {}", if t.loop_on { "on" } else { "off" })
                    }
                }
                TlOp::AddMarker(at) => {
                    let t = song.timeline.as_deref_mut().unwrap();
                    let i = tl::add_marker(t, at);
                    rename = Some(i);
                    let p = t.bar_pos(at);
                    format!("Marker added at bar {}.{}", p.bar, p.beat)
                }
                TlOp::Capture(..) if captured.is_empty() => {
                    "Nothing played on the MIDI keyboard to capture".into()
                }
                TlOp::Capture(tr, at) => match tl::new_note_clip(song, tr, at) {
                    Some(id) => {
                        let t = song.timeline.as_deref_mut().unwrap();
                        let i = t.clips.iter().position(|c| c.id == id).unwrap();
                        if let tl::ClipKind::Notes { data } = t.clips[i].kind {
                            let d = t.notes[data as usize].make_mut();
                            d.name = format!("Capture {}", data + 1);
                            d.notes = captured.clone();
                            let bar = 4 * PPQ;
                            d.len = (d.fit_len() + bar - 1) / bar * bar;
                            t.clips[i].len = d.len;
                        }
                        new_sel = Some(vec![id]);
                        open_roll = true;
                        format!("Captured {} notes into a note clip on track {:02}", captured.len(), tr + 1)
                    }
                    None => "No track there".into(),
                },
                TlOp::NewNotes(tr, at) => match tl::new_note_clip(song, tr, at) {
                    Some(id) => {
                        new_sel = Some(vec![id]);
                        open_roll = true;
                        format!("New note clip on track {:02}: draw notes in the piano roll (lower panel)", tr + 1)
                    }
                    None => "No track there".into(),
                },
                TlOp::NewClip(tr, at) => match tl::new_clip(song, tr, at) {
                    Some((b, id)) => {
                        new_sel = Some(vec![id]);
                        open = Some((b, tr));
                        String::new()
                    }
                    None => "No room for another block".into(),
                },
                TlOp::Speed(tr, v) => {
                    let t = song.timeline.as_deref_mut().unwrap();
                    if let Some(x) = t.tracks.get_mut(tr) {
                        x.speed = v;
                    }
                    if v == 0 {
                        format!("Track {:02} follows the song speed", tr + 1)
                    } else {
                        format!("Track {:02} speed {v} (its effects step {v} times a row)", tr + 1)
                    }
                }
                TlOp::DrumPattern { .. } => String::new(),
                TlOp::PlaceBlock {
                    block,
                    col,
                    track,
                    at,
                } => {
                    let (b, c, tr) = (block as usize, col as usize, track as usize);
                    match tl::place_block(song, tr, at, b, c) {
                        Some(id) => {
                            new_sel = Some(vec![id]);
                            let name = song.patterns.get(b).map_or(String::new(), |p| p.name.clone());
                            let bar = song.timeline.as_deref().map_or(1, |t| t.bar_pos(at).bar);
                            format!(
                                "Tracker clip: block {b:02} {name}, column {} on track {:02} at bar {bar}",
                                c + 1,
                                tr + 1
                            )
                        }
                        None => "No track there for that clip".into(),
                    }
                }
                TlOp::PlaceAudio => {
                    let (msg, ids) = App::place_ready_audio(song, ready);
                    if !ids.is_empty() {
                        new_sel = Some(ids);
                    }
                    msg
                }
                TlOp::SelectAll => {
                    let t = song.timeline.as_deref().unwrap();
                    new_sel = Some(t.clips.iter().map(|c| c.id).collect());
                    format!("{} clips selected", t.clips.len())
                }
            };
            let Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            msg
        });
        if let Some(ids) = new_sel {
            self.tlv.sel = ids.last().copied();
            self.tlv.sels = ids;
        }
        if let Some(i) = rename {
            let name = self.with(|s| {
                s.song
                    .timeline
                    .as_deref()
                    .and_then(|t| t.markers.get(i))
                    .map(|m| m.name.clone())
            });
            self.tlv.rename = name.map(|n| (i, n));
        }
        self.dirty = true;
        if open_roll {
            self.set_lower(3);
        }
        if !msg.is_empty() {
            self.status = msg;
        }
        if let Some((b, tr)) = open {
            self.timeline_open(b, tr, 0, usize::MAX);
            self.status = format!(
                "New block {b:02} on track {:02}: write it here (Ctrl+T: back to the timeline)",
                tr + 1
            );
        }
    }

    /// switch this song to timeline mode (one undo step)
    pub(crate) fn enter_timeline(&mut self) {
        self.with(|s| {
            crate::timeline::enter_timeline_mode(&mut s.song);
            let Shared { song, engine, .. } = s;
            engine.stop();
            engine.ensure_plan(song);
        });
        self.tlv.cue = self
            .tlv
            .cache
            .as_ref()
            .map_or(0, |c| c.map.tick_of(self.cue.0, self.cue.1))
            / ROW
            * ROW;
        self.dirty = true;
        self.status =
            "Timeline mode: the song is arranged with clips now (Ctrl+Z switches back)".into();
    }

    /// back to tracker mode: the timeline becomes blocks and an order list
    pub(crate) fn leave_timeline(&mut self) {
        let r = self.with(|s| {
            let r = crate::timeline::leave_timeline_mode(&mut s.song);
            let Shared { song, engine, .. } = s;
            if r.is_ok() {
                engine.stop();
            }
            engine.ensure_plan(song);
            r
        });
        self.tlv.sels.clear();
        self.tlv.sel = None;
        self.dirty = true;
        self.status = match r {
            Ok(()) => {
                self.cue = (0, 0);
                let n = self.order_len();
                format!(
                    "Tracker mode: the timeline is now {n} order entries (Ctrl+Z switches back)"
                )
            }
            Err(e) => format!("Still in timeline mode: {e}"),
        };
    }

    /// the "switch to timeline mode?" / "back to tracker mode?" prompts
    pub(crate) fn timeline_prompts(&mut self, ctx: &egui::Context) {
        if self.tlv.ask_switch.is_some() {
            let (mut yes, mut no) = (false, false);
            egui::Window::new("Switch to timeline mode?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_max_width(440.0);
                    ui.label(
                        "This song is in tracker mode: its order list decides what plays. \
                         To move, loop, split and copy clips on the timeline, switch it to \
                         timeline mode. It plays exactly the same.",
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "Saved in timeline mode, ChipWave Studio 0.2.4.0 and older can't open the song \
                             (File > Export tracker-compatible copy makes one they can). \
                             Undo (Ctrl+Z) puts it back in tracker mode.",
                        )
                        .small(),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        yes = ui.button("Switch to timeline mode").clicked();
                        no = ui.button("Not now").clicked();
                    });
                });
            if yes || no {
                self.answer_switch(yes);
            }
        }
        if self.tlv.ask_leave {
            let (mut yes, mut no) = (false, false);
            egui::Window::new("Back to tracker mode?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_max_width(440.0);
                    ui.label(
                        "The timeline is turned into blocks and an order list that play the \
                         same (overlapping or partial clips become new blocks). Markers, the loop \
                         region and track speeds are dropped. Undo (Ctrl+Z) puts the timeline back.",
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        yes = ui.button("Back to tracker mode").clicked();
                        no = ui.button("Cancel").clicked();
                    });
                });
            if yes || no {
                self.answer_leave(yes);
            }
        }
    }

    /// "switch to timeline mode?" answered (buttons, Enter / Esc)
    pub(crate) fn answer_switch(&mut self, yes: bool) {
        let Some(op) = self.tlv.ask_switch.take() else {
            return;
        };
        if yes {
            self.enter_timeline();
            // its own undo step, then the edit that asked
            self.hist_boundary_now();
            if let Some(op) = op {
                if op != TlOp::SelectAll {
                    self.tl_apply(op);
                }
            }
        } else {
            self.status = "Still in tracker mode (the timeline only shows the order list)".into();
        }
    }

    /// "back to tracker mode?" answered
    pub(crate) fn answer_leave(&mut self, yes: bool) {
        self.tlv.ask_leave = false;
        if yes {
            self.leave_timeline();
        }
    }

    /// end the current undo step here (the mode switch and the edit that
    /// asked for it undo separately)
    fn hist_boundary_now(&mut self) {
        let shared = self.shared.clone();
        let s = audio::lock(&shared);
        self.hist.commit(&s.song, std::time::Instant::now(), false);
    }

    /// marker rename popup
    pub(super) fn timeline_rename(&mut self, ctx: &egui::Context) {
        let Some((i, mut text)) = self.tlv.rename.take() else {
            return;
        };
        let (mut ok, mut cancel) = (false, false);
        egui::Window::new("Rename marker")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -80.0])
            .show(ctx, |ui| {
                let r = ui.text_edit_singleline(&mut text);
                r.request_focus();
                ok = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.horizontal(|ui| {
                    ok |= ui.button("OK").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        cancel |= ctx.input(|i| i.key_pressed(Key::Escape));
        if ok {
            let name = text.trim().to_string();
            self.with(|s| {
                if let Some(m) = s
                    .song
                    .timeline
                    .as_deref_mut()
                    .and_then(|t| t.markers.get_mut(i))
                {
                    if !name.is_empty() {
                        m.name = name;
                    }
                }
            });
            self.dirty = true;
        } else if !cancel {
            self.tlv.rename = Some((i, text));
        }
    }

    /// File > Export tracker-compatible copy…
    pub(crate) fn export_tracker_copy(&mut self) {
        let copy = self.with(|s| crate::timeline::tracker_copy(&s.song));
        let copy = match copy {
            Ok(c) => c,
            Err(e) => {
                self.status = format!("Can't make a tracker copy: {e}");
                return;
            }
        };
        let stem = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "song".into());
        let Some(p) = rfd::FileDialog::new()
            .set_title("Export tracker-compatible copy")
            .add_filter("ChipWave song", &["cwt"])
            .set_file_name(format!("{stem} (tracker).cwt"))
            .save_file()
        else {
            return;
        };
        let p = if p.extension().is_none() {
            p.with_extension("cwt")
        } else {
            p
        };
        self.status = match crate::fileio::save_cwt(&copy, &p) {
            Ok(()) => format!(
                "Saved a tracker-mode copy (opens in ChipWave 0.2.4 and older): {}",
                p.display()
            ),
            Err(e) => format!("Export failed: {e}"),
        };
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// status line while dragging
fn describe(t: &tl::Timeline, e: DragEdit) -> String {
    let at = |x: Tick| {
        let p = t.bar_pos(x);
        format!("{}.{}.{}", p.bar, p.beat, p.tick / ROW + 1)
    };
    match e {
        DragEdit::Move(dt, dtr) => format!(
            "Move {}{} rows{}",
            if dt >= 0 { "+" } else { "-" },
            dt.abs() / ROW,
            if dtr != 0 {
                format!(" · {dtr:+} tracks")
            } else {
                String::new()
            }
        ),
        DragEdit::End(_, x) => format!("Clip end: {}", at(x)),
        DragEdit::Start(_, x) => format!("Clip start: {}", at(x)),
        DragEdit::Marker(_, x) => format!("Marker: {}", at(x)),
        DragEdit::Loop(a, b) => format!("Loop: {} to {}", at(a.min(b)), at(a.max(b))),
        DragEdit::Fade(id, inn, _) => {
            let c = t.clips.iter().find(|c| c.id == id);
            let len = c.map_or(0, |c| if inn { c.fade_in } else { c.fade_out });
            format!(
                "Fade {}: {:.2} beats{}",
                if inn { "in" } else { "out" },
                len as f64 / PPQ as f64,
                if len == 0 { " (off)" } else { "" }
            )
        }
    }
}
