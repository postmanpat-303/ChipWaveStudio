//! The track pie's Add › as rings around the pie: Add (level 1), then
//! Instrument › (melodic only), Drum kit › / Drum pattern › (kits and drum
//! instruments only), Sounds › (the instrument's presets) and Automation
//! lane ›. What level 1 holds follows the track (`TrackClass`). A ring of `n` equal slots (`radial::slot_*`) drawn
//! like the pie's segments; going deeper swaps the ring, the likely next
//! choice centred where the pointer already is and ‹ Back beside it.
//! Esc / Backspace / ‹ Back / a click on the dimmed pie step back one ring.
use super::radial::{slot_angle, slot_arc, slot_at, strip_mesh};
use super::trackpie::{
    ease_out, hit, pal, r_in, r_out, seg_angle, Builtin, Hit, Pal, Pie, PieAct, Seg, TrackClass,
    TrackInfo, AIM_SETTLE_S, HOVER_S, MARGIN, SEGS, SUB_S,
};
use super::*;
use egui::{Shape, Vec2};
use std::f32::consts::TAU;

/// inner and outer radius of the rings
pub(crate) fn r1() -> f32 {
    r_out() + 10.0 * super::piesize::k()
}
pub(crate) fn r2() -> f32 {
    r_out() + 112.0 * super::piesize::k()
}
/// the pie's centre keeps this far from the window's edges, so every ring
/// fits on screen (1366x768 too) without moving the pie under the pointer
pub(crate) fn r_fit() -> f32 {
    r2() + 4.0 * super::piesize::k() + MARGIN
}
const SLOT_GAP: f32 = 0.008;
const SLOT_STEPS: usize = 24;
/// resting this long on a › slot during a right-drag opens its ring
pub(crate) const DWELL_S: f64 = 0.35;
/// Drum pattern › Drum Kit ›: the kits (all in `drumpat::KIT_NAMES`)
pub(crate) const RING_KITS: [&str; 6] = [
    "909 Kit",
    "Psy Kit",
    "Techno Kit",
    "Acoustic-ish Kit",
    "Trap Kit",
    "Lo-fi Kit",
];
/// Drum pattern ›: the drum instruments after the kits (`Builtin::is_drum`)
pub(crate) const DRUM_INSTRS: [Builtin; 1] = [Builtin::DrumSynth];

/// Instrument ›: the melodic built-ins (every one that is not a drum)
pub(crate) fn melodic() -> impl Iterator<Item = Builtin> {
    Builtin::ALL.into_iter().filter(|b| !b.is_drum())
}
/// Sounds ›: at most this many of the instrument's presets
pub(crate) const MAX_SOUNDS: usize = 10;
/// Automation lane ›: at most this many of the track's lanes
const MAX_LANES: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Level {
    Add,
    Instrument,
    DrumInstr,
    Sounds,
    Automation,
}

/// what choosing a slot does
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Go {
    Act(PieAct),
    Open(Level),
    Back,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Slot {
    pub(crate) label: String,
    /// None: disabled (choosing it shows the tip on the status line)
    pub(crate) go: Option<Go>,
    pub(crate) tip: String,
    /// a check mark (an automation lane that is on)
    pub(crate) on: bool,
}

/// the open ring and the way back
#[derive(Clone, Debug)]
pub(crate) struct Ring {
    pub(crate) level: Level,
    /// angle of slot 0's centre
    pub(crate) a0: f32,
    pub(crate) key: Option<usize>,
    /// the rings it came from: (level, a0, key)
    pub(crate) back: Vec<(Level, f32, Option<usize>)>,
    /// input time it opened (or swapped) at
    pub(crate) t: f64,
    /// a › slot the pointer rests on during a right-drag, since
    pub(crate) dwell: Option<(usize, f64)>,
}

/// the pie's Add segment
pub(crate) fn add_seg() -> usize {
    SEGS.iter().position(|s| *s == Seg::Add).unwrap_or(0)
}

impl Ring {
    /// level 1 of `n` slots, slot 1 centred on pie segment `seg`'s angle
    pub(crate) fn root_n(now: f64, n: usize, seg: usize) -> Ring {
        Ring {
            level: Level::Add,
            a0: seg_angle(seg) - TAU / n.max(1) as f32,
            key: Some(1),
            back: vec![],
            t: now,
            dwell: None,
        }
    }

    /// go into `level` from slot `k` of the current `n` slots: its slot 1
    /// (the likely next choice) centred on slot `k`, ‹ Back (slot 0) beside
    pub(crate) fn enter(&mut self, level: Level, k: usize, n: usize, n_new: usize, now: f64) {
        self.back.push((self.level, self.a0, Some(k)));
        let pa = slot_angle(self.a0, n, k);
        self.a0 = pa - TAU / n_new.max(1) as f32;
        self.level = level;
        self.key = Some(1);
        self.t = now;
        self.dwell = None;
    }

    /// one ring back (false: this is level 1)
    pub(crate) fn pop(&mut self, now: f64) -> bool {
        let Some((l, a, k)) = self.back.pop() else {
            return false;
        };
        self.level = l;
        self.a0 = a;
        self.key = k;
        self.t = now;
        self.dwell = None;
        true
    }
}

impl Pie {
    /// open level 1 of `n` slots (hovering / choosing Add)
    pub(crate) fn open_ring(&mut self, now: f64, n: usize) {
        if self.ring.is_none() {
            self.ring = Some(Ring::root_n(now, n, add_seg()));
        }
        self.key = Some(add_seg());
        self.sub = None;
        self.sub_key = None;
    }
}

const NEED_TL: &str = "needs timeline mode (Timeline › Switch to timeline mode)";
const FULL: &str = "all 32 tracks are used (the channel limit)";
const INSTR_FULL: &str = "all 255 instrument slots are used";
const NEW_PATTERN: &str = "a new track with it and an empty 1-bar pattern at the play marker";

fn slot(label: &str, go: Option<Go>, tip: impl Into<String>) -> Slot {
    Slot {
        label: label.into(),
        go,
        tip: tip.into(),
        on: false,
    }
}

fn back() -> Slot {
    slot("Back", Some(Go::Back), "back one ring (Esc, Backspace)")
}

/// the track pie's rings: everything acts on THIS track (`i.index`) and
/// none makes a new one (Duplicate copies it on purpose). They follow what
/// the track plays (`TrackClass`): an empty track offers Instrument › and
/// Drum kit › first; a drum track its kits, patterns and Dice kit; a
/// melodic track melodic swaps, its Sounds ›, patterns, Chords…, Melody… and Bass….
/// None: the level is the same in both pies (Automation)
fn this_track_slots(level: Level, i: &TrackInfo) -> Option<Vec<Slot>> {
    // an instrument and its pattern on this track
    let pattern_ok = i.timeline && !i.instr_full;
    let pattern_why = if !i.timeline { NEED_TL } else { INSTR_FULL };
    let tl = |a: PieAct| i.timeline.then_some(Go::Act(a));
    let tl_tip = |t: &'static str| if i.timeline { t } else { NEED_TL };
    let instrument =
        |label: &str, tip: &'static str| slot(label, Some(Go::Open(Level::Instrument)), tip);
    let drums = |label: &str, tip: &'static str| {
        slot(
            label,
            pattern_ok.then_some(Go::Open(Level::DrumInstr)),
            if pattern_ok { tip } else { pattern_why },
        )
    };
    let automation = || {
        slot(
            "Automation lane",
            i.timeline.then_some(Go::Open(Level::Automation)),
            tl_tip("automate a parameter on this track, or show / hide its lanes. Next: pick"),
        )
    };
    let tail = || {
        let paste = (i.paste && i.timeline).then(|| {
            slot(
                "Paste at playhead",
                Some(Go::Act(PieAct::PasteHere)),
                "the copied clips onto this track at the play marker (on the snap grid), as independent copies",
            )
        });
        paste.into_iter().chain([
            Slot {
                on: i.mute,
                ..slot(
                    "Mute",
                    Some(Go::Act(PieAct::Mute)),
                    if i.mute {
                        "unmute this track"
                    } else {
                        "mute this track (F9)"
                    },
                )
            },
            Slot {
                on: i.solo,
                ..slot(
                    "Solo",
                    Some(Go::Act(PieAct::Solo)),
                    if i.solo {
                        "unsolo this track"
                    } else {
                        "solo this track (F10)"
                    },
                )
            },
            slot(
                "Duplicate",
                (!i.full).then_some(Go::Act(PieAct::Duplicate)),
                if i.full {
                    FULL
                } else {
                    "a copy of this track below it: notes, clips, mixer strip, lanes"
                },
            ),
            slot(
                "Open in Rack",
                Some(Go::Act(PieAct::Devices)),
                "this track's instrument and effects in the Rack",
            ),
        ])
    };
    Some(match level {
        Level::Add => {
            let mut v = match i.class {
                TrackClass::Empty => vec![
                    instrument(
                        "Instrument",
                        "a melodic instrument for THIS track (WaveForge, FM-4, Strings … or Browse). Next: pick it",
                    ),
                    drums(
                        "Drum kit",
                        "THIS track plays a drum kit and its pattern opens (an empty pattern gets a starter groove). Next: pick a kit",
                    ),
                    automation(),
                    slot(
                        "Browse…",
                        Some(Go::Act(PieAct::ChangeInstrument)),
                        "the Browser's Instruments (your Favourites first): the one you pick plays this track",
                    ),
                ],
                TrackClass::Drum => vec![
                    drums(
                        "Swap kit",
                        "another kit or drum instrument on THIS track (the notes stay; an empty pattern gets a starter groove). Next: pick it",
                    ),
                    slot(
                        "Drum pattern",
                        tl(PieAct::NewPattern),
                        tl_tip("an empty 1-bar drum pattern on this track at the play marker, in the step sequencer"),
                    ),
                    slot(
                        "Dice kit",
                        (i.kit && i.timeline).then_some(Go::Act(PieAct::DiceKit)),
                        if !i.timeline {
                            NEED_TL
                        } else if i.kit {
                            "a random kit: an empty pattern gets its starter groove, else every pad's unlocked knobs re-roll (the pattern stays). Undo takes it back"
                        } else {
                            "Dice kit is for a Drum Kit (this track plays another drum instrument)"
                        },
                    ),
                    automation(),
                    slot(
                        "More kits…",
                        Some(Go::Act(PieAct::MoreKitsHere)),
                        "the Browser, on Drum Kits: the one you pick plays this track",
                    ),
                ],
                TrackClass::Melodic => vec![
                    instrument(
                        "Instrument",
                        "another melodic instrument for THIS track (WaveForge, FM-4, Strings … or Browse). Next: pick it",
                    ),
                    slot(
                        "Sounds",
                        (!i.sounds.is_empty()).then_some(Go::Open(Level::Sounds)),
                        if i.sounds.is_empty() {
                            "this instrument has no built-in presets: Browse… for another sound"
                        } else {
                            "a preset of this instrument (keeps its name and effects). Next: pick it"
                        },
                    ),
                    slot(
                        "New pattern",
                        tl(PieAct::NewPattern),
                        tl_tip("an empty 1-bar pattern on this track at the play marker, in the piano roll"),
                    ),
                    slot(
                        "Chords…",
                        tl(PieAct::Chords),
                        tl_tip("the chord generator on this track's first pattern (a new one if it has none)"),
                    ),
                    slot(
                        "Melody…",
                        tl(PieAct::Melody),
                        tl_tip("the melody generator over this track's first pattern's chords (writes a new Melody track or this one)"),
                    ),
                    slot(
                        "Bass…",
                        tl(PieAct::Bass),
                        tl_tip("the bass generator over this track's first pattern's chords (writes a new Bass track or this one)"),
                    ),
                    automation(),
                    slot(
                        "Browse…",
                        Some(Go::Act(PieAct::ChangeInstrument)),
                        "the Browser's Instruments (your Favourites first): the one you pick plays this track",
                    ),
                ],
            };
            v.extend(tail());
            v
        }
        Level::Instrument => {
            let mut v = vec![back()];
            v.extend(melodic().map(|b| {
                slot(
                    b.label(),
                    (!i.instr_full).then_some(Go::Act(PieAct::NewInstrument(b))),
                    if i.instr_full {
                        INSTR_FULL.to_string()
                    } else {
                        format!("{}. Click: it plays this track", b.tip())
                    },
                )
            }));
            v.push(slot(
                "Browse…",
                Some(Go::Act(PieAct::ChangeInstrument)),
                "the Browser's Instruments (your Favourites first): the one you pick plays this track",
            ));
            v
        }
        Level::DrumInstr => {
            let mut v = vec![back()];
            v.extend(RING_KITS.iter().map(|&k| {
                slot(
                    k,
                    pattern_ok.then_some(Go::Act(PieAct::DrumKitHere(k))),
                    if pattern_ok {
                        format!(
                            "{k}: {}. Click: this track plays it; an empty pattern gets its starter groove",
                            crate::drumpat::kit_desc(k)
                        )
                    } else {
                        pattern_why.to_string()
                    },
                )
            }));
            v.extend(DRUM_INSTRS.iter().map(|&b| {
                slot(
                    b.label(),
                    pattern_ok.then_some(Go::Act(PieAct::DrumHere(b))),
                    if pattern_ok {
                        format!("{}. Click: this track plays it, its pattern opens", b.tip())
                    } else {
                        pattern_why.to_string()
                    },
                )
            }));
            v.push(slot(
                "More kits…",
                Some(Go::Act(PieAct::MoreKitsHere)),
                "the Browser, on Drum Kits: the one you pick plays this track",
            ));
            v
        }
        Level::Sounds => {
            let mut v = vec![back()];
            v.extend(i.sounds.iter().take(MAX_SOUNDS).enumerate().map(|(k, n)| Slot {
                on: i.sound.as_deref() == Some(n.as_str()),
                ..slot(
                    n,
                    Some(Go::Act(PieAct::Sound(k as u8))),
                    format!("{n}: this instrument's sound becomes it (keeps its name and effects). Undo takes it back"),
                )
            }));
            v
        }
        Level::Automation => return None,
    })
}

/// the slots of a ring for the track `i`
pub(crate) fn ring_slots(level: Level, i: &TrackInfo) -> Vec<Slot> {
    if !i.new {
        if let Some(v) = this_track_slots(level, i) {
            return v;
        }
    }
    let new_track = |go: Go| (!i.full).then_some(go);
    // a new track with a new instrument and a pattern
    let pattern_ok = i.timeline && !i.full && !i.instr_full;
    let pattern_why = if !i.timeline {
        NEED_TL
    } else if i.full {
        FULL
    } else {
        INSTR_FULL
    };
    match level {
        Level::Add => vec![
            slot(
                "Instrument track",
                (!i.full && !i.instr_full).then_some(Go::Open(Level::Instrument)),
                if i.full {
                    FULL
                } else if i.instr_full {
                    INSTR_FULL
                } else {
                    "a new track playing a new melodic instrument (WaveForge, FM-4, Strings …). Next: pick it"
                },
            ),
            slot(
                "Drum pattern",
                pattern_ok.then_some(Go::Open(Level::DrumInstr)),
                if pattern_ok {
                    "Drum pattern: a new drum track with a 1-bar pattern at the play marker. Next: pick a kit"
                } else {
                    pattern_why
                },
            ),
            slot(
                "Audio track",
                if i.timeline {
                    new_track(Go::Act(PieAct::AddAudio))
                } else {
                    None
                },
                if i.full {
                    FULL
                } else if i.timeline {
                    "a new track for audio clips"
                } else {
                    NEED_TL
                },
            ),
            slot(
                "Empty track",
                new_track(Go::Act(PieAct::AddEmpty(false))),
                if i.full {
                    FULL
                } else {
                    "a new empty track (no instrument yet)"
                },
            ),
            slot(
                "Browse…",
                new_track(Go::Act(PieAct::AddInstrument)),
                if i.full {
                    FULL
                } else {
                    "pick any instrument in the Browser (your presets, samples, plugins) for a new track"
                },
            ),
            slot(
                "FX track",
                None,
                "FX (bus) tracks: coming soon (docs/ROADMAP.md)",
            ),
        ],
        Level::Instrument => {
            let mut v = vec![back()];
            v.extend(melodic().map(|b| {
                slot(
                    b.label(),
                    (!i.instr_full && !i.full).then_some(Go::Act(PieAct::NewTrackWith(b))),
                    if i.instr_full {
                        INSTR_FULL.to_string()
                    } else if i.full {
                        FULL.to_string()
                    } else {
                        format!("{}. Click: a new track playing it", b.tip())
                    },
                )
            }));
            v.push(slot(
                "Browse…",
                new_track(Go::Act(PieAct::AddInstrument)),
                "the Browser's Instruments: the one you pick makes a new track",
            ));
            v
        }
        Level::DrumInstr => {
            let mut v = vec![back()];
            v.extend(RING_KITS.iter().map(|&k| {
                slot(
                    k,
                    pattern_ok.then_some(Go::Act(PieAct::DrumKitTrack(k))),
                    if pattern_ok {
                        format!(
                            "{k}: {}. Click: a new track with this Drum Kit and an empty 1-bar pattern at the play marker",
                            crate::drumpat::kit_desc(k)
                        )
                    } else {
                        pattern_why.to_string()
                    },
                )
            }));
            v.extend(DRUM_INSTRS.iter().map(|&b| {
                slot(
                    b.label(),
                    pattern_ok.then_some(Go::Act(PieAct::DrumTrack(b))),
                    if pattern_ok {
                        format!("{}. Click: {NEW_PATTERN}", b.tip())
                    } else {
                        pattern_why.to_string()
                    },
                )
            }));
            v.push(slot(
                "More kits…",
                pattern_ok.then_some(Go::Act(PieAct::MoreKits)),
                if pattern_ok {
                    "the Browser, on Drum Kits: the one you pick makes a new track with an empty 1-bar pattern"
                } else {
                    pattern_why
                },
            ));
            v
        }
        // only the track pie has a track with presets
        Level::Sounds => vec![back()],
        Level::Automation => {
            let any = !i.lanes.is_empty();
            let mut v = vec![
                back(),
                slot(
                    "New lane…",
                    i.timeline.then_some(Go::Act(PieAct::Automation)),
                    if i.timeline {
                        "pick a parameter to automate on this track (the lane menu)"
                    } else {
                        NEED_TL
                    },
                ),
                slot(
                    if i.lanes_shown {
                        "Hide lanes"
                    } else {
                        "Show lanes"
                    },
                    any.then_some(Go::Act(PieAct::AutoToggle)),
                    if !any {
                        "no lanes on this track yet: New lane… first"
                    } else if i.lanes_shown {
                        "fold this track's lanes away"
                    } else {
                        "this track's lanes, under it"
                    },
                ),
            ];
            v.extend(
                i.lanes
                    .iter()
                    .take(MAX_LANES)
                    .enumerate()
                    .map(|(k, (l, on))| Slot {
                        label: l.clone(),
                        go: Some(Go::Act(PieAct::AutoLane(k as u8))),
                        tip: format!(
                            "{l}: click switches it {}",
                            if *on { "off (kept, not played)" } else { "on" }
                        ),
                        on: *on,
                    }),
            );
            v
        }
    }
}

/// the centre disc's two lines on a ring
fn titles(level: Level, name: &str, new: bool) -> (&'static str, String) {
    match level {
        Level::Add if new => ("ADD", "New track".into()),
        Level::Add => ("ADD", name.to_string()),
        Level::Instrument if new => ("INSTRUMENT", "New track".into()),
        Level::Instrument => ("INSTRUMENT", "For this track".into()),
        Level::DrumInstr if new => ("DRUM PATTERN", "New track".into()),
        Level::DrumInstr => ("DRUM KIT", name.to_string()),
        Level::Sounds => ("SOUNDS", name.to_string()),
        Level::Automation => ("AUTOMATION", name.to_string()),
    }
}

/// what is under `p` with a ring of `n` slots (slot 0 at `a0`) open
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RingHit {
    Centre,
    /// a segment of the pie inside the ring
    Inner(usize),
    Slot(usize),
    Outside,
}

pub(crate) fn ring_hit(c: Pos2, a0: f32, n: usize, p: Pos2) -> RingHit {
    let d = p - c;
    let r = d.length();
    if r <= r_out() + 6.0 {
        return match hit(c, p) {
            Hit::Seg(i) => RingHit::Inner(i),
            _ => RingHit::Centre,
        };
    }
    if r <= r2() + 8.0 {
        return RingHit::Slot(slot_at(a0, n, d.y.atan2(d.x)));
    }
    RingHit::Outside
}

/// the pointer and buttons this frame (`track_pie`)
pub(crate) struct RingInput {
    pub(crate) ptr: Option<Pos2>,
    pub(crate) moved: bool,
    pub(crate) primary_rel: bool,
    pub(crate) primary_press: bool,
    pub(crate) sec_rel: bool,
    pub(crate) sec_press: bool,
    pub(crate) now: f64,
}

impl App {
    /// the open ring: (track, level, a0, its slots)
    pub(crate) fn ring_now(&self) -> Option<(usize, Level, f32, Vec<Slot>)> {
        let p = self.pie.as_ref()?;
        let r = p.ring.as_ref()?;
        let slots = ring_slots(r.level, &self.ring_info_for(p.track, p.empty));
        Some((p.track, r.level, r.a0, slots))
    }

    /// choose slot `k` of the open ring
    pub(crate) fn ring_choose(&mut self, k: usize, ctx: &egui::Context) {
        let Some((tr, _, _, slots)) = self.ring_now() else {
            return;
        };
        let Some(s) = slots.get(k) else { return };
        let now = ctx.input(|i| i.time);
        match s.go {
            None => self.status = s.tip.clone(),
            Some(Go::Back) => self.ring_back(ctx),
            Some(Go::Open(l)) => {
                let empty = self.pie.as_ref().is_some_and(|p| p.empty);
                let n_new = ring_slots(l, &self.ring_info_for(tr, empty)).len();
                if let Some(r) = self.pie.as_mut().and_then(|p| p.ring.as_mut()) {
                    r.enter(l, k, slots.len(), n_new, now);
                }
            }
            Some(Go::Act(a)) => {
                let empty = self.pie.as_ref().is_some_and(|p| p.empty);
                self.pie_close(ctx);
                if empty {
                    self.pie_apply_new(a);
                } else {
                    self.pie_apply(tr, a, ctx);
                }
            }
        }
        ctx.request_repaint();
    }

    /// one ring back; from level 1 back to the plain pie (Add lit)
    pub(crate) fn ring_back(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if let Some(p) = &mut self.pie {
            let mut close = false;
            if let Some(r) = &mut p.ring {
                if !r.pop(now) {
                    // the empty-area pie is only its ring: back closes it
                    close = p.empty;
                    p.ring = None;
                    p.key = Some(add_seg());
                }
            }
            if close {
                self.pie_close(ctx);
            }
        }
        ctx.request_repaint();
    }

    /// a key while a ring is open: arrows / Tab walk it, Enter / Space
    /// choose, Esc / Backspace step back one ring
    pub(crate) fn ring_key(&mut self, key: Key, ctx: &egui::Context) {
        let Some((_, _, _, slots)) = self.ring_now() else {
            return;
        };
        let n = slots.len() as isize;
        let Some(r) = self.pie.as_mut().and_then(|p| p.ring.as_mut()) else {
            return;
        };
        let cur = r.key;
        let mv = |d: isize| -> usize {
            match cur {
                None => 1.min(n as usize - 1),
                Some(c) => (c as isize + d).rem_euclid(n) as usize,
            }
        };
        match key {
            Key::ArrowRight | Key::ArrowDown | Key::Tab => r.key = Some(mv(1)),
            Key::ArrowLeft | Key::ArrowUp => r.key = Some(mv(-1)),
            Key::Enter | Key::Space => {
                if let Some(k) = cur {
                    self.ring_choose(k, ctx);
                }
            }
            Key::Escape | Key::Backspace => self.ring_back(ctx),
            _ => {}
        }
    }

    /// pointer input while a ring is open (instead of the pie's own)
    pub(crate) fn ring_input(&mut self, ctx: &egui::Context, inp: RingInput) {
        let Some((_, level, a0, slots)) = self.ring_now() else {
            return;
        };
        let n = slots.len();
        let now = inp.now;
        let add = add_seg();
        let p = self.pie.as_mut().unwrap();
        // the empty-area pie has no segments: its centre steps back / closes
        let deep = level != Level::Add || p.empty;
        let c = p.centre;
        let h = inp.ptr.map(|q| ring_hit(c, a0, n, q));
        let ptr_moved =
            inp.moved || (inp.ptr.is_some() && p.last_ptr != inp.ptr && p.last_ptr.is_some());
        p.last_ptr = inp.ptr;
        if ptr_moved {
            p.moved_at = now;
        }
        let rested = now - p.moved_at >= AIM_SETTLE_S;
        let held = p.press.is_some();
        let mut leave_for: Option<usize> = None;
        let r = p.ring.as_mut().unwrap();
        match h {
            Some(RingHit::Slot(k)) => {
                if ptr_moved {
                    r.key = Some(k);
                }
                if held && matches!(slots[k].go, Some(Go::Open(_))) {
                    if r.dwell.map(|d| d.0) != Some(k) {
                        r.dwell = Some((k, now));
                    }
                } else {
                    r.dwell = None;
                }
            }
            Some(RingHit::Inner(i)) if !deep => {
                r.dwell = None;
                if i == add {
                    p.key = Some(add);
                } else {
                    // another segment: the ring stays while the pointer
                    // only grazes it on the way out (near the rim, moving)
                    let d = inp.ptr.map_or(0.0, |q| (q - c).length());
                    if d < r_out() * 0.75 || rested {
                        leave_for = Some(i);
                    } else {
                        ctx.request_repaint_after(std::time::Duration::from_secs_f64(AIM_SETTLE_S));
                    }
                }
            }
            _ => r.dwell = None,
        }
        if let Some(i) = leave_for {
            p.ring = None;
            p.key = Some(i);
            if SEGS[i].has_sub() {
                p.sub = Some((i, now));
                p.sub_key = None;
            }
            return;
        }
        // resting on a › slot during a right-drag opens its ring
        if let Some((k, t0)) = p.ring.as_ref().and_then(|r| r.dwell) {
            if now - t0 >= DWELL_S {
                self.ring_choose(k, ctx);
                return;
            }
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(DWELL_S - (now - t0)));
        }
        let p = self.pie.as_mut().unwrap();
        let press = p.press;
        if inp.sec_rel {
            if let Some(at) = press {
                let far = inp
                    .ptr
                    .is_some_and(|q| (q - at).length() > super::trackpie::DRAG_CHOOSE_PX);
                p.press = None;
                if far {
                    match h {
                        Some(RingHit::Slot(k)) => self.ring_choose(k, ctx),
                        Some(RingHit::Inner(i)) if !deep => self.pie_choose(i, ctx),
                        _ => {}
                    }
                }
            }
        } else if inp.primary_rel {
            match h {
                Some(RingHit::Slot(k)) => self.ring_choose(k, ctx),
                Some(RingHit::Inner(i)) if !deep => self.pie_choose(i, ctx),
                Some(RingHit::Centre) if !deep => self.pie_close(ctx),
                // the dimmed pie: one ring back
                Some(RingHit::Centre | RingHit::Inner(_)) => self.ring_back(ctx),
                _ => {}
            }
        }
        if (inp.primary_press || (inp.sec_press && press.is_none())) && h == Some(RingHit::Outside)
        {
            self.pie_close(ctx);
        }
    }

    /// draw the open ring over the pie (true: still animating)
    pub(crate) fn ring_draw(
        &self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        fade: f32,
        now: f64,
        screen: Rect,
    ) -> bool {
        let Some((_, level, a0, slots)) = self.ring_now() else {
            return false;
        };
        let p = self.pie.as_ref().unwrap();
        let r = p.ring.as_ref().unwrap();
        let reduce = self.extra.reduce_motion;
        let open = p.closing.is_none();
        let t = th();
        let pl = pal(&t);
        let c = p.centre;
        let n = slots.len();
        let ks = if reduce {
            1.0
        } else {
            ease_out(((now - r.t) / SUB_S) as f32)
        } * fade;
        let mut animating = ks < 1.0 && open;
        let deep = level != Level::Add;
        let id = egui::Id::unique("track_ring");
        if deep {
            painter.circle_filled(
                c,
                r_out() + 6.0,
                Color32::from_black_alpha((110.0 * ks) as u8),
            );
        }
        let sc = 0.92 + 0.08 * ks;
        let (r_lo, r_hi) = (r1(), r2());
        let (r1, r2) = (r_lo + (r_hi - r_lo) * (1.0 - sc), r_hi * sc);
        for (dr, a) in [(10.0, 18u8), (5.0, 28), (2.0, 34)] {
            painter.circle_stroke(
                c + egui::vec2(0.0, 4.0),
                r2 + dr / 2.0,
                Stroke::new(dr, Color32::from_black_alpha((a as f32 * ks) as u8)),
            );
        }
        let mut tip: Option<(String, Vec2)> = None;
        for (k, s) in slots.iter().enumerate() {
            let lit = r.key == Some(k) && open;
            let hv = if reduce {
                lit as u8 as f32
            } else {
                ctx.animate_value_with_time(id.with((level as u8, k)), lit as u8 as f32, HOVER_S)
            };
            if hv > 0.0 && hv < 1.0 {
                animating = true;
            }
            let a = slot_angle(a0, n, k);
            let dir = egui::vec2(a.cos(), a.sin());
            let cc = c + dir * (4.0 * hv);
            let en = s.go.is_some();
            let is_back = s.go == Some(Go::Back);
            let pts = slot_arc(a0, n, k, SLOT_GAP, SLOT_STEPS);
            let fi = crate::theme::mix(pl.ring, pl.hover, hv * 0.85).gamma_multiply(ks);
            let fo = crate::theme::mix(if is_back { pl.ring } else { pl.ring_out }, pl.hover, hv)
                .gamma_multiply(ks);
            painter.add(Shape::mesh(strip_mesh(cc, &pts, r1, r2, fi, fo)));
            let mut outline: Vec<Pos2> = pts.iter().map(|v| cc + *v * r2).collect();
            outline.extend(pts.iter().rev().map(|v| cc + *v * r1));
            painter.add(Shape::closed_line(
                outline,
                Stroke::new(1.0, pl.edge.gamma_multiply(ks * 0.9)),
            ));
            if hv > 0.01 && en {
                let arc: Vec<Pos2> = pts.iter().map(|v| cc + *v * (r2 + 4.0)).collect();
                painter.add(Shape::line(
                    arc,
                    Stroke::new(3.0, pl.accent.gamma_multiply(hv * ks)),
                ));
            }
            // the label, wrapped to the slot's width
            let label = if is_back {
                "‹ Back".to_string()
            } else if matches!(s.go, Some(Go::Open(_))) {
                format!("{} ›", s.label)
            } else if s.on {
                format!("{}  ✓", s.label)
            } else {
                s.label.clone()
            };
            let rm = (r1 + r2) / 2.0;
            let chord = 2.0 * rm * (TAU / n as f32 / 2.0).sin();
            let ink = if !en || (is_back && hv < 0.5) {
                pl.ink2
            } else if hv > 0.5 && !t.light {
                Color32::WHITE
            } else {
                pl.ink
            };
            let mut job = egui::text::LayoutJob::simple(
                label,
                crate::theme::prop(super::piesize::k().sqrt() * if is_back { 12.0 } else { 13.0 }),
                ink.gamma_multiply(ks),
                (chord * 0.86).min(118.0 * super::piesize::k()),
            );
            job.halign = egui::Align::Center;
            job.wrap.max_rows = 3;
            let g = painter.layout_job(job);
            let at = cc + dir * rm - g.rect.center().to_vec2();
            painter.galley(at, g, ink);
            if lit && !s.tip.is_empty() {
                tip = Some((s.tip.clone(), dir));
            }
        }
        // the centre disc names the ring
        let rr = r_in() - 4.0;
        painter.circle_filled(c, rr, pl.disc.gamma_multiply(fade));
        painter.circle_stroke(c, rr, Stroke::new(1.0, pl.edge.gamma_multiply(fade)));
        let (top, sub) = titles(level, &p.name, p.empty);
        let pk = super::piesize::k();
        painter.text(
            c - egui::vec2(0.0, 9.0 * pk),
            egui::Align2::CENTER_CENTER,
            top,
            crate::theme::prop(crate::theme::ts::TINY * pk.sqrt()),
            pl.disc_ink2.gamma_multiply(fade),
        );
        let f = crate::theme::prop(crate::theme::ts::COMPACT * pk.sqrt());
        let nm = super::pattern::fit(painter, &sub, &f, 2.0 * rr - 12.0);
        painter.text(
            c + egui::vec2(0.0, 8.0 * pk),
            egui::Align2::CENTER_CENTER,
            nm,
            f,
            pl.disc_ink.gamma_multiply(fade),
        );
        if let Some((s, dir)) = tip {
            if ks > 0.9 {
                ring_tip(painter, &pl, &s, c + dir * (r_hi + 10.0), dir, screen);
            }
        }
        animating
    }
}

/// a hover hint outside the ring, away from it along `dir`
fn ring_tip(p: &egui::Painter, pl: &Pal, text: &str, at: Pos2, dir: Vec2, screen: Rect) {
    let g = p.layout(
        text.to_string(),
        crate::theme::prop(crate::theme::ts::LABEL),
        pl.ink,
        260.0,
    );
    let size = g.size() + egui::vec2(14.0, 10.0);
    let x = if dir.x > 0.35 {
        at.x
    } else if dir.x < -0.35 {
        at.x - size.x
    } else {
        at.x - size.x / 2.0
    };
    let y = if dir.y > 0.35 {
        at.y
    } else if dir.y < -0.35 {
        at.y - size.y
    } else {
        at.y - size.y / 2.0
    };
    let mut r = Rect::from_min_size(Pos2::new(x, y), size);
    let dx =
        (screen.right() - 4.0 - r.right()).min(0.0) + (screen.left() + 4.0 - r.left()).max(0.0);
    let dy =
        (screen.bottom() - 4.0 - r.bottom()).min(0.0) + (screen.top() + 4.0 - r.top()).max(0.0);
    r = r.translate(egui::vec2(dx, dy));
    p.rect_filled(r, 5, pl.panel);
    p.rect_stroke(r, 5, Stroke::new(1.0, pl.edge), egui::StrokeKind::Inside);
    p.galley(r.min + egui::vec2(7.0, 5.0), g, pl.ink);
}
