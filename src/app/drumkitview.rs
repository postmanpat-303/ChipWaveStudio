//! The Drum Kit editor (lower panel › Instrument on a Drum Kit; layout A of
//! /workspace/shots/drumkit/mock-a.png): Kit, Pattern and Groove cards, the
//! pad grid (click = hear and select), the step sequencer over the selected
//! note clip's notes (a pattern is a note clip: placed copies are linked,
//! Make unique splits one off), the velocity lane and the lower Sound /
//! Generate tabs. Play loops the song transport over the pattern's clip.
use super::*;
use crate::drumkit::{self as dk, DrumKitP, DrumMeta};
use crate::gen::{DrumKind, DrumP, GenEngine, MacroDef};
use crate::theme::ts;
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
    /// a lane for every pad (else the used ones)
    pub all_lanes: bool,
    /// lower tab: 0 Sound, 1 Generate
    pub tab: u8,
    /// the clip the sequencer edits
    pub clip: Option<u32>,
    /// drag-paint: the state the first cell was set to
    paint: Option<bool>,
    /// drawn step cells (pad, step, rect) and pads (pad, rect), for tests
    pub cells: Vec<(usize, usize, egui::Rect)>,
    pub pads: Vec<(usize, egui::Rect)>,
    /// the editor's visible area as last drawn (tests)
    pub view: Option<egui::Rect>,
    /// key controls as last drawn: ("tabs" | "add" | "gen_add" | "regen", rect) (tests)
    pub ctl: Vec<(&'static str, egui::Rect)>,
    /// the last frame used the compact (laptop) layout
    pub compact: bool,
    /// audition key counter (live keys 200..=215)
    akey: u8,
    pub gen: GenSet,
    /// Kits… / Dice kit, applied after this frame's edits (one undo step)
    load: Option<KitLoad>,
}

/// a pending Kits… pick or Dice kit
#[derive(Clone, Debug)]
enum KitLoad {
    Kit(String),
    Dice(u32),
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
pub(crate) struct Cur {
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
        crate::theme::prop(crate::theme::ts::TINY),
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
    // a child that leaves the parent's cursor alone: the card around it is
    // already allocated
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
    f(&mut child)
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
        self.dk.view = Some(ui.clip_rect().intersect(ui.max_rect()));
        self.dk.cells.clear();
        self.dk.pads.clear();
        self.dk.ctl.clear();
        // the full layout needs about 840 px; shorter (laptops: the panel
        // under the timeline is 250-350 px) or narrow panels get the compact
        // one, which fits without scrolling
        let avail = ui.available_rect_before_wrap();
        let compact = avail.height() < 840.0 || avail.width() < 1000.0;
        self.dk.compact = compact;
        if compact {
            self.dk_compact(
                ui,
                &mut kit,
                &pats,
                cur.as_ref(),
                tlon,
                (play_at, playing),
                &mut edit,
            );
        } else {
            let out = egui::ScrollArea::vertical()
            .id_salt("dk_scroll")
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(ui, |ui| {
                ui.add_space(4.0);
                self.dk_header(ui, &name, &mut kit, &pats, cur.as_ref(), tlon);
                ui.add_space(6.0);
                let ch = cur.as_ref().map_or(self.ch, |c| c.track);
                self.dk_pads(ui, &mut kit, ch, None);
                ui.add_space(6.0);
                if let Some(c) = &cur {
                    let mut d = c.d.clone();
                    self.dk_sequencer(ui, &mut kit, c, &mut d, play_at, playing, slot, None);
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
                        crate::theme::prop(crate::theme::ts::BODY),
                        th().ink2,
                    );
                }
                ui.add_space(6.0);
                self.dk_lower(ui, &mut kit, cur.as_ref(), slot);
                ui.add_space(6.0);
            });
            // more below: a fade and a hint over the bottom edge
            let vis = out.inner_rect;
            if out.state.clamped_offset().y + vis.height() < out.content_size.y - 4.0 {
                let fr =
                    egui::Rect::from_min_max(egui::pos2(vis.left(), vis.bottom() - 30.0), vis.max);
                let p = ui.painter_at(vis);
                let mut m = egui::Mesh::default();
                let bg = th().crt_bot;
                crate::theme::round_mesh(&mut m, fr, [0.0; 4], bg.gamma_multiply(0.0), bg);
                p.add(egui::Shape::mesh(m));
                p.text(
                    egui::pos2(fr.center().x, fr.bottom() - 6.0),
                    egui::Align2::CENTER_BOTTOM,
                    "More below: scroll down",
                    crate::theme::prop(crate::theme::ts::LABEL),
                    th().ochre,
                );
            }
        }
        if let Some(l) = self.dk.load.take() {
            // the kit's own edits this frame first, then the load as one step
            if kit != orig {
                self.dk_store_kit(idx, &kit);
            }
            if let (Some(d), Some(c)) = (edit, cur.as_ref()) {
                self.dk_write(c.data, d);
            }
            self.dk_apply_load(idx, l, cur.as_ref());
            return;
        }
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

    /// the compact (laptop) layout: a one-line toolbar, the step sequencer
    /// on the left (it gets the height), and Pads / Sound / Generate tabs in
    /// a side panel; it fits the panel without scrolling
    #[allow(clippy::too_many_arguments)]
    fn dk_compact(
        &mut self,
        ui: &mut egui::Ui,
        kit: &mut DrumKitP,
        pats: &[PatRow],
        cur: Option<&Cur>,
        tlon: bool,
        (play_at, playing): (Option<f64>, bool),
        edit: &mut Option<tl::NoteClipData>,
    ) {
        let slot = self.dk.inst as u8 + 1;
        let full = ui.available_rect_before_wrap();
        let tb = egui::Rect::from_min_max(full.min, egui::pos2(full.right(), full.top() + 26.0));
        area(ui, tb, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(4.0);
                // (the instrument header above names the kit)
                self.dk_btn_kits(ui, kit);
                self.dk_btn_dice(ui, kit);
                ui.separator();
                self.dk_pat_combo(ui, pats, cur);
                self.dk_btn_new(ui, cur, slot);
                self.dk_btn_dup(ui, cur);
                if let Some(c) = cur {
                    ui.separator();
                    ui.label(egui::RichText::new("STEPS").small().color(th().ink2));
                    self.dk_steps_leds(ui, c, [16, 32, 64]);
                    ui.separator();
                    let swing = c.d.drum.as_ref().map_or(0.0, |m| m.swing);
                    let mut pc = dk::swing_percent(swing).round();
                    ui.label("Swing");
                    if ui
                        .add(egui::DragValue::new(&mut pc).range(50.0..=75.0).suffix(" %"))
                        .on_hover_text("50 % = straight, 66 % = triplet feel, 75 % max; the off-beat 16ths move")
                        .changed()
                    {
                        self.dk_set_swing(c, (pc - 50.0) / 50.0);
                    }
                }
            });
        });
        let rest = egui::Rect::from_min_max(egui::pos2(full.left(), tb.bottom() + 2.0), full.max);
        let side_w = (rest.width() * 0.34).clamp(300.0, 390.0);
        let seq_r = egui::Rect::from_min_max(
            rest.min,
            egui::pos2(rest.right() - side_w - 6.0, rest.bottom()),
        );
        let side_r =
            egui::Rect::from_min_max(egui::pos2(seq_r.right() + 6.0, rest.top()), rest.max);
        let ch = cur.map_or(self.ch, |c| c.track);
        match cur {
            Some(c) => {
                let mut d = c.d.clone();
                area(ui, seq_r, |ui| {
                    self.dk_sequencer(
                        ui,
                        kit,
                        c,
                        &mut d,
                        play_at,
                        playing,
                        slot,
                        Some(seq_r.height()),
                    )
                });
                if d != c.d {
                    *edit = Some(d);
                }
            }
            None => {
                crate::ember::card(ui.painter(), seq_r);
                ui.painter().text(
                    seq_r.center(),
                    egui::Align2::CENTER_CENTER,
                    if tlon {
                        "No pattern yet: + New makes one, or Generate › Add to track"
                    } else {
                        "The step sequencer edits note clips: switch the song to timeline mode first"
                    },
                    crate::theme::prop(crate::theme::ts::COMPACT),
                    th().ink2,
                );
            }
        }
        // side panel
        crate::ember::card(ui.painter(), side_r);
        let tabs_r = egui::Rect::from_min_max(
            side_r.min + egui::vec2(8.0, 3.0),
            egui::pos2(side_r.right() - 8.0, side_r.top() + 29.0),
        );
        area(ui, tabs_r, |ui| {
            ui.horizontal_centered(|ui| {
                for (i, t) in [(0u8, "Pads"), (1, "Sound"), (2, "Generate")] {
                    if crate::ember::tab(ui, self.dk.tab == i, t).clicked() {
                        self.dk.tab = i;
                    }
                }
            });
        });
        self.dk.ctl.push(("tabs", tabs_r));
        let body_r = egui::Rect::from_min_max(
            egui::pos2(side_r.left() + 8.0, tabs_r.bottom() + 4.0),
            side_r.max - egui::vec2(6.0, 6.0),
        );
        let tab = self.dk.tab;
        area(ui, body_r, |ui| {
            egui::ScrollArea::vertical()
                .id_salt(("dk_side", tab))
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                .show(ui, |ui| match tab {
                    0 => self.dk_pads(ui, kit, ch, Some(body_r.height())),
                    1 => self.dk_sound(ui, kit, true),
                    _ => self.dk_generate(ui, kit, cur, slot, true),
                });
        });
        ui.allocate_rect(full, egui::Sense::hover());
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
    pub(crate) fn dk_write(&mut self, data: usize, mut d: tl::NoteClipData) {
        self.with(|s| {
            let ninst = s.song.instruments.len();
            if let Some(t) = s.song.timeline.as_deref_mut() {
                // a note without an instrument (0) plays whatever its
                // channel last played: slot 01 on a fresh track, so a new
                // kit's steps and starter groove played the first track's
                // StringWave (Del (postmanpat) 25 Oct 2026). They get the
                // track's own instrument when every clip of the pattern
                // sits on tracks that play the same one
                let mut slots = t
                    .clips
                    .iter()
                    .chain(t.launcher.iter().map(|l| &l.clip))
                    .filter(|c| c.notes() == Some(data))
                    .map(|c| t.track_instr(c.track as usize));
                let first = slots.next().unwrap_or(0);
                if first > 0 && first as usize <= ninst && slots.all(|k| k == first) {
                    for n in d.notes.iter_mut().filter(|n| n.instr == 0) {
                        n.instr = first;
                    }
                }
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
                crate::theme::prop(crate::theme::ts::HEADING),
                th().ink,
            );
        };
        let small = |ui: &egui::Ui, r: egui::Rect, s: &str| {
            ui.painter().text(
                r.left_top() + egui::vec2(16.0, 50.0),
                egui::Align2::LEFT_TOP,
                s,
                crate::theme::prop(crate::theme::ts::SMALL),
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
                self.dk_btn_dice(ui, kit);
                self.dk_btn_kits(ui, kit);
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
                self.dk_btn_dup(ui, cur);
                self.dk_btn_new(ui, cur, slot);
                self.dk_pat_combo(ui, pats, cur);
            });
        });
        if let Some(c) = cur {
            let br = egui::Rect::from_min_max(
                egui::pos2(r2.right() - 220.0, r2.top() + 42.0),
                egui::pos2(r2.right() - 14.0, r2.top() + 66.0),
            );
            area(ui, br, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.dk_steps_leds(ui, c, [64, 32, 16]);
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
            self.dk_set_swing(c, sw);
        }
        let _ = t;
    }

    /// swing `sw` (0..0.5) on pattern `c`, in whole percents as shown
    fn dk_set_swing(&mut self, c: &Cur, sw: f32) {
        let swing = c.d.drum.as_ref().map_or(0.0, |m| m.swing);
        let sw = ((dk::swing_percent(sw).round() - 50.0) / 50.0).clamp(0.0, 0.5);
        if (sw - swing).abs() > 1e-6 {
            let mut d = c.d.clone();
            dk::set_swing(&mut d.notes, sw);
            d.drum = Some(DrumMeta { swing: sw });
            self.dk_write(c.data, d);
        }
    }

    /// queue a Dice kit on Drum Kit instrument `idx` (the track pie's
    /// Dice kit): it lands the next time its editor draws
    pub(super) fn dk_queue_dice(&mut self, idx: usize, seed: u32) {
        if self.dk.inst != idx {
            self.dk = DkUi {
                inst: idx,
                ..Default::default()
            };
        }
        self.dk.load = Some(KitLoad::Dice(seed | 1));
    }

    fn dk_btn_dice(&mut self, ui: &mut egui::Ui, _kit: &mut DrumKitP) {
        if crate::ember::button(ui, "Dice kit", false)
            .on_hover_text(
                "an empty pattern: a random kit and its starter groove; else re-roll every pad's unlocked knobs (the pattern stays). Undo takes it back",
            )
            .clicked()
        {
            let seed = (ui.input(|i| i.time) * 1000.0) as u32 | 1;
            self.dk.load = Some(KitLoad::Dice(seed));
        }
    }

    fn dk_btn_kits(&mut self, ui: &mut egui::Ui, _kit: &mut DrumKitP) {
        ui.menu_button("Kits…", |ui| {
            ui.label(
                egui::RichText::new(
                    "load a kit's sounds into the pads with the same keys; an empty pattern gets the kit's starter groove (notes stay otherwise)",
                )
                .small()
                .color(th().ink2),
            );
            for (n, tip) in dk::presets().into_iter().map(|(n, t, _)| (n, t)) {
                if ui.button(&n).on_hover_text(tip).clicked() {
                    self.dk.load = Some(KitLoad::Kit(n));
                    ui.close();
                }
            }
        });
    }

    /// write `kit` into instrument `idx`
    fn dk_store_kit(&mut self, idx: usize, kit: &DrumKitP) {
        let mut kit = kit.clone();
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

    /// the notes of note data `data`
    fn dk_clip_notes(&self, data: usize) -> Option<Vec<Note>> {
        self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.notes.get(data))
                .map(|d| d.notes.clone())
        })
    }

    /// Kits… / Dice kit on instrument `idx`, one undo step. A kit pick loads
    /// its sounds, pad names and name into the pads with the same keys; an
    /// empty pattern `cur` also gets the kit's starter groove. Dice on an
    /// empty pattern picks a random kit (and its groove); with notes it
    /// re-rolls the pads' unlocked knobs and the pattern stays.
    fn dk_apply_load(&mut self, idx: usize, l: KitLoad, cur: Option<&Cur>) {
        let Some((iname, mut kit)) = self.with(|s| {
            let i = s.song.instruments.get(idx)?;
            Some((i.name.clone(), dk::kit_of(i)?.clone()))
        }) else {
            return;
        };
        let empty = cur
            .and_then(|c| self.dk_clip_notes(c.data))
            .is_some_and(|n| n.is_empty());
        let pick = match &l {
            KitLoad::Kit(n) => Some(n.clone()),
            KitLoad::Dice(seed) if empty => Some(dice_kit(*seed, &iname)),
            KitLoad::Dice(_) => None,
        };
        self.snapshot();
        self.undo_hold += 1;
        let slot = idx + 1;
        let msg = match (pick, l) {
            (Some(n), l) => {
                let Some(p) = dk::preset(&n) else {
                    self.undo_hold -= 1;
                    self.status = format!("No {n} preset");
                    return;
                };
                let mut got = 0;
                if let GenEngine::Drums(k) = p.engine {
                    for pad in kit.pads.iter_mut() {
                        if let Some(src) = k.pads.iter().find(|x| x.key == pad.key) {
                            pad.drum = src.drum;
                            pad.tune = src.tune;
                            pad.name = src.name.clone();
                            got += 1;
                        }
                    }
                }
                self.dk_store_kit(idx, &kit);
                self.with(|s| {
                    if let Some(i) = s.song.instruments.get_mut(idx) {
                        i.name = n.clone();
                    }
                    // a track named after the old kit follows it (the
                    // header read "Psy Kit" over "02 909 Kit")
                    let slot = idx as u8 + 1;
                    let nch = s.song.channels;
                    let mine: Vec<usize> = s.song.timeline.as_deref().map_or(vec![], |t| {
                        (0..nch).filter(|&tr| t.track_instr(tr) == slot).collect()
                    });
                    for tr in mine {
                        if let Some(c) = s.song.channel_names.get_mut(tr) {
                            if c.trim() == iname.trim() {
                                *c = n.chars().take(24).collect();
                            }
                        }
                    }
                });
                let dice = if matches!(l, KitLoad::Dice(_)) {
                    "Dice kit: "
                } else {
                    ""
                };
                match cur.filter(|_| empty).zip(starter_groove(&n)) {
                    Some((c, g)) => {
                        let mut d = c.d.clone();
                        d.notes = g.notes(slot as u8);
                        d.len = g.len();
                        d.name = g.name.to_string();
                        d.drum = Some(DrumMeta { swing: g.swing });
                        let hits = d.notes.len();
                        self.dk_write(c.data, d);
                        format!(
                            "{dice}{slot:02X} is now {n} ({got} pads), with its starter groove {} ({hits} hits)",
                            g.name
                        )
                    }
                    None if empty || cur.is_none() => {
                        format!("{dice}{slot:02X} is now {n} ({got} pads)")
                    }
                    None => format!(
                        "{dice}{slot:02X} is now {n} ({got} pads); the pattern's notes stay"
                    ),
                }
            }
            (None, KitLoad::Dice(seed)) => {
                for (i, p) in kit.pads.iter_mut().enumerate() {
                    crate::gen::roll(
                        &mut p.drum,
                        seed.wrapping_add((i as u32).wrapping_mul(0x9E37_79B9)),
                    );
                }
                self.dk_store_kit(idx, &kit);
                format!(
                    "Dice kit: {} pads of {iname} re-rolled (unlocked knobs); the pattern's notes stay",
                    kit.pads.len()
                )
            }
            (None, KitLoad::Kit(_)) => unreachable!(),
        };
        self.undo_hold -= 1;
        self.dirty = true;
        self.status = msg;
    }

    fn dk_btn_dup(&mut self, ui: &mut egui::Ui, cur: Option<&Cur>) {
        if crate::ember::button(ui, "Dup", false)
            .on_hover_text("a copy of this pattern of its own, right after it")
            .clicked()
        {
            if let Some(c) = cur {
                self.dk_new_pattern(c.track, Some(c));
            }
        }
    }

    fn dk_btn_new(&mut self, ui: &mut egui::Ui, cur: Option<&Cur>, slot: u8) {
        if crate::ember::button(ui, "+ New", false)
            .on_hover_text("an empty 1-bar pattern on this kit's track, after its last clip")
            .clicked()
        {
            let tr = cur
                .map(|c| c.track)
                .or_else(|| self.dk_track(slot))
                .unwrap_or(self.ch);
            self.dk_new_pattern(tr, None);
        }
    }

    fn dk_pat_combo(&mut self, ui: &mut egui::Ui, pats: &[PatRow], cur: Option<&Cur>) {
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
    }

    /// the pattern-length LEDs, in `order`
    fn dk_steps_leds(&mut self, ui: &mut egui::Ui, c: &Cur, order: [usize; 3]) {
        let steps = ((c.d.len + ROW - 1) / ROW).max(1) as usize;
        for n in order {
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
    /// the pad grid: 8 big pads a row, or (compact) small pads, 4 a row
    /// fit = Some(height): the compact grid (4 a row), pads sized to fit
    fn dk_pads(&mut self, ui: &mut egui::Ui, kit: &mut DrumKitP, ch: usize, fit: Option<f32>) {
        let compact = fit.is_some();
        let w = ui.available_width();
        let cols = if compact { 4usize } else { 8 };
        let n = kit.pads.len() + usize::from(kit.pads.len() < dk::MAX_PADS);
        let gap = if compact { 4.0 } else { 6.0 };
        let (top, side) = if compact { (2.0, 2.0) } else { (26.0, 16.0) };
        // compact: an "+ ADD PAD" that would start a row of its own is a
        // short strip instead, so the pads keep room for two lines of text
        let strip_h = if compact && n > kit.pads.len() && kit.pads.len().is_multiple_of(cols) {
            crate::theme::row_h(ui.painter(), &crate::theme::prop(ts::SMALL)) + 8.0
        } else {
            0.0
        };
        let rows = if strip_h > 0.0 {
            kit.pads.len().div_ceil(cols).max(1)
        } else {
            n.div_ceil(cols).max(1)
        };
        let strip = if strip_h > 0.0 { strip_h + gap } else { 0.0 };
        let ph = match fit {
            Some(h) => ((h - top - 6.0 - strip) / rows as f32 - gap).clamp(24.0, 40.0),
            None => 50.0,
        };
        let h = top + rows as f32 * (ph + gap) + strip + 6.0;
        let (r, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        if !compact {
            card(
                ui,
                r,
                &format!("Pads · {} of {}", kit.pads.len(), dk::MAX_PADS),
            );
            ui.painter().text(
                egui::pos2(r.right() - 16.0, r.top() + 7.0),
                egui::Align2::RIGHT_TOP,
                "click = hear and select · right-click = more",
                crate::theme::prop(crate::theme::ts::TINY),
                th().ink2,
            );
        }
        let (f_name, f_tag) = if compact {
            (ts::LABEL, ts::TINY)
        } else {
            (ts::BODY, ts::SMALL)
        };
        let name_font = crate::theme::prop(f_name);
        let tag_font = crate::theme::mono(f_tag);
        let (name_h, tag_h) = {
            let p = ui.painter();
            (
                crate::theme::row_h(p, &name_font),
                crate::theme::row_h(p, &tag_font),
            )
        };
        // two lines (name over key) when the pad is tall enough, else one
        // line with the key on the right; rows follow the text
        let two_lines = ph >= name_h + tag_h + 4.0;
        let inset = if two_lines {
            ((ph - name_h - tag_h) / 2.0).clamp(2.0, 8.0)
        } else {
            ((ph - name_h) / 2.0).max(1.0)
        };
        let pw = (r.width() - 2.0 * side - gap * (cols - 1) as f32) / cols as f32;
        let mut act: Option<(u8, usize)> = None;
        for i in 0..n {
            let (cx, cy) = (i % cols, i / cols);
            let pr = egui::Rect::from_min_size(
                egui::pos2(
                    r.left() + side + cx as f32 * (pw + gap),
                    r.top() + top + cy as f32 * (ph + gap),
                ),
                egui::vec2(
                    pw,
                    if i == kit.pads.len() && strip_h > 0.0 {
                        strip_h
                    } else {
                        ph
                    },
                ),
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
                    crate::theme::prop(ts::SMALL),
                    crate::theme::on2(crate::ember::DISP1),
                );
                if resp
                    .on_hover_text(
                        "a new pad on the next free key (a copy of the selected pad's sound)",
                    )
                    .clicked()
                {
                    act = Some((0, i));
                }
                self.dk.ctl.push(("add_pad", pr));
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
                    pr.min + egui::vec2(5.0, inset),
                    egui::vec2(3.0, ph - 2.0 * inset),
                ),
                1.5,
                if kit.audible(i) {
                    col
                } else {
                    col.gamma_multiply(0.3)
                },
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
            let ink = if kit.audible(i) {
                crate::theme::on(crate::ember::DISP1)
            } else {
                crate::theme::on2(crate::ember::DISP1)
            };
            let ink2 = crate::theme::on2(crate::ember::DISP1);
            let text_w = pw - 13.0 - 5.0;
            let name_font = crate::theme::shrink_to_fit(p, &pad.name, name_font.clone(), text_w);
            if two_lines {
                crate::theme::text_fit(
                    p,
                    pr.left_top() + egui::vec2(13.0, inset),
                    egui::Align2::LEFT_TOP,
                    &pad.name,
                    name_font.clone(),
                    ink,
                    text_w,
                );
                crate::theme::text_fit(
                    p,
                    pr.left_bottom() + egui::vec2(13.0, -inset),
                    egui::Align2::LEFT_BOTTOM,
                    &tag,
                    tag_font.clone(),
                    ink2,
                    text_w,
                );
            } else {
                // one line: the name only (the key and choke group are in
                // the tooltip and on the lanes)
                crate::theme::text_fit(
                    p,
                    pr.left_center() + egui::vec2(13.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &pad.name,
                    name_font.clone(),
                    ink,
                    text_w,
                );
            }
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
        fit: Option<f32>,
    ) {
        // fit = Some(height): the compact layout, filling that height
        let compact = fit.is_some();
        let steps = ((d.len + ROW - 1) / ROW).max(1) as usize;
        let pages = steps.div_ceil(16);
        self.dk.page = self.dk.page.min(pages - 1);
        let swing = d.drum.as_ref().map_or(0.0, |m| m.swing);
        let w = ui.available_width();
        // the pads this pattern uses and the selected one (at least 8), or all
        let mut lane_ix: Vec<usize> = (0..kit.pads.len())
            .filter(|&p| {
                self.dk.all_lanes || p == self.dk.sel || d.notes.iter().any(|n| on_pad(kit, n, p))
            })
            .collect();
        for p in 0..kit.pads.len() {
            if lane_ix.len() >= 8 {
                break;
            }
            if !lane_ix.contains(&p) {
                lane_ix.push(p);
            }
        }
        lane_ix.sort_unstable();
        // compact: the Add-to-track line (at 29) and the bar numbers stack
        // above the grid at their own text heights (Text size)
        let line_h = crate::theme::row_h(ui.painter(), &crate::theme::prop(ts::SMALL));
        let ruler_h = crate::theme::row_h(ui.painter(), &crate::theme::mono(ts::TINY));
        let (top, vel_h, side) = if compact {
            ((29.0 + line_h + ruler_h + 1.0f32).max(50.0), 20.0, 8.0)
        } else {
            (62.0, 46.0, 16.0)
        };
        let mut hidden = 0;
        let lh = match fit {
            Some(h) => {
                // as many lanes as fit at 12 px or more (the selected pad stays)
                let room = (h - top - 6.0 - vel_h - 6.0).max(12.0);
                let max = ((room / 12.0).floor() as usize).max(1);
                if lane_ix.len() > max {
                    hidden = lane_ix.len() - max;
                    let sel = self.dk.sel;
                    let mut keep: Vec<usize> = lane_ix
                        .iter()
                        .copied()
                        .filter(|&p| p != sel)
                        .take(max - 1)
                        .collect();
                    if lane_ix.contains(&sel) {
                        keep.push(sel);
                    }
                    keep.sort_unstable();
                    lane_ix = keep;
                }
                (room / lane_ix.len().max(1) as f32).clamp(12.0, 22.0)
            }
            None => 22.0,
        };
        let lanes = lane_ix.len();
        let h = fit.unwrap_or(top + lanes as f32 * lh + 10.0 + vel_h + 14.0);
        let (r, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        if compact {
            crate::ember::card(ui.painter(), r);
        } else {
            card(
                ui,
                r,
                &format!("Step sequencer · {} · {steps} steps", d.name),
            );
        }
        // transport + tools row
        let looping = playing
            && self.with(|s| {
                s.song.timeline.as_deref().is_some_and(|t| {
                    t.loop_on && t.loop_range == Some((c.start, c.start + c.len.max(ROW)))
                })
            });
        let row_y = if compact { 2.0 } else { 24.0 };
        let row = egui::Rect::from_min_max(
            egui::pos2(r.left() + side - 2.0, r.top() + row_y),
            egui::pos2(r.right() - side + 2.0, r.top() + row_y + 26.0),
        );
        let mut tool: Option<u8> = None;
        // where Add to track puts a linked copy: on the card (compact: under
        // the buttons)
        let add_line = self.dk_add_plan(slot, Some(c.track), &d.name, c.len).line;
        let line_at = if compact {
            egui::pos2(r.right() - side, r.top() + 29.0)
        } else {
            egui::pos2(r.right() - 16.0, r.top() + 7.0)
        };
        // cut with "…" so the pattern's name keeps 120 pt (the full line
        // is the Add to track tooltip)
        let (line_r, _) = crate::theme::text_fit(
            ui.painter(),
            line_at,
            egui::Align2::RIGHT_TOP,
            &add_line,
            crate::theme::prop(if compact { ts::SMALL } else { ts::TINY }),
            if compact { th().ink } else { th().ink2 },
            r.width() - 2.0 * side - if compact { 160.0 } else { 260.0 },
        );
        if compact {
            // the pattern's name (and hidden lanes) in what's left of the line
            let free = egui::Rect::from_min_max(
                egui::pos2(r.left() + side, r.top() + 27.0),
                egui::pos2(line_r.left() - 10.0, r.top() + 29.0 + line_h),
            );
            crate::theme::text_fit(
                &ui.painter_at(free),
                egui::pos2(r.left() + side, r.top() + 29.0),
                egui::Align2::LEFT_TOP,
                &if hidden > 0 {
                    format!(
                        "{} · {steps} steps · {hidden} more lanes: drag the panel taller",
                        d.name
                    )
                } else {
                    format!("{} · {steps} steps", d.name)
                },
                crate::theme::prop(crate::theme::ts::SMALL),
                if hidden > 0 { th().ochre } else { th().ink2 },
                free.width(),
            );
        }
        area(ui, row, |ui| {
            ui.horizontal_centered(|ui| {
                if crate::ember::button(ui, if looping { "Stop" } else { "Play loop" }, looping)
                    .on_hover_text("loop the song over this pattern's clip and play it (the timeline's loop moves here)")
                    .clicked()
                {
                    tool = Some(if looping { 1 } else { 0 });
                }
                ui.add_space(8.0);
                let all = self.dk.all_lanes;
                if crate::ember::led_item(ui, "All pads", all)
                    .on_hover_text("show a lane for every pad (off: the pads this pattern uses)")
                    .clicked()
                {
                    self.dk.all_lanes = !all;
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
                    if crate::ember::button(ui, if compact { ">" } else { "Shift >" }, false)
                        .on_hover_text("shift the selected lane one step later (wraps)")
                        .clicked()
                    {
                        tool = Some(5);
                    }
                    if crate::ember::button(ui, if compact { "<" } else { "< Shift" }, false)
                        .on_hover_text("shift the selected lane one step earlier (wraps)")
                        .clicked()
                    {
                        tool = Some(4);
                    }
                    if crate::ember::button(ui, if compact { "Clear" } else { "Clear lane" }, false).clicked() {
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
                        .add_enabled_ui(uses > 1, |ui| crate::ember::button(ui, if compact { "Unique" } else { "Make unique" }, false))
                        .inner
                        .on_hover_text("give this clip its own copy of the pattern (the other placed copies stay linked)")
                        .on_disabled_hover_text("only this clip plays this pattern")
                        .clicked()
                    {
                        tool = Some(6);
                    }
                    let add = crate::ember::button_ex(ui, "Add to track", false, true, egui::vec2(0.0, 0.0))
                        .on_hover_text(format!("{} (a linked copy: edits change every copy; Make unique splits one off)", add_line));
                    self.dk.ctl.push(("add", add.rect));
                    if add.clicked() {
                        tool = Some(2);
                    }
                });
            });
        });
        // grid
        let names_w = if compact { 96.0 } else { 150.0 };
        let ms_w = 44.0;
        let gx = r.left() + side + names_w + ms_w;
        let gw = r.right() - side - gx;
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
        for (row, &p) in lane_ix.iter().enumerate() {
            let pad = &mut kit.pads[p];
            let y = gy + row as f32 * lh;
            let lane = egui::Rect::from_min_size(
                egui::pos2(r.left() + side, y),
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
                    egui::vec2(3.0, (lh - 10.0).max(4.0)),
                ),
                1.0,
                col,
            );
            painter.text(
                lane.left_center() + egui::vec2(12.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &pad.name,
                crate::theme::prop(crate::theme::ts::COMPACT),
                if sel { th().ink } else { th().ink2 },
            );
            // M / S
            for (j, (lbl, on)) in [("M", pad.mute), ("S", pad.solo)].into_iter().enumerate() {
                let br = egui::Rect::from_min_size(
                    egui::pos2(r.left() + side + names_w + j as f32 * 21.0, y + 1.0),
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
                    crate::theme::mono(crate::theme::ts::TINY),
                    if on {
                        if j == 0 {
                            th().brick
                        } else {
                            th().ochre
                        }
                    } else {
                        // the well is dark in every theme
                        crate::theme::on2(crate::ember::DISP1)
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
        for (row, &p) in lane_ix.iter().enumerate() {
            let y = gy + row as f32 * lh;
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
                if resp.secondary_clicked() && !self.extra.clip_list_menu {
                    // the pattern's pie, on this pad's lane
                    self.dk.sel = p;
                    if let Some(at) = resp.interact_pointer_pos() {
                        self.opie_req = Some((
                            super::objpie::PieFor::Drum {
                                clip: c.id,
                                data: c.data,
                                track: c.track,
                                pad: p,
                            },
                            at,
                        ));
                    }
                } else if resp.drag_started() || resp.clicked() && self.dk.paint.is_none() {
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
                    egui::pos2(
                        gx + s as f32 * sw + 3.0,
                        gy - if compact { ruler_h / 2.0 + 1.0 } else { 12.0 },
                    ),
                    egui::Align2::LEFT_CENTER,
                    format!("{}.{}", k / 16 + 1, (k % 16) / 4 + 1),
                    crate::theme::mono(crate::theme::ts::TINY),
                    th().ink2,
                );
            }
        }
        // velocity lane for the selected pad
        let vy = gy + lanes as f32 * lh + if compact { 6.0 } else { 10.0 };
        let vr = egui::Rect::from_min_size(egui::pos2(gx, vy), egui::vec2(gw, vel_h));
        crate::ember::well(ui.painter(), vr, 4);
        ui.painter().text(
            egui::pos2(r.left() + side + 12.0, vy + vel_h / 2.0),
            egui::Align2::LEFT_CENTER,
            format!(
                "Velocity · {}",
                kit.pads.get(self.dk.sel).map_or("", |p| p.name.as_str())
            ),
            crate::theme::prop(crate::theme::ts::SMALL),
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
            Some(2) => {
                self.dk_add_to_track(slot, Some(c.track), AddSrc::Linked(c.clone()));
            }
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

    /// where Add to track puts a pattern of `len` ticks named `name` for
    /// kit `slot`: on `track` (None = the kit's track; no track = a new one),
    /// at the play marker snapped to its bar, or right after the track's last
    /// clip when that spot is taken
    pub(crate) fn dk_add_plan(
        &self,
        slot: u8,
        track: Option<usize>,
        name: &str,
        len: Tick,
    ) -> AddPlan {
        let track = track.or_else(|| self.dk_track(slot));
        let cue = self.tlv.cue.max(0);
        self.with(|s| {
            let Some(t) = s.song.timeline.as_deref() else {
                return AddPlan {
                    track,
                    at: 0,
                    line: "Switch the song to timeline mode to add patterns to a track".into(),
                };
            };
            let snap_down = |x: Tick| {
                let sig = t.sig_at(x);
                let bar = sig.bar_ticks().max(1);
                let from = sig.tick.min(x);
                from + (x - from) / bar * bar
            };
            let snap_up = |x: Tick| {
                let d = snap_down(x);
                if d == x {
                    x
                } else {
                    d + t.sig_at(d).bar_ticks().max(1)
                }
            };
            let mut at = snap_down(cue);
            let len = len.max(ROW);
            let mut after = false;
            if let Some(tr) = track {
                let on: Vec<&tl::Clip> = t.clips.iter().filter(|c| c.track as usize == tr).collect();
                if on.iter().any(|c| c.start < at + len && c.end() > at) {
                    let last = on.iter().map(|c| c.end()).max().unwrap_or(0);
                    at = snap_up(last);
                    after = true;
                }
            }
            let bar_len = t.sig_at(at).bar_ticks().max(1);
            let size = if len % bar_len == 0 {
                let n = len / bar_len;
                format!("{n} bar{}", if n == 1 { "" } else { "s" })
            } else {
                format!("{} steps", len / ROW)
            };
            let bar = t.bar_pos(at).bar;
            let line = match track {
                None => format!(
                    "Adds '{name}' ({size}) to a new track at bar {bar} (this kit isn't on a track yet)"
                ),
                Some(tr) if after => format!(
                    "Adds '{name}' ({size}) to Track {} at bar {bar}, after its last clip (the marker's bar is taken)",
                    tr + 1
                ),
                Some(tr) => format!("Adds '{name}' ({size}) to Track {} at bar {bar}", tr + 1),
            };
            AddPlan { track, at, line }
        })
    }

    /// Add to track: place `src` (a linked copy of a clip, or new note data)
    /// as the plan says, making the kit's track when it has none. One undo
    /// step; selects and flashes the new clip. Returns its id.
    pub(crate) fn dk_add_to_track(
        &mut self,
        slot: u8,
        track: Option<usize>,
        src: AddSrc,
    ) -> Option<u32> {
        let (name, len) = match &src {
            AddSrc::Linked(c) => (c.d.name.clone(), c.len),
            AddSrc::New(d) => (d.name.clone(), d.len),
        };
        let plan = self.dk_add_plan(slot, track, &name, len);
        if self.with(|s| s.song.timeline.is_none()) {
            self.status = plan.line;
            return None;
        }
        self.snapshot();
        self.undo_hold += 1;
        let tr = match plan.track {
            Some(t) => t,
            None => {
                let n = self.with(|s| s.song.channels);
                let kit_name = self.with(|s| {
                    s.song
                        .instruments
                        .get(slot as usize - 1)
                        .map_or("Drums".to_string(), |i| i.name.clone())
                });
                if !self.track_insert(n, &kit_name) {
                    self.undo_hold -= 1;
                    return None;
                }
                self.with(|s| crate::chanops::assign_track_instrument(&mut s.song, n, slot));
                n
            }
        };
        let at = plan.at;
        let id = self.with(|s| {
            let own = s
                .song
                .timeline
                .as_deref()
                .and_then(|t| t.tracks.get(tr))
                .is_some_and(|x| x.instr == slot);
            let id = match src {
                AddSrc::Linked(c) => {
                    let t = s.song.timeline.as_deref_mut()?;
                    let from = t.clips.iter().find(|x| x.id == c.id)?.clone();
                    let id = t.new_id();
                    t.clips.push(tl::Clip {
                        id,
                        start: at,
                        track: tr as _,
                        ..from
                    });
                    id
                }
                AddSrc::New(mut d) => {
                    let i = if own { 0 } else { slot };
                    for n in d.notes.iter_mut() {
                        n.instr = i;
                    }
                    let id = tl::new_note_clip(&mut s.song, tr, at)?;
                    let t = s.song.timeline.as_deref_mut()?;
                    let k = t.clips.iter().position(|c| c.id == id)?;
                    t.clips[k].len = d.len;
                    if let tl::ClipKind::Notes { data } = t.clips[k].kind {
                        *t.notes[data as usize].make_mut() = d;
                    }
                    id
                }
            };
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some(id)
        });
        self.undo_hold -= 1;
        let Some(id) = id else {
            self.status = "Couldn't add the pattern to the track".into();
            return None;
        };
        self.dirty = true;
        self.dk.clip = Some(id);
        self.tlv.sel = Some(id);
        self.tlv.sels = vec![id];
        // the timeline starts the flash on its next frame
        self.tlv.flash = Some((id, f64::NAN));
        self.tlv.reveal = Some((tr, at));
        self.ch = tr;
        let mut line = plan.line.replacen("Adds", "Added", 1);
        if plan.track.is_none() {
            line = line.replace(
                "(this kit isn't on a track yet)",
                "(made the track for this kit)",
            );
        }
        self.status = format!("{line}. Undo takes it back");
        Some(id)
    }
}

/// Add to track's target (see `dk_add_plan`)
pub(crate) struct AddPlan {
    /// None = a new track
    pub track: Option<usize>,
    pub at: Tick,
    /// "Adds 'Groove 3' (2 bars) to Track 2 at bar 5"
    pub line: String,
}

/// what Add to track places
pub(crate) enum AddSrc {
    /// a linked copy of this clip
    Linked(Cur),
    /// new note data (a generated groove)
    New(tl::NoteClipData),
}

// ------------------------------------------------------------ lower tabs
impl App {
    fn dk_lower(&mut self, ui: &mut egui::Ui, kit: &mut DrumKitP, cur: Option<&Cur>, slot: u8) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            for (i, t) in [(1u8, "Sound"), (2, "Generate")] {
                let on = self.dk.tab == i || (i == 1 && self.dk.tab == 0);
                if crate::ember::tab(ui, on, t).clicked() {
                    self.dk.tab = i;
                }
            }
        });
        ui.add_space(4.0);
        if self.dk.tab == 2 {
            self.dk_generate(ui, kit, cur, slot, false);
        } else {
            self.dk_sound(ui, kit, false);
        }
    }

    /// the selected pad's sound; compact = wrapped rows in the side panel
    fn dk_sound(&mut self, ui: &mut egui::Ui, kit: &mut DrumKitP, compact: bool) {
        let sel = self.dk.sel;
        let n_pads = kit.pads.len();
        let Some(pad) = kit.pads.get_mut(sel) else {
            return;
        };
        let mut audition = false;
        let row = |ui: &mut egui::Ui, f: &mut dyn FnMut(&mut egui::Ui)| {
            if compact {
                ui.horizontal_wrapped(|ui| f(ui));
            } else {
                ui.horizontal(|ui| f(ui));
            }
        };
        let mut body = |ui: &mut egui::Ui| {
            row(ui, &mut |ui: &mut egui::Ui| {
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
            row(ui, &mut |ui: &mut egui::Ui| {
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
        };
        if compact {
            ui.label(
                egui::RichText::new(format!("Pad {} of {n_pads}", sel + 1))
                    .small()
                    .color(th().ink2),
            );
            body(ui);
        } else {
            let w = ui.available_width();
            let (r, _) = ui.allocate_exact_size(egui::vec2(w, 196.0), egui::Sense::hover());
            card(ui, r, &format!("Sound · pad {} of {n_pads}", sel + 1));
            let inner = egui::Rect::from_min_max(
                r.min + egui::vec2(16.0, 26.0),
                r.max - egui::vec2(16.0, 10.0),
            );
            area(ui, inner, body);
        }
        if audition {
            let k = kit.clone();
            self.dk_audition(&k, sel, self.ch, 1.0);
        }
    }

    fn dk_generate(
        &mut self,
        ui: &mut egui::Ui,
        kit: &mut DrumKitP,
        cur: Option<&Cur>,
        slot: u8,
        compact: bool,
    ) {
        use crate::drumpat::STYLES;
        // what Add to track will do, under the buttons
        let gen_name = format!("Groove {}", self.dk_patterns(slot).0.len() + 1);
        let add_line = match &self.dk.gen.result {
            Some((_, steps, _, _)) => {
                self.dk_add_plan(slot, cur.map(|c| c.track), &gen_name, *steps as Tick * ROW)
                    .line
            }
            None => {
                "Regenerate makes a groove; Add to track then puts it on this kit's track".into()
            }
        };
        let mut act: Option<u8> = None;
        if compact {
            act = self.dk_gen_compact(ui, kit, &add_line, cur.is_some());
        } else {
            let w = ui.available_width();
            let (r, _) = ui.allocate_exact_size(egui::vec2(w, 232.0), egui::Sense::hover());
            card(
                ui,
                r,
                "Generate · a new groove from the pattern library, in this kit",
            );
            ui.painter().text(
                egui::pos2(r.right() - 16.0, r.bottom() - 12.0),
                egui::Align2::RIGHT_BOTTOM,
                &add_line,
                crate::theme::prop(crate::theme::ts::COMPACT),
                th().ink,
            );
            let inner = egui::Rect::from_min_max(
                r.min + egui::vec2(16.0, 26.0),
                r.max - egui::vec2(16.0, 10.0),
            );
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
                            crate::theme::prop(crate::theme::ts::COMPACT),
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
                            format!("from \"{src}\" · {steps} steps · {} hits", notes.len()),
                            crate::theme::prop(crate::theme::ts::SMALL),
                            th().ink2,
                        );
                    }
                }
                ui.vertical(|ui| {
                    let has = g.result.is_some();
                    if ui
                        .add_enabled_ui(has, |ui| {
                            crate::ember::button_ex(ui, "Add to track", false, true, egui::vec2(180.0, 34.0))
                        })
                        .inner
                        .on_hover_text("keep it as a new pattern and place it on this kit's track, in one step (Undo takes it back)")
                        .on_disabled_hover_text("Regenerate first")
                        .clicked()
                    {
                        act = Some(3);
                    }
                    ui.add_space(4.0);
                    let prev = if g.previewing { "Stop preview" } else { "Preview" };
                    if ui
                        .add_enabled_ui(has, |ui| crate::ember::button(ui, prev, g.previewing))
                        .inner
                        .on_hover_text("hear it in this kit, looping at the song tempo (nothing changes yet)")
                        .clicked()
                    {
                        act = Some(0);
                    }
                    if ui
                        .add_enabled_ui(has && cur.is_some(), |ui| crate::ember::button(ui, "Replace pattern", false))
                        .inner
                        .on_hover_text("put it in the pattern being edited instead (every linked copy changes; Undo takes it back)")
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
                });
            });
            });
        }
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
                if let Some((notes, steps, swing, _)) = self.dk.gen.result.clone() {
                    let d = tl::NoteClipData {
                        name: gen_name,
                        len: steps as Tick * ROW,
                        notes,
                        chords: vec![],
                        drum: Some(DrumMeta { swing }),
                    };
                    let tr = cur.map(|c| c.track);
                    self.dk_gen_stop();
                    self.dk_add_to_track(slot, tr, AddSrc::New(d));
                }
            }
            _ => {}
        }
    }

    /// the Generate controls in the side panel: settings, a small result
    /// view, Add to track (primary) with its target line, then the rest
    fn dk_gen_compact(
        &mut self,
        ui: &mut egui::Ui,
        kit: &DrumKitP,
        add_line: &str,
        has_cur: bool,
    ) -> Option<u8> {
        use crate::drumpat::STYLES;
        let mut act = None;
        let g = &mut self.dk.gen;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("dk_style")
                .width(118.0)
                .selected_text(STYLES[g.style.min(STYLES.len() - 1)])
                .show_ui(ui, |ui| {
                    for (i, s) in STYLES.iter().enumerate() {
                        if *s != "Fills" {
                            ui.selectable_value(&mut g.style, i, *s);
                        }
                    }
                });
            ui.label("Fill");
            for (f, l) in [(0u8, "0"), (1, "1"), (2, "2")] {
                if crate::ember::led_item(ui, l, g.fill == f)
                    .on_hover_text(format!("a fill over the last {f} beat(s)"))
                    .clicked()
                {
                    g.fill = f;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Energy");
            let mut e = (g.energy * 100.0).round();
            if ui
                .add(egui::DragValue::new(&mut e).range(0.0..=100.0).suffix(" %"))
                .on_hover_text(ENERGY.tip)
                .changed()
            {
                g.energy = e / 100.0;
            }
            ui.label("Seed");
            ui.add(egui::DragValue::new(&mut g.seed).range(1..=99_999));
        });
        // the result, small
        let (gr, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::hover());
        crate::ember::well(ui.painter(), gr, 4);
        if let Some((notes, steps, _, _)) = &g.result {
            let used: Vec<usize> = (0..kit.pads.len())
                .filter(|&p| notes.iter().any(|n| on_pad(kit, n, p)))
                .collect();
            let lh = ((gr.height() - 4.0) / used.len().max(1) as f32).min(8.0);
            let sw = gr.width() / (*steps).max(1) as f32;
            for (row, &p) in used.iter().enumerate() {
                let col = pad_col(&kit.pads[p]);
                for n in notes.iter().filter(|n| on_pad(kit, n, p)) {
                    let k = dk::step_of(n.start);
                    let cr = egui::Rect::from_min_size(
                        egui::pos2(
                            gr.left() + k as f32 * sw + 0.5,
                            gr.top() + 2.0 + row as f32 * lh,
                        ),
                        egui::vec2((sw - 1.0).max(1.0), (lh - 1.0).max(1.0)),
                    );
                    ui.painter().rect_filled(cr, 1.0, col);
                }
            }
        }
        let has = g.result.is_some();
        let previewing = g.previewing;
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let add = ui
                .add_enabled_ui(has, |ui| {
                    crate::ember::button_ex(ui, "Add to track", false, true, egui::vec2(120.0, 28.0))
                })
                .inner
                .on_hover_text("keep it as a new pattern and place it on this kit's track, in one step (Undo takes it back)")
                .on_disabled_hover_text("Regenerate first");
            self.dk.ctl.push(("gen_add", add.rect));
            if add.clicked() {
                act = Some(3);
            }
            let p = ui
                .add_enabled_ui(has, |ui| {
                    crate::ember::button(ui, if previewing { "Stop" } else { "Preview" }, previewing)
                })
                .inner
                .on_hover_text("hear it in this kit, looping at the song tempo (nothing changes yet)");
            if p.clicked() {
                act = Some(0);
            }
            let rg = crate::ember::button(ui, "Regenerate", false)
                .on_hover_text("a new groove with the next seed");
            self.dk.ctl.push(("regen", rg.rect));
            if rg.clicked() {
                act = Some(2);
            }
        });
        ui.add(
            egui::Label::new(
                egui::RichText::new(add_line)
                    .size(crate::theme::ts::px(crate::theme::ts::LABEL))
                    .color(th().ink),
            )
            .wrap(),
        );
        if ui
            .add_enabled_ui(has && has_cur, |ui| crate::ember::button(ui, "Replace pattern", false))
            .inner
            .on_hover_text("put it in the pattern being edited instead (every linked copy changes; Undo takes it back)")
            .clicked()
        {
            act = Some(1);
        }
        act
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
            "Preview: the generated groove in this kit (Add to track puts it on the timeline)"
                .into();
    }

    fn dk_gen_stop(&mut self) {
        if std::mem::take(&mut self.dk.gen.previewing) {
            self.with(|s| drop(s.engine.set_pattern_preview(None)));
        }
    }
}

/// the built-in groove kit `name` starts with: its first 1-bar one, else its first
pub(crate) fn starter_groove(name: &str) -> Option<&'static crate::drumpat::Pat> {
    let mine = || {
        crate::drumpat::PATTERNS
            .iter()
            .filter(move |p| p.kit == name)
    };
    mine().find(|p| p.bars == 1).or_else(|| mine().next())
}

/// Dice kit's pick: a kit from `seed`, never `not` (the current one)
fn dice_kit(seed: u32, not: &str) -> String {
    let all: Vec<String> = dk::presets()
        .into_iter()
        .map(|(n, _, _)| n)
        .filter(|n| !n.eq_ignore_ascii_case(not))
        .collect();
    let h = seed.wrapping_mul(0x9E37_79B9).rotate_left(13);
    all.get(h as usize % all.len().max(1))
        .cloned()
        .unwrap_or_else(|| not.to_string())
}

#[cfg(test)]
#[path = "drumkitview_tests.rs"]
mod tests;

impl App {
    /// the step grid's pie (objpie.rs): pattern clip `clip` (notes `data`)
    /// on `track`, the lane of pad `pad`
    pub(crate) fn dk_pie(
        &mut self,
        clip: u32,
        data: usize,
        track: usize,
        pad: usize,
        label: &str,
        child: Option<&str>,
    ) {
        let slot = self.dk.inst as u8 + 1;
        let Some((kit, d, start, len)) = self.with(|s| {
            let kit = dk::kit_of(s.song.instruments.get(self.dk.inst)?)?.clone();
            let t = s.song.timeline.as_deref()?;
            let c = t.clips.iter().find(|c| c.id == clip)?;
            Some((kit, (**t.notes.get(data)?).clone(), c.start, c.len))
        }) else {
            return;
        };
        let ins = if self.dk_track(slot) == Some(track) {
            0
        } else {
            slot
        };
        let key = kit.pads.get(pad).map(|p| p.key);
        let swing = d.drum.as_ref().map_or(0.0, |m| m.swing);
        let steps = ((d.len + ROW - 1) / ROW).max(1) as usize;
        let mut nd = d.clone();
        match (label, child) {
            ("Play", _) => {
                let c = Cur {
                    id: clip,
                    data,
                    track,
                    start,
                    len,
                    d,
                };
                self.dk_play(&c);
                return;
            }
            ("Generate", _) => {
                self.dk.tab = 2;
                self.status = "Generate: a new groove from the pattern library, in this kit".into();
                return;
            }
            ("Copy pattern", _) => {
                let c = Cur {
                    id: clip,
                    data,
                    track,
                    start,
                    len,
                    d,
                };
                self.dk_new_pattern(track, Some(&c));
                self.status =
                    "A copy of this pattern of its own, right after it: change it freely".into();
                return;
            }
            ("Fill", c) => {
                let Some(key) = key else { return };
                let every = match c {
                    Some("Every 8th") => 2,
                    Some("Every 16th") => 1,
                    _ => 4,
                };
                nd.notes.retain(|n| !on_pad(&kit, n, pad));
                for k in (0..steps).step_by(every) {
                    dk::toggle(&mut nd.notes, key, k, true, swing, tl::vel_from_7(100), ins);
                }
                self.status = format!(
                    "Pad lane {} filled ({})",
                    pad + 1,
                    c.unwrap_or("Every beat").to_lowercase()
                );
            }
            ("Variation", _) => {
                // off-beat hits move a step later, every third one (repeatable)
                let mut moved = 0;
                let mut i = 0;
                for n in nd.notes.iter_mut() {
                    let k = dk::step_of(n.start);
                    if !k.is_multiple_of(4) {
                        i += 1;
                        if i % 3 == 1 && k + 1 < steps {
                            n.start = dk::step_tick(k + 1, swing);
                            moved += 1;
                        }
                    }
                }
                nd.notes.sort_by_key(|n| (n.start, n.key));
                nd.notes
                    .dedup_by(|a, b| a.start == b.start && a.key == b.key);
                self.status = format!("Variation: {moved} off-beat hits moved a step");
            }
            ("Clear", Some("All lanes")) => {
                nd.notes.clear();
                self.status = "Pattern cleared (Ctrl+Z brings it back)".into();
            }
            ("Clear", _) => {
                nd.notes.retain(|n| !on_pad(&kit, n, pad));
                self.status = format!("Pad lane {} cleared", pad + 1);
            }
            _ => return,
        }
        self.dk_write(data, nd);
    }
}

impl App {
    /// the Rack card's face for a Drum Kit: a pad picker, then the selected
    /// pad's sound knobs and its level in rows of `cols` (at most `rows`)
    pub(super) fn dk_rack_face(&mut self, ui: &mut egui::Ui, idx: usize, cols: usize, rows: usize) {
        let Some(mut kit) = self.with(|s| dk::kit_of(s.song.instruments.get(idx)?).cloned()) else {
            return;
        };
        if self.dk.inst != idx {
            self.dk = DkUi {
                inst: idx,
                ..Default::default()
            };
        }
        if kit.pads.is_empty() {
            ui.label(egui::RichText::new("No pads: Advanced › adds them").color(th().ink2));
            return;
        }
        self.dk.sel = self.dk.sel.min(kit.pads.len() - 1);
        let orig = kit.clone();
        let sel = self.dk.sel;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Pad").small().color(th().ink2));
            let name = |_i: usize, p: &dk::Pad| format!("{} {}", dk::key_name(p.key), p.name);
            egui::ComboBox::from_id_salt(("rack_dk_pad", idx))
                .width(150.0)
                .selected_text(name(sel, &kit.pads[sel]))
                .show_ui(ui, |ui| {
                    for (i, p) in kit.pads.iter().enumerate() {
                        ui.selectable_value(&mut self.dk.sel, i, name(i, p));
                    }
                })
                .response
                .on_hover_text("the pad whose sound these knobs edit");
        });
        ui.add_space(2.0);
        let sel = self.dk.sel;
        let pad = &mut kit.pads[sel];
        let defs = pad.drum.kind.macros();
        let cols = cols.max(1);
        let max = cols * rows.max(1);
        // the pad's macros, then Level (always shown: the last cell)
        let nm = defs.len().min(max.saturating_sub(1));
        let cells: Vec<Option<usize>> = (0..nm).map(Some).chain([None]).collect();
        for row in cells.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for c in row {
                    let (def, mut n): (&MacroDef, f32) = match c {
                        Some(i) => (&defs[*i], pad.drum.macros[*i]),
                        None => (&MIX_LEVEL, MIX_LEVEL.unmap(pad.drum.level)),
                    };
                    let tip = def.tip.to_string();
                    let vt = def.show(n);
                    let out = super::knob::knob(
                        ui,
                        egui::Id::unique(("rack_dk", idx, sel, *c)),
                        &mut n,
                        super::knob::KnobArgs {
                            name: def.name,
                            caption: None,
                            value_text: vt,
                            default: (def.dice.0 + def.dice.1) * 0.5,
                            steps: 0.0,
                            enabled: true,
                            highlight: false,
                            badge: None,
                            locked: None,
                            tip: &tip,
                            rings: &[],
                            live: None,
                            auto_led: None,
                            mod_dot: None,
                        },
                    );
                    if out.changed {
                        match c {
                            Some(i) => pad.drum.macros[*i] = n,
                            None => pad.drum.level = MIX_LEVEL.map(n),
                        }
                    }
                }
            });
        }
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
    }
}
