//! The Generators panel (Generators mockup B, picked by Del (postmanpat)):
//! a wide panel with a GENERATORS header and pill tabs (Chords | Melody |
//! Bass), the generator on the left and a fixed WRITE card on the
//! right: the target track, the bars, what the write does to the notes
//! already there, and one highlighted "Write to timeline" (one Ctrl+Z).
//! Narrow windows stack the card under the generator. Dragging the
//! GENERATORS grip moves the panel (kept on screen and remembered in the
//! settings); double-clicking it puts it back at the top.
//!
//! Chords is the chord generator (`crate::chordgen`, written through the
//! chord lane's `cl_insert` / "Replace notes?" confirm); Melody is
//! `crate::melodygen` over the Chords tab's progression or the clip's
//! chord lane; Bass is `crate::bassgen` over the same chords.
use super::chordlane::{mark_colour, BAR, CONFIRM_OVER};
use super::pianoview::Target;
use super::*;
use crate::bassgen::{self as bg, BassOpts, BassStyle};
use crate::chordgen::{self as cg, ChordMark, Mode, Voicing};
use crate::chords::{Key, Scale};
use crate::melodygen::{self as mg, Contour, Locks, MNote, MelOpts, Motif, Rhythm};
use crate::timeline::{self as tl, Tick, PPQ};
use std::time::Instant;

/// where the Chords tab's Write goes
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ChordDest {
    /// a new clip on the chosen melodic track (at `GenDlg::at`)
    #[default]
    Track,
    /// a new "Chords" track below the chosen one, with a new clip there
    NewTrack,
    /// into the clip open in the piano roll
    Clip,
}

/// the panel's tabs
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum GenTab {
    #[default]
    Chords,
    Melody,
    Bass,
}

impl GenTab {
    pub(crate) const ALL: [GenTab; 3] = [GenTab::Chords, GenTab::Melody, GenTab::Bass];
    pub(crate) fn name(self) -> &'static str {
        match self {
            GenTab::Chords => "Chords",
            GenTab::Melody => "Melody",
            GenTab::Bass => "Bass",
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
pub(crate) type MelDlg = NotesDlg<MelOpts>;
/// the Bass tab
pub(crate) type BassDlg = NotesDlg<BassOpts>;

/// a note generator's tab (Melody, Bass): its settings, the locks, the
/// result shown and where Write puts it
pub(crate) struct NotesDlg<O> {
    pub(crate) opts: O,
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
    /// write to a new "Melody" / "Bass" track (the default) or the open clip
    pub(crate) new_track: bool,
    /// writing to the open clip: on top of its notes (else replace)
    pub(crate) on_top: bool,
}

impl<O: Default> Default for NotesDlg<O> {
    fn default() -> Self {
        Self {
            opts: O::default(),
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

impl<O> NotesDlg<O> {
    fn locks(&self) -> Locks<'_> {
        Locks {
            rhythm: self.lock_rhythm.then_some(self.locked_rhythm.as_slice()),
            pitches: self.lock_notes.then_some(self.locked_keys.as_slice()),
        }
    }
}

/// Seed, Lock rhythm and Lock notes (taking the shown result when a lock
/// is ticked)
fn seed_locks<O>(ui: &mut egui::Ui, m: &mut NotesDlg<O>, seed: impl FnOnce(&mut O) -> &mut u64) {
    ui.horizontal_wrapped(|ui| {
        cap(ui, "Seed");
        ui.add(egui::DragValue::new(seed(&mut m.opts)).range(0..=99_999));
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
}

/// the Bass tab's fallback sound (a track with no instrument or a drum
/// kit): an analog bass preset, the squelchy one for Acid
fn bass_preset(acid: bool) -> Option<(String, InstrumentKind)> {
    let all = crate::gen::presets();
    let want = if acid {
        "Acid Squelch"
    } else {
        "Fat Moog Bass"
    };
    all.iter()
        .find(|(n, _, _)| *n == want)
        .or_else(|| all.iter().find(|(n, _, _)| n.contains("Bass")))
        .cloned()
        .map(|(n, _, mut p)| {
            p.base.name = n.into();
            (n.to_string(), InstrumentKind::Gen(p))
        })
}

/// a button in the theme's accent with readable ink on every theme
fn accent_btn(ui: &mut egui::Ui, text: &str, w: f32, h: f32) -> egui::Response {
    let th = th();
    ui.add_sized(
        [w, h],
        egui::Button::new(
            egui::RichText::new(text)
                .strong()
                .size(crate::theme::ts::px(crate::theme::ts::LARGE))
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

/// the Melody tab's pad's width (beside the controls)
const PAD_W: f32 = 200.0;

/// the Melody / Bass tab's pad: a big Preview with Dice and Another one
/// under it (`tall`), or the three in a row; a rounded square tinted with
/// the accent
fn melody_pad(ui: &mut egui::Ui, seed: &mut u64, acts: &mut Acts, tall: bool) {
    let th = th();
    egui::Frame::new()
        .fill(th.accent.gamma_multiply(0.14))
        .stroke(egui::Stroke::new(1.5, th.accent))
        .corner_radius(18.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            let iw = PAD_W - 24.0;
            let (pw, ph) = if tall { (iw, 64.0) } else { (170.0, 52.0) };
            let half = if tall { (iw - 8.0) / 2.0 } else { 120.0 };
            let mut row = |ui: &mut egui::Ui| {
                let pr = ui
                    .add_sized(
                        [pw, ph],
                        egui::Button::new(
                            egui::RichText::new("Preview")
                                .strong()
                                .size(crate::theme::ts::px(crate::theme::ts::TITLE))
                                .color(accent_ink()),
                        )
                        .fill(th.accent)
                        .corner_radius(14.0),
                    )
                    .on_hover_text("hear it on the track's instrument");
                // a play triangle left of the word
                let c = Pos2::new(pr.rect.left() + 26.0, pr.rect.center().y);
                ui.painter().add(egui::Shape::convex_polygon(
                    vec![
                        c + egui::vec2(-6.0, -9.0),
                        c + egui::vec2(9.0, 0.0),
                        c + egui::vec2(-6.0, 9.0),
                    ],
                    accent_ink(),
                    egui::Stroke::NONE,
                ));
                if pr.clicked() {
                    acts.preview = true;
                }
                pr
            };
            let big = |t: &str| {
                egui::Button::new(
                    egui::RichText::new(t)
                        .strong()
                        .size(crate::theme::ts::px(crate::theme::ts::LARGE)),
                )
                .corner_radius(10.0)
            };
            let dice = |ui: &mut egui::Ui, seed: &mut u64| {
                if ui
                    .add_sized([half, 40.0], big("Dice"))
                    .on_hover_text("a random seed")
                    .clicked()
                {
                    let r = Instant::now().elapsed().as_nanos() as u64
                        ^ std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(7, |d| d.as_nanos() as u64);
                    *seed = r % 100_000;
                }
                if ui
                    .add_sized([half, 40.0], big("Another one"))
                    .on_hover_text("the next seed")
                    .clicked()
                {
                    *seed = seed.wrapping_add(1) % 100_000;
                }
            };
            if tall {
                ui.set_width(iw);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    row(ui);
                    ui.horizontal(|ui| dice(ui, seed));
                });
            } else {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    row(ui);
                    ui.vertical(|ui| {
                        ui.add_space((ph - 40.0) / 2.0);
                        ui.horizontal(|ui| dice(ui, seed));
                    });
                });
            }
        });
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
            crate::theme::prop(crate::theme::ts::SMALL),
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
                crate::theme::prop(if h > 40.0 { 14.0 } else { 11.0 }),
                Color32::WHITE,
            );
            if h > 30.0 {
                p.text(
                    r.left_bottom() + egui::vec2(5.0, -5.0),
                    egui::Align2::LEFT_BOTTOM,
                    m.numeral(key),
                    crate::theme::prop(crate::theme::ts::SMALL),
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

/// "2–9" / "3" (1-based bars of the note data, for a "Bars" label)
fn bars_range(a: Tick, b: Tick) -> String {
    if b > a + BAR {
        format!("{}–{}", a / BAR + 1, (b - 1) / BAR + 1)
    } else {
        format!("{}", a / BAR + 1)
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

/// the panel's top-left kept on screen: all of its width, and its header
/// plus a useful part of the body (the body scrolls in what is left)
fn gen_clamp(scr: egui::Rect, w: f32, p: egui::Pos2) -> egui::Pos2 {
    // the window frame's margins either side of the content width
    let max_x = (scr.right() - w - 16.0).max(scr.left());
    let max_y = (scr.bottom() - 260.0).max(scr.top());
    egui::pos2(p.x.clamp(scr.left(), max_x), p.y.clamp(scr.top(), max_y))
}

/// the header's GENERATORS title as a grip: drag it to move the panel,
/// double-click to put it back at the top
fn gen_grip(ui: &mut egui::Ui) -> egui::Response {
    let th = th();
    let font = crate::theme::prop(crate::theme::ts::BODY);
    let text = "GENERATORS";
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), font.clone(), th.ink2);
    let size = egui::vec2(18.0 + galley.size().x + 6.0, 26.0);
    let (r, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, text));
    let hot = resp.hovered() || resp.dragged();
    let p = ui.painter();
    if hot {
        p.rect_filled(r, 4.0, well());
    }
    // six dots: the grip
    let ink = if hot { th.accent } else { th.ink2 };
    for row in 0..3 {
        for col in 0..2 {
            let c = egui::pos2(
                r.left() + 6.0 + col as f32 * 5.0,
                r.center().y - 5.0 + row as f32 * 5.0,
            );
            p.circle_filled(c, 1.4, ink);
        }
    }
    // the title, bold-looking (drawn twice, a hair apart)
    let at = egui::pos2(r.left() + 18.0, r.center().y - galley.size().y / 2.0);
    p.galley(at, galley.clone(), th.ink2);
    p.galley(at + egui::vec2(0.6, 0.0), galley, th.ink2);
    let resp = if resp.dragged() {
        resp.on_hover_cursor(egui::CursorIcon::Grabbing)
    } else {
        resp.on_hover_cursor(egui::CursorIcon::Grab)
    };
    resp.on_hover_text("drag to move the panel · double-click to put it back at the top")
}

/// the track the panel writes for: the open clip's, else the one it was
/// opened for (`track`) and its name (`inst`)
pub(crate) struct Dest {
    pub(crate) track: Option<usize>,
    pub(crate) inst: String,
}

impl App {
    /// open the panel on the Melody tab (over the open clip, if any)
    pub(crate) fn gen_open_melody(&mut self, at: Tick) {
        self.cl_open_gen_at(at);
        self.cl.gen.tab = GenTab::Melody;
        self.cl.gen.chords_seen = false;
        self.cl.gen.mel.from = None;
    }

    /// open the panel on the Bass tab (over the open clip, if any)
    pub(crate) fn gen_open_bass(&mut self, at: Tick) {
        self.cl_open_gen_at(at);
        self.cl.gen.tab = GenTab::Bass;
        self.cl.gen.chords_seen = false;
        self.cl.gen.bass.from = None;
    }

    /// open the panel on `tab` for track `tr` (the track pie, the timeline;
    /// None: no track, e.g. an empty song). Chords' Write makes a new clip
    /// at the loop start, else the play marker's bar: on `tr` when it is a
    /// melodic track with room there, else on a new "Chords" track.
    pub(crate) fn gen_open_for(&mut self, tr: Option<usize>, tab: GenTab) {
        self.cl_open_gen_at(0);
        let tr = tr.filter(|&k| k < self.with(|s| s.song.channels));
        let g = &mut self.cl.gen;
        g.track = tr;
        g.dest = ChordDest::Track;
        g.tab = tab;
        g.chords_seen = false;
        g.mel.from = None;
        g.bass.from = None;
        if self.gen_target().is_none() {
            // no clip to write into: Melody / Bass go on a new track
            self.cl.gen.mel.new_track = true;
            self.cl.gen.bass.new_track = true;
        }
    }

    /// where a new clip goes: the loop start while a loop is on, else the
    /// start of the play marker's bar
    pub(crate) fn gen_play_pos(&self) -> Tick {
        let cue = self.tlv.cue.max(0);
        self.with(|s| {
            let Some(t) = s.song.timeline.as_deref() else {
                return 0;
            };
            if let (true, Some((a, _))) = (t.loop_on, t.loop_range) {
                return a.max(0);
            }
            let sig = t.sig_at(cue);
            let bar = sig.bar_ticks().max(1);
            let from = if cue >= sig.tick { sig.tick } else { 0 };
            from + (cue - from).max(0) / bar * bar
        })
    }

    /// track `tr` is a track that takes chords: not drums (a track with
    /// no instrument yet gets a synth when chords go on it)
    pub(crate) fn gen_melodic(&self, tr: usize) -> bool {
        tr < self.with(|s| s.song.channels)
            && (self.track_needs_instrument(tr) || self.gen_sound(tr).is_some())
    }

    /// track `tr`'s melodic instrument (None: none, or drums)
    pub(super) fn gen_sound(&self, tr: usize) -> Option<Instrument> {
        if self.track_needs_instrument(tr) {
            return None;
        }
        let slot = self.track_instr(tr);
        self.with(|s| {
            s.song
                .instruments
                .get((slot as usize).wrapping_sub(1))
                .filter(|i| !super::trackpie::is_drum_kind(&i.kind))
                .cloned()
        })
    }

    /// the track the panel's Preview plays on: where Write goes when it
    /// has a melodic sound, else the first track that has one (None: the
    /// song has none yet, the preview stays silent)
    pub(super) fn gen_preview_ch(&self) -> Option<usize> {
        let tr = self.gen_dest(self.gen_target()).track;
        let n = self.with(|s| s.song.channels);
        tr.into_iter()
            .chain(0..n)
            .find(|&k| self.gen_sound(k).is_some())
    }

    /// a synth for chords on a track with no melodic sound
    fn gen_synth() -> Option<Instrument> {
        super::trackpie::Builtin::WaveForge
            .make()
            .map(|(name, kind)| Instrument {
                name,
                volume: 0.8,
                transpose: 0,
                fx: vec![],
                kind,
            })
    }

    /// the clip the panel can write into: the piano roll's open, melodic
    /// note clip (None: the roll is closed or shows a drum pattern)
    pub(crate) fn gen_target(&self) -> Option<Target> {
        if !self.pr.open || self.view != View::Timeline || !self.lower_visible() {
            return None;
        }
        let t = self.pr_target()?;
        let Target::Notes {
            clip: Some(id),
            data,
        } = t
        else {
            return None;
        };
        let drum = self.with(|s| {
            let tl = s.song.timeline.as_deref()?;
            tl.clips.iter().find(|c| c.id == id)?;
            Some(tl.notes.get(data as usize)?.drum.is_some())
        })?;
        (!drum).then_some(t)
    }

    /// where Chords' Write goes this frame for `len` ticks: the choice,
    /// unless it can't be (no clip open; the track is drums or has clips
    /// there: a new track)
    pub(crate) fn gen_chord_dest(&self, t: Option<Target>, len: Tick) -> ChordDest {
        let g = &self.cl.gen;
        let clip_ok = t.is_some_and(|t| {
            self.gen_dest(Some(t))
                .track
                .is_none_or(|k| self.gen_melodic(k))
        });
        match g.dest {
            ChordDest::Clip if clip_ok => ChordDest::Clip,
            ChordDest::NewTrack => ChordDest::NewTrack,
            _ if self.gen_track_free(len) => ChordDest::Track,
            _ => ChordDest::NewTrack,
        }
    }

    /// the panel's track is melodic and has no clip over the new clip's bars
    fn gen_track_free(&self, len: Tick) -> bool {
        let Some(tr) = self.cl.gen.track.filter(|&k| self.gen_melodic(k)) else {
            return false;
        };
        let at = self.cl.gen.at;
        self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .is_some_and(|t| tl::overlaps(t, tr, at, at + len.max(1), &[]).is_empty())
        })
    }

    pub(super) fn gen_dest(&self, t: Option<Target>) -> Dest {
        let track = match t {
            Some(Target::Notes { clip: Some(id), .. }) => self.with(|s| {
                s.song
                    .timeline
                    .as_deref()
                    .and_then(|tl| tl.clips.iter().find(|x| x.id == id))
                    .map(|c| c.track as usize)
            }),
            _ => self.cl.gen.track,
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
    /// (drawn over every view: it needs no open clip)
    pub(super) fn gen_panel(&mut self, ctx: &egui::Context) {
        if !self.cl.gen.open {
            return;
        }
        let t = self.gen_target();
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
        let tab = self.cl.gen.tab;
        let mel = match tab {
            GenTab::Chords => None,
            GenTab::Melody => Some(self.gen_melody(t, &res)),
            GenTab::Bass => Some(self.gen_bass(t, &res)),
        };
        let mut acts = Acts::default();
        // dragged somewhere (the GENERATORS grip): that spot, kept on
        // screen; else top-centre
        let pos = self
            .extra
            .gen_pos
            .map(|[x, y]| gen_clamp(scr, w, egui::pos2(x, y)));
        let top_y = pos.map_or(scr.top() + top, |p| p.y);
        let mut drag = egui::Vec2::ZERO;
        let (mut drag_end, mut reset) = (false, false);
        let win = egui::Window::new("Generators")
            .id(egui::Id::unique("gen_panel"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .min_width(w)
            .max_width(w)
            // over the whole window (the roll's bottom panel included)
            .constrain_to(scr);
        let win = match pos {
            Some(p) => win.fixed_pos(p),
            None => win.anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, top)),
        };
        win.show(ctx, |ui| {
            ui.set_width(w);
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            // the header: GENERATORS, the pill tabs, the hint, Close
            ui.horizontal(|ui| {
                let g = gen_grip(ui);
                if g.dragged() {
                    drag += g.drag_delta();
                }
                drag_end |= g.drag_stopped();
                reset |= g.double_clicked();
                ui.add_space(8.0);
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
                                GenTab::Bass => "a bass line under your chords",
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
            let body_h = (scr.bottom() - top_y - 100.0).max(140.0);
            egui::ScrollArea::vertical()
                .max_height(body_h)
                .min_scrolled_height(body_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let left = |app: &mut App, ui: &mut egui::Ui, acts: &mut Acts| match &mel {
                        None => app.gen_chords_left(ui, &res, left_w, acts),
                        Some(m) if tab == GenTab::Bass => app.gen_bass_left(ui, m, left_w, acts),
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
                                    None => {
                                        self.gen_chords_card(ui, t, &res, &dest, card_w, &mut acts)
                                    }
                                    Some(m) => {
                                        self.gen_melody_card(ui, m, t, &dest, card_w, &mut acts)
                                    }
                                }
                            });
                        });
                    }
                    if stacked {
                        match &mel {
                            None => self.gen_chords_card(ui, t, &res, &dest, card_w, &mut acts),
                            Some(m) => self.gen_melody_card(ui, m, t, &dest, card_w, &mut acts),
                        }
                    }
                });
        });
        if drag != egui::Vec2::ZERO {
            // from where the panel is now (top-centre until first dragged)
            let from = pos.unwrap_or(egui::pos2(scr.center().x - w / 2.0, top_y));
            let p = gen_clamp(scr, w, from + drag);
            self.extra.gen_pos = Some([p.x, p.y]);
        }
        if reset {
            self.extra.gen_pos = None;
            self.status = "Generators panel back at the top".into();
        }
        if drag_end || reset {
            self.save_settings();
        }
        if let Some(tab) = acts.tab {
            self.cl.gen.tab = tab;
        }
        match &mel {
            None => {
                if acts.preview {
                    let (v, o) = (self.cl.gen.voicing, self.cl.gen.octave);
                    self.cl_play_seq_as(&res.marks, v, o);
                }
                if acts.write && self.gen_write_chords(t, &res) {
                    self.cl.gen.open = false;
                }
            }
            Some(m) => {
                if acts.preview {
                    self.gen_play_notes(&m.notes);
                }
                if acts.write && self.gen_write_melody(t, m) {
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
            ui.label(
                egui::RichText::new(key_name(key))
                    .strong()
                    .size(crate::theme::ts::px(crate::theme::ts::LARGE)),
            );
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
        t: Option<Target>,
        res: &cg::GenResult,
        dest: &Dest,
        w: f32,
        acts: &mut Acts,
    ) {
        let th = th();
        let (a, b) = span_of(&res.marks);
        let len = ((b - a).max(BAR) + BAR - 1) / BAR * BAR;
        let eff = self.gen_chord_dest(t, len);
        let track_ok = self.gen_track_free(len);
        let melodic = dest.track.is_some_and(|k| self.gen_melodic(k));
        let at = self.cl.gen.at;
        let (bar_a, bar_b) = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .map_or((at / BAR + 1, (at + len - 1) / BAR + 1), |tl| {
                    (tl.bar_pos(at).bar, tl.bar_pos(at + len - 1).bar)
                })
        });
        let looped = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .is_some_and(|tl| tl.loop_on && tl.loop_range.is_some())
        });
        let scope = t.map(|t| self.cl_scope(t, a, b)).unwrap_or_default();
        let trk = |k: usize| format!("track {:02}", k + 1);
        let below = dest.track.map_or("after the last track".to_string(), |k| {
            format!("below {}", trk(k))
        });
        card(ui, w, |ui| {
            let g = &mut self.cl.gen;
            card_title(ui);
            cap(ui, "Target");
            let this_track = match dest.track {
                Some(k) => format!("Track {:02}", k + 1),
                None => "A track".into(),
            };
            let bw = ((w - 40.0) / 3.0).clamp(70.0, 110.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let opts = [
                    (ChordDest::Track, this_track.as_str(), track_ok, if melodic {
                        "a new clip on this track (needs free bars there)"
                    } else {
                        "a new clip on this track (not on a drum track)"
                    }),
                    (ChordDest::NewTrack, "New track", true, "a new “Chords” track below, with a copy of this track's sound (a synth if it is drums or has none)"),
                    (ChordDest::Clip, "This clip", t.is_some(), "into the clip open in the piano roll"),
                ];
                for (d, label, ok, tip) in opts {
                    let r = ui.add_enabled(
                        ok,
                        egui::Button::new(label).selected(eff == d).min_size(egui::vec2(bw, 22.0)),
                    );
                    if r.on_hover_text(tip).on_disabled_hover_text(tip).clicked() {
                        g.dest = d;
                    }
                }
            });
            let inst = &dest.inst;
            ui.label(
                egui::RichText::new(match (eff, dest.track) {
                    (ChordDest::Clip, Some(k)) => format!("Track {:02} · {inst}: this clip", k + 1),
                    (ChordDest::Clip, None) => "this clip".into(),
                    (ChordDest::Track, Some(k)) => {
                        format!("Track {:02} · {inst}: a new clip", k + 1)
                    }
                    _ => format!("a new track “Chords” {below}"),
                })
                .strong(),
            );
            if eff == ChordDest::Clip {
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
                    cap(ui, "(of the clip)");
                });
                cap(ui, "Write");
                let mode = if !g.write {
                    2
                } else if g.on_top {
                    1
                } else {
                    0
                };
                match seg(
                    ui,
                    &["Replace notes", "Notes on top", "Markers only"],
                    mode,
                    bw,
                ) {
                    Some(0) => (g.write, g.on_top) = (true, false),
                    Some(1) => (g.write, g.on_top) = (true, true),
                    Some(2) => g.write = false,
                    _ => {}
                }
            } else {
                ui.horizontal(|ui| {
                    cap(ui, "Song bars");
                    ui.label(egui::RichText::new(format!("{bar_a}–{bar_b}")).strong());
                    if ui
                        .small_button(" - ")
                        .on_hover_text("a bar earlier")
                        .clicked()
                    {
                        g.at = (g.at - BAR).max(0);
                    }
                    if ui
                        .small_button(" + ")
                        .on_hover_text("a bar later")
                        .clicked()
                    {
                        g.at += BAR;
                    }
                });
                cap(
                    ui,
                    if looped {
                        "starts at the loop start (Length sets the end)"
                    } else {
                        "starts at the play marker's bar (Length sets the end)"
                    },
                );
            }
            let notes = eff != ChordDest::Clip || g.write;
            ui.add_enabled_ui(notes, |ui| {
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
            let n = res.marks.len();
            let bars_n = len / BAR;
            warn(
                ui,
                &match eff {
                    ChordDest::Track => format!(
                        "adds a new {bars_n}-bar clip of {n} chords on {} at song bars {bar_a}–{bar_b}",
                        dest.track.map_or("the track".into(), trk)
                    ),
                    ChordDest::NewTrack => format!(
                        "adds a new track “Chords” {below} with a {bars_n}-bar clip of {n} chords at song bars {bar_a}–{bar_b}"
                    ),
                    ChordDest::Clip => {
                        let tr = dest.track.map_or("this clip".into(), trk);
                        let bars = bars_text(a, b);
                        if !g.write {
                            format!("writes chord markers only in {bars} on {tr} (notes untouched)")
                        } else if g.on_top {
                            format!("adds notes on top in {bars} on {tr} (keeps the notes there)")
                        } else {
                            format!(
                                "replaces notes in {bars} on {tr} (inside each chord's span; asks first \
                                 if that deletes more than {CONFIRM_OVER})"
                            )
                        }
                    }
                },
            );
            ui.add_space(4.0);
            if accent_btn(ui, "Write to timeline", ui.available_width(), 38.0).clicked() {
                acts.write = true;
            }
            ui.vertical_centered(|ui| cap(ui, "one Ctrl+Z takes it back"));
        });
    }

    /// Chords' Write: into the open clip, or a new clip on the panel's
    /// track or a new track (one undo); false: nothing was written
    pub(crate) fn gen_write_chords(&mut self, t: Option<Target>, res: &cg::GenResult) -> bool {
        if res.marks.is_empty() {
            self.status = "No chords to write".into();
            return false;
        }
        let (a, b) = span_of(&res.marks);
        let len = ((b - a).max(BAR) + BAR - 1) / BAR * BAR;
        let dest = self.gen_chord_dest(t, len);
        let (w, v, o) = (self.cl.gen.write, self.cl.gen.voicing, self.cl.gen.octave);
        if let (ChordDest::Clip, Some(t)) = (dest, t) {
            // (cl_insert takes the undo snapshot, or asks first)
            if self.with(|s| s.song.info.key.is_none()) {
                self.with(|s| s.song.info.key = Some(res.key));
            }
            self.cl_insert(t, res.marks.clone(), w, v, o);
            return true;
        }
        if self.with(|s| s.song.timeline.is_none()) {
            self.status = "Chords need the timeline: switch to the timeline view first".into();
            return false;
        }
        let src = self
            .cl
            .gen
            .track
            .filter(|&k| k < self.with(|s| s.song.channels));
        let new_track = dest != ChordDest::Track || src.is_none();
        if new_track && self.with(|s| s.song.channels) >= MAX_CHANNELS {
            self.status = "All 32 tracks are used: pick a track with free bars".into();
            return false;
        }
        // a new track's sound: a copy of the chosen track's melodic
        // instrument, else a synth
        // a new track's sound: a copy of the chosen track's melodic
        // instrument, else a synth; the chosen track with no instrument
        // yet gets the synth
        let needs = new_track || src.is_some_and(|k| self.track_needs_instrument(k));
        let inst = if needs {
            let heard = if new_track {
                src.and_then(|k| self.gen_sound(k))
            } else {
                None
            };
            match heard.or_else(Self::gen_synth) {
                Some(i) => Some(i),
                None => {
                    self.status = "Couldn't make an instrument for the chords".into();
                    return false;
                }
            }
        } else {
            None
        };
        if inst.is_some() && self.with(|s| s.song.instruments.len() >= 255) {
            self.status = "All 255 instrument slots are used: pick a track for the chords".into();
            return false;
        }
        let marks: Vec<ChordMark> = res
            .marks
            .iter()
            .map(|m| ChordMark {
                start: m.start - a,
                ..m.clone()
            })
            .collect();
        let at = self.cl.gen.at.max(0);
        self.snapshot();
        self.undo_hold += 1;
        if self.with(|s| s.song.info.key.is_none()) {
            self.with(|s| s.song.info.key = Some(res.key));
        }
        let tr = match (inst, src) {
            (None, Some(k)) => Some(k),
            (Some(inst), Some(k)) if !new_track => self.with(|s| {
                let slot = s.song.add_instrument(inst)?;
                crate::chanops::assign_track_instrument(&mut s.song, k, slot as u8 + 1);
                Some(k)
            }),
            (Some(inst), _) => {
                let k = src.map_or(self.with(|s| s.song.channels), |k| k + 1);
                (self.track_insert(k, "Chords")
                    && self
                        .with(|s| {
                            let slot = s.song.add_instrument(inst)?;
                            crate::chanops::assign_track_instrument(&mut s.song, k, slot as u8 + 1);
                            Some(())
                        })
                        .is_some())
                .then_some(k)
            }
            (None, None) => None,
        };
        let id = tr.and_then(|tr| {
            let slot = self.with(|s| s.song.timeline.as_deref().map_or(0, |t| t.track_instr(tr)));
            let notes: Vec<tl::Note> = self
                .cl_notes(&marks, v, o)
                .into_iter()
                .map(|n| tl::Note { instr: slot, ..n })
                .collect();
            self.with(|s| {
                let id = tl::new_note_clip(&mut s.song, tr, at)?;
                let tlm = s.song.timeline.as_deref_mut()?;
                let c = tlm.clips.iter_mut().find(|c| c.id == id)?;
                c.len = len;
                c.name = Some("Chords".into());
                let tl::ClipKind::Notes { data } = c.kind else {
                    return None;
                };
                let d = tlm.notes[data as usize].make_mut();
                d.name = "Chords".into();
                d.len = len;
                d.chords = marks.clone();
                d.notes = notes;
                let crate::audio::Shared { song, engine, .. } = s;
                engine.ensure_plan(song);
                Some(id)
            })
        });
        self.undo_hold -= 1;
        self.dirty = true;
        crate::plugin::sync(&self.shared, None);
        let (Some(tr), Some(id)) = (tr, id) else {
            self.status = "Couldn't add the chords".into();
            return false;
        };
        self.track_open_pattern(tr, id);
        let (bar_a, bar_b, name) = self.with(|s| {
            let tl = s.song.timeline.as_deref();
            (
                tl.map_or(at / BAR + 1, |t| t.bar_pos(at).bar),
                tl.map_or((at + len - 1) / BAR + 1, |t| t.bar_pos(at + len - 1).bar),
                s.song.channel_names.get(tr).cloned().unwrap_or_default(),
            )
        });
        self.status = format!(
            "Chords: {} chords in a new {}-bar clip on {}track {:02} “{name}”, song bars {bar_a}–{bar_b} · Ctrl+Z takes it back",
            marks.len(),
            len / BAR,
            if new_track { "new " } else { "" },
            tr + 1,
        );
        true
    }

    // ------------------------------------------------------------ melody

    /// the chords a Melody / Bass goes over (`pref`: the tab's Over
    /// choice): the key, the marks, where they came from and whether the
    /// clip has chords of its own
    fn gen_source(
        &self,
        t: Option<Target>,
        res: &cg::GenResult,
        pref: Option<MelFrom>,
    ) -> (Key, Vec<ChordMark>, MelFrom, bool) {
        let clip_marks = t.map(|t| self.cl_marks(t)).unwrap_or_default();
        let g = &self.cl.gen;
        let from = pref.unwrap_or(if clip_marks.is_empty() || g.chords_seen {
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
        (key, marks, from, !clip_marks.is_empty())
    }

    /// the melody's chords, key and span, and the melody itself
    fn gen_melody(&mut self, t: Option<Target>, res: &cg::GenResult) -> MelView {
        let (key, marks, from, clip_has_chords) = self.gen_source(t, res, self.cl.gen.mel.from);
        let (start, end) = span_of(&marks);
        let m = &mut self.cl.gen.mel;
        let notes = mg::generate(key, &marks, start, end, &m.opts, m.locks());
        m.last = notes.clone();
        MelView {
            key,
            marks,
            start,
            end,
            notes,
            from,
            clip_has_chords,
        }
    }

    /// the bass line over the same chords as the Melody tab
    fn gen_bass(&mut self, t: Option<Target>, res: &cg::GenResult) -> MelView {
        let (key, marks, from, clip_has_chords) = self.gen_source(t, res, self.cl.gen.bass.from);
        let (start, end) = span_of(&marks);
        let b = &mut self.cl.gen.bass;
        let notes = bg::generate(key, &marks, start, end, &b.opts, b.locks());
        b.last = notes.clone();
        MelView {
            key,
            marks,
            start,
            end,
            notes,
            from,
            clip_has_chords,
        }
    }

    /// the Over row (Chords tab / Clip's chord lane) for Melody and Bass
    fn gen_over_row(&mut self, ui: &mut egui::Ui, mv: &MelView) {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(key_name(mv.key))
                    .strong()
                    .size(crate::theme::ts::px(crate::theme::ts::LARGE)),
            );
            ui.add_space(8.0);
            cap(ui, "Over");
            let on = usize::from(mv.from == MelFrom::Clip);
            let hit = seg(ui, &["Chords tab", "Clip's chord lane"], on, 120.0);
            let g = &mut self.cl.gen;
            let from = if g.tab == GenTab::Bass {
                &mut g.bass.from
            } else {
                &mut g.mel.from
            };
            match hit {
                Some(0) => *from = Some(MelFrom::Chords),
                Some(1) if mv.clip_has_chords => *from = Some(MelFrom::Clip),
                Some(1) => {
                    self.status = "This clip has no chords yet (the Chords tab writes them)".into()
                }
                _ => {}
            }
        });
    }

    fn gen_bass_left(&mut self, ui: &mut egui::Ui, mv: &MelView, w: f32, acts: &mut Acts) {
        let th = th();
        self.gen_over_row(ui, mv);
        melody_view(ui, mv, w, 140.0);
        strip(ui, mv.key, &mv.marks, w, 22.0);
        let m = &mut self.cl.gen.bass;
        let side = w >= 620.0;
        let cw = if side { w - PAD_W - 34.0 } else { w };
        let controls = |ui: &mut egui::Ui, m: &mut BassDlg| {
            seed_locks(ui, m, |o| &mut o.seed);
            ui.horizontal_wrapped(|ui| {
                cap(ui, "Style");
                let names: Vec<&str> = BassStyle::ALL.iter().map(|s| s.name()).collect();
                let on = BassStyle::ALL
                    .iter()
                    .position(|&s| s == m.opts.style)
                    .unwrap_or(0);
                let bw = ((cw - 60.0) / 6.0 - 2.0).clamp(60.0, 96.0);
                if let Some(i) = seg(ui, &names, on, bw) {
                    m.opts.style = BassStyle::ALL[i];
                }
            });
            ui.label(egui::RichText::new(m.opts.style.hint()).color(th.ink2));
            ui.horizontal_wrapped(|ui| {
                let sw = ((cw - 380.0) / 2.0).clamp(70.0, 150.0);
                ui.add_enabled_ui(m.opts.style != BassStyle::Euclid, |ui| {
                    cap(ui, "Density");
                    pct(
                        ui,
                        &mut m.opts.density,
                        "",
                        sw,
                        "how many notes (strong beats first)",
                    );
                });
                ui.add_space(12.0);
                cap(ui, "Range");
                ui.spacing_mut().slider_width = sw;
                ui.add(
                    egui::Slider::new(&mut m.opts.range, 12..=36)
                        .trailing_fill(true)
                        .suffix(" st"),
                )
                .on_hover_text("from the lowest to the highest note, in semitones");
                ui.add_space(12.0);
                cap(ui, "Octave");
                ui.add(egui::DragValue::new(&mut m.opts.octave).range(0..=5))
                    .on_hover_text("the lowest note's octave (2: the bass register)");
            });
        };
        if side {
            ui.horizontal_top(|ui| {
                melody_pad(ui, &mut m.opts.seed, acts, true);
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.set_width(cw);
                    controls(ui, m);
                });
            });
        } else {
            melody_pad(ui, &mut m.opts.seed, acts, false);
            controls(ui, m);
        }
        let rr = ui
            .add(egui::Button::new("Advanced...").selected(m.adv))
            .on_hover_text("Euclid steps, approach notes, fifths, accents, ties, note length");
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
                            egui::DragValue::new(&mut m.opts.steps)
                                .range(1..=16)
                                .prefix("steps "),
                        );
                        m.opts.pulses = m.opts.pulses.min(m.opts.steps);
                        ui.add(
                            egui::DragValue::new(&mut m.opts.pulses)
                                .range(0..=m.opts.steps)
                                .prefix("pulses "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut m.opts.rotate)
                                .range(0..=15)
                                .prefix("rotate "),
                        );
                        ui.label(
                            egui::RichText::new(mg::euclid_str(&mg::euclid(
                                m.opts.pulses,
                                m.opts.steps,
                                m.opts.rotate,
                            )))
                            .monospace()
                            .color(th.ink2),
                        )
                        .on_hover_text("one bar (Style: Euclid)");
                    });
                    let sw = ((w - 330.0) / 3.0).clamp(70.0, 130.0);
                    ui.horizontal_wrapped(|ui| {
                        cap(ui, "Approach");
                        pct(
                            ui,
                            &mut m.opts.approach,
                            "",
                            sw,
                            "the chance the note before a chord change leads into the next root",
                        );
                        ui.add_space(10.0);
                        cap(ui, "Fifths/octaves");
                        pct(
                            ui,
                            &mut m.opts.fifths,
                            "",
                            sw,
                            "the chance a step plays the fifth or the octave instead of the root",
                        );
                        let mut l = m.opts.legato;
                        ui.add_space(10.0);
                        cap(ui, "Note length");
                        pct(ui, &mut l, "", sw, "a note's share of the gap to the next");
                        m.opts.legato = l.max(0.1);
                    });
                    ui.add_enabled_ui(m.opts.style == BassStyle::Acid, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            cap(ui, "Accent");
                            pct(
                                ui,
                                &mut m.opts.accent,
                                "",
                                sw,
                                "Acid: the chance a note is accented (played louder)",
                            );
                            ui.add_space(10.0);
                            cap(ui, "Tie");
                            pct(
                                ui,
                                &mut m.opts.tie,
                                "",
                                sw,
                                "Acid: the chance a note is held into the next step",
                            );
                        });
                    });
                });
        }
    }

    fn gen_melody_left(&mut self, ui: &mut egui::Ui, mv: &MelView, w: f32, acts: &mut Acts) {
        let th = th();
        self.gen_over_row(ui, mv);
        melody_view(ui, mv, w, 140.0);
        strip(ui, mv.key, &mv.marks, w, 22.0);
        let m = &mut self.cl.gen.mel;
        // Preview, Dice and Another one in a pad of their own, beside the
        // controls (used all the time here; Del (postmanpat) 23 Oct 2026);
        // above them when the panel is narrow
        let side = w >= 620.0;
        // the pad, the gap and the item spacing either side of it
        let cw = if side { w - PAD_W - 34.0 } else { w };
        let controls = |ui: &mut egui::Ui, m: &mut MelDlg| {
            seed_locks(ui, m, |o| &mut o.seed);
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
                    let bw = ((cw - 90.0) / 6.0 - 2.0).clamp(54.0, 96.0);
                    if let Some(i) = seg(ui, &names, on, bw) {
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
                    if let Some(i) = seg(ui, &names, on, 60.0) {
                        m.opts.motif = Motif::ALL[i];
                    }
                    ui.end_row();
                });
            // each caption before its slider, a gap between the groups
            ui.horizontal_wrapped(|ui| {
                let sw = ((cw - 380.0) / 2.0).clamp(70.0, 150.0);
                ui.add_enabled_ui(m.opts.rhythm != Rhythm::Euclid, |ui| {
                    cap(ui, "Density");
                    pct(
                        ui,
                        &mut m.opts.density,
                        "",
                        sw,
                        "how many notes (strong beats first)",
                    );
                });
                ui.add_space(12.0);
                cap(ui, "Range");
                ui.spacing_mut().slider_width = sw;
                ui.add(
                    egui::Slider::new(&mut m.opts.range, 12..=36)
                        .trailing_fill(true)
                        .suffix(" st"),
                )
                .on_hover_text("from the lowest to the highest note, in semitones");
                ui.add_space(12.0);
                cap(ui, "Octave");
                ui.add(egui::DragValue::new(&mut m.opts.octave).range(1..=8))
                    .on_hover_text("the lowest note's octave");
            });
        };
        if side {
            ui.horizontal_top(|ui| {
                melody_pad(ui, &mut m.opts.seed, acts, true);
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.set_width(cw);
                    controls(ui, m);
                });
            });
        } else {
            melody_pad(ui, &mut m.opts.seed, acts, false);
            controls(ui, m);
        }
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

    /// the Melody / Bass WRITE card (the open tab's settings)
    fn gen_melody_card(
        &mut self,
        ui: &mut egui::Ui,
        mv: &MelView,
        t: Option<Target>,
        dest: &Dest,
        w: f32,
        acts: &mut Acts,
    ) {
        let th = th();
        let bars = bars_text(mv.start, mv.end.max(mv.start + BAR));
        let at = self.cl.gen.at;
        let song_bars = self.with(|s| {
            let n = (mv.end - mv.start).max(BAR);
            s.song.timeline.as_deref().map_or_else(
                || bars_range(at, at + n),
                |tl| format!("{}–{}", tl.bar_pos(at).bar, tl.bar_pos(at + n - 1).bar),
            )
        });
        let tr = dest
            .track
            .map_or("this clip".to_string(), |k| format!("track {:02}", k + 1));
        let g = &mut self.cl.gen;
        let part = g.tab.name();
        let (new_track, on_top, seed) = if g.tab == GenTab::Bass {
            (&mut g.bass.new_track, &mut g.bass.on_top, g.bass.opts.seed)
        } else {
            (&mut g.mel.new_track, &mut g.mel.on_top, g.mel.opts.seed)
        };
        card(ui, w, |ui| {
            card_title(ui);
            cap(ui, "Target");
            let this = match (t, dest.track) {
                (Some(_), Some(k)) => format!("Track {:02} · {}", k + 1, dest.inst),
                _ => "This clip".into(),
            };
            // no clip open (or a drum pattern): a new track it is
            let can_clip = t.is_some();
            if !can_clip {
                *new_track = true;
            }
            let can_new = true;
            ui.horizontal(|ui| {
                ui.add_enabled_ui(can_new, |ui| {
                    if ui
                        .add(egui::Button::new(format!("New “{part}” track")).selected(*new_track))
                        .on_hover_text(if part == "Bass" {
                            "a new track below, with a copy of this track's instrument (a bass synth if it has none or plays a drum kit)"
                        } else {
                            "a new track below, with a copy of this track's instrument (the sound Preview plays)"
                        })
                        .clicked()
                    {
                        *new_track = true;
                    }
                });
                if ui
                    .add_enabled(can_clip, egui::Button::new(this.as_str()).selected(!*new_track))
                    .on_hover_text("into the open clip")
                    .on_disabled_hover_text("open a melodic clip in the piano roll to write into it")
                    .clicked()
                {
                    *new_track = false;
                }
            });
            ui.horizontal(|ui| {
                if can_clip {
                    cap(ui, "Bars");
                    ui.label(
                        egui::RichText::new(bars_range(mv.start, mv.end.max(mv.start + BAR)))
                            .strong(),
                    );
                    cap(ui, "(follows the chords)");
                } else {
                    cap(ui, "Song bars");
                    ui.label(egui::RichText::new(&song_bars).strong());
                    cap(ui, "(loop start, else the play marker)");
                }
            });
            if !*new_track {
                cap(ui, "Write");
                let sw = ((w - 40.0) / 2.0).clamp(80.0, 140.0);
                if let Some(i) = seg(
                    ui,
                    &["Replace notes", "Notes on top"],
                    usize::from(*on_top),
                    sw,
                ) {
                    *on_top = i == 1;
                }
            }
            ui.label(
                egui::RichText::new(format!("{} notes · seed {}", mv.notes.len(), seed))
                    .color(th.ink2),
            );
            ui.add_space(2.0);
            warn(
                ui,
                &if *new_track && !can_clip {
                    let below = dest.track.map_or("after the last track".into(), |k| {
                        format!("below track {:02}", k + 1)
                    });
                    format!(
                        "adds a new track “{part}” {below} with a clip at song bars {song_bars} (nothing is replaced)"
                    )
                } else if *new_track {
                    format!(
                        "adds a new track “{part}” below {tr} with {bars} (nothing is replaced)"
                    )
                } else if *on_top {
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

    /// Melody's / Bass's Write: a new "Melody" / "Bass" track or the open
    /// clip (one undo)
    fn gen_write_melody(&mut self, t: Option<Target>, mv: &MelView) -> bool {
        if mv.notes.is_empty() {
            return false;
        }
        let n = mv.notes.len();
        let g = &self.cl.gen;
        let (new_track, on_top) = if g.tab == GenTab::Bass {
            (g.bass.new_track, g.bass.on_top)
        } else {
            (g.mel.new_track, g.mel.on_top)
        };
        let what = g.tab.name().to_lowercase();
        if (new_track || t.is_none()) && self.gen_melody_track(t, mv) {
            return true;
        }
        let Some(t) = t else {
            return false;
        };
        // the open clip: replace the notes in the melody's span (asks
        // first when that deletes many), or on top; markers unchanged
        let marks = self.cl_marks(t);
        let span = [ChordMark {
            start: mv.start,
            len: (mv.end - mv.start).max(BAR),
            ..Default::default()
        }];
        let notes = Self::mel_notes(&mv.notes, 0);
        self.cl_put_notes_ex(
            t,
            marks,
            notes,
            &span,
            format!("{what} of {n} note(s) written"),
            on_top,
        );
        true
    }

    /// a new track "Melody" / "Bass" below the clip's track with a copy of
    /// the track's instrument (what Preview played) and a
    /// note clip where the source clip is; false: not possible (the caller
    /// writes into the clip instead)
    fn gen_melody_track(&mut self, t: Option<Target>, mv: &MelView) -> bool {
        let bass = self.cl.gen.tab == GenTab::Bass;
        let part = self.cl.gen.tab.name();
        let what = part.to_lowercase();
        // (track, clip start, offset, len, shift): where the source clip
        // is; with no clip, at the panel's song position (the notes move
        // so the first chord's bar starts the clip)
        let src = match t {
            Some(Target::Notes {
                clip: Some(src), ..
            }) => self.with(|s| {
                let c = s
                    .song
                    .timeline
                    .as_deref()?
                    .clips
                    .iter()
                    .find(|c| c.id == src)?;
                Some((Some(c.track as usize), c.start, c.offset, c.len, 0))
            }),
            _ => None,
        };
        let no_clip = src.is_none();
        let (tr, start, offset, len, shift) = match src {
            Some(x) => x,
            None if self.with(|s| s.song.timeline.is_none()) => {
                self.status =
                    format!("The {what} needs the timeline: switch to the timeline view first");
                return false;
            }
            None => {
                let tr = self
                    .cl
                    .gen
                    .track
                    .filter(|&k| k < self.with(|s| s.song.channels));
                (tr, self.cl.gen.at.max(0), 0, BAR, mv.start)
            }
        };
        if self.with(|s| s.song.channels) >= MAX_CHANNELS {
            self.status = format!("All 32 tracks are used: the {what} goes into this clip");
            return !no_clip;
        }
        if self.with(|s| s.song.instruments.len() >= 255) {
            self.status =
                format!("All 255 instrument slots are used: the {what} goes into this clip");
            return !no_clip;
        }
        // the sound Preview played: a copy of the clip's track instrument
        // (its patch, parameters and effects), so the written melody sounds
        // the same; a lead synth (Bass: a bass synth) only when the track
        // has none or plays a drum kit
        let heard = tr
            .filter(|&tr| !self.track_needs_instrument(tr))
            .map(|tr| self.track_instr(tr))
            .and_then(|slot| {
                self.with(|s| {
                    s.song
                        .instruments
                        .get((slot as usize).wrapping_sub(1))
                        // a drum kit is no bass or lead (Bass: any drum
                        // engine, as the track pie tells drums apart)
                        .filter(|i| {
                            if bass {
                                !super::trackpie::is_drum_kind(&i.kind)
                            } else {
                                crate::drumkit::kit_of(i).is_none()
                            }
                        })
                        .cloned()
                })
            });
        let inst = match heard {
            Some(i) => i,
            None => {
                let made = if bass {
                    bass_preset(self.cl.gen.bass.opts.style == BassStyle::Acid)
                        .or_else(|| super::trackpie::Builtin::Analog.make())
                } else {
                    super::trackpie::Builtin::WaveForge.make()
                };
                let Some((name, kind)) = made else {
                    return false;
                };
                Instrument {
                    name,
                    volume: 0.8,
                    transpose: 0,
                    fx: vec![],
                    kind,
                }
            }
        };
        let at = tr.map_or(self.with(|s| s.song.channels), |k| k + 1);
        self.snapshot();
        self.undo_hold += 1;
        let slot = if self.track_insert(at, part) {
            self.with(|s| {
                let k = s.song.add_instrument(inst)?;
                crate::chanops::assign_track_instrument(&mut s.song, at, k as u8 + 1);
                Some(k as u8 + 1)
            })
        } else {
            None
        };
        let mut notes = Self::mel_notes(&mv.notes, slot.unwrap_or(0));
        for n in &mut notes {
            n.start -= shift;
        }
        let end = mv.end - shift;
        let id = slot.and_then(|_| {
            self.with(|s| {
                let id = tl::new_note_clip(&mut s.song, at, start)?;
                let tlm = s.song.timeline.as_deref_mut()?;
                let need = end.max(offset + len);
                let data_len = (need + BAR - 1) / BAR * BAR;
                let c = tlm.clips.iter_mut().find(|c| c.id == id)?;
                c.offset = offset;
                c.len = len.max(end - offset).max(BAR);
                c.name = Some(part.into());
                let tl::ClipKind::Notes { data } = c.kind else {
                    return None;
                };
                let d = tlm.notes[data as usize].make_mut();
                d.name = part.into();
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
            self.status = format!("Couldn't add the {part} track");
            return true;
        };
        self.track_open_pattern(at, id);
        self.status = format!(
            "New track {:02} “{part}”: {} note(s) in {} (Ctrl+Z undoes it all)",
            at + 1,
            mv.notes.len(),
            if no_clip {
                self.with(|s| {
                    let n = (end - 1).max(0);
                    s.song.timeline.as_deref().map_or_else(String::new, |tl| {
                        let (a, b) = (tl.bar_pos(start).bar, tl.bar_pos(start + n).bar);
                        if b > a {
                            format!("song bars {a}–{b}")
                        } else {
                            format!("song bar {a}")
                        }
                    })
                })
            } else {
                bars_text(mv.start, mv.end)
            }
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
            .size(crate::theme::ts::px(crate::theme::ts::BODY))
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
            crate::theme::prop(crate::theme::ts::COMPACT),
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
