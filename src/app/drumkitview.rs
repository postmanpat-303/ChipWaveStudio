//! The Drum Kit editor (lower panel › Instrument on a Drum Kit; layout A of
//! /workspace/shots/drumkit/mock-a.png): Kit, Pattern and Groove cards, the
//! pad grid (click = hear and select), the step sequencer over the selected
//! note clip's notes (a pattern is a note clip: placed copies are linked,
//! Make unique splits one off), the velocity lane and the lower Sound /
//! Generate tabs. Play loops the song transport over the pattern's clip.
use super::*;
use crate::drumkit::{self as dk, DrumKitP, DrumMeta};
use crate::gen::{DrumKind, DrumP, GenEngine, MacroDef};
use crate::timeline::{self as tl, Note, Tick, ROW};

/// editor state that is not part of the song
#[derive(Default)]
pub(crate) struct DkUi {
    /// the instrument it belongs to (0-based)
    pub inst: usize,
    /// selected pad
    pub sel: usize,
    /// sequencer page (16 steps each)
    pub page: usize,
    /// lower tab: 0 Sound, 1 Generate
    pub tab: u8,
    /// the clip the sequencer edits
    pub clip: Option<u32>,
    /// drag-paint: the state the first cell was set to
    paint: Option<bool>,
    /// drawn step cells (pad, step, rect) and pads (pad, rect), for tests
    pub cells: Vec<(usize, usize, egui::Rect)>,
    pub pads: Vec<(usize, egui::Rect)>,
    /// audition key counter (live keys 200..=215)
    akey: u8,
    pub gen: GenSet,
}

pub(crate) struct GenSet {
    pub style: usize,
    pub seed: u32,
    pub energy: f32,
    pub fill: u8,
    /// the last result: notes, steps, swing, source pattern name
    pub result: Option<(Vec<Note>, usize, f32, &'static str)>,
    pub previewing: bool,
}

impl Default for GenSet {
    fn default() -> Self {
        GenSet {
            style: 0,
            seed: 1,
            energy: 0.5,
            fill: 0,
            result: None,
            previewing: false,
        }
    }
}

/// one pattern of the kit: note data `data`, placed `uses` times
#[derive(Clone)]
struct PatRow {
    data: usize,
    name: String,
    uses: usize,
    clip: u32,
}

/// the clip being edited
#[derive(Clone)]
struct Cur {
    id: u32,
    data: usize,
    track: usize,
    start: Tick,
    len: Tick,
    d: tl::NoteClipData,
}

const fn mix(
    name: &'static str,
    lo: f32,
    hi: f32,
    exp: bool,
    unit: &'static str,
    mid: f32,
    tip: &'static str,
) -> MacroDef {
    MacroDef {
        name,
        lo,
        hi,
        exp,
        unit,
        dice: (mid, mid),
        tip,
    }
}
const MIX_LEVEL: MacroDef = mix("Level", 0.0, 1.5, false, "%", 0.667, "pad level");
const MIX_PAN: MacroDef = mix("Pan", -1.0, 1.0, false, "v", 0.5, "left -1 .. right +1");
const MIX_TUNE: MacroDef = mix(
    "Tune",
    -24.0,
    24.0,
    false,
    "st",
    0.5,
    "semitones, on top of the drum's own Tune",
);
const MIX_ATTACK: MacroDef = mix(
    "Attack",
    0.0,
    0.1,
    false,
    "s",
    0.0,
    "a fade-in (0 = the drum's own attack)",
);
const MIX_DECAY: MacroDef = mix(
    "Decay",
    0.1,
    4.0,
    true,
    "x",
    0.6242,
    "scales the drum's Decay (1.00x = as designed)",
);
const MIX_VEL: MacroDef = mix(
    "Vel",
    0.0,
    1.0,
    false,
    "%",
    1.0,
    "how much the note velocity sets the level (100 % = fully)",
);
const ENERGY: MacroDef = mix(
    "Energy",
    0.0,
    1.0,
    false,
    "%",
    0.5,
    "low: sparse (fewer off-beat hats and ghosts); high: busier (16th hats, ghost snares, kick pickups)",
);

fn kind_col(k: DrumKind) -> Color32 {
    let t = th();
    match k {
        DrumKind::Kick => t.accent,
        DrumKind::Snare => t.ochre,
        DrumKind::Clap => Color32::from_rgb(0x9a, 0x7a, 0xc8),
        DrumKind::Hat => t.sage,
        DrumKind::Tom => Color32::from_rgb(0x6a, 0xb0, 0x5a),
        DrumKind::Perc => t.brick,
    }
}

fn pad_col(p: &dk::Pad) -> Color32 {
    if p.colour != 0 {
        let c = p.colour;
        Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
    } else {
        kind_col(p.drum.kind)
    }
}

/// a section card with a caption, filling `r`
fn card(ui: &egui::Ui, r: egui::Rect, caption: &str) {
    let p = ui.painter();
    crate::ember::card(p, r);
    p.text(
        r.left_top() + egui::vec2(16.0, 7.0),
        egui::Align2::LEFT_TOP,
        caption.to_uppercase(),
        egui::FontId::proportional(10.0),
        th().ink2,
    );
}

/// a pad-mix knob over a real value: `def` maps it to 0..1
fn mix_knob(ui: &mut egui::Ui, v: &mut f32, def: &'static MacroDef, id: &str) -> bool {
    let mut n = def.unmap(*v);
    let mut lock = false;
    let ch = super::genview::knob(ui, &mut n, def, &mut lock, ("dkmix", id.to_string()));
    if ch {
        *v = def.map(n);
    }
    ch
}

/// an area at `r` for widgets
fn area<R>(ui: &mut egui::Ui, r: egui::Rect, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope_builder(egui::UiBuilder::new().max_rect(r), f)
        .inner
}

impl App {
    /// the Drum Kit editor for instrument `idx` (0-based)
    pub(super) fn drumkit_view(&mut self, ui: &mut egui::Ui, idx: usize) {
        let slot = idx as u8 + 1;
        let Some((name, mut kit, tlon)) = self.with(|s| {
            let i = s.song.instruments.get(idx)?;
            Some((
                i.name.clone(),
                dk::kit_of(i)?.clone(),
                s.song.timeline.is_some(),
            ))
        }) else {
            return;
        };
        if self.dk.inst != idx {
            self.dk = DkUi {
                inst: idx,
                ..Default::default()
            };
        }
        self.dk.sel = self.dk.sel.min(kit.pads.len().saturating_sub(1));
        let orig = kit.clone();
        let (pats, cur, play_at, playing) = self.dk_patterns(slot);
        self.dk.clip = cur.as_ref().map(|c| c.id);
        let mut edit: Option<tl::NoteClipData> = None;
        self.dk.cells.clear();
        self.dk.pads.clear();
        egui::ScrollArea::vertical()
            .id_salt("dk_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                self.dk_header(ui, &name, &mut kit, &pats, cur.as_ref(), tlon);
                ui.add_space(6.0);
                let ch = cur.as_ref().map_or(self.ch, |c| c.track);
                self.dk_pads(ui, &mut kit, ch);
                ui.add_space(6.0);
                if let Some(c) = &cur {
                    let mut d = c.d.clone();
                    self.dk_sequencer(ui, &mut kit, c, &mut d, play_at, playing, slot);
                    if d != c.d {
                        edit = Some(d);
                    }
                } else {
                    let (r, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 70.0),
                        egui::Sense::hover(),
                    );
                    card(ui, r, "Step sequencer");
                    ui.painter().text(
                        r.center(),
                        egui::Align2::CENTER_CENTER,
                        if tlon {
                            "No pattern yet: + New in the Pattern card makes one on this kit's track"
                        } else {
                            "The step sequencer edits note clips: switch the song to timeline mode first"
                        },
                        egui::FontId::proportional(13.0),
                        th().ink2,
                    );
                }
                ui.add_space(6.0);
                self.dk_lower(ui, &mut kit, cur.as_ref(), slot);
                ui.add_space(6.0);
            });
        if kit != orig {
            kit.fix();
            self.with(|s| {
                if let Some(InstrumentKind::Gen(g)) =
                    s.song.instruments.get_mut(idx).map(|i| &mut i.kind)
                {
                    g.engine = GenEngine::Drums(kit);
                }
            });
            self.dirty = true;
        }
        if let (Some(d), Some(c)) = (edit, cur) {
            self.dk_write(c.data, d);
        }
        if ui.input(|i| i.pointer.any_released()) {
            self.dk.paint = None;
        }
    }

    /// the kit's patterns (note data on clips that play it) and the clip to edit
    fn dk_patterns(&self, slot: u8) -> (Vec<PatRow>, Option<Cur>, Option<f64>, bool) {
        let want = self.dk.clip.or(self.tlv.sel);
        self.with(|s| {
            let playing = s.engine.playing;
            let at = s.engine.tl_ticks();
            let Some(t) = s.song.timeline.as_deref() else {
                return (vec![], None, at, playing);
            };
            let mut pats: Vec<PatRow> = vec![];
            let mut cur = None;
            let mk = |c: &tl::Clip, data: usize| Cur {
                id: c.id,
                data,
                track: c.track as usize,
                start: c.start,
                len: c.len,
                d: (*t.notes[data]).clone(),
            };
            for c in &t.clips {
                let tl::ClipKind::Notes { data } = c.kind else {
                    continue;
                };
                let Some(d) = t.notes.get(data as usize) else {
                    continue;
                };
                let track_kit = t
                    .tracks
                    .get(c.track as usize)
                    .is_some_and(|x| x.instr == slot);
                let plays = d.notes.iter().any(|n| n.instr == slot)
                    || (track_kit && d.notes.iter().all(|n| n.instr == 0 || n.instr == slot));
                if !plays {
                    continue;
                }
                match pats.iter_mut().find(|p| p.data == data as usize) {
                    Some(p) => p.uses += 1,
                    None => pats.push(PatRow {
                        data: data as usize,
                        name: d.name.clone(),
                        uses: 1,
                        clip: c.id,
                    }),
                }
                if cur.is_none() && want == Some(c.id) {
                    cur = Some(mk(c, data as usize));
                }
            }
            if cur.is_none() {
                if let Some(p) = pats.first() {
                    if let Some(c) = t.clips.iter().find(|c| c.id == p.clip) {
                        cur = Some(mk(c, p.data));
                    }
                }
            }
            (pats, cur, at, playing)
        })
    }

    /// write note data `data`; the gesture is one undo step (undo_commit)
    fn dk_write(&mut self, data: usize, d: tl::NoteClipData) {
        self.with(|s| {
            if let Some(t) = s.song.timeline.as_deref_mut() {
                let new_len = d.len;
                let old_len = t.notes.get(data).map_or(0, |x| x.len);
                if let Some(x) = t.notes.get_mut(data) {
                    *x.make_mut() = d;
                }
                // an untrimmed, unlooped clip follows its pattern's length
                if new_len != old_len {
                    for c in t.clips.iter_mut() {
                        let mine =
                            matches!(c.kind, tl::ClipKind::Notes { data: k } if k as usize == data);
                        if mine && c.loop_len.is_none() && c.offset == 0 && c.len == old_len {
                            c.len = new_len;
                        }
                    }
                }
            }
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
        });
        self.dirty = true;
    }

    /// hear pad `p` through track `ch`'s FX, as a live note (no note-off:
    /// the drum ends by itself)
    fn dk_audition(&mut self, kit: &DrumKitP, p: usize, ch: usize, vel: f32) {
        let Some(pad) = kit.pads.get(p) else { return };
        let note = pad.key;
        let ins = self.dk.inst as u8 + 1;
        self.dk.akey = (self.dk.akey + 1) % 16;
        let key = 200 + self.dk.akey;
        self.with(|s| {
            let audio::Shared { song, engine, .. } = s;
            engine.live_event(
                song,
                crate::midi::LiveEv::On {
                    key,
                    note,
                    vel,
                    instr: ins,
                    ch: ch.min(255) as u8,
                },
            );
        });
    }

    /// the first track that was given this kit (1-based slot)
    fn dk_track(&self, slot: u8) -> Option<usize> {
        self.with(|s| {
            s.song
                .timeline
                .as_deref()?
                .tracks
                .iter()
                .position(|t| t.instr == slot)
        })
    }
}

// ------------------------------------------------------------------ header
impl App {
    fn dk_header(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        kit: &mut DrumKitP,
        pats: &[PatRow],
        cur: Option<&Cur>,
        tlon: bool,
    ) {
        let w = ui.available_width();
        let (r, _) = ui.allocate_exact_size(egui::vec2(w, 76.0), egui::Sense::hover());
        let gap = 8.0;
        let cw = [0.32, 0.42, 0.26].map(|f| (w - 2.0 * gap) * f);
        let r1 = egui::Rect::from_min_size(r.min, egui::vec2(cw[0], r.height()));
        let r2 = egui::Rect::from_min_size(
            egui::pos2(r1.right() + gap, r.top()),
            egui::vec2(cw[1], r.height()),
        );
        let r3 = egui::Rect::from_min_size(
            egui::pos2(r2.right() + gap, r.top()),
            egui::vec2(cw[2], r.height()),
        );
        let t = th();
        let big = |ui: &egui::Ui, r: egui::Rect, s: &str| {
            ui.painter().text(
                r.left_top() + egui::vec2(16.0, 24.0),
                egui::Align2::LEFT_TOP,
                s,
                egui::FontId::proportional(17.0),
                th().ink,
            );
        };
        let small = |ui: &egui::Ui, r: egui::Rect, s: &str| {
            ui.painter().text(
                r.left_top() + egui::vec2(16.0, 50.0),
                egui::Align2::LEFT_TOP,
                s,
                egui::FontId::proportional(11.0),
                th().ink2,
            );
        };
        // ---- Kit
        card(ui, r1, "Kit");
        big(ui, r1, name);
        let legacy = kit.keymap == dk::KeyMap::Legacy12;
        small(
            ui,
            r1,
            &format!(
                "{} pads · synth{}",
                kit.pads.len(),
                if legacy { " · old Kit note map" } else { "" }
            ),
        );
        let br = egui::Rect::from_min_max(
            egui::pos2(r1.right() - 200.0, r1.top() + 24.0),
            egui::pos2(r1.right() - 14.0, r1.top() + 48.0),
        );
        area(ui, br, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::ember::button(ui, "Dice kit", false)
                    .on_hover_text("re-roll every pad's unlocked knobs (Undo takes it back)")
                    .clicked()
                {
                    let seed = (ui.input(|i| i.time) * 1000.0) as u32 | 1;
                    for (i, p) in kit.pads.iter_mut().enumerate() {
                        crate::gen::roll(&mut p.drum, seed.wrapping_add(i as u32 * 0x9E37_79B9));
                    }
                }
                ui.menu_button("Kits ▾", |ui| {
                    ui.label(
                        egui::RichText::new(
                            "load a kit's sounds into the pads with the same keys (patterns stay)",
                        )
                        .small()
                        .color(th().ink2),
                    );
                    for (n, tip, p) in dk::presets() {
                        if ui.button(&n).on_hover_text(tip).clicked() {
                            if let GenEngine::Drums(k) = p.engine {
                                for pad in kit.pads.iter_mut() {
                                    if let Some(src) = k.pads.iter().find(|x| x.key == pad.key) {
                                        pad.drum = src.drum;
                                        pad.tune = src.tune;
                                    }
                                }
                            }
                            ui.close();
                        }
                    }
                });
            });
        });
        // ---- Pattern
        card(ui, r2, "Pattern");
        big(ui, r2, &cur.map_or("—".to_string(), |c| c.d.name.clone()));
        let sub = match cur {
            Some(c) => {
                let uses = pats.iter().find(|p| p.data == c.data).map_or(1, |p| p.uses);
                let bars = self.with(|s| {
                    s.song.timeline.as_deref().map(|t| {
                        let (a, b) = (t.bar_pos(c.start).bar, t.bar_pos(c.start + c.len - 1).bar);
                        if a == b {
                            format!(" · bar {a}")
                        } else {
                            format!(" · bars {a}–{b}")
                        }
                    })
                });
                let bars = bars.unwrap_or_default();
                if uses > 1 {
                    format!(
                        "linked ×{uses} on the timeline · track {:02}{bars}",
                        c.track + 1
                    )
                } else {
                    format!("track {:02}{bars}", c.track + 1)
                }
            }
            None if tlon => "no pattern yet".into(),
            None => "timeline mode only".into(),
        };
        small(ui, r2, &sub);
        let slot = self.dk.inst as u8 + 1;
        let br = egui::Rect::from_min_max(
            egui::pos2(r2.right() - 330.0, r2.top() + 6.0),
            egui::pos2(r2.right() - 14.0, r2.top() + 30.0),
        );
        area(ui, br, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::ember::button(ui, "Dup", false)
                    .on_hover_text("a copy of this pattern of its own, right after it")
                    .clicked()
                {
                    if let Some(c) = cur {
                        self.dk_new_pattern(c.track, Some(c));
                    }
                }
                if crate::ember::button(ui, "+ New", false)
                    .on_hover_text(
                        "an empty 1-bar pattern on this kit's track, after its last clip",
                    )
                    .clicked()
                {
                    let tr = cur
                        .map(|c| c.track)
                        .or_else(|| self.dk_track(slot))
                        .unwrap_or(self.ch);
                    self.dk_new_pattern(tr, None);
                }
                let sel = cur.map(|c| c.data);
                let at = pats
                    .iter()
                    .position(|p| Some(p.data) == sel)
                    .map_or(0, |i| i + 1);
                egui::ComboBox::from_id_salt("dk_pats")
                    .width(130.0)
                    .selected_text(format!("{at} / {} patterns", pats.len()))
                    .show_ui(ui, |ui| {
                        for p in pats {
                            let label = if p.uses > 1 {
                                format!("{}  ×{}", p.name, p.uses)
                            } else {
                                p.name.clone()
                            };
                            if ui.selectable_label(Some(p.data) == sel, label).clicked() {
                                self.dk.clip = Some(p.clip);
                                self.tlv.sel = Some(p.clip);
                                self.tlv.sels = vec![p.clip];
                            }
                        }
                    });
            });
        });
        if let Some(c) = cur {
            let steps = ((c.d.len + ROW - 1) / ROW).max(1) as usize;
            let br = egui::Rect::from_min_max(
                egui::pos2(r2.right() - 220.0, r2.top() + 42.0),
                egui::pos2(r2.right() - 14.0, r2.top() + 66.0),
            );
            area(ui, br, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for n in [64usize, 32, 16] {
                        if crate::ember::led_item(ui, &n.to_string(), steps == n)
                            .on_hover_text(format!(
                                "{n} steps ({} bar{})",
                                n / 16,
                                if n > 16 { "s" } else { "" }
                            ))
                            .clicked()
                            && steps != n
                        {
                            let mut d = c.d.clone();
                            d.len = n as Tick * ROW;
                            if n < steps {
                                d.notes.retain(|x| dk::step_of(x.start) < n);
                            }
                            self.dk_write(c.data, d);
                            self.dk.page = 0;
                        }
                    }
                    ui.label(egui::RichText::new("STEPS").small().color(th().ink2));
                });
            });
        }
        // ---- Groove
        card(ui, r3, "Groove");
        let swing = cur.and_then(|c| c.d.drum.as_ref()).map_or(0.0, |m| m.swing);
        big(ui, r3, &format!("Swing {:.0} %", dk::swing_percent(swing)));
        let bpm = self.with(|s| s.song.bpm);
        small(ui, r3, &format!("16ths · the song tempo ({bpm:.0} BPM)"));
        if let Some(c) = cur {
            let kr = egui::Rect::from_center_size(
                egui::pos2(r3.right() - 38.0, r3.center().y + 4.0),
                egui::vec2(40.0, 40.0),
            );
            let resp = ui
                .interact(kr, ui.make_persistent_id("dk_swing"), egui::Sense::click_and_drag())
                .on_hover_text(
                    "swing: drag up / down or scroll (50 % = straight, 66 % = triplet feel, 75 % max); the off-beat 16ths move; double-click = straight",
                );
            let mut sw = swing;
            if resp.dragged() {
                let d = resp.drag_delta();
                sw = (sw + (d.x - d.y) * 0.002).clamp(0.0, 0.5);
            }
            if resp.hovered() {
                let s = ui.input(|i| i.smooth_scroll_delta.y);
                if s != 0.0 {
                    sw = (sw + s * 0.0005).clamp(0.0, 0.5);
                }
            }
            if resp.double_clicked() {
                sw = 0.0;
            }
            crate::theme::knob_face(
                ui.painter(),
                kr.center(),
                16.0,
                crate::theme::KnobFace::new(sw * 2.0),
            );
            // whole percents, as shown
            let sw = ((dk::swing_percent(sw).round() - 50.0) / 50.0).clamp(0.0, 0.5);
            if (sw - swing).abs() > 1e-6 {
                let mut d = c.d.clone();
                dk::set_swing(&mut d.notes, sw);
                d.drum = Some(DrumMeta { swing: sw });
                self.dk_write(c.data, d);
            }
        }
        let _ = t;
    }

    /// a new pattern clip on `track`: a unique copy of `from`, or an empty
    /// bar, placed after the track's last clip; selects it
    fn dk_new_pattern(&mut self, track: usize, from: Option<&Cur>) {
        let slot = self.dk.inst as u8 + 1;
        let res = self.with(|s| {
            let song = &mut s.song;
            let t = song.timeline.as_deref()?;
            let bar = t.sig_at(0).bar_ticks().max(1);
            let end = t
                .clips
                .iter()
                .filter(|c| c.track as usize == track)
                .map(|c| c.end())
                .max()
                .unwrap_or(0);
            let at = from.map_or(end, |c| (c.start + c.len).max(end));
            let at = (at + bar - 1) / bar * bar;
            let n = t.notes.len();
            let id = tl::new_note_clip(song, track, at)?;
            let t = song.timeline.as_deref_mut()?;
            let k = t.clips.iter().position(|c| c.id == id)?;
            let d = match from {
                Some(c) => tl::NoteClipData {
                    name: format!("{} copy", c.d.name),
                    ..c.d.clone()
                },
                None => tl::NoteClipData {
                    name: format!("Pattern {}", n + 1),
                    len: 16 * ROW,
                    notes: vec![],
                    chords: vec![],
                    drum: Some(DrumMeta::default()),
                },
            };
            t.clips[k].len = d.len;
            if let tl::ClipKind::Notes { data } = t.clips[k].kind {
                *t.notes[data as usize].make_mut() = d;
            }
            if let Some(tr) = t.tracks.get_mut(track) {
                if tr.instr == 0 {
                    tr.instr = slot;
                }
            }
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some(id)
        });
        match res {
            Some(id) => {
                self.dk.clip = Some(id);
                self.tlv.sel = Some(id);
                self.tlv.sels = vec![id];
                self.dirty = true;
                self.status =
                    "New pattern: a note clip on the timeline (click the steps to add hits)".into();
            }
            None => {
                self.status =
                    "Can't add a pattern: the song needs timeline mode and a track".into();
            }
        }
    }
}

/// note `n` is pad `p` of `kit` at step `k`
fn on_pad(kit: &DrumKitP, n: &Note, p: usize) -> bool {
    kit.pad_for_key(n.key as i32).is_some_and(|(i, _)| i == p)
}
fn hit(kit: &DrumKitP, notes: &[Note], p: usize, k: usize) -> Option<usize> {
    notes
        .iter()
        .position(|n| dk::step_of(n.start) == k && on_pad(kit, n, p))
}

// -------------------------------------------------------------------- pads
impl App {
    fn dk_pads(&mut self, ui: &mut egui::Ui, kit: &mut DrumKitP, ch: usize) {
        let w = ui.available_width();
        let cols = 8usize;
        let n = kit.pads.len() + usize::from(kit.pads.len() < dk::MAX_PADS);
        let rows = n.div_ceil(cols).max(1);
        let ph = 50.0;
        let gap = 6.0;
        let h = 26.0 + rows as f32 * (ph + gap) + 6.0;
        let (r, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        card(
            ui,
            r,
            &format!("Pads · {} of {}", kit.pads.len(), dk::MAX_PADS),
        );
        ui.painter().text(
            egui::pos2(r.right() - 16.0, r.top() + 7.0),
            egui::Align2::RIGHT_TOP,
            "click = hear and select · right-click = more",
            egui::FontId::proportional(10.0),
            th().ink2,
        );
        let pw = (r.width() - 32.0 - gap * (cols - 1) as f32) / cols as f32;
        let mut act: Option<(u8, usize)> = None;
        for i in 0..n {
            let (cx, cy) = (i % cols, i / cols);
            let pr = egui::Rect::from_min_size(
                egui::pos2(
                    r.left() + 16.0 + cx as f32 * (pw + gap),
                    r.top() + 26.0 + cy as f32 * (ph + gap),
                ),
                egui::vec2(pw, ph),
            );
            let id = ui.make_persistent_id(("dk_pad", i));
            let resp = ui.interact(pr, id, egui::Sense::click());
            let p = ui.painter();
            if i == kit.pads.len() {
                crate::ember::well(p, pr, 5);
                p.text(
                    pr.center(),
                    egui::Align2::CENTER_CENTER,
                    "+ ADD PAD",
                    egui::FontId::proportional(12.0),
                    th().ink2,
                );
                if resp
                    .on_hover_text(
                        "a new pad on the next free key (a copy of the selected pad's sound)",
                    )
                    .clicked()
                {
                    act = Some((0, i));
                }
                continue;
            }
            self.dk.pads.push((i, pr));
            let pad = &kit.pads[i];
            let col = pad_col(pad);
            let sel = self.dk.sel == i;
            crate::ember::well(p, pr, 5);
            if sel {
                crate::ember::glow_rect(p, pr, 5, col);
                p.rect_stroke(
                    pr,
                    5.0,
                    egui::Stroke::new(1.5, col),
                    egui::StrokeKind::Inside,
                );
            }
            p.rect_filled(
                egui::Rect::from_min_size(
                    pr.min + egui::vec2(6.0, 8.0),
                    egui::vec2(3.0, ph - 16.0),
                ),
                1.5,
                if kit.audible(i) {
                    col
                } else {
                    col.gamma_multiply(0.3)
                },
            );
            p.text(
                pr.left_top() + egui::vec2(15.0, 8.0),
                egui::Align2::LEFT_TOP,
                &pad.name,
                egui::FontId::proportional(13.0),
                if kit.audible(i) { th().ink } else { th().ink2 },
            );
            let mut tag = dk::key_name(pad.key);
            if pad.choke > 0 {
                tag += &format!(" · choke {}", pad.choke);
            }
            if pad.mute {
                tag += " · M";
            }
            if pad.solo {
                tag += " · S";
            }
            p.text(
                pr.left_bottom() + egui::vec2(15.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                tag,
                egui::FontId::monospace(10.0),
                th().ink2,
            );
            let resp = resp.on_hover_text(format!(
                "{} · {:?} {} · key {}",
                pad.name,
                pad.drum.kind,
                pad.drum
                    .kind
                    .models()
                    .get(pad.drum.model as usize)
                    .unwrap_or(&""),
                dk::key_name(pad.key)
            ));
            if resp.clicked() {
                act = Some((1, i));
            }
            resp.context_menu(|ui| {
                if ui.button("Duplicate pad").clicked() {
                    act = Some((2, i));
                    ui.close();
                }
                if ui
                    .add_enabled(kit.pads.len() > 1, egui::Button::new("Remove pad"))
                    .on_hover_text("its hits stay in the patterns but play nothing")
                    .clicked()
                {
                    act = Some((3, i));
                    ui.close();
                }
            });
        }
        match act {
            Some((0, _)) | Some((2, _)) => {
                let from = match act {
                    Some((2, i)) => i,
                    _ => self.dk.sel,
                };
                if let Some(k) = kit.add_pad(Some(from)) {
                    self.dk.sel = k;
                }
            }
            Some((1, i)) => {
                self.dk.sel = i;
                self.dk_audition(kit, i, ch, 1.0);
            }
            Some((3, i)) if kit.pads.len() > 1 => {
                kit.pads.remove(i);
                self.dk.sel = self.dk.sel.min(kit.pads.len() - 1);
            }
            _ => {}
        }
    }
}

// --------------------------------------------------------------- sequencer
impl App {
    #[allow(clippy::too_many_arguments)]
    fn dk_sequencer(
        &mut self,
        ui: &mut egui::Ui,
        kit: &mut DrumKitP,
        c: &Cur,
        d: &mut tl::NoteClipData,
        play_at: Option<f64>,
        playing: bool,
        slot: u8,
    ) {
        let steps = ((d.len + ROW - 1) / ROW).max(1) as usize;
        let pages = steps.div_ceil(16);
        self.dk.page = self.dk.page.min(pages - 1);
        let swing = d.drum.as_ref().map_or(0.0, |m| m.swing);
        let w = ui.available_width();
        let lanes = kit.pads.len();
        let lh = 22.0;
        let top = 62.0;
        let vel_h = 46.0;
        let h = top + lanes as f32 * lh + 10.0 + vel_h + 14.0;
        let (r, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        card(
            ui,
            r,
            &format!("Step sequencer · {} · {steps} steps", d.name),
        );
        // transport + tools row
        let looping = playing
            && self.with(|s| {
                s.song.timeline.as_deref().is_some_and(|t| {
                    t.loop_on && t.loop_range == Some((c.start, c.start + c.len.max(ROW)))
                })
            });
        let row = egui::Rect::from_min_max(
            egui::pos2(r.left() + 14.0, r.top() + 24.0),
            egui::pos2(r.right() - 14.0, r.top() + 50.0),
        );
        let mut tool: Option<u8> = None;
        area(ui, row, |ui| {
            ui.horizontal_centered(|ui| {
                if crate::ember::button(ui, if looping { "■ Stop" } else { "▶ Play loop" }, looping)
                    .on_hover_text("loop the song over this pattern's clip and play it (the timeline's loop moves here)")
                    .clicked()
                {
                    tool = Some(if looping { 1 } else { 0 });
                }
                ui.add_space(8.0);
                if pages > 1 {
                    for pg in 0..pages {
                        if crate::ember::led_item(ui, &format!("{}–{}", pg * 16 + 1, pg * 16 + 16), self.dk.page == pg)
                            .clicked()
                        {
                            self.dk.page = pg;
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::ember::button(ui, "▶", false)
                        .on_hover_text("shift the selected lane one step later (wraps)")
                        .clicked()
                    {
                        tool = Some(5);
                    }
                    if crate::ember::button(ui, "◀", false)
                        .on_hover_text("shift the selected lane one step earlier (wraps)")
                        .clicked()
                    {
                        tool = Some(4);
                    }
                    if crate::ember::button(ui, "Clear lane", false).clicked() {
                        tool = Some(3);
                    }
                    let uses = self.with(|s| {
                        s.song.timeline.as_deref().map_or(1, |t| {
                            t.clips
                                .iter()
                                .filter(|x| matches!(x.kind, tl::ClipKind::Notes { data } if data as usize == c.data))
                                .count()
                        })
                    });
                    if ui
                        .add_enabled_ui(uses > 1, |ui| crate::ember::button(ui, "Make unique", false))
                        .inner
                        .on_hover_text("give this clip its own copy of the pattern (the other placed copies stay linked)")
                        .on_disabled_hover_text("only this clip plays this pattern")
                        .clicked()
                    {
                        tool = Some(6);
                    }
                    if crate::ember::button(ui, "Insert at marker", false)
                        .on_hover_text("place a linked copy of this pattern at the timeline marker on this track (edits change every copy; Make unique splits one off)")
                        .clicked()
                    {
                        tool = Some(2);
                    }
                });
            });
        });
        // grid
        let names_w = 150.0;
        let ms_w = 44.0;
        let gx = r.left() + 16.0 + names_w + ms_w;
        let gw = r.right() - 16.0 - gx;
        let sw = gw / 16.0;
        let gy = r.top() + top;
        let first = self.dk.page * 16;
        let shown = (steps - first).min(16);
        let cur_step = play_at.and_then(|t| {
            let rel = t - c.start as f64;
            if playing && rel >= 0.0 && rel < c.len as f64 {
                let lenp = d.len.max(1) as f64;
                Some(((rel % lenp) / ROW as f64) as usize)
            } else {
                None
            }
        });
        let mut toggles: Vec<(usize, usize, bool)> = vec![];
        let ptr = ui.input(|i| i.pointer.interact_pos());
        let down = ui.input(|i| i.pointer.primary_down());
        for (p, pad) in kit.pads.iter_mut().enumerate() {
            let y = gy + p as f32 * lh;
            let lane = egui::Rect::from_min_size(
                egui::pos2(r.left() + 16.0, y),
                egui::vec2(names_w, lh - 2.0),
            );
            let col = pad_col(pad);
            let sel = self.dk.sel == p;
            let lr = ui.interact(
                lane,
                ui.make_persistent_id(("dk_lane", p)),
                egui::Sense::click(),
            );
            if lr.clicked() {
                self.dk.sel = p;
            }
            let painter = ui.painter();
            if sel {
                painter.rect_filled(lane, 3.0, col.gamma_multiply(0.18));
            }
            painter.rect_filled(
                egui::Rect::from_min_size(
                    lane.min + egui::vec2(2.0, 4.0),
                    egui::vec2(3.0, lh - 10.0),
                ),
                1.0,
                col,
            );
            painter.text(
                lane.left_center() + egui::vec2(12.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &pad.name,
                egui::FontId::proportional(12.0),
                if sel { th().ink } else { th().ink2 },
            );
            // M / S
            for (j, (lbl, on)) in [("M", pad.mute), ("S", pad.solo)].into_iter().enumerate() {
                let br = egui::Rect::from_min_size(
                    egui::pos2(r.left() + 16.0 + names_w + j as f32 * 21.0, y + 1.0),
                    egui::vec2(19.0, lh - 4.0),
                );
                let rr = ui
                    .interact(
                        br,
                        ui.make_persistent_id(("dk_ms", p, j)),
                        egui::Sense::click(),
                    )
                    .on_hover_text(if j == 0 {
                        "mute this pad"
                    } else {
                        "solo this pad"
                    });
                let pc = ui.painter();
                crate::ember::well(pc, br, 3);
                pc.text(
                    br.center(),
                    egui::Align2::CENTER_CENTER,
                    lbl,
                    egui::FontId::monospace(10.0),
                    if on {
                        if j == 0 {
                            th().brick
                        } else {
                            th().ochre
                        }
                    } else {
                        th().ink2
                    },
                );
                if rr.clicked() {
                    if j == 0 {
                        pad.mute = !pad.mute;
                    } else {
                        pad.solo = !pad.solo;
                    }
                }
            }
        }
        for p in 0..lanes {
            let y = gy + p as f32 * lh;
            let col = pad_col(&kit.pads[p]);
            for s in 0..16 {
                let k = first + s;
                let cr = egui::Rect::from_min_size(
                    egui::pos2(gx + s as f32 * sw + 1.5, y + 1.0),
                    egui::vec2(sw - 3.0, lh - 4.0),
                );
                if s >= shown {
                    continue;
                }
                self.dk.cells.push((p, k, cr));
                let on = hit(kit, &d.notes, p, k);
                let painter = ui.painter();
                let beat = (k / 4).is_multiple_of(2);
                painter.rect_filled(
                    cr,
                    3.0,
                    if beat {
                        Color32::from_white_alpha(10)
                    } else {
                        Color32::from_white_alpha(4)
                    },
                );
                if let Some(ni) = on {
                    let v = d.notes[ni].vel as f32 / 65535.0;
                    let fill = col.gamma_multiply(0.45 + 0.55 * v);
                    painter.rect_filled(cr.shrink(1.0), 3.0, fill);
                    crate::ember::glow_rect(painter, cr, 3, col.gamma_multiply(0.5 * v));
                }
                if cur_step == Some(k) {
                    painter.rect_stroke(
                        cr,
                        3.0,
                        egui::Stroke::new(1.5, th().ink),
                        egui::StrokeKind::Outside,
                    );
                }
                let id = ui.make_persistent_id(("dk_cell", p, k));
                let resp = ui.interact(cr, id, egui::Sense::click_and_drag());
                if resp.drag_started() || resp.clicked() && self.dk.paint.is_none() {
                    let want = on.is_none();
                    self.dk.paint = Some(want);
                    self.dk.sel = p;
                    toggles.push((p, k, want));
                    if resp.clicked() {
                        self.dk.paint = None;
                    }
                } else if let (Some(want), Some(pos), true) = (self.dk.paint, ptr, down) {
                    if cr.contains(pos) && on.is_some() != want {
                        toggles.push((p, k, want));
                    }
                }
            }
        }
        // bar / beat numbers
        let painter = ui.painter();
        for s in 0..shown {
            let k = first + s;
            if k.is_multiple_of(4) {
                painter.text(
                    egui::pos2(gx + s as f32 * sw + 3.0, gy - 12.0),
                    egui::Align2::LEFT_CENTER,
                    format!("{}.{}", k / 16 + 1, (k % 16) / 4 + 1),
                    egui::FontId::monospace(9.5),
                    th().ink2,
                );
            }
        }
        // velocity lane for the selected pad
        let vy = gy + lanes as f32 * lh + 10.0;
        let vr = egui::Rect::from_min_size(egui::pos2(gx, vy), egui::vec2(gw, vel_h));
        crate::ember::well(ui.painter(), vr, 4);
        ui.painter().text(
            egui::pos2(r.left() + 28.0, vy + vel_h / 2.0),
            egui::Align2::LEFT_CENTER,
            format!(
                "Velocity · {}",
                kit.pads.get(self.dk.sel).map_or("", |p| p.name.as_str())
            ),
            egui::FontId::proportional(11.0),
            th().ink2,
        );
        let sp = self.dk.sel;
        let vresp = ui.interact(
            vr,
            ui.make_persistent_id("dk_vel"),
            egui::Sense::click_and_drag(),
        );
        let mut vel_set: Option<(usize, u16)> = None;
        if let (true, Some(pos)) = (
            vresp.dragged() || vresp.clicked(),
            vresp.interact_pointer_pos(),
        ) {
            let s = ((pos.x - gx) / sw).floor().clamp(0.0, 15.0) as usize;
            let v = ((vr.bottom() - pos.y) / vel_h).clamp(0.05, 1.0);
            vel_set = Some((first + s, (v * 65535.0) as u16));
        }
        let col = kit.pads.get(sp).map_or(th().accent, pad_col);
        for s in 0..shown {
            if let Some(ni) = hit(kit, &d.notes, sp, first + s) {
                let v = d.notes[ni].vel as f32 / 65535.0;
                let br = egui::Rect::from_min_max(
                    egui::pos2(
                        gx + s as f32 * sw + sw * 0.3,
                        vr.bottom() - 3.0 - (vel_h - 6.0) * v,
                    ),
                    egui::pos2(gx + s as f32 * sw + sw * 0.7, vr.bottom() - 3.0),
                );
                ui.painter().rect_filled(br, 2.0, col);
            }
        }
        // apply edits
        let ins = if self.dk_track(slot) == Some(c.track) {
            0
        } else {
            slot
        };
        let mut heard = false;
        for (p, k, want) in toggles {
            let Some(key) = kit.pads.get(p).map(|x| x.key) else {
                continue;
            };
            if want {
                if hit(kit, &d.notes, p, k).is_none() {
                    dk::toggle(&mut d.notes, key, k, true, swing, tl::vel_from_7(100), ins);
                    if !heard && !playing {
                        heard = true;
                        self.dk_audition(kit, p, c.track, 100.0 / 127.0);
                    }
                }
            } else {
                d.notes
                    .retain(|n| !(dk::step_of(n.start) == k && on_pad(kit, n, p)));
            }
        }
        if let Some((k, v)) = vel_set {
            if let Some(ni) = hit(kit, &d.notes, sp, k) {
                d.notes[ni].vel = v;
            }
        }
        if d.drum.is_none() && *d != c.d {
            d.drum = Some(DrumMeta { swing });
        }
        match tool {
            Some(0) => self.dk_play(c),
            Some(1) => self.with(|s| s.engine.stop()),
            Some(2) => self.dk_insert(c),
            Some(3) => d.notes.retain(|n| !on_pad(kit, n, sp)),
            Some(4) | Some(5) => {
                if let Some(key) = kit.pads.get(sp).map(|x| x.key) {
                    let by = if tool == Some(4) { -1 } else { 1 };
                    dk::shift(&mut d.notes, key, by, steps, swing);
                }
            }
            Some(6) => {
                let id = c.id;
                self.snapshot();
                self.with(|s| {
                    if let Some(t) = s.song.timeline.as_deref_mut() {
                        tl::make_unique_notes(t, id);
                    }
                    let audio::Shared { song, engine, .. } = s;
                    engine.ensure_plan(song);
                });
                self.dirty = true;
                self.status = "This clip has its own pattern now".into();
            }
            _ => {}
        }
    }

    /// loop the song over clip `c` and play from its start
    fn dk_play(&mut self, c: &Cur) {
        self.with(|s| {
            if let Some(t) = s.song.timeline.as_deref_mut() {
                tl::set_loop(t, c.start, c.start + c.len);
            }
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
        });
        self.tlv.cue = c.start;
        let was = self.with(|s| s.engine.playing);
        if was {
            self.with(|s| s.engine.stop());
        }
        self.play(0);
    }

    /// a linked copy of clip `c` at the timeline marker, on its track
    fn dk_insert(&mut self, c: &Cur) {
        let at = self.tlv.cue;
        self.snapshot();
        let id = self.with(|s| {
            let t = s.song.timeline.as_deref_mut()?;
            let src = t.clips.iter().find(|x| x.id == c.id)?.clone();
            let id = t.new_id();
            t.clips.push(tl::Clip {
                id,
                start: at,
                ..src
            });
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some(id)
        });
        if let Some(id) = id {
            self.dk.clip = Some(id);
            self.tlv.sel = Some(id);
            self.tlv.sels = vec![id];
            self.dirty = true;
            self.status = "Placed a linked copy at the marker (Make unique splits it off)".into();
        }
    }
}

// ------------------------------------------------------------ lower tabs
impl App {
    fn dk_lower(&mut self, ui: &mut egui::Ui, kit: &mut DrumKitP, cur: Option<&Cur>, slot: u8) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            for (i, t) in ["Sound", "Generate"].into_iter().enumerate() {
                if crate::ember::tab(ui, self.dk.tab == i as u8, t).clicked() {
                    self.dk.tab = i as u8;
                }
            }
        });
        ui.add_space(4.0);
        if self.dk.tab == 1 {
            self.dk_generate(ui, kit, cur, slot);
        } else {
            self.dk_sound(ui, kit);
        }
    }

    fn dk_sound(&mut self, ui: &mut egui::Ui, kit: &mut DrumKitP) {
        let sel = self.dk.sel;
        let n_pads = kit.pads.len();
        let Some(pad) = kit.pads.get_mut(sel) else {
            return;
        };
        let w = ui.available_width();
        let (r, _) = ui.allocate_exact_size(egui::vec2(w, 196.0), egui::Sense::hover());
        card(
            ui,
            r,
            &format!("Sound · pad {} of {n_pads} · {}", sel + 1, pad.name),
        );
        let inner = egui::Rect::from_min_max(
            r.min + egui::vec2(16.0, 26.0),
            r.max - egui::vec2(16.0, 10.0),
        );
        let mut audition = false;
        area(ui, inner, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut pad.name).desired_width(110.0));
                ui.label("Drum");
                let before = pad.drum.kind;
                egui::ComboBox::from_id_salt(("dk_kind", sel))
                    .width(80.0)
                    .selected_text(format!("{:?}", pad.drum.kind))
                    .show_ui(ui, |ui| {
                        for k in DrumKind::ALL {
                            ui.selectable_value(&mut pad.drum.kind, k, format!("{k:?}"));
                        }
                    });
                if pad.drum.kind != before {
                    let k = pad.drum.kind;
                    pad.drum = DrumP::new(k, 0, &[]);
                    audition = true;
                }
                let models = pad.drum.kind.models();
                let before = pad.drum.model;
                egui::ComboBox::from_id_salt(("dk_model", sel))
                    .width(100.0)
                    .selected_text(*models.get(pad.drum.model as usize).unwrap_or(&"?"))
                    .show_ui(ui, |ui| {
                        for (i, m) in models.iter().enumerate() {
                            ui.selectable_value(&mut pad.drum.model, i as u8, *m);
                        }
                    });
                audition |= pad.drum.model != before;
                ui.label("Key");
                let mut key = pad.key;
                ui.add(
                    egui::DragValue::new(&mut key)
                        .range(1..=96)
                        .custom_formatter(|v, _| dk::key_name(v as u8)),
                )
                .on_hover_text("the note that plays this pad (patterns and MIDI)");
                pad.key = key;
                ui.label("Choke");
                egui::ComboBox::from_id_salt(("dk_choke", sel))
                    .width(64.0)
                    .selected_text(if pad.choke == 0 { "none".to_string() } else { format!("group {}", pad.choke) })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut pad.choke, 0, "none");
                        for g in 1..=8u8 {
                            ui.selectable_value(&mut pad.choke, g, format!("group {g}"));
                        }
                    })
                    .response
                    .on_hover_text("a hit cuts the other pads of its group (an open hat stops when the closed hat plays)");
                ui.checkbox(&mut pad.mute, "Mute");
                ui.checkbox(&mut pad.solo, "Solo");
                ui.checkbox(&mut pad.drum.follow_note, "Follow note")
                    .on_hover_text("notes away from the pad's key re-pitch it");
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let defs = pad.drum.kind.macros();
                for (i, def) in defs.iter().enumerate() {
                    let mut lock = pad.drum.locks & (1 << i) != 0;
                    if super::genview::knob(
                        ui,
                        &mut pad.drum.macros[i],
                        def,
                        &mut lock,
                        ("dkm", sel, i),
                    ) {
                        audition = false;
                    }
                    if lock {
                        pad.drum.locks |= 1 << i;
                    } else {
                        pad.drum.locks &= !(1 << i);
                    }
                }
                ui.separator();
                mix_knob(ui, &mut pad.drum.level, &MIX_LEVEL, &format!("lv{sel}"));
                mix_knob(ui, &mut pad.pan, &MIX_PAN, &format!("pn{sel}"));
                mix_knob(ui, &mut pad.tune, &MIX_TUNE, &format!("tn{sel}"));
                mix_knob(ui, &mut pad.attack, &MIX_ATTACK, &format!("at{sel}"));
                mix_knob(ui, &mut pad.decay, &MIX_DECAY, &format!("dc{sel}"));
                mix_knob(ui, &mut pad.vel, &MIX_VEL, &format!("vl{sel}"));
            });
        });
        if audition {
            let k = kit.clone();
            self.dk_audition(&k, sel, self.ch, 1.0);
        }
    }

    fn dk_generate(&mut self, ui: &mut egui::Ui, kit: &mut DrumKitP, cur: Option<&Cur>, slot: u8) {
        use crate::drumpat::STYLES;
        let w = ui.available_width();
        let (r, _) = ui.allocate_exact_size(egui::vec2(w, 210.0), egui::Sense::hover());
        card(
            ui,
            r,
            "Generate · a new groove from the pattern library, in this kit",
        );
        let inner = egui::Rect::from_min_max(
            r.min + egui::vec2(16.0, 26.0),
            r.max - egui::vec2(16.0, 10.0),
        );
        let mut act: Option<u8> = None;
        area(ui, inner, |ui| {
            ui.horizontal(|ui| {
                let g = &mut self.dk.gen;
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("Style");
                        egui::ComboBox::from_id_salt("dk_style")
                            .width(110.0)
                            .selected_text(STYLES[g.style.min(STYLES.len() - 1)])
                            .show_ui(ui, |ui| {
                                for (i, s) in STYLES.iter().enumerate() {
                                    if *s != "Fills" {
                                        ui.selectable_value(&mut g.style, i, *s);
                                    }
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Fill");
                        for (f, l) in [(0u8, "none"), (1, "1 beat"), (2, "2 beats")] {
                            if crate::ember::led_item(ui, l, g.fill == f).clicked() {
                                g.fill = f;
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Seed");
                        ui.add(egui::DragValue::new(&mut g.seed).range(1..=99_999));
                    });
                });
                let mut n = g.energy;
                let mut lock = false;
                if super::genview::knob(ui, &mut n, &ENERGY, &mut lock, "dk_energy") {
                    g.energy = n;
                }
                ui.separator();
                // the result, as a mini grid in the kit's lanes
                let (gr, _) = ui.allocate_exact_size(egui::vec2((ui.available_width() - 200.0).max(160.0), 150.0), egui::Sense::hover());
                crate::ember::well(ui.painter(), gr, 4);
                match &g.result {
                    None => {
                        ui.painter().text(
                            gr.center(),
                            egui::Align2::CENTER_CENTER,
                            "Regenerate makes a groove",
                            egui::FontId::proportional(12.0),
                            th().ink2,
                        );
                    }
                    Some((notes, steps, _, src)) => {
                        let used: Vec<usize> = (0..kit.pads.len())
                            .filter(|&p| notes.iter().any(|n| on_pad(kit, n, p)))
                            .collect();
                        let lh = ((gr.height() - 22.0) / used.len().max(1) as f32).min(14.0);
                        let sw = gr.width() / (*steps).max(1) as f32;
                        for (row, &p) in used.iter().enumerate() {
                            let col = pad_col(&kit.pads[p]);
                            for n in notes.iter().filter(|n| on_pad(kit, n, p)) {
                                let k = dk::step_of(n.start);
                                let v = n.vel as f32 / 65535.0;
                                let cr = egui::Rect::from_min_size(
                                    egui::pos2(gr.left() + k as f32 * sw + 1.0, gr.top() + 4.0 + row as f32 * lh),
                                    egui::vec2((sw - 2.0).max(1.0), lh - 2.0),
                                );
                                ui.painter().rect_filled(cr, 1.5, col.gamma_multiply(0.4 + 0.6 * v));
                            }
                        }
                        ui.painter().text(
                            gr.left_bottom() + egui::vec2(6.0, -6.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!("from “{src}” · {steps} steps · {} hits", notes.len()),
                            egui::FontId::proportional(10.5),
                            th().ink2,
                        );
                    }
                }
                ui.vertical(|ui| {
                    let has = g.result.is_some();
                    let prev = if g.previewing { "■ Stop preview" } else { "▶ Preview" };
                    if ui
                        .add_enabled_ui(has, |ui| crate::ember::button(ui, prev, g.previewing))
                        .inner
                        .on_hover_text("hear it in this kit, looping at the song tempo (nothing changes yet)")
                        .clicked()
                    {
                        act = Some(0);
                    }
                    if ui
                        .add_enabled_ui(has && cur.is_some(), |ui| crate::ember::button(ui, "Keep ▸ pattern", false))
                        .inner
                        .on_hover_text("replace the current pattern's notes with it (every linked copy; Undo takes it back)")
                        .clicked()
                    {
                        act = Some(1);
                    }
                    if crate::ember::button(ui, "Regenerate", false)
                        .on_hover_text("a new groove with the next seed")
                        .clicked()
                    {
                        act = Some(2);
                    }
                    if ui
                        .add_enabled_ui(has, |ui| crate::ember::button(ui, "New pattern", false))
                        .inner
                        .on_hover_text("keep it as a new pattern clip on this kit's track")
                        .clicked()
                    {
                        act = Some(3);
                    }
                });
            });
        });
        let ins = |me: &Self, c: Option<&Cur>| -> u8 {
            match c {
                Some(c) if me.dk_track(slot) == Some(c.track) => 0,
                Some(_) => slot,
                None => 0,
            }
        };
        match act {
            Some(0) => {
                if self.dk.gen.previewing {
                    self.dk_gen_stop();
                } else {
                    self.dk_gen_preview(kit);
                }
            }
            Some(1) => {
                if let (Some(c), Some((notes, steps, swing, _))) = (cur, self.dk.gen.result.clone())
                {
                    let i = ins(self, cur);
                    self.snapshot();
                    let mut d = c.d.clone();
                    d.notes = notes.into_iter().map(|n| Note { instr: i, ..n }).collect();
                    d.len = steps as Tick * ROW;
                    d.drum = Some(DrumMeta { swing });
                    self.dk_write(c.data, d);
                    self.dk_gen_stop();
                    self.status = "Kept: the pattern has the generated groove".into();
                }
            }
            Some(2) => {
                let g = &mut self.dk.gen;
                if g.result.is_some() {
                    g.seed = g.seed % 99_999 + 1;
                }
                g.result = Some(dk::generate(g.style, g.seed, g.energy, g.fill, 0));
                if self.dk.gen.previewing {
                    self.dk_gen_preview(kit);
                }
            }
            Some(3) => {
                if let Some((notes, steps, swing, src)) = self.dk.gen.result.clone() {
                    let tr = cur
                        .map(|c| c.track)
                        .or_else(|| self.dk_track(slot))
                        .unwrap_or(self.ch);
                    self.snapshot();
                    let tmp = Cur {
                        id: 0,
                        data: 0,
                        track: tr,
                        start: cur.map_or(0, |c| c.start),
                        len: steps as Tick * ROW,
                        d: tl::NoteClipData {
                            name: src.to_string(),
                            len: steps as Tick * ROW,
                            notes: notes.clone(),
                            chords: vec![],
                            drum: Some(DrumMeta { swing }),
                        },
                    };
                    self.dk_new_pattern(tr, Some(&tmp));
                    // the copy is named "<src> copy": use the source name
                    if let Some(id) = self.dk.clip {
                        let i = ins(self, Some(&tmp));
                        self.with(|s| {
                            if let Some(t) = s.song.timeline.as_deref_mut() {
                                if let Some(tl::ClipKind::Notes { data }) =
                                    t.clips.iter().find(|c| c.id == id).map(|c| c.kind.clone())
                                {
                                    let d = t.notes[data as usize].make_mut();
                                    d.name = src.to_string();
                                    for n in d.notes.iter_mut() {
                                        n.instr = i;
                                    }
                                }
                            }
                        });
                    }
                    self.dk_gen_stop();
                }
            }
            _ => {}
        }
    }

    fn dk_gen_preview(&mut self, kit: &DrumKitP) {
        let Some((notes, steps, _, _)) = self.dk.gen.result.clone() else {
            return;
        };
        self.pat_stop();
        let idx = self.dk.inst;
        let Some(mut patch) = self.with(|s| match s.song.instruments.get(idx).map(|i| &i.kind) {
            Some(InstrumentKind::Gen(g)) => Some(g.clone()),
            _ => None,
        }) else {
            return;
        };
        patch.engine = GenEngine::Drums(kit.clone());
        let ev: Vec<(Tick, u8, u8)> = notes
            .iter()
            .map(|n| (n.start, n.key, tl::vel_to_7(n.vel)))
            .collect();
        let bar = self.pat_bar();
        let pp = crate::engine::patprev::PatPreview::new(
            patch,
            &ev,
            steps as Tick * ROW,
            bar,
            self.extra.pattern_vol,
        );
        self.with(|s| drop(s.engine.set_pattern_preview(Some(Box::new(pp)))));
        self.dk.gen.previewing = true;
        self.status =
            "Preview: the generated groove in this kit (Keep ▸ pattern or New pattern keeps it)"
                .into();
    }

    fn dk_gen_stop(&mut self) {
        if std::mem::take(&mut self.dk.gen.previewing) {
            self.with(|s| drop(s.engine.set_pattern_preview(None)));
        }
    }
}
