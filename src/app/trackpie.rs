//! The track pie menu (inspired by other DAWs; requested by Del (postmanpat), 19 Oct 2026): right-click
//! a track header (Timeline track list or tracker channel header) and a
//! radial menu opens centred on the pointer. The centre disc names the
//! track; nine segments around it: Rename, Colour, Icon, Add, Edit,
//! Devices, Routing, Organize, Delete. Point to highlight (the segment
//! lightens with an accent arc on the rim), click (or release the right
//! button after dragging onto one) to choose; Esc, the centre or a click
//! outside cancels. Colour / Icon / Add / Organize pop out a submenu next to
//! their segment. Arrow keys walk the ring (and a submenu), Enter chooses.
//!
//! Motion: opens in `OPEN_S` with an ease-out (scale 0.6 -> 1 and fade, the
//! segments staggered `STAGGER_S` apart around the ring), closes in
//! `CLOSE_S`, hover fades in `HOVER_S`. Repaints are requested only while
//! something moves. Settings › Look › Reduce motion shows it at once.
//! The ring geometry is computed once (unit vectors per segment) and only
//! scaled per frame. Every action reuses the app's own track operations
//! (one undo step each). See docs/TIMELINE.md "Track pie menu".
use super::glyphs::{self, Glyph};
use super::*;
use crate::timeline::TrackIcon;
use egui::{epaint::Mesh, Shape, Vec2};

pub(crate) const OPEN_S: f64 = 0.18;
pub(crate) const CLOSE_S: f64 = 0.12;
pub(crate) const HOVER_S: f32 = 0.08;
pub(crate) const STAGGER_S: f64 = 0.012;
pub(crate) const SUB_S: f64 = 0.12;
pub(crate) const R_OUT: f32 = 148.0;
pub(crate) const R_IN: f32 = 56.0;
/// room kept between the pie (with its rim arc) and the window edge
pub(crate) const MARGIN: f32 = 10.0;
/// a right-button drag longer than this, released on a segment, chooses it
pub(crate) const DRAG_CHOOSE_PX: f32 = 12.0;
const STEPS: usize = 14;
const GAP: f32 = 0.012;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Seg {
    Rename,
    Colour,
    Icon,
    Add,
    /// the track's instrument editor in the lower panel (or the chooser)
    Edit,
    Devices,
    Routing,
    Organize,
    Delete,
}

/// clockwise from the top
pub(crate) const SEGS: [Seg; 9] = [
    Seg::Rename,
    Seg::Colour,
    Seg::Icon,
    Seg::Add,
    Seg::Edit,
    Seg::Devices,
    Seg::Routing,
    Seg::Organize,
    Seg::Delete,
];

impl Seg {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Seg::Rename => "Rename",
            Seg::Colour => "Colour",
            Seg::Icon => "Icon",
            Seg::Add => "Add",
            Seg::Devices => "Devices",
            Seg::Routing => "Routing",
            Seg::Organize => "Organize",
            Seg::Edit => "Edit",
            Seg::Delete => "Delete",
        }
    }
    fn glyph(self) -> Glyph {
        match self {
            Seg::Rename => Glyph::Pencil,
            Seg::Colour => Glyph::Palette,
            Seg::Icon => Glyph::Tiles,
            Seg::Add => Glyph::Plus,
            Seg::Devices => Glyph::Rack,
            Seg::Routing => Glyph::Route,
            Seg::Organize => Glyph::Folder,
            Seg::Edit => Glyph::Keys,
            Seg::Delete => Glyph::Trash,
        }
    }
    /// opens a submenu instead of acting
    pub(crate) fn has_sub(self) -> bool {
        matches!(self, Seg::Colour | Seg::Icon | Seg::Organize)
    }
    /// opens the Add rings around the pie (`trackring.rs`)
    pub(crate) fn has_ring(self) -> bool {
        self == Seg::Add
    }
    /// the action of a segment without a submenu
    pub(crate) fn act(self) -> Option<PieAct> {
        match self {
            Seg::Rename => Some(PieAct::Rename),
            Seg::Edit => Some(PieAct::EditInstrument),
            Seg::Devices => Some(PieAct::Devices),
            Seg::Routing => Some(PieAct::Routing),
            Seg::Delete => Some(PieAct::Delete),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MoveTo {
    Up,
    Down,
    Top,
    Bottom,
}

/// what a choice in the pie does (`App::pie_apply`)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PieAct {
    Rename,
    Colour(Option<u32>),
    Icon(TrackIcon),
    AddInstrument,
    AddAudio,
    /// an empty track above (true) or below
    AddEmpty(bool),
    Duplicate,
    Devices,
    Routing,
    Move(MoveTo),
    Speed(u8),
    Delete,
    /// the track's automation: its lanes (show / hide, on / off) and the
    /// parameter picker for a new one (`App::track_auto_menu`)
    Automation,
    /// the Browser's Instruments (Favourites first when there are any):
    /// the next pick replaces this track's instrument
    ChangeInstrument,
    /// Add › Drum pattern › <instrument>: a new track below with it and an
    /// empty 1-bar pattern at the play marker (`track_new_pattern_track`)
    DrumTrack(Builtin),
    /// ... › Drum Kit › <kit>: the same with that Drum Kit
    DrumKitTrack(&'static str),
    /// ... › Browse…: the Browser's Instruments; the pick makes that track
    DrumBrowse,
    /// ... › Drum Kit › More kits…: the Browser on Drum Kits, the same
    MoreKits,
    /// Add › Automation lane › Show / Hide lanes
    AutoToggle,
    /// Add › Automation lane › lane `k` (`track_lanes`) on / off
    AutoLane(u8),
    /// the track's instrument editor (the chooser when it has none)
    EditInstrument,
    /// a new built-in instrument (its Init / first preset) becomes this
    /// track's instrument
    NewInstrument(Builtin),
    /// the track pie's Drum pattern › <instrument> on THIS track: it plays
    /// a new one and its pattern opens (`track_set_pattern_instrument`)
    DrumHere(Builtin),
    /// ... › Drum Kit › <kit> on THIS track (an empty pattern gets the
    /// kit's starter groove)
    DrumKitHere(&'static str),
    /// ... › Drum Kit › More kits…: the Browser on Drum Kits, the pick
    /// plays THIS track
    MoreKitsHere,
    /// an empty 1-bar pattern on this track at the play marker
    NewPattern,
    /// mute / solo this track (as its M / S buttons)
    Mute,
    Solo,
    /// the empty-area pie: a new track at the end playing a new built-in
    NewTrackWith(Builtin),
}

/// the built-in instruments the pie's Add › puts straight on a track
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Builtin {
    WaveForge,
    DrumKit,
    StringWave,
    Synth,
    Analog,
    Fm4,
    Wavetable,
    Grid,
    Noise,
    DrumSynth,
    ChipFx,
    Voice,
}

impl Builtin {
    pub(crate) const ALL: [Builtin; 12] = [
        Builtin::WaveForge,
        Builtin::DrumKit,
        Builtin::StringWave,
        Builtin::Synth,
        Builtin::Analog,
        Builtin::Fm4,
        Builtin::Wavetable,
        Builtin::Grid,
        Builtin::Noise,
        Builtin::DrumSynth,
        Builtin::ChipFx,
        Builtin::Voice,
    ];
    pub(crate) fn label(self) -> &'static str {
        match self {
            Builtin::WaveForge => crate::poly::POLY_DISPLAY_NAME,
            Builtin::DrumKit => "Drum Kit",
            Builtin::StringWave => crate::strings::STRINGS_DISPLAY_NAME,
            Builtin::Synth => "Synth",
            Builtin::Analog => "Analog",
            Builtin::Fm4 => "FM-4",
            Builtin::Wavetable => "Wavetable",
            Builtin::Grid => "Grid",
            Builtin::Noise => "Noise",
            Builtin::DrumSynth => "Drum Synth",
            Builtin::ChipFx => "Chip FX",
            Builtin::Voice => "Voice",
        }
    }
    pub(crate) fn tip(self) -> &'static str {
        match self {
            Builtin::WaveForge => "WaveForge polyphonic synth · starts on Saw Lead",
            Builtin::DrumKit => "Drum Kit: 16 pads and a step sequencer · starts on Psy Kit",
            Builtin::StringWave => concat!(
                crate::strings_name!(),
                ": a lush string ensemble · starts on Velvet Ensemble"
            ),
            Builtin::Synth => "the classic ChipWave synth · starts on Supersaw lead",
            Builtin::Analog => "Analog: 3 oscillators into a ladder filter · starts on Warm Lead",
            Builtin::Fm4 => "FM-4: four-operator FM · starts on E-Piano",
            Builtin::Wavetable => "Wavetable synth · starts on Wave Supersaw",
            Builtin::Grid => "Grid: a modular patch · starts on Grid Saw Bass",
            Builtin::Noise => "Noise: risers, impacts, wind · starts on White Riser",
            Builtin::DrumSynth => "Drum Synth: a synthesized kit on the keys · starts on Psy Kit",
            Builtin::ChipFx => "Chip FX: sfxr game sounds · starts on Blip",
            Builtin::Voice => "Voice: synthetic vowels and words · starts on Choir Eee",
        }
    }
    /// the browser row's glyph (`glyphs::glyph_for`)
    pub(crate) fn glyph_tag(self) -> &'static str {
        match self {
            Builtin::WaveForge | Builtin::Synth | Builtin::Fm4 | Builtin::StringWave => "Synth",
            Builtin::DrumKit | Builtin::DrumSynth => "Drum",
            Builtin::Analog => "Analog",
            Builtin::Wavetable => "Wave",
            Builtin::Grid => "Grid",
            Builtin::Noise => "Noise",
            Builtin::ChipFx => "Chip FX",
            Builtin::Voice => "Voice",
        }
    }
    /// a fresh instrument: its name and kind, on a playable preset (the
    /// one named in `tip`), else the engine's first preset
    pub(crate) fn make(self) -> Option<(String, InstrumentKind)> {
        use crate::gen::GenEngine as G;
        let gen = |f: fn(&G) -> bool, prefer: &str| {
            let all = crate::gen::presets();
            let pick = all
                .iter()
                .find(|(n, _, p)| f(&p.engine) && *n == prefer)
                .or_else(|| all.iter().find(|(_, _, p)| f(&p.engine)))
                .cloned();
            pick.map(|(n, _, mut p)| {
                p.base.name = n.into();
                (n.to_string(), InstrumentKind::Gen(p))
            })
        };
        match self {
            Builtin::WaveForge => gen(|e| matches!(e, G::Poly(_)), "WaveForge Saw Lead"),
            Builtin::DrumKit => crate::drumkit::presets()
                .into_iter()
                .next()
                .map(|(n, _, p)| (n, InstrumentKind::Gen(p))),
            Builtin::Synth => Some(match crate::presets::factory("psytrance/supersaw-lead") {
                Some(mut p) => {
                    p.name = "Supersaw lead".into();
                    ("Supersaw lead".into(), InstrumentKind::Synth(p))
                }
                None => ("Synth".into(), InstrumentKind::Synth(Default::default())),
            }),
            Builtin::StringWave => gen(
                |e| matches!(e, G::Strings(_)),
                concat!(crate::strings_name!(), " Velvet Ensemble"),
            ),
            Builtin::Analog => gen(|e| matches!(e, G::Analog(_)), "Warm Lead"),
            Builtin::Fm4 => gen(|e| matches!(e, G::Fm(_)), "E-Piano"),
            Builtin::Wavetable => gen(|e| matches!(e, G::Wave(_)), "Wave Supersaw"),
            Builtin::Grid => gen(|e| matches!(e, G::Grid(_)), "Grid Saw Bass"),
            Builtin::Noise => gen(|e| matches!(e, G::Noise(_)), "White Riser"),
            Builtin::DrumSynth => gen(|e| matches!(e, G::Kit(_)), "Psy Kit"),
            Builtin::ChipFx => gen(|e| matches!(e, G::ChipFx(_)), "Blip"),
            Builtin::Voice => Some(match crate::voice::preset("Choir Eee") {
                Some(mut v) => {
                    v.base.name = "Choir Eee".into();
                    ("Choir Eee".into(), InstrumentKind::Voice(v))
                }
                None => (
                    "Voice".into(),
                    InstrumentKind::Voice(crate::voice::VoicePatch::default()),
                ),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ItemKind {
    Text(String),
    Swatch(u32),
    Icon(TrackIcon),
}

/// one submenu entry; `act` None = disabled (with `tip`)
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Item {
    pub(crate) act: Option<PieAct>,
    pub(crate) kind: ItemKind,
    pub(crate) tip: &'static str,
    /// the current value (lit)
    pub(crate) on: bool,
    /// shown in the header strip (the Auto toggle)
    pub(crate) header: bool,
}

/// what the pie needs to know about its track
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TrackInfo {
    pub(crate) index: usize,
    pub(crate) tracks: usize,
    pub(crate) timeline: bool,
    pub(crate) colour: Option<u32>,
    pub(crate) icon: TrackIcon,
    pub(crate) speed: u8,
    pub(crate) full: bool,
    /// all 255 instrument slots are used
    pub(crate) instr_full: bool,
    /// its automation lanes: (label, on)
    pub(crate) lanes: Vec<(String, bool)>,
    pub(crate) lanes_shown: bool,
    pub(crate) mute: bool,
    pub(crate) solo: bool,
    /// the empty-area pie (right-click below the tracks): every choice
    /// makes a NEW track at the end; the track pie never does (but Duplicate)
    pub(crate) new: bool,
}

pub(crate) fn sub_title(seg: Seg) -> &'static str {
    match seg {
        Seg::Colour => "TRACK COLOUR",
        Seg::Icon => "TRACK ICON",
        Seg::Organize => "ORGANIZE",
        _ => "",
    }
}

/// the entries of a segment's submenu (empty: none)
pub(crate) fn sub_items(seg: Seg, i: &TrackInfo) -> Vec<Item> {
    let text = |l: &str, act: Option<PieAct>, tip: &'static str| Item {
        act,
        kind: ItemKind::Text(l.to_string()),
        tip,
        on: false,
        header: false,
    };
    let need_tl = "needs timeline mode (Timeline › Switch to timeline mode)";
    match seg {
        Seg::Colour => {
            let mut v = vec![Item {
                act: i.timeline.then_some(PieAct::Colour(None)),
                kind: ItemKind::Text("Auto".into()),
                tip: if i.timeline {
                    "the palette colour of the track's position"
                } else {
                    need_tl
                },
                on: i.colour.is_none(),
                header: true,
            }];
            for c in super::timeline::track_palette() {
                v.push(Item {
                    act: i.timeline.then_some(PieAct::Colour(Some(c))),
                    kind: ItemKind::Swatch(c),
                    tip: if i.timeline { "" } else { need_tl },
                    on: i.colour == Some(c),
                    header: false,
                });
            }
            v
        }
        Seg::Icon => {
            let mut v = vec![Item {
                act: i.timeline.then_some(PieAct::Icon(TrackIcon::Auto)),
                kind: ItemKind::Text("Auto".into()),
                tip: if i.timeline {
                    "picked from the instrument type"
                } else {
                    need_tl
                },
                on: i.icon == TrackIcon::Auto,
                header: true,
            }];
            for ic in TrackIcon::ALL {
                v.push(Item {
                    act: i.timeline.then_some(PieAct::Icon(ic)),
                    kind: ItemKind::Icon(ic),
                    tip: if i.timeline { ic.label() } else { need_tl },
                    on: i.icon == ic,
                    header: false,
                });
            }
            v
        }
        Seg::Organize => {
            let first = i.index == 0;
            let last = i.index + 1 >= i.tracks;
            let mut v = vec![
                text(
                    "Move up",
                    (!first).then_some(PieAct::Move(MoveTo::Up)),
                    "already the first track",
                ),
                text(
                    "Move down",
                    (!last).then_some(PieAct::Move(MoveTo::Down)),
                    "already the last track",
                ),
                text(
                    "Move to top",
                    (!first).then_some(PieAct::Move(MoveTo::Top)),
                    "already the first track",
                ),
                text(
                    "Move to bottom",
                    (!last).then_some(PieAct::Move(MoveTo::Bottom)),
                    "already the last track",
                ),
            ];
            for (s, l) in [
                (0u8, "Speed: follow the song"),
                (3, "Speed 3"),
                (6, "Speed 6"),
                (12, "Speed 12"),
            ] {
                v.push(Item {
                    on: i.speed == s,
                    ..text(
                        l,
                        Some(PieAct::Speed(s)),
                        "ticks per row for this track's effects (timeline mode)",
                    )
                });
            }
            v.push(text("Save CPU (coming soon)", None, SAVE_CPU_TIP));
            v
        }
        _ => vec![],
    }
}

/// columns and cell size of a submenu grid
pub(crate) fn sub_grid(seg: Seg) -> (usize, Vec2) {
    match seg {
        Seg::Icon => (4, egui::vec2(44.0, 34.0)),
        Seg::Colour => (6, egui::vec2(28.0, 28.0)),
        _ => (1, egui::vec2(184.0, 26.0)),
    }
}

const SUB_PAD: f32 = 10.0;
const SUB_HEAD: f32 = 26.0;

/// the submenu's size and its items' rects relative to its top-left
pub(crate) fn sub_layout(seg: Seg, items: &[Item]) -> (Vec2, Vec<Rect>) {
    let (cols, cell) = sub_grid(seg);
    let gap = 4.0;
    let grid: Vec<usize> = (0..items.len()).filter(|&k| !items[k].header).collect();
    let rows = grid.len().div_ceil(cols).max(1);
    let w = SUB_PAD * 2.0 + cols as f32 * cell.x + (cols - 1) as f32 * gap;
    let w = w.max(200.0);
    let top = SUB_PAD + SUB_HEAD;
    let h = top + rows as f32 * cell.y + (rows - 1) as f32 * gap + SUB_PAD;
    let mut rects = vec![Rect::NOTHING; items.len()];
    let mut n = 0;
    for (k, it) in items.iter().enumerate() {
        if it.header {
            rects[k] = Rect::from_min_size(
                Pos2::new(w - SUB_PAD - 58.0, SUB_PAD - 1.0),
                egui::vec2(58.0, 20.0),
            );
        } else {
            let (c, r) = (n % cols, n / cols);
            let x0 = if cols == 1 {
                SUB_PAD
            } else {
                (w - (cols as f32 * cell.x + (cols - 1) as f32 * gap)) / 2.0
            };
            rects[k] = Rect::from_min_size(
                Pos2::new(
                    x0 + c as f32 * (cell.x + gap),
                    top + r as f32 * (cell.y + gap),
                ),
                if cols == 1 {
                    egui::vec2(w - 2.0 * SUB_PAD, cell.y)
                } else {
                    cell
                },
            );
            n += 1;
        }
    }
    (egui::vec2(w, h), rects)
}

/// how far the nearest point of `r` is from `c`
fn rect_dist(r: Rect, c: Pos2) -> f32 {
    let q = Pos2::new(
        c.x.clamp(r.left(), r.right()),
        c.y.clamp(r.top(), r.bottom()),
    );
    (q - c).length()
}

/// the panel keeps this far from the pie's centre (clear of the ring and
/// its rim arc)
const SUB_CLEAR: f32 = R_OUT + 10.0;

/// keep `r` on `screen` (4 px in)
fn on_screen(mut r: Rect, screen: Rect) -> Rect {
    if r.right() > screen.right() - 4.0 {
        r = r.translate(egui::vec2(screen.right() - 4.0 - r.right(), 0.0));
    }
    if r.left() < screen.left() + 4.0 {
        r = r.translate(egui::vec2(screen.left() + 4.0 - r.left(), 0.0));
    }
    if r.bottom() > screen.bottom() - 4.0 {
        r = r.translate(egui::vec2(0.0, screen.bottom() - 4.0 - r.bottom()));
    }
    if r.top() < screen.top() + 4.0 {
        r = r.translate(egui::vec2(0.0, screen.top() + 4.0 - r.top()));
    }
    r
}

/// where a segment's submenu sits: just outside the rim in the segment's
/// direction, on the side it points to, kept on screen. It is pushed out
/// along the segment's direction until it clears the ring (so the way to
/// it from the segment crosses no other segment); only when the screen
/// edge leaves no room there does it go to the pie's other side.
pub(crate) fn sub_rect(centre: Pos2, seg: usize, size: Vec2, screen: Rect) -> Rect {
    let a = seg_angle(seg);
    let d = egui::vec2(a.cos(), a.sin());
    let right = d.x >= -0.2;
    let place = |dir: Vec2, right: bool| -> Rect {
        let anchor = centre + dir * (R_OUT + 12.0);
        let x = if right { anchor.x } else { anchor.x - size.x };
        let mut r = Rect::from_min_size(Pos2::new(x, anchor.y - size.y / 2.0), size);
        // out along the segment until it clears the ring
        let mut n = 0;
        while rect_dist(r, centre) < SUB_CLEAR && n < 200 {
            r = r.translate(dir * 2.0);
            n += 1;
        }
        let mut r = on_screen(r, screen);
        // the screen edge pushed it back over the ring: sideways out instead
        let side = egui::vec2(if right { 1.0 } else { -1.0 }, 0.0);
        let mut n = 0;
        while rect_dist(r, centre) < SUB_CLEAR && n < 200 {
            r = r.translate(side * 2.0);
            n += 1;
        }
        r
    };
    let r = place(d, right);
    if screen.shrink(3.9).contains_rect(r) && rect_dist(r, centre) >= SUB_CLEAR - 0.5 {
        return r;
    }
    // no room on this side: the mirror side, else the first try on screen
    let m = place(egui::vec2(-d.x, d.y), !right);
    if screen.shrink(3.9).contains_rect(m) && rect_dist(m, centre) >= SUB_CLEAR - 0.5 {
        return m;
    }
    on_screen(r, screen)
}

/// The way from the open submenu's segment to its panel: the pointer is
/// "on its way" while it is inside the hull of `from` (where it last was
/// on the segment or the panel) and the panel's corners (a few px bigger).
pub(crate) fn in_aim(from: Pos2, panel: Rect, p: Pos2) -> bool {
    let r = panel.expand(8.0);
    if r.contains(p) {
        return true;
    }
    let cs = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    ];
    let cross = |a: Pos2, b: Pos2, c: Pos2| (b - a).x * (c - a).y - (b - a).y * (c - a).x;
    let tri = |a: Pos2, b: Pos2, c: Pos2| {
        let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
        let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(neg && pos)
    };
    (0..4).any(|i| (i + 1..4).any(|j| tri(from, cs[i], cs[j])))
}

/// pointer resting this long on another segment (inside the way to the
/// panel) switches to it after all
pub(crate) const AIM_SETTLE_S: f64 = 0.35;

// ------------------------------------------------------------ motion maths

/// ease-out cubic: fast start, gentle landing (0 -> 0, 1 -> 1)
pub(crate) fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// ease-in cubic (closing: lingers, then goes)
pub(crate) fn ease_in(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

/// how far segment `i` has opened, `elapsed` seconds after the pie opened
/// (eased, 0..=1); each segment starts `STAGGER_S` after the one before
pub(crate) fn seg_progress(elapsed: f64, i: usize, reduce: bool) -> f32 {
    if reduce {
        return 1.0;
    }
    ease_out(((elapsed - i as f64 * STAGGER_S) / OPEN_S) as f32)
}

/// the centre disc (opens first)
pub(crate) fn disc_progress(elapsed: f64, reduce: bool) -> f32 {
    if reduce {
        1.0
    } else {
        ease_out((elapsed / OPEN_S) as f32)
    }
}

/// 1 while open, falling to 0 over `CLOSE_S` after closing started
pub(crate) fn close_progress(since_close: f64, reduce: bool) -> f32 {
    if reduce {
        return 0.0;
    }
    1.0 - ease_in((since_close / CLOSE_S) as f32)
}

/// the whole open animation is done (no more repaints needed)
pub(crate) fn open_done(elapsed: f64, reduce: bool) -> bool {
    reduce || elapsed >= OPEN_S + (SEGS.len() - 1) as f64 * STAGGER_S
}

/// scale of a part that is `k` open (0.6 -> 1)
pub(crate) fn scale_of(k: f32) -> f32 {
    0.6 + 0.4 * k
}

// ------------------------------------------------------------ geometry

/// centre angle of segment `i` (radians; 0 = right, clockwise, top first)
pub(crate) fn seg_angle(i: usize) -> f32 {
    -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / SEGS.len() as f32
}

/// the pie's centre for a click at `p`: moved in so the whole pie and its
/// Add rings (with their rim arc and `MARGIN`) are inside `screen`
pub(crate) fn clamp_centre(p: Pos2, screen: Rect) -> Pos2 {
    super::radial::clamp_disc(p, screen, super::trackring::R_FIT)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hit {
    Centre,
    Seg(usize),
    Outside,
}

/// what is under `p` for a pie at `centre`
pub(crate) fn hit(centre: Pos2, p: Pos2) -> Hit {
    let d = p - centre;
    let r = d.length();
    if r < R_IN {
        return Hit::Centre;
    }
    if r > R_OUT + 6.0 {
        return Hit::Outside;
    }
    let step = std::f32::consts::TAU / SEGS.len() as f32;
    let a = d.y.atan2(d.x) + std::f32::consts::FRAC_PI_2 + step / 2.0;
    let a = a.rem_euclid(std::f32::consts::TAU);
    Hit::Seg(((a / step) as usize).min(SEGS.len() - 1))
}

/// unit geometry of every segment: (inner, outer) unit vectors along its
/// arc, computed once; drawing only scales and translates it
fn ring() -> &'static Vec<Vec<Vec2>> {
    static RING: std::sync::OnceLock<Vec<Vec<Vec2>>> = std::sync::OnceLock::new();
    RING.get_or_init(|| {
        let step = std::f32::consts::TAU / SEGS.len() as f32;
        (0..SEGS.len())
            .map(|i| {
                let a0 = seg_angle(i) - step / 2.0 + GAP;
                let a1 = seg_angle(i) + step / 2.0 - GAP;
                (0..=STEPS)
                    .map(|k| {
                        let a = a0 + (a1 - a0) * k as f32 / STEPS as f32;
                        egui::vec2(a.cos(), a.sin())
                    })
                    .collect()
            })
            .collect()
    })
}

// ------------------------------------------------------------ state

pub(crate) struct Pie {
    pub(crate) track: usize,
    pub(crate) name: String,
    pub(crate) centre: Pos2,
    /// input time it opened at
    pub(crate) t0: f64,
    /// input time closing started (the pie is drawn fading out, inert)
    pub(crate) closing: Option<f64>,
    /// keyboard / hover highlight on the ring
    pub(crate) key: Option<usize>,
    /// the open submenu (segment index) and when it opened
    pub(crate) sub: Option<(usize, f64)>,
    pub(crate) sub_key: Option<usize>,
    /// opened by a right press at this point that may still be held
    pub(crate) press: Option<Pos2>,
    /// screenshots: draw as if this long after opening
    pub(crate) frozen: Option<f64>,
    /// pointer position last frame (hover follows the mouse only when it moves)
    pub(crate) last_ptr: Option<Pos2>,
    /// where the pointer last was on the open submenu's segment or panel
    /// (the tip of the way to the panel, `in_aim`)
    aim_from: Option<Pos2>,
    /// input time the pointer last moved (resting on another segment on
    /// the way to the panel switches to it after `AIM_SETTLE_S`)
    pub(crate) moved_at: f64,
    /// Add's rings around the pie (`trackring.rs`)
    pub(crate) ring: Option<super::trackring::Ring>,
    /// opened on the empty area below the tracks: only the New track ring
    /// (no segments); `track` is the new track's index (the end)
    pub(crate) empty: bool,
}

/// colours of the pie in the current theme
pub(crate) struct Pal {
    pub(crate) ring: Color32,
    pub(crate) ring_out: Color32,
    pub(crate) edge: Color32,
    pub(crate) hover: Color32,
    pub(crate) disc: Color32,
    pub(crate) disc_ink: Color32,
    pub(crate) disc_ink2: Color32,
    pub(crate) ink: Color32,
    pub(crate) ink2: Color32,
    pub(crate) icon_bg: Color32,
    pub(crate) accent: Color32,
    pub(crate) danger: Color32,
    pub(crate) panel: Color32,
}

pub(crate) fn pal(t: &crate::theme::Theme) -> Pal {
    if t.light {
        // Amiga Workbench: light grey ring, navy disc, the orange accent
        Pal {
            ring: hex(0xD5DCE5),
            ring_out: hex(0xE9EDF2),
            edge: hex(0x3E4652),
            hover: hex(0xF7F9FB),
            disc: hex(0x1747A8),
            disc_ink: Color32::WHITE,
            disc_ink2: hex(0xC9D6F2),
            ink: hex(0x1B2330),
            ink2: hex(0x6A7380),
            icon_bg: hex(0xC2CCD8),
            accent: hex(0xF5821F),
            danger: hex(0xC0392B),
            panel: hex(0xE4E9EF),
        }
    } else {
        Pal {
            ring: hex(0x2A2E33),
            ring_out: hex(0x353A40),
            edge: hex(0x4A5058),
            hover: hex(0x4A525B),
            disc: hex(0x1F2226),
            disc_ink: t.ink,
            disc_ink2: t.ink2,
            ink: t.ink,
            ink2: hex(0x7D848C),
            icon_bg: hex(0x3E444B),
            accent: t.accent,
            danger: hex(0xE0645A),
            panel: hex(0x2A2E33),
        }
    }
}

fn seg_enabled(_seg: Seg) -> bool {
    true
}

const SAVE_CPU_TIP: &str = "Save CPU (freeze / deactivate a track): coming soon (docs/ROADMAP.md)";

impl App {
    /// open the pie for track `tr` at `at` (`press`: by a right press that
    /// may be held, so releasing on a segment chooses it)
    pub(crate) fn open_track_pie(&mut self, tr: usize, at: Pos2, press: bool, ctx: &egui::Context) {
        let name = self.with(|s| s.song.channel_names.get(tr).cloned().unwrap_or_default());
        let screen = ctx.content_rect();
        self.ch = tr;
        self.track_rename = None;
        self.pie = Some(Pie {
            track: tr,
            name,
            centre: clamp_centre(at, screen),
            t0: ctx.input(|i| i.time),
            closing: None,
            key: None,
            sub: None,
            sub_key: None,
            press: press.then_some(at),
            frozen: None,
            last_ptr: None,
            aim_from: None,
            moved_at: ctx.input(|i| i.time),
            ring: None,
            empty: false,
        });
        ctx.request_repaint();
    }

    /// right-click on the empty area below the tracks (or an empty lane):
    /// the New track ring (Instrument track ›, Drum pattern ›, Audio, Empty,
    /// Browse…) at `at`; every choice adds a track at the end
    pub(crate) fn open_add_pie(&mut self, at: Pos2, press: bool, ctx: &egui::Context) {
        let n = self.with(|s| s.song.channels);
        let screen = ctx.content_rect();
        let now = ctx.input(|i| i.time);
        self.track_rename = None;
        let slots = super::trackring::ring_slots(
            super::trackring::Level::Add,
            &self.ring_info_for(n, true),
        );
        self.pie = Some(Pie {
            track: n,
            name: "New track".into(),
            centre: clamp_centre(at, screen),
            t0: now,
            closing: None,
            key: None,
            sub: None,
            sub_key: None,
            press: press.then_some(at),
            frozen: None,
            last_ptr: None,
            aim_from: None,
            moved_at: now,
            ring: Some(super::trackring::Ring::root_n(now, slots.len(), 0)),
            empty: true,
        });
        ctx.request_repaint();
    }

    /// `pie_info` with the pie's mode (`new`: the empty-area pie)
    pub(crate) fn ring_info_for(&self, tr: usize, new: bool) -> TrackInfo {
        TrackInfo {
            new,
            ..self.pie_info(tr)
        }
    }

    pub(crate) fn pie_info(&self, tr: usize) -> TrackInfo {
        let lanes = self
            .track_lanes(tr)
            .into_iter()
            .map(|(_, l, on)| (l, on))
            .collect();
        let lanes_shown = self.track_lanes_shown(tr);
        self.with(|s| {
            let t = s.song.timeline.as_deref().and_then(|t| t.tracks.get(tr));
            let mx = s.song.mixer.channels.get(tr);
            TrackInfo {
                index: tr,
                tracks: s.song.channels,
                timeline: s.song.timeline.is_some(),
                colour: t.and_then(|t| t.colour),
                icon: t.map_or(TrackIcon::Auto, |t| t.icon),
                speed: t.map_or(0, |t| t.speed),
                full: s.song.channels >= MAX_CHANNELS,
                instr_full: s.song.instruments.len() >= 255,
                lanes,
                lanes_shown,
                mute: mx.is_some_and(|m| m.mute),
                solo: mx.is_some_and(|m| m.solo),
                new: false,
            }
        })
    }

    /// start closing (the pie fades out, taking no input)
    pub(crate) fn pie_close(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if let Some(p) = &mut self.pie {
            if p.closing.is_none() {
                p.closing = Some(now);
            }
        }
        if self.extra.reduce_motion {
            self.pie = None;
        }
        ctx.request_repaint();
    }

    /// the pie is open and taking input
    pub(crate) fn pie_open(&self) -> bool {
        self.pie.as_ref().is_some_and(|p| p.closing.is_none())
    }

    /// choose segment `i` (a submenu opens; others act and close)
    pub(crate) fn pie_choose(&mut self, i: usize, ctx: &egui::Context) {
        let Some(seg) = SEGS.get(i).copied() else {
            return;
        };
        let now = ctx.input(|i| i.time);
        if seg.has_ring() {
            if let Some(p) = &mut self.pie {
                p.open_ring(now);
            }
            return;
        }
        if seg.has_sub() {
            if let Some(p) = &mut self.pie {
                p.key = Some(i);
                if p.sub.map(|s| s.0) != Some(i) {
                    p.sub = Some((i, now));
                    p.sub_key = None;
                }
            }
            return;
        }
        if let Some(a) = seg.act() {
            let tr = self.pie.as_ref().map_or(0, |p| p.track);
            self.pie_close(ctx);
            self.pie_apply(tr, a, ctx);
        }
    }

    /// choose submenu item `k` of the open submenu
    pub(crate) fn pie_choose_item(&mut self, k: usize, ctx: &egui::Context) {
        let Some(p) = &self.pie else { return };
        let Some((si, _)) = p.sub else { return };
        let tr = p.track;
        let items = sub_items(SEGS[si], &self.pie_info(tr));
        let Some(it) = items.get(k) else { return };
        match it.act {
            Some(a) => {
                self.pie_close(ctx);
                self.pie_apply(tr, a, ctx);
            }
            None => self.status = it.tip.into(),
        }
    }

    /// keys while the pie is open: arrows walk the ring or the submenu,
    /// Enter / Space choose, Esc closes the submenu, then the pie. Every
    /// other key is swallowed so shortcuts don't act behind the menu.
    fn pie_keys(&mut self, ctx: &egui::Context) {
        let events = std::mem::take(&mut self.pending);
        let n = SEGS.len();
        for e in events {
            let egui::Event::Key {
                key, pressed: true, ..
            } = e
            else {
                continue;
            };
            let Some(p) = &mut self.pie else { return };
            if p.closing.is_some() {
                return;
            }
            if p.ring.is_some() {
                self.ring_key(key, ctx);
                ctx.request_repaint();
                continue;
            }
            let tr = p.track;
            if let Some((si, _)) = p.sub {
                let items = sub_items(SEGS[si], &self.pie_info(tr));
                let (cols, _) = sub_grid(SEGS[si]);
                let p = self.pie.as_mut().unwrap();
                let m = items.len();
                let cur = p.sub_key;
                let mv = |d: isize| -> usize {
                    match cur {
                        None => 0,
                        Some(c) => (c as isize + d).rem_euclid(m as isize) as usize,
                    }
                };
                match key {
                    Key::ArrowRight => p.sub_key = Some(mv(1)),
                    Key::ArrowLeft => p.sub_key = Some(mv(-1)),
                    Key::ArrowDown => p.sub_key = Some(mv(cols as isize)),
                    Key::ArrowUp => p.sub_key = Some(mv(-(cols as isize))),
                    Key::Enter | Key::Space => {
                        if let Some(k) = cur {
                            self.pie_choose_item(k, ctx);
                        }
                    }
                    Key::Escape | Key::Backspace => {
                        p.sub = None;
                        p.sub_key = None;
                    }
                    _ => {}
                }
            } else {
                let cur = p.key;
                let mv = |d: isize| -> usize {
                    match cur {
                        None => {
                            if d > 0 {
                                0
                            } else {
                                n - 1
                            }
                        }
                        Some(c) => (c as isize + d).rem_euclid(n as isize) as usize,
                    }
                };
                match key {
                    Key::ArrowRight | Key::ArrowDown | Key::Tab => p.key = Some(mv(1)),
                    Key::ArrowLeft | Key::ArrowUp => p.key = Some(mv(-1)),
                    Key::Enter | Key::Space => {
                        if let Some(k) = cur {
                            self.pie_choose(k, ctx);
                        }
                    }
                    Key::Escape => self.pie_close(ctx),
                    _ => {}
                }
            }
            ctx.request_repaint();
        }
    }

    /// the pie, every frame (before the views' key handling)
    pub(crate) fn track_pie(&mut self, ctx: &egui::Context) {
        let Some(p) = &self.pie else { return };
        let reduce = self.extra.reduce_motion;
        let now = ctx.input(|i| i.time);
        // closed and faded out: gone
        if let Some(c) = p.closing {
            if reduce || now - c >= CLOSE_S {
                self.pie = None;
                return;
            }
        }
        if self.pie_open() {
            self.pie_keys(ctx);
        }
        let Some(p) = &self.pie else { return };
        let tr = p.track;
        let empty = p.empty;
        let info = self.pie_info(tr);
        if tr >= info.tracks && !empty {
            self.pie = None;
            return;
        }
        let screen = ctx.content_rect();
        let open = p.closing.is_none();
        let elapsed = p.frozen.unwrap_or(now - p.t0);
        let fade = p.closing.map_or(1.0, |c| close_progress(now - c, reduce));
        let centre = p.centre;
        let sub = p.sub;
        let items: Vec<Item> = sub
            .map(|(si, _)| sub_items(SEGS[si], &info))
            .unwrap_or_default();
        let sub_geo = sub.map(|(si, t)| {
            let (size, rects) = sub_layout(SEGS[si], &items);
            (si, t, sub_rect(centre, si, size, screen), rects)
        });
        // ---- input
        let (ptr, moved, primary_rel, primary_press, sec_rel, sec_press) = ctx.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.delta() != Vec2::ZERO,
                i.pointer.button_released(egui::PointerButton::Primary),
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.button_released(egui::PointerButton::Secondary),
                i.pointer.button_pressed(egui::PointerButton::Secondary),
            )
        });
        let id = egui::Id::unique("track_pie");
        // a blanket over the window while open: nothing behind reacts
        let mut over_item: Option<usize> = None;
        let mut over_seg: Option<usize> = None;
        let mut on_centre = false;
        let mut in_sub = false;
        if let Some(pp) = ptr {
            if let Some((_, _, r, rects)) = &sub_geo {
                if r.contains(pp) {
                    in_sub = true;
                    over_item = rects
                        .iter()
                        .position(|q| q.translate(r.min.to_vec2()).contains(pp));
                }
            }
            if !in_sub {
                match hit(centre, pp) {
                    Hit::Seg(k) => over_seg = Some(k),
                    Hit::Centre => on_centre = true,
                    Hit::Outside => {}
                }
            }
        }
        if open {
            egui::Area::new(id.with("blanket"))
                .order(egui::Order::Foreground)
                .fixed_pos(screen.min)
                .show(ctx, |ui| {
                    ui.allocate_rect(screen, Sense::click_and_drag());
                });
            if self.pie.as_ref().is_some_and(|p| p.ring.is_some()) {
                self.ring_input(
                    ctx,
                    super::trackring::RingInput {
                        ptr,
                        moved,
                        primary_rel,
                        primary_press,
                        sec_rel,
                        sec_press,
                        now,
                    },
                );
            } else {
                let p = self.pie.as_mut().unwrap();
                let ptr_moved =
                    moved || (ptr.is_some() && p.last_ptr != ptr && p.last_ptr.is_some());
                p.last_ptr = ptr;
                if ptr_moved {
                    p.moved_at = now;
                }
                let open_si = p.sub.map(|s| s.0);
                // on the open submenu's own segment or its panel: the way to the
                // panel starts here
                if in_sub || (over_seg.is_some() && over_seg == open_si) {
                    p.aim_from = ptr;
                }
                // another segment, but the pointer is on its way to the open
                // panel: keep the panel (until it rests there a moment)
                let aiming = |p: &Pie| match (over_seg, open_si, &sub_geo, p.aim_from, ptr) {
                    (Some(k), Some(si), Some((_, _, r, _)), Some(from), Some(q)) if k != si => {
                        in_aim(from, *r, q)
                    }
                    _ => false,
                };
                let rested = now - p.moved_at >= AIM_SETTLE_S;
                let hold = aiming(p) && !rested;
                if hold && !ptr_moved {
                    // check again once it may have rested
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(AIM_SETTLE_S));
                }
                if (ptr_moved || (rested && aiming(p))) && !hold {
                    if let Some(k) = over_seg {
                        p.key = Some(k);
                        // pointing at a submenu segment pops its submenu out
                        if SEGS[k].has_sub() {
                            if p.sub.map(|s| s.0) != Some(k) {
                                p.sub = Some((k, now));
                                p.sub_key = None;
                                p.aim_from = ptr;
                            }
                        } else if SEGS[k].has_ring() {
                            p.open_ring(now);
                            p.aim_from = None;
                        } else if p.sub.is_some() {
                            p.sub = None;
                            p.sub_key = None;
                            p.aim_from = None;
                        }
                    }
                }
                if ptr_moved && in_sub {
                    p.sub_key = over_item;
                }
                let press = p.press;
                // right button held from the opening press: release chooses
                if sec_rel {
                    if let Some(at) = press {
                        let far = ptr.is_some_and(|q| (q - at).length() > DRAG_CHOOSE_PX);
                        self.pie.as_mut().unwrap().press = None;
                        if far {
                            if let Some(k) = over_item {
                                self.pie_choose_item(k, ctx);
                            } else if let Some(k) = over_seg {
                                self.pie_choose(k, ctx);
                            }
                        }
                    }
                } else if primary_rel {
                    if let Some(k) = over_item {
                        self.pie_choose_item(k, ctx);
                    } else if let Some(k) = over_seg {
                        self.pie_choose(k, ctx);
                    } else if on_centre {
                        self.pie_close(ctx);
                    }
                }
                if (primary_press || (sec_press && press.is_none()))
                    && !in_sub
                    && over_seg.is_none()
                    && !on_centre
                {
                    // a click outside cancels
                    self.pie_close(ctx);
                }
            }
        }
        // ---- draw
        let Some(p) = &self.pie else { return };
        let t = th();
        let pl = pal(&t);
        let key = p.key;
        let sub_key = p.sub_key;
        let name = p.name.clone();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, id));
        let mut animating = p.closing.is_some() || !open_done(elapsed, reduce);
        let k0 = disc_progress(elapsed, reduce) * fade;
        // a soft shadow under the ring
        if k0 > 0.0 {
            let sr = R_OUT * scale_of(k0);
            for (dr, a) in [(10.0, 18u8), (5.0, 28), (2.0, 34)] {
                painter.circle_filled(
                    centre + egui::vec2(0.0, 4.0),
                    sr + dr,
                    Color32::from_black_alpha((a as f32 * k0) as u8),
                );
            }
        }
        let ring = ring();
        for (i, seg) in SEGS.iter().enumerate() {
            let k = seg_progress(elapsed, i, reduce) * fade;
            // the empty-area pie: only its ring around the centre
            if k <= 0.001 || empty {
                continue;
            }
            let lit = key == Some(i) && open;
            let hv = if reduce {
                lit as u8 as f32
            } else {
                ctx.animate_value_with_time(id.with(("hv", i)), lit as u8 as f32, HOVER_S)
            };
            if hv > 0.0 && hv < 1.0 {
                animating = true;
            }
            let sc = scale_of(k);
            let dir = egui::vec2(seg_angle(i).cos(), seg_angle(i).sin());
            let c = centre + dir * (5.0 * hv);
            let (ri, ro) = (R_IN * sc, R_OUT * sc);
            let en = seg_enabled(*seg);
            let alpha = k;
            let base_in = crate::theme::mix(pl.ring, pl.hover, hv * 0.85);
            let base_out = crate::theme::mix(pl.ring_out, pl.hover, hv);
            let fi = base_in.gamma_multiply(alpha);
            let fo = base_out.gamma_multiply(alpha);
            // fill: a strip between the inner and the outer arc (cached
            // unit vectors, only scaled here)
            let pts = &ring[i];
            let mut mesh = Mesh::default();
            for v in pts {
                mesh.colored_vertex(c + *v * ri, fi);
                mesh.colored_vertex(c + *v * ro, fo);
            }
            for j in 0..pts.len() as u32 - 1 {
                let a = 2 * j;
                mesh.add_triangle(a, a + 1, a + 3);
                mesh.add_triangle(a, a + 3, a + 2);
            }
            painter.add(Shape::mesh(mesh));
            // outline (anti-aliased)
            let mut outline: Vec<Pos2> = pts.iter().map(|v| c + *v * ro).collect();
            outline.extend(pts.iter().rev().map(|v| c + *v * ri));
            painter.add(Shape::closed_line(
                outline,
                Stroke::new(1.0, pl.edge.gamma_multiply(alpha * 0.9)),
            ));
            // the accent arc on the rim
            if hv > 0.01 {
                let arc: Vec<Pos2> = pts.iter().map(|v| c + *v * (ro + 4.0)).collect();
                let col = if *seg == Seg::Delete {
                    pl.danger
                } else {
                    pl.accent
                };
                painter.add(Shape::line(
                    arc,
                    Stroke::new(3.0, col.gamma_multiply(hv * alpha)),
                ));
            }
            // icon disc + label
            let rm = (ri + ro) / 2.0;
            let ic = c + dir * rm - egui::vec2(0.0, 7.0 * sc);
            let ink = if !en {
                pl.ink2
            } else if *seg == Seg::Delete {
                pl.danger
            } else if hv > 0.5 && !t.light {
                Color32::WHITE
            } else {
                pl.ink
            };
            painter.circle_filled(
                ic,
                13.0 * sc,
                crate::theme::mix(pl.icon_bg, pl.accent, hv * 0.35 * en as u8 as f32)
                    .gamma_multiply(alpha),
            );
            glyphs::paint(
                &painter,
                Rect::from_center_size(ic, Vec2::splat(13.0 * sc)),
                seg.glyph(),
                ink.gamma_multiply(alpha),
            );
            let label = if seg.has_sub() || seg.has_ring() {
                format!("{} ›", seg.label())
            } else {
                seg.label().to_string()
            };
            painter.text(
                ic + egui::vec2(0.0, 22.0 * sc),
                egui::Align2::CENTER_CENTER,
                label,
                FontId::proportional(11.5 * sc.max(0.8)),
                (if en { pl.ink } else { pl.ink2 }).gamma_multiply(alpha),
            );
        }
        // centre disc
        if k0 > 0.0 {
            let sc = scale_of(k0);
            let rr = (R_IN - 4.0) * sc;
            let hc = if reduce {
                (on_centre && open) as u8 as f32
            } else {
                ctx.animate_value_with_time(
                    id.with("hc"),
                    (on_centre && open) as u8 as f32,
                    HOVER_S,
                )
            };
            painter.circle_filled(
                centre,
                rr,
                crate::theme::mix(pl.disc, pl.hover, hc * 0.4).gamma_multiply(k0),
            );
            painter.circle_stroke(centre, rr, Stroke::new(1.0, pl.edge.gamma_multiply(k0)));
            painter.text(
                centre - egui::vec2(0.0, 9.0 * sc),
                egui::Align2::CENTER_CENTER,
                if empty { "ADD" } else { "TRACK" },
                FontId::proportional(10.0 * sc.max(0.8)),
                pl.disc_ink2.gamma_multiply(k0),
            );
            let f = FontId::proportional(12.5 * sc.max(0.8));
            let nm = super::pattern::fit(&painter, &name, &f, 2.0 * rr - 12.0);
            painter.text(
                centre + egui::vec2(0.0, 8.0 * sc),
                egui::Align2::CENTER_CENTER,
                nm,
                f,
                pl.disc_ink.gamma_multiply(k0),
            );
        }
        // the submenu, popping out from its segment
        if let Some((si, st, r, rects)) = sub_geo {
            let ks = if reduce {
                1.0
            } else {
                ease_out(((now - st) / SUB_S) as f32)
            } * fade;
            if ks < 1.0 && p.closing.is_none() {
                animating = true;
            }
            let a = seg_angle(si);
            let from = centre + egui::vec2(a.cos(), a.sin()) * R_OUT;
            let sc = 0.85 + 0.15 * ks;
            let tf = |q: Pos2| from + (q - from) * sc;
            let rr = Rect::from_min_max(tf(r.min), tf(r.max));
            painter.rect_filled(
                rr.expand(3.0).translate(egui::vec2(0.0, 3.0)),
                10,
                Color32::from_black_alpha((40.0 * ks) as u8),
            );
            painter.rect_filled(rr, 9, pl.panel.gamma_multiply(ks));
            painter.rect_stroke(
                rr,
                9,
                Stroke::new(1.0, pl.edge.gamma_multiply(ks)),
                egui::StrokeKind::Inside,
            );
            painter.text(
                tf(r.min + egui::vec2(SUB_PAD + 2.0, SUB_PAD + 9.0)),
                egui::Align2::LEFT_CENTER,
                sub_title(SEGS[si]),
                FontId::proportional(11.0 * sc),
                pl.ink2.gamma_multiply(ks),
            );
            let mut tip: Option<(&'static str, Pos2)> = None;
            for (k, it) in items.iter().enumerate() {
                let q = rects[k].translate(r.min.to_vec2());
                let q = Rect::from_min_max(tf(q.min), tf(q.max));
                let lit = sub_key == Some(k) && open;
                let hv = if reduce {
                    lit as u8 as f32
                } else {
                    ctx.animate_value_with_time(id.with(("sub", si, k)), lit as u8 as f32, HOVER_S)
                };
                if hv > 0.0 && hv < 1.0 {
                    animating = true;
                }
                let en = it.act.is_some();
                if lit && !it.tip.is_empty() {
                    // grids: under the panel (it would cover the next row)
                    let at = if matches!(it.kind, ItemKind::Text(_)) {
                        q.center_bottom()
                    } else {
                        Pos2::new(rr.center().x, rr.bottom())
                    };
                    tip = Some((it.tip, at));
                }
                match &it.kind {
                    ItemKind::Swatch(c) => {
                        painter.rect_filled(
                            q.shrink(2.0 - 2.0 * hv),
                            5,
                            hex(*c).gamma_multiply(ks),
                        );
                        if it.on || hv > 0.01 {
                            painter.rect_stroke(
                                q,
                                6,
                                Stroke::new(
                                    2.0,
                                    (if it.on { pl.ink } else { pl.accent })
                                        .gamma_multiply(ks * if it.on { 1.0 } else { hv }),
                                ),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                    ItemKind::Icon(ic) => {
                        let bg = if it.on {
                            pl.accent.gamma_multiply(0.35)
                        } else {
                            crate::theme::mix(pl.icon_bg, pl.hover, hv)
                        };
                        painter.rect_filled(q, 6, bg.gamma_multiply(ks));
                        if hv > 0.01 {
                            painter.rect_stroke(
                                q,
                                6,
                                Stroke::new(1.5, pl.accent.gamma_multiply(hv * ks)),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if let Some(g) = glyphs::icon_glyph(*ic) {
                            glyphs::paint(
                                &painter,
                                Rect::from_center_size(q.center(), Vec2::splat(16.0 * sc)),
                                g,
                                (if en { pl.ink } else { pl.ink2 }).gamma_multiply(ks),
                            );
                        }
                    }
                    ItemKind::Text(l) => {
                        let pill = it.header;
                        let bg = if pill && it.on {
                            pl.accent
                        } else if pill {
                            pl.icon_bg
                        } else {
                            crate::theme::mix(pl.panel, pl.hover, hv)
                        };
                        if pill || hv > 0.01 {
                            painter.rect_filled(
                                q,
                                if pill { 10 } else { 5 },
                                bg.gamma_multiply(ks),
                            );
                        }
                        let ink = if !en {
                            pl.ink2
                        } else if pill && it.on {
                            Color32::WHITE
                        } else {
                            pl.ink
                        };
                        let txt = if it.on && !pill {
                            format!("{l}  ✓")
                        } else {
                            l.clone()
                        };
                        painter.text(
                            if pill {
                                q.center()
                            } else {
                                q.left_center() + egui::vec2(8.0, 0.0)
                            },
                            if pill {
                                egui::Align2::CENTER_CENTER
                            } else {
                                egui::Align2::LEFT_CENTER
                            },
                            txt,
                            FontId::proportional(12.0 * sc),
                            ink.gamma_multiply(ks),
                        );
                    }
                }
            }
            if let Some((s, at)) = tip {
                tooltip(&painter, &pl, s, at + egui::vec2(0.0, 6.0), screen);
            }
        }
        if self.ring_draw(ctx, &painter, fade, now, screen) {
            animating = true;
        }
        if animating {
            ctx.request_repaint();
        }
        let _ = sub_key;
    }
}

/// a small tooltip box drawn by the pie (its own layer stays on top)
fn tooltip(p: &egui::Painter, pl: &Pal, text: &str, at: Pos2, screen: Rect) {
    let g = p.layout(text.to_string(), FontId::proportional(11.5), pl.ink, 260.0);
    let size = g.size() + egui::vec2(14.0, 10.0);
    let mut r = Rect::from_min_size(Pos2::new(at.x - size.x / 2.0, at.y), size);
    let dx =
        (screen.right() - 4.0 - r.right()).min(0.0) + (screen.left() + 4.0 - r.left()).max(0.0);
    let dy = (screen.bottom() - 4.0 - r.bottom()).min(0.0);
    r = r.translate(egui::vec2(dx, dy));
    p.rect_filled(r, 5, pl.panel);
    p.rect_stroke(r, 5, Stroke::new(1.0, pl.edge), egui::StrokeKind::Inside);
    p.galley(r.min + egui::vec2(7.0, 5.0), g, pl.ink);
}

impl App {
    /// do a pie action on track `tr` (each one undo step)
    /// a new audio track at `at`
    pub(crate) fn track_add_audio(&mut self, at: usize) {
        if self.track_add(at) {
            let at = self.ch;
            let name = format!("Audio {}", at + 1);
            self.with(|s| {
                if let Some(x) = s.song.channel_names.get_mut(at) {
                    *x = name.clone();
                }
                if let Some(t) = s
                    .song
                    .timeline
                    .as_deref_mut()
                    .and_then(|t| t.tracks.get_mut(at))
                {
                    t.kind = crate::timeline::TrackKind::Audio;
                    t.name = name.clone();
                }
            });
            self.status = format!("Added audio track {:02}: drop audio files on it", at + 1);
        }
    }

    /// Drum pattern › Browse… / More kits…: the Browser's pick makes a new
    /// track at `at` with an empty 1-bar pattern at the play marker
    fn pie_drum_browse(&mut self, at: usize, kits: bool) {
        if self.with(|s| s.song.channels) >= MAX_CHANNELS {
            self.status = "All 32 tracks are used (the channel limit)".into();
            return;
        }
        self.open_browser_tab(crate::browser::Tab::Instruments);
        if kits {
            self.br.filter.category = Some("Drum Kit".into());
            self.status = "Pick a Drum Kit: it goes on a new track with an empty 1-bar pattern at the play marker".into();
        } else {
            self.status = "Pick an instrument: it goes on a new track with an empty 1-bar pattern at the play marker".into();
        }
        self.br.new_track_at = Some(at);
        self.br.new_track_pattern = true;
    }

    /// a choice in the empty-area pie: a NEW track at the end
    pub(crate) fn pie_apply_new(&mut self, a: PieAct) {
        let n = self.with(|s| s.song.channels);
        match a {
            PieAct::NewTrackWith(b) => self.track_new_with_instrument(n, b),
            PieAct::DrumTrack(b) => {
                // below the last track (`tr + 1`, clamped: 0 in an empty song)
                self.track_new_pattern_track(n.saturating_sub(1), b, None);
            }
            PieAct::DrumKitTrack(k) => {
                self.track_new_pattern_track(n.saturating_sub(1), Builtin::DrumKit, Some(k));
            }
            PieAct::DrumBrowse | PieAct::MoreKits => self.pie_drum_browse(n, a == PieAct::MoreKits),
            PieAct::AddInstrument => self.track_add_with(n),
            PieAct::AddAudio => self.track_add_audio(n),
            PieAct::AddEmpty(_) => {
                let added = self.track_add(n);
                if added {
                    self.status = format!("Added track {:02}", n + 1);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn pie_apply(&mut self, tr: usize, a: PieAct, ctx: &egui::Context) {
        let n = self.with(|s| s.song.channels);
        if tr >= n {
            return;
        }
        match a {
            PieAct::Rename => {
                let name = self.with(|s| s.song.channel_names.get(tr).cloned().unwrap_or_default());
                self.track_rename = Some((tr, name));
                self.tr_focus = false;
                self.status =
                    "Rename: type a name · Enter or click away keeps it · Esc cancels".into();
            }
            PieAct::Colour(c) => {
                self.snapshot();
                self.with(|s| {
                    if let Some(x) = s
                        .song
                        .timeline
                        .as_deref_mut()
                        .and_then(|t| t.tracks.get_mut(tr))
                    {
                        x.colour = c;
                    }
                });
                self.status = format!(
                    "Track {:02} colour {}",
                    tr + 1,
                    if c.is_some() {
                        "set"
                    } else {
                        "back to automatic"
                    }
                );
            }
            PieAct::Icon(i) => {
                self.snapshot();
                self.with(|s| {
                    if let Some(x) = s
                        .song
                        .timeline
                        .as_deref_mut()
                        .and_then(|t| t.tracks.get_mut(tr))
                    {
                        x.icon = i;
                    }
                });
                self.status = format!("Track {:02} icon: {}", tr + 1, i.label());
            }
            PieAct::AddInstrument => self.track_add_with(tr + 1),
            PieAct::ChangeInstrument => self.track_change_instrument(tr),
            PieAct::EditInstrument => {
                self.tl_select_track(tr, false);
                self.ch = tr;
                self.show_instrument_editor();
            }
            PieAct::NewInstrument(b) => self.track_new_instrument(tr, b),
            PieAct::DrumHere(b) => {
                self.track_set_pattern_instrument(tr, b, None);
            }
            PieAct::DrumKitHere(k) => {
                self.track_set_pattern_instrument(tr, Builtin::DrumKit, Some(k));
            }
            PieAct::MoreKitsHere => {
                self.open_browser_tab(crate::browser::Tab::Instruments);
                self.br.filter.category = Some("Drum Kit".into());
                self.br.track_target = Some((tr, crate::browser::Tab::Instruments));
                self.status = format!(
                    "Pick a Drum Kit in the Browser (double-click or Enter): it plays track {:02}",
                    tr + 1
                );
            }
            PieAct::NewPattern => {
                self.snapshot();
                self.undo_hold += 1;
                let r = self.track_add_empty_pattern(tr);
                self.undo_hold -= 1;
                self.status = match r {
                    Some((_, bar)) => {
                        format!("Track {:02}: an empty 1-bar pattern at bar {bar}", tr + 1)
                    }
                    None => "Can't add a pattern there".into(),
                };
            }
            PieAct::Mute | PieAct::Solo => {
                let on = self.with(|s| {
                    let m = s.song.mixer.channels.get_mut(tr)?;
                    let f = if a == PieAct::Mute {
                        &mut m.mute
                    } else {
                        &mut m.solo
                    };
                    *f = !*f;
                    Some(*f)
                });
                if let Some(on) = on {
                    self.dirty = true;
                    let w = if a == PieAct::Mute { "muted" } else { "soloed" };
                    self.status =
                        format!("Track {:02} {}{w}", tr + 1, if on { "" } else { "not " });
                }
            }
            PieAct::NewTrackWith(b) => self.track_new_with_instrument(tr + 1, b),
            PieAct::DrumTrack(b) => {
                self.track_new_pattern_track(tr, b, None);
            }
            PieAct::DrumKitTrack(k) => {
                self.track_new_pattern_track(tr, Builtin::DrumKit, Some(k));
            }
            PieAct::DrumBrowse | PieAct::MoreKits => {
                self.pie_drum_browse(tr + 1, a == PieAct::MoreKits)
            }
            PieAct::AutoToggle => {
                self.auto_op(super::timeline::autolanes::AutoOp::Toggle(tr));
            }
            PieAct::AutoLane(k) => {
                if let Some((tg, l, on)) = self.track_lanes(tr).into_iter().nth(k as usize) {
                    self.auto_op(super::timeline::autolanes::AutoOp::Enable(tr, tg, !on));
                    self.status = format!(
                        "Track {:02} lane {l} {}",
                        tr + 1,
                        if on { "off (kept, not played)" } else { "on" }
                    );
                }
            }
            PieAct::Automation => {
                let at = self.pie.as_ref().map_or(Pos2::new(200.0, 200.0), |p| {
                    p.centre + egui::vec2(R_OUT + 12.0, -R_OUT)
                });
                self.track_auto_open(tr, at);
            }
            PieAct::AddEmpty(above) => {
                let at = if above { tr } else { tr + 1 };
                if self.track_add(at) {
                    self.status = format!("Added track {:02}", at + 1);
                }
            }
            PieAct::AddAudio => self.track_add_audio(tr + 1),
            PieAct::Duplicate => {
                if n >= MAX_CHANNELS {
                    self.status = "All 32 tracks are used (the channel limit)".into();
                    return;
                }
                self.snapshot();
                let ok = self.with(|s| {
                    let ok = crate::chanops::duplicate_channel(&mut s.song, tr);
                    let Shared { song, engine, .. } = s;
                    engine.ensure_plan(song);
                    ok
                });
                if ok {
                    self.ch = tr + 1;
                    crate::plugin::sync(&self.shared, None);
                    self.status = format!("Duplicated track {:02} (Ctrl+Z undoes)", tr + 1);
                }
            }
            PieAct::Devices => {
                self.ch = tr;
                self.vtab_select(super::viewbar::VTab::Devices);
                self.status = format!("Rack: track {:02}'s instrument and effects", tr + 1);
            }
            PieAct::Routing => {
                self.ch = tr;
                self.insp.focus = super::inspector::Focus::Track;
                if !self.extra.inspector_open {
                    self.toggle_inspector();
                }
                self.status = format!(
                    "Routing: track {:02}'s sends and ducking are in the Inspector › Sends",
                    tr + 1
                );
            }
            PieAct::Move(m) => {
                let to = match m {
                    MoveTo::Up => tr.saturating_sub(1),
                    MoveTo::Down => (tr + 1).min(n - 1),
                    MoveTo::Top => 0,
                    MoveTo::Bottom => n - 1,
                };
                if to != tr {
                    self.track_move(tr, to);
                }
            }
            PieAct::Speed(v) => self.track_speed(tr, v),
            PieAct::Delete => self.track_delete(tr),
        }
        ctx.request_repaint();
    }

    /// delete track `tr` (one undo step); refused with a message when the
    /// song can't lose it (see `chanops::remove_channel`)
    pub(crate) fn track_delete(&mut self, tr: usize) {
        let mut song = self.with(|s| s.song.clone());
        let name = song.channel_names.get(tr).cloned().unwrap_or_default();
        match crate::chanops::remove_channel(&mut song, tr) {
            Ok(()) => {
                self.snapshot();
                self.with(|s| {
                    s.song = song;
                    let Shared { song, engine, .. } = s;
                    engine.ensure_plan(song);
                });
                let n = self.with(|s| s.song.channels);
                self.ch = self.ch.min(n.saturating_sub(1));
                self.tlv.sel = None;
                self.tlv.sels.clear();
                self.dirty = true;
                crate::plugin::sync(&self.shared, None);
                self.status = format!(
                    "Deleted track {:02} {} (Ctrl+Z brings it back)",
                    tr + 1,
                    name.trim()
                );
            }
            Err(e) => self.status = format!("Can't delete track {:02}: {e}", tr + 1),
        }
    }

    /// rename track `tr` (one undo step; empty or unchanged names do nothing)
    pub(crate) fn rename_track(&mut self, tr: usize, name: &str) {
        let n = name.trim().to_string();
        let old = self.with(|s| s.song.channel_names.get(tr).cloned());
        if n.is_empty() || old.as_deref() == Some(n.as_str()) || old.is_none() {
            return;
        }
        self.snapshot();
        self.with(|s| {
            if let Some(x) = s.song.channel_names.get_mut(tr) {
                *x = n.clone();
            }
            if let Some(x) = s
                .song
                .timeline
                .as_deref_mut()
                .and_then(|x| x.tracks.get_mut(tr))
            {
                x.name = n.clone();
            }
        });
        self.status = format!("Track {:02} renamed to {n}", tr + 1);
    }

    /// the inline rename field over a track name at `r` (track headers and
    /// channel headers): Enter or a click away keeps it, Esc cancels
    pub(crate) fn rename_field(&mut self, ui: &mut egui::Ui, r: Rect) {
        let Some((tr, mut text)) = self.track_rename.clone() else {
            return;
        };
        let bg = ui.visuals().extreme_bg_color;
        let ink = if crate::theme::luma(bg) > 0.5 {
            Color32::from_rgb(0x10, 0x12, 0x16)
        } else {
            Color32::from_rgb(0xE8, 0xEC, 0xEF)
        };
        let id = egui::Id::unique(("track_rename", tr));
        let resp = ui.put(
            r,
            egui::TextEdit::singleline(&mut text)
                .id(id)
                .text_color(ink)
                .font(FontId::proportional(13.0))
                .char_limit(32),
        );
        if !self.tr_focus {
            resp.request_focus();
            ui.ctx().memory_mut(|m| m.request_focus(id));
            if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), id) {
                let all = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(text.chars().count()),
                );
                st.cursor.set_char_range(Some(all));
                st.store(ui.ctx(), id);
            }
            self.tr_focus = true;
            self.track_rename = Some((tr, text));
            return;
        }
        if resp.lost_focus() {
            let esc = ui.input(|i| i.key_pressed(Key::Escape));
            self.track_rename = None;
            self.tr_focus = false;
            if esc {
                self.status = "Rename cancelled".into();
            } else {
                self.rename_track(tr, &text);
            }
        } else {
            self.track_rename = Some((tr, text));
        }
    }
}
