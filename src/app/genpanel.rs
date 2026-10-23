//! The Generators panel (Generators mockup B, picked by Del (postmanpat)):
//! a wide panel with a GENERATORS header and pill tabs (Chords | Melody,
//! room for more), the generator on the left and a fixed WRITE card on the
//! right: the target track, the bars, what the write does to the notes
//! already there, and one highlighted "Write to timeline" (one Ctrl+Z).
//! Narrow windows stack the card under the generator.
//!
//! Chords is the chord generator (`crate::chordgen`, written through the
//! chord lane's `cl_insert` / "Replace notes?" confirm); Melody is
//! `crate::melodygen` over the Chords tab's progression or the clip's
//! chord lane.
use super::chordlane::{mark_colour, BAR, CONFIRM_OVER};
use super::pianoview::Target;
use super::*;
use crate::chordgen::{self as cg, ChordMark, Mode, Voicing};
use crate::chords::{Key, Scale};
use crate::melodygen::{self as mg, Contour, Locks, MNote, MelOpts, Motif, Rhythm};
use crate::timeline::{self as tl, Tick, PPQ};
use std::time::Instant;

/// the panel's tabs
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum GenTab {
    #[default]
    Chords,
    Melody,
}

impl GenTab {
    pub(crate) const ALL: [GenTab; 2] = [GenTab::Chords, GenTab::Melody];
    pub(crate) fn name(self) -> &'static str {
        match self {
            GenTab::Chords => "Chords",
            GenTab::Melody => "Melody",
        }
    }
}

/// where the melody's chords come from
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MelFrom {
    /// the Chords tab's progression (written or not)
    Chords,
    /// the open clip's chord lane
    Clip,
}

/// the Melody tab
pub(crate) struct MelDlg {
    pub(crate) opts: MelOpts,
    /// the Advanced fold is open
    pub(crate) adv: bool,
    pub(crate) lock_rhythm: bool,
    pub(crate) lock_notes: bool,
    locked_rhythm: Vec<(Tick, Tick)>,
    locked_keys: Vec<u8>,
    /// the melody shown (what Write writes; the locks keep parts of it)
    pub(crate) last: Vec<MNote>,
    /// None: the clip's chord lane when it has chords and the Chords tab
    /// wasn't shown since the panel opened, else the Chords tab
    pub(crate) from: Option<MelFrom>,
    /// write to a new "Melody" track (the default) or the open clip
    pub(crate) new_track: bool,
    /// writing to the open clip: on top of its notes (else replace)
    pub(crate) on_top: bool,
}

impl Default for MelDlg {
    fn default() -> Self {
        Self {
            opts: MelOpts::default(),
            adv: false,
            lock_rhythm: false,
            lock_notes: false,
            locked_rhythm: vec![],
            locked_keys: vec![],
            last: vec![],
            from: None,
            new_track: true,
            on_top: false,
        }
    }
}

/// a button in the theme's accent with readable ink on every theme
fn accent_btn(ui: &mut egui::Ui, text: &str, w: f32, h: f32) -> egui::Response {
    let th = th();
    ui.add_sized(
        [w, h],
        egui::Button::new(
            egui::RichText::new(text)
                .strong()
                .size(14.0)
                .color(accent_ink()),
        )
        .fill(th.accent),
    )
}

fn accent_ink() -> Color32 {
    if th().light {
        Color32::WHITE
    } else {
        Color32::from_rgb(20, 20, 24)
    }
}

fn cap(ui: &mut egui::Ui, t: &str) {
    ui.label(egui::RichText::new(t).color(th().ink2));
}

/// the "what happens" line: an ochre "!" and the text
fn warn(ui: &mut egui::Ui, text: &str) {
    let th = th();
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 7.5, th.ochre);
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            "!",
            FontId::proportional(11.0),
            Color32::BLACK,
        );
        ui.add(egui::Label::new(egui::RichText::new(text).color(th.ochre)).wrap());
    });
}

/// a row of toggle buttons; returns the clicked index
fn seg(ui: &mut egui::Ui, opts: &[&str], on: usize, w: f32) -> Option<usize> {
    let mut hit = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, o) in opts.iter().enumerate() {
            if ui
                .add_sized([w, 22.0], egui::Button::new(*o).selected(i == on))
                .clicked()
            {
                hit = Some(i);
            }
        }
    });
    hit
}

fn pct(ui: &mut egui::Ui, v: &mut f32, label: &str, w: f32, tip: &str) {
    ui.spacing_mut().slider_width = w;
    let mut x = *v * 100.0;
    ui.add(
        egui::Slider::new(&mut x, 0.0..=100.0)
            .trailing_fill(true)
            .fixed_decimals(0)
            .suffix("%")
            .text(label),
    )
    .on_hover_text(tip);
    *v = x / 100.0;
}

/// the progression strip: one coloured block per chord
fn strip(ui: &mut egui::Ui, key: Key, marks: &[ChordMark], w: f32, h: f32) -> Rect {
    let (sr, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
    let (t0, t1) = span_of(marks);
    let total = (t1 - t0).max(1) as f32;
    let p = ui.painter_at(sr);
    if marks.is_empty() {
        p.rect_filled(sr, 3.0, th().crt_bot);
    }
    for m in marks {
        let x0 = sr.left() + sr.width() * ((m.start - t0) as f32 / total);
        let x1 = sr.left() + sr.width() * ((m.end() - t0) as f32 / total);
        let r = Rect::from_min_max(
            Pos2::new(x0 + 1.0, sr.top()),
            Pos2::new(x1 - 1.0, sr.bottom()),
        );
        p.rect_filled(r, 3.0, mark_colour(key, m));
        if r.width() > 20.0 {
            p.text(
                r.left_top() + egui::vec2(5.0, if h > 30.0 { 5.0 } else { 3.0 }),
                egui::Align2::LEFT_TOP,
                m.name(key),
                FontId::proportional(if h > 40.0 { 14.0 } else { 11.0 }),
                Color32::WHITE,
            );
            if h > 30.0 {
                p.text(
                    r.left_bottom() + egui::vec2(5.0, -5.0),
                    egui::Align2::LEFT_BOTTOM,
                    m.numeral(key),
                    FontId::proportional(11.0),
                    Color32::from_white_alpha(190),
                );
            }
        }
    }
    sr
}

/// (first start, last end) of `marks`
fn span_of(marks: &[ChordMark]) -> (Tick, Tick) {
    let a = marks.iter().map(|m| m.start).min().unwrap_or(0);
    let b = marks.iter().map(|m| m.end()).max().unwrap_or(a);
    (a, b)
}

/// "bars 2–9" / "bar 3" (1-based bars of the note data)
fn bars_text(a: Tick, b: Tick) -> String {
    if b > a + BAR {
        format!("bars {}–{}", a / BAR + 1, (b - 1) / BAR + 1)
    } else {
        format!("bar {}", a / BAR + 1)
    }
}

fn key_name(key: Key) -> String {
    format!(
        "{} {}",
        key.note_name(key.root),
        key.scale.name().to_lowercase()
    )
}

/// what the panel asks for this frame
#[derive(Default)]
struct Acts {
    close: bool,
    tab: Option<GenTab>,
    preview: bool,
    write: bool,
}

/// the open clip's track / instrument
struct Dest {
    track: Option<usize>,
    inst: String,
}

impl App {
    /// open the panel on the Melody tab (track pie "Melody…")
    pub(crate) fn gen_open_melody(&mut self, at: Tick) {
        self.cl_open_gen_at(at);
        self.cl.gen.tab = GenTab::Melody;
        self.cl.gen.chords_seen = false;
        self.cl.gen.mel.from = None;
    }

    fn gen_dest(&self, t: Target) -> Dest {
        let track = match t {
            Target::Notes { clip: Some(id), .. } => self.with(|s| {
                s.song
                    .timeline
                    .as_deref()
                    .and_then(|tl| tl.clips.iter().find(|x| x.id == id))
                    .map(|c| c.track as usize)
            }),
            _ => None,
        };
        let inst = self.with(|s| {
            track
                .and_then(|k| s.song.channel_names.get(k).cloned())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "the track".into())
        });
        Dest { track, inst }
    }

    /// the Generators panel (while `cl.gen.open`)
    pub(super) fn gen_panel(&mut self, ctx: &egui::Context, t: Target) {
        if !self.cl.gen.open {
            return;
        }
        let th = th();
        let scr = ctx.content_rect();
        let w = (scr.width() - 24.0).clamp(300.0, 1180.0);
        let stacked = w < 980.0;
        let top = (scr.height() * 0.08).clamp(6.0, 64.0);
        let dest = self.gen_dest(t);
        let res = cg::generate(&self.cl.gen.opts);
        if self.cl.gen.tab == GenTab::Chords {
            self.cl.gen.chords_seen = true;
        }
        // the melody (every frame: it is cheap and follows the chords)
        let mel = (self.cl.gen.tab == GenTab::Melody).then(|| self.gen_melody(t, &res));
        let mut acts = Acts::default();
        egui::Window::new("Generators")
            .id(egui::Id::unique("gen_panel"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .min_width(w)
            .max_width(w)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, top))
            // over the whole window (the roll's bottom panel included)
            .constrain_to(scr)
            .show(ctx, |ui| {
                ui.set_width(w);
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                // the header: GENERATORS, the pill tabs, the hint, Close
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("GENERATORS")
                            .strong()
                            .size(13.0)
                            .color(th.ink2),
                    );
                    ui.add_space(12.0);
                    egui::Frame::new()
                        .fill(well())
                        .corner_radius(14.0)
                        .inner_margin(3.0)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            for tab in GenTab::ALL {
                                let on = self.cl.gen.tab == tab;
                                let b = egui::Button::new(
                                    egui::RichText::new(tab.name()).strong().color(if on {
                                        accent_ink()
                                    } else {
                                        th.ink2
                                    }),
                                )
                                .fill(if on { th.accent } else { Color32::TRANSPARENT })
                                .corner_radius(12.0);
                                if ui.add_sized([104.0, 26.0], b).clicked() && !on {
                                    acts.tab = Some(tab);
                                }
                            }
                        });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized([64.0, 24.0], egui::Button::new("Close"))
                            .on_hover_text("close the generators (nothing is written)")
                            .clicked()
                        {
                            acts.close = true;
                        }
                        if !stacked {
                            cap(
                                ui,
                                match self.cl.gen.tab {
                                    GenTab::Chords => "a chord progression for the timeline",
                                    GenTab::Melody => "a tune over your chords",
                                },
                            );
                        }
                    });
                });
                ui.separator();
                let card_w = if stacked {
                    w
                } else {
                    (w * 0.32).clamp(300.0, 380.0)
                };
                let left_w = if stacked { w } else { w - card_w - 18.0 };
                // an auto-sized window offers last frame's height, so ask
                // for the full screen height whenever the content is taller
                let body_h = (scr.height() - top - 100.0).max(200.0);
                egui::ScrollArea::vertical()
                    .max_height(body_h)
                    .min_scrolled_height(body_h)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let left = |app: &mut App, ui: &mut egui::Ui, acts: &mut Acts| match &mel {
                            None => app.gen_chords_left(ui, &res, left_w, acts),
                            Some(m) => app.gen_melody_left(ui, m, left_w, acts),
                        };
                        if stacked {
                            ui.vertical(|ui| left(self, ui, &mut acts));
                            ui.add_space(6.0);
                        } else {
                            ui.horizontal_top(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(left_w);
                                    left(self, ui, &mut acts);
                                });
                                ui.add_space(6.0);
                                ui.vertical(|ui| {
                                    ui.set_width(card_w);
                                    match &mel {
                                        None => self
                                            .gen_chords_card(ui, t, &res, &dest, card_w, &mut acts),
                                        Some(m) => {
                                            self.gen_melody_card(ui, m, &dest, card_w, &mut acts)
                                        }
                                    }
                                });
                            });
                        }
                        if stacked {
                            match &mel {
                                None => self.gen_chords_card(ui, t, &res, &dest, card_w, &mut acts),
                                Some(m) => self.gen_melody_card(ui, m, &dest, card_w, &mut acts),
                            }
                        }
                    });
            });
        if let Some(tab) = acts.tab {
            self.cl.gen.tab = tab;
        }
        match &mel {
            None => {
                if acts.preview {
                    self.cl_play_seq(&res.marks);
                }
                if acts.write {
                    let (w, v, o) = (self.cl.gen.write, self.cl.gen.voicing, self.cl.gen.octave);
                    if self.with(|s| s.song.info.key.is_none()) {
                        self.with(|s| s.song.info.key = Some(res.key));
                    }
                    self.cl_insert(t, res.marks, w, v, o);
                    self.cl.gen.open = false;
                }
            }
            Some(m) => {
                if acts.preview {
                    self.gen_play_notes(&m.notes);
                }
                if acts.write {
                    self.gen_write_melody(t, m);
                    self.cl.gen.open = false;
                }
            }
        }
        if acts.close {
            self.cl.gen.open = false;
        }
    }

    // ------------------------------------------------------------ chords

    fn gen_chords_left(&mut self, ui: &mut egui::Ui, res: &cg::GenResult, w: f32, acts: &mut Acts) {
        let th = th();
        // the style `res` was made in: a style button clicked below changes
        // `opts.style` this frame, and `res.prog` indexes the old style's list
        let res_style = self.cl.gen.opts.style.min(cg::STYLES.len() - 1);
        let res_len = cg::STYLES[res_style]
            .progs
            .get(res.prog)
            .map_or(0, |p| cg::parse_prog(p).len());
        let key = res.key;
        let mut seen = vec![];
        let nums: Vec<String> = res
            .marks
            .iter()
            .filter_map(|m| {
                let n = m.numeral(key);
                (seen.len() < 16 && seen.len() < res_len).then(|| {
                    seen.push(n.clone());
                    n
                })
            })
            .collect();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(key_name(key)).strong().size(15.0));
            ui.label(egui::RichText::new(nums.join(" - ")).color(th.ink2));
        });
        strip(ui, key, &res.marks, w, 72.0);
        let g = &mut self.cl.gen;
        ui.horizontal(|ui| {
            if ui
                .add_sized([110.0, 26.0], egui::Button::new("Another one"))
                .on_hover_text("roll a new progression in this style")
                .clicked()
            {
                g.opts.seed = g.opts.seed.wrapping_add(1);
                g.opts.prog = None;
            }
            if ui
                .add_sized([80.0, 26.0], egui::Button::new("Preview"))
                .on_hover_text("hear it on the track's instrument")
                .clicked()
            {
                acts.preview = true;
            }
        });
        cap(ui, "Style");
        let cols = ((w + 6.0) / 124.0).floor().max(2.0) as usize;
        let bw = (w - 6.0 * (cols as f32 - 1.0)) / cols as f32;
        egui::Grid::new("gen_styles")
            .spacing(egui::vec2(6.0, 6.0))
            .show(ui, |ui| {
                for (i, s) in cg::STYLES.iter().enumerate() {
                    let on = g.opts.style == i;
                    if ui
                        .add_sized([bw, 24.0], egui::Button::new(s.name).selected(on))
                        .on_hover_text(s.desc)
                        .clicked()
                    {
                        g.opts.style = i;
                        g.opts.prog = None;
                    }
                    if i % cols == cols - 1 {
                        ui.end_row();
                    }
                }
            });
        ui.label(
            egui::RichText::new(cg::STYLES[g.opts.style].desc)
                .italics()
                .color(th.ink2),
        );
        ui.horizontal_wrapped(|ui| {
            cap(ui, "Key");
            let k = Key::new(
                0,
                if g.opts.mode == Mode::Major {
                    Scale::Major
                } else {
                    Scale::Minor
                },
            );
            ui.spacing_mut().item_spacing.x = 2.0;
            for r in 0..12u8 {
                let name = match r {
                    1 => "C#",
                    3 => "Eb",
                    6 => "F#",
                    8 => "Ab",
                    10 => "Bb",
                    _ => k.note_name(r),
                };
                if ui
                    .add_sized(
                        [30.0, 22.0],
                        egui::Button::new(name).selected(g.opts.root == r),
                    )
                    .clicked()
                {
                    g.opts.root = r;
                }
            }
            ui.add_space(10.0);
            for (m, l) in [
                (Mode::Any, "Any"),
                (Mode::Major, "Major"),
                (Mode::Minor, "Minor"),
            ] {
                if ui
                    .add_sized(
                        [50.0, 22.0],
                        egui::Button::new(l).selected(g.opts.mode == m),
                    )
                    .clicked()
                {
                    g.opts.mode = m;
                    g.opts.prog = None;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            cap(ui, "Length");
            ui.spacing_mut().item_spacing.x = 2.0;
            for b in [4u32, 8, 16, 32] {
                if ui
                    .add_sized(
                        [30.0, 22.0],
                        egui::Button::new(b.to_string()).selected(g.opts.bars == b),
                    )
                    .on_hover_text(format!("{b} bars"))
                    .clicked()
                {
                    g.opts.bars = b;
                }
            }
            ui.add_space(14.0);
            cap(ui, "Chord speed");
            for (s, l) in [(-1i8, "Slower"), (0, "Style"), (1, "Faster")] {
                if ui
                    .add_sized(
                        [54.0, 22.0],
                        egui::Button::new(l).selected(g.opts.speed == s),
                    )
                    .clicked()
                {
                    g.opts.speed = s;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            let sw = ((w - 200.0) / 2.0).clamp(80.0, 200.0);
            pct(
                ui,
                &mut g.opts.colour,
                "Colour",
                sw,
                "7ths, 9ths and suspensions",
            );
            ui.add_space(8.0);
            pct(
                ui,
                &mut g.opts.variation,
                "Variation",
                sw,
                "swap in related chords (relative, sus, 7ths)",
            );
        });
        ui.checkbox(&mut g.opts.walking_bass, "Walking bass (inversions)")
            .on_hover_text("slash chords: the bass moves to the nearest chord tone");
    }

    #[allow(clippy::too_many_arguments)]
    fn gen_chords_card(
        &mut self,
        ui: &mut egui::Ui,
        t: Target,
        res: &cg::GenResult,
        dest: &Dest,
        w: f32,
        acts: &mut Acts,
    ) {
        let th = th();
        let (a, b) = span_of(&res.marks);
        let scope = self.cl_scope(t, a, b);
        let tr = dest
            .track
            .map_or("this clip".to_string(), |k| format!("track {:02}", k + 1));
        card(ui, w, |ui| {
            let g = &mut self.cl.gen;
            card_title(ui);
            ui.horizontal(|ui| {
                cap(ui, "Target");
                ui.label(
                    egui::RichText::new(match dest.track {
                        Some(k) => format!("Track {:02} · {}", k + 1, dest.inst),
                        None => "this clip".into(),
                    })
                    .strong(),
                );
            });
            ui.add(egui::Label::new(egui::RichText::new(&scope).color(th.ink2).small()).wrap());
            ui.horizontal(|ui| {
                cap(ui, "Bars");
                ui.label(egui::RichText::new(format!("{}", g.opts.start_bar + 1)).strong());
                if ui
                    .small_button(" - ")
                    .on_hover_text("start a bar earlier")
                    .clicked()
                {
                    g.opts.start_bar = g.opts.start_bar.saturating_sub(1);
                }
                if ui
                    .small_button(" + ")
                    .on_hover_text("start a bar later")
                    .clicked()
                {
                    g.opts.start_bar += 1;
                }
                cap(ui, "to");
                ui.label(egui::RichText::new(format!("{}", (b - 1).max(a) / BAR + 1)).strong());
                cap(ui, "(Length sets the end)");
            });
            cap(ui, "Write");
            let mode = if !g.write {
                0
            } else if g.on_top {
                2
            } else {
                1
            };
            let sw = ((w - 40.0) / 3.0).clamp(70.0, 110.0);
            match seg(
                ui,
                &["Markers only", "Replace notes", "Notes on top"],
                mode,
                sw,
            ) {
                Some(0) => g.write = false,
                Some(1) => (g.write, g.on_top) = (true, false),
                Some(2) => (g.write, g.on_top) = (true, true),
                _ => {}
            }
            ui.add_enabled_ui(g.write, |ui| {
                ui.horizontal(|ui| {
                    cap(ui, "Voicing");
                    egui::ComboBox::from_id_salt("cl_voicing")
                        .width(90.0)
                        .selected_text(g.voicing.name())
                        .show_ui(ui, |ui| {
                            for v in Voicing::ALL {
                                ui.selectable_value(&mut g.voicing, v, v.name());
                            }
                        });
                    cap(ui, "Octave");
                    ui.add(egui::DragValue::new(&mut g.octave).range(1..=7));
                });
            });
            ui.add_space(2.0);
            let bars = bars_text(a, b);
            warn(
                ui,
                &if !g.write {
                    format!("writes chord markers only in {bars} on {tr} (notes untouched)")
                } else if g.on_top {
                    format!("adds notes on top in {bars} on {tr} (keeps the notes there)")
                } else {
                    format!(
                        "replaces notes in {bars} on {tr} (inside each chord's span; asks first \
                         if that deletes more than {CONFIRM_OVER})"
                    )
                },
            );
            ui.add_space(4.0);
            if accent_btn(ui, "Write to timeline", ui.available_width(), 38.0).clicked() {
                acts.write = true;
            }
            ui.vertical_centered(|ui| cap(ui, "one Ctrl+Z takes it back"));
        });
    }

    // ------------------------------------------------------------ melody

    /// the melody's chords, key and span, and the melody itself
    fn gen_melody(&mut self, t: Target, res: &cg::GenResult) -> MelView {
        let clip_marks = self.cl_marks(t);
        let g = &self.cl.gen;
        let from = g
            .mel
            .from
            .unwrap_or(if clip_marks.is_empty() || g.chords_seen {
                MelFrom::Chords
            } else {
                MelFrom::Clip
            });
        let from = if clip_marks.is_empty() {
            MelFrom::Chords
        } else {
            from
        };
        let (key, marks) = match from {
            MelFrom::Chords => (res.key, res.marks.clone()),
            MelFrom::Clip => (
                self.with(|s| s.song.info.key)
                    .unwrap_or_else(|| guess_key(&clip_marks)),
                clip_marks.clone(),
            ),
        };
        let (start, end) = span_of(&marks);
        let m = &mut self.cl.gen.mel;
        let locks = Locks {
            rhythm: m.lock_rhythm.then_some(m.locked_rhythm.as_slice()),
            pitches: m.lock_notes.then_some(m.locked_keys.as_slice()),
        };
        let notes = mg::generate(key, &marks, start, end, &m.opts, locks);
        m.last = notes.clone();
        MelView {
            key,
            marks,
            start,
            end,
            notes,
            from,
            clip_has_chords: !clip_marks.is_empty(),
        }
    }

    fn gen_melody_left(&mut self, ui: &mut egui::Ui, mv: &MelView, w: f32, acts: &mut Acts) {
        let th = th();
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(key_name(mv.key)).strong().size(15.0));
            ui.add_space(8.0);
            cap(ui, "Over");
            let on = usize::from(mv.from == MelFrom::Clip);
            ui.add_enabled_ui(true, |ui| {
                let hit = seg(ui, &["Chords tab", "Clip's chord lane"], on, 120.0);
                match hit {
                    Some(0) => self.cl.gen.mel.from = Some(MelFrom::Chords),
                    Some(1) if mv.clip_has_chords => self.cl.gen.mel.from = Some(MelFrom::Clip),
                    Some(1) => {
                        self.status =
                            "This clip has no chords yet (the Chords tab writes them)".into()
                    }
                    _ => {}
                }
            });
        });
        melody_view(ui, mv, w, 140.0);
        strip(ui, mv.key, &mv.marks, w, 22.0);
        let m = &mut self.cl.gen.mel;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_sized([60.0, 26.0], egui::Button::new("Dice"))
                .on_hover_text("a random seed")
                .clicked()
            {
                let r = Instant::now().elapsed().as_nanos() as u64
                    ^ std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(7, |d| d.as_nanos() as u64);
                m.opts.seed = r % 100_000;
            }
            if ui
                .add_sized([110.0, 26.0], egui::Button::new("Another one"))
                .on_hover_text("the next seed")
                .clicked()
            {
                m.opts.seed = m.opts.seed.wrapping_add(1) % 100_000;
            }
            if ui
                .add_sized([80.0, 26.0], egui::Button::new("Preview"))
                .on_hover_text("hear it on the track's instrument")
                .clicked()
            {
                acts.preview = true;
            }
            cap(ui, "Seed");
            ui.add(egui::DragValue::new(&mut m.opts.seed).range(0..=99_999));
            ui.add_space(8.0);
            let was = (m.lock_rhythm, m.lock_notes);
            ui.checkbox(&mut m.lock_rhythm, "Lock rhythm")
                .on_hover_text("keep these note times while you change the rest");
            ui.checkbox(&mut m.lock_notes, "Lock notes")
                .on_hover_text("keep these pitches (in order) while the rhythm changes");
            if m.lock_rhythm && !was.0 {
                m.locked_rhythm = m.last.iter().map(|n| (n.start, n.len)).collect();
            }
            if m.lock_notes && !was.1 {
                m.locked_keys = m.last.iter().map(|n| n.key).collect();
            }
        });
        egui::Grid::new("gen_mel_main")
            .num_columns(2)
            .spacing(egui::vec2(8.0, 6.0))
            .show(ui, |ui| {
                cap(ui, "Rhythm");
                let names: Vec<&str> = Rhythm::ALL.iter().map(|r| r.name()).collect();
                let on = Rhythm::ALL
                    .iter()
                    .position(|&r| r == m.opts.rhythm)
                    .unwrap_or(0);
                if let Some(i) = seg(ui, &names, on, ((w - 90.0) / 6.0 - 2.0).clamp(54.0, 96.0)) {
                    m.opts.rhythm = Rhythm::ALL[i];
                }
                ui.end_row();
                cap(ui, "Contour");
                let names: Vec<&str> = Contour::ALL.iter().map(|r| r.name()).collect();
                let on = Contour::ALL
                    .iter()
                    .position(|&r| r == m.opts.contour)
                    .unwrap_or(0);
                if let Some(i) = seg(ui, &names, on, 60.0) {
                    m.opts.contour = Contour::ALL[i];
                }
                ui.end_row();
                cap(ui, "Motif");
                let names: Vec<&str> = Motif::ALL.iter().map(|r| r.name()).collect();
                let on = Motif::ALL
                    .iter()
                    .position(|&r| r == m.opts.motif)
                    .unwrap_or(0);
                if let Some(i) = seg(ui, &names, on, 0.0_f32.max(60.0)) {
                    m.opts.motif = Motif::ALL[i];
                }
                ui.end_row();
            });
        ui.horizontal_wrapped(|ui| {
            let sw = ((w - 330.0) / 2.0).clamp(80.0, 170.0);
            ui.add_enabled_ui(m.opts.rhythm != Rhythm::Euclid, |ui| {
                pct(
                    ui,
                    &mut m.opts.density,
                    "Density",
                    sw,
                    "how many notes (strong beats first)",
                );
            });
            ui.spacing_mut().slider_width = sw;
            ui.add(
                egui::Slider::new(&mut m.opts.range, 12..=36)
                    .trailing_fill(true)
                    .suffix(" st")
                    .text("Range"),
            )
            .on_hover_text("from the lowest to the highest note, in semitones");
            cap(ui, "Octave");
            ui.add(egui::DragValue::new(&mut m.opts.octave).range(1..=8))
                .on_hover_text("the lowest note's octave");
        });
        let rr = ui
            .add(egui::Button::new("Advanced...").selected(m.adv))
            .on_hover_text("Euclid rhythm, leaps, chord tones, variation, rests, the ending");
        if rr.clicked() {
            m.adv = !m.adv;
        }
        if m.adv {
            egui::Frame::new()
                .stroke(egui::Stroke::new(1.0, th.line2))
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_width(w - 18.0);
                    ui.horizontal_wrapped(|ui| {
                        cap(ui, "Euclid");
                        ui.add(
                            egui::DragValue::new(&mut m.opts.pulses)
                                .range(0..=32)
                                .prefix("pulses "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut m.opts.steps)
                                .range(1..=32)
                                .prefix("steps "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut m.opts.rotate)
                                .range(0..=31)
                                .prefix("rotate "),
                        );
                        m.opts.pulses = m.opts.pulses.min(m.opts.steps);
                        ui.label(
                            egui::RichText::new(mg::euclid_str(&mg::euclid(
                                m.opts.pulses,
                                m.opts.steps,
                                m.opts.rotate,
                            )))
                            .monospace()
                            .color(th.ink2),
                        )
                        .on_hover_text("one bar (Rhythm: Euclid)");
                    });
                    let sw = ((w - 330.0) / 3.0).clamp(70.0, 130.0);
                    ui.horizontal_wrapped(|ui| {
                        pct(
                            ui,
                            &mut m.opts.leap,
                            "Leap",
                            sw,
                            "how often the tune jumps (a jump steps back after)",
                        );
                        pct(
                            ui,
                            &mut m.opts.chord_bias,
                            "Chord-tone bias",
                            sw,
                            "the chance a strong beat lands on a chord tone",
                        );
                        pct(
                            ui,
                            &mut m.opts.variation,
                            "Variation",
                            sw,
                            "repeated phrases change a few notes",
                        );
                    });
                    ui.horizontal_wrapped(|ui| {
                        pct(
                            ui,
                            &mut m.opts.rest,
                            "Rests",
                            sw,
                            "the chance a note becomes a rest",
                        );
                        let mut l = m.opts.legato;
                        pct(
                            ui,
                            &mut l,
                            "Note length",
                            sw,
                            "a note's share of the gap to the next",
                        );
                        m.opts.legato = l.max(0.1);
                        ui.checkbox(&mut m.opts.end_tonic, "End on tonic");
                    });
                });
        }
    }

    fn gen_melody_card(
        &mut self,
        ui: &mut egui::Ui,
        mv: &MelView,
        dest: &Dest,
        w: f32,
        acts: &mut Acts,
    ) {
        let th = th();
        let bars = bars_text(mv.start, mv.end.max(mv.start + BAR));
        let tr = dest
            .track
            .map_or("this clip".to_string(), |k| format!("track {:02}", k + 1));
        card(ui, w, |ui| {
            let m = &mut self.cl.gen.mel;
            card_title(ui);
            cap(ui, "Target");
            let this = match dest.track {
                Some(k) => format!("Track {:02} · {}", k + 1, dest.inst),
                None => "This clip".into(),
            };
            let can_new = dest.track.is_some();
            if !can_new {
                m.new_track = false;
            }
            ui.horizontal(|ui| {
                ui.add_enabled_ui(can_new, |ui| {
                    if ui
                        .add(egui::Button::new("New “Melody” track").selected(m.new_track))
                        .on_hover_text("a new track below with a lead synth")
                        .clicked()
                    {
                        m.new_track = true;
                    }
                });
                if ui
                    .add(egui::Button::new(this.as_str()).selected(!m.new_track))
                    .on_hover_text("into the open clip")
                    .clicked()
                {
                    m.new_track = false;
                }
            });
            ui.horizontal(|ui| {
                cap(ui, "Bars");
                ui.label(egui::RichText::new(&bars).strong());
                cap(ui, "(follows the chords)");
            });
            if !m.new_track {
                cap(ui, "Write");
                let sw = ((w - 40.0) / 2.0).clamp(80.0, 140.0);
                if let Some(i) = seg(
                    ui,
                    &["Replace notes", "Notes on top"],
                    usize::from(m.on_top),
                    sw,
                ) {
                    m.on_top = i == 1;
                }
            }
            ui.label(
                egui::RichText::new(format!("{} notes · seed {}", mv.notes.len(), m.opts.seed))
                    .color(th.ink2),
            );
            ui.add_space(2.0);
            warn(
                ui,
                &if m.new_track {
                    format!(
                        "adds a new track “Melody” below {tr} with {bars} (nothing is replaced)"
                    )
                } else if m.on_top {
                    format!("adds notes on top in {bars} on {tr} (keeps the notes there)")
                } else {
                    format!(
                        "replaces notes in {bars} on {tr} (asks first if that deletes more than {CONFIRM_OVER})"
                    )
                },
            );
            ui.add_space(4.0);
            let ok = !mv.notes.is_empty();
            ui.add_enabled_ui(ok, |ui| {
                if accent_btn(ui, "Write to timeline", ui.available_width(), 38.0).clicked() {
                    acts.write = true;
                }
            });
            ui.vertical_centered(|ui| cap(ui, "one Ctrl+Z takes it back"));
        });
    }

    /// queue the melody's notes as auditions (the first 64)
    fn gen_play_notes(&mut self, notes: &[MNote]) {
        let bpm = self.with(|s| s.song.bpm).max(20.0) as f64;
        let spt = 60.0 / (bpm * PPQ as f64);
        let t0 = notes.first().map_or(0, |n| n.start);
        let q = notes
            .iter()
            .take(64)
            .map(|n| {
                (
                    (n.start - t0) as f64 * spt,
                    vec![n.key],
                    (n.len as f64 * spt).clamp(0.05, 2.0),
                )
            })
            .collect();
        self.cl_queue(q);
    }

    fn mel_notes(notes: &[MNote], instr: u8) -> Vec<tl::Note> {
        notes
            .iter()
            .map(|n| tl::Note {
                start: n.start,
                len: n.len,
                key: n.key,
                vel: tl::roll::vel_u16(n.vel),
                instr,
                ..Default::default()
            })
            .collect()
    }

    /// Melody's Write: a new "Melody" track or the open clip (one undo)
    fn gen_write_melody(&mut self, t: Target, mv: &MelView) {
        if mv.notes.is_empty() {
            return;
        }
        let n = mv.notes.len();
        let new_track = self.cl.gen.mel.new_track;
        if new_track && self.gen_melody_track(t, mv) {
            return;
        }
        // the open clip: replace the notes in the melody's span (asks
        // first when that deletes many), or on top; markers unchanged
        let marks = self.cl_marks(t);
        let span = [ChordMark {
            start: mv.start,
            len: (mv.end - mv.start).max(BAR),
            ..Default::default()
        }];
        let notes = Self::mel_notes(&mv.notes, 0);
        let on_top = self.cl.gen.mel.on_top;
        self.cl_put_notes_ex(
            t,
            marks,
            notes,
            &span,
            format!("melody of {n} note(s) written"),
            on_top,
        );
    }

    /// a new track "Melody" below the clip's track with a lead synth and a
    /// note clip where the source clip is; false: not possible (the caller
    /// writes into the clip instead)
    fn gen_melody_track(&mut self, t: Target, mv: &MelView) -> bool {
        let Target::Notes {
            clip: Some(src), ..
        } = t
        else {
            return false;
        };
        let Some((tr, start, offset, len)) = self.with(|s| {
            let c = s
                .song
                .timeline
                .as_deref()?
                .clips
                .iter()
                .find(|c| c.id == src)?;
            Some((c.track as usize, c.start, c.offset, c.len))
        }) else {
            return false;
        };
        if self.with(|s| s.song.channels) >= MAX_CHANNELS {
            self.status = "All 32 tracks are used: the melody goes into this clip".into();
            return false;
        }
        if self.with(|s| s.song.instruments.len() >= 255) {
            self.status =
                "All 255 instrument slots are used: the melody goes into this clip".into();
            return false;
        }
        let Some((iname, kind)) = super::trackpie::Builtin::WaveForge.make() else {
            return false;
        };
        let at = tr + 1;
        self.snapshot();
        self.undo_hold += 1;
        let slot = if self.track_insert(at, "Melody") {
            self.with(|s| {
                let k = s.song.add_instrument(Instrument {
                    name: iname,
                    volume: 0.8,
                    transpose: 0,
                    fx: vec![],
                    kind,
                })?;
                crate::chanops::assign_track_instrument(&mut s.song, at, k as u8 + 1);
                Some(k as u8 + 1)
            })
        } else {
            None
        };
        let notes = Self::mel_notes(&mv.notes, slot.unwrap_or(0));
        let id = slot.and_then(|_| {
            self.with(|s| {
                let id = tl::new_note_clip(&mut s.song, at, start)?;
                let tlm = s.song.timeline.as_deref_mut()?;
                let need = mv.end.max(offset + len);
                let data_len = (need + BAR - 1) / BAR * BAR;
                let c = tlm.clips.iter_mut().find(|c| c.id == id)?;
                c.offset = offset;
                c.len = len.max(mv.end - offset).max(BAR);
                c.name = Some("Melody".into());
                let tl::ClipKind::Notes { data } = c.kind else {
                    return None;
                };
                let d = tlm.notes[data as usize].make_mut();
                d.name = "Melody".into();
                d.len = data_len;
                d.notes = notes;
                let crate::audio::Shared { song, engine, .. } = s;
                engine.ensure_plan(song);
                Some(id)
            })
        });
        self.undo_hold -= 1;
        self.dirty = true;
        crate::plugin::sync(&self.shared, None);
        let Some(id) = id else {
            self.status = "Couldn't add the Melody track".into();
            return true;
        };
        self.track_open_pattern(at, id);
        self.status = format!(
            "New track {:02} “Melody”: {} note(s) in {} (Ctrl+Z undoes it all)",
            at + 1,
            mv.notes.len(),
            bars_text(mv.start, mv.end)
        );
        true
    }
}

/// the melody tab's frame data
pub(crate) struct MelView {
    key: Key,
    marks: Vec<ChordMark>,
    start: Tick,
    end: Tick,
    notes: Vec<MNote>,
    from: MelFrom,
    clip_has_chords: bool,
}

/// a key for chords with no song key: the first chord's root, major or
/// minor by its quality
fn guess_key(marks: &[ChordMark]) -> Key {
    match marks.first() {
        Some(m) if m.pcs().contains(&((m.root + 3) % 12)) => Key::new(m.root, Scale::Minor),
        Some(m) => Key::new(m.root, Scale::Major),
        None => Key::new(0, Scale::Major),
    }
}

/// the WRITE card's frame
/// the recessed fill behind the pill tabs and the WRITE card: the dark
/// screen colour on dark themes, a light panel tone on light ones (Amiga),
/// so the widget text stays readable
fn well() -> egui::Color32 {
    let th = th();
    if th.light {
        th.cart_top
    } else {
        th.crt_bot
    }
}

fn card(ui: &mut egui::Ui, w: f32, body: impl FnOnce(&mut egui::Ui)) {
    let th = th();
    egui::Frame::new()
        .fill(well())
        .corner_radius(6.0)
        .inner_margin(12.0)
        .stroke(egui::Stroke::new(1.0, th.line2))
        .show(ui, |ui| {
            ui.set_width(w - 26.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 7.0);
                body(ui)
            });
        });
}

fn card_title(ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new("WRITE")
            .strong()
            .size(13.0)
            .color(th().ink2),
    );
}

/// the melody over the chords: a small piano roll (chord tones tinted)
fn melody_view(ui: &mut egui::Ui, mv: &MelView, w: f32, h: f32) {
    let th = th();
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
    let p = ui.painter_at(r);
    p.rect_filled(r, 4.0, th.crt_bot);
    p.rect_stroke(
        r,
        4.0,
        egui::Stroke::new(1.0, th.line2),
        egui::StrokeKind::Inside,
    );
    let (t0, t1) = (mv.start, mv.end.max(mv.start + BAR));
    let x = |t: Tick| r.left() + r.width() * ((t - t0) as f32 / (t1 - t0) as f32);
    // bar lines and chord spans
    let mut b = t0;
    while b <= t1 {
        p.line_segment(
            [Pos2::new(x(b), r.top()), Pos2::new(x(b), r.bottom())],
            egui::Stroke::new(1.0, th.line2),
        );
        b += BAR;
    }
    for m in &mv.marks {
        let cr = Rect::from_min_max(
            Pos2::new(x(m.start), r.top()),
            Pos2::new(x(m.end()), r.bottom()),
        );
        p.rect_filled(
            cr.shrink2(egui::vec2(1.0, 1.0)),
            0.0,
            mark_colour(mv.key, m).gamma_multiply(0.16),
        );
    }
    if mv.notes.is_empty() {
        p.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            "no notes",
            FontId::proportional(12.0),
            th.ink2,
        );
        return;
    }
    let lo = mv.notes.iter().map(|n| n.key).min().unwrap_or(60) as f32 - 2.0;
    let hi = mv.notes.iter().map(|n| n.key).max().unwrap_or(72) as f32 + 2.0;
    let y = |k: u8| r.bottom() - 6.0 - (r.height() - 12.0) * ((k as f32 - lo) / (hi - lo).max(1.0));
    let nh = ((r.height() - 12.0) / (hi - lo).max(1.0)).clamp(3.0, 8.0);
    for n in &mv.notes {
        let pcs = mv
            .marks
            .iter()
            .find(|m| n.start >= m.start && n.start < m.end())
            .map(|m| m.pcs())
            .unwrap_or_default();
        let ct = pcs.contains(&crate::chords::pc_of(n.key));
        let nr = Rect::from_min_max(
            Pos2::new(x(n.start) + 1.0, y(n.key) - nh / 2.0),
            Pos2::new(
                (x(n.start + n.len) - 1.0).max(x(n.start) + 3.0),
                y(n.key) + nh / 2.0,
            ),
        );
        p.rect_filled(
            nr,
            2.0,
            if ct {
                th.accent
            } else {
                th.accent.gamma_multiply(0.55)
            },
        );
    }
}
