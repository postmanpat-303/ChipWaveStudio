//! The eight-place menus (Del (postmanpat), 6 Nov 2026). One ring, the same
//! place for the same kind of action, and a › wedge replaces the ring
//! instead of growing a second circle. Back is always place 7 (10:30).
//! Remove… does nothing until the next ring names the delete.
//!
//! Places, clockwise from 12: Make, Repeat, Edit, Sound, Time, Look, Hear,
//! Remove. An empty place is left empty so the others do not slide.
//!
//! Instrument inserts, send returns, the master limiter and pattern-command
//! row speed stay separate. The list menus, the Rack ⋮ button, + lane and
//! the keyboard shortcuts are unchanged.
use super::objpie::PieFor;
use super::radial::RadialItem;
use super::trackpie::{Builtin, MoveTo, PieAct};
use super::*;
use crate::automation::Target;
use crate::fx::FxKind;
use crate::timeline::TrackIcon;

fn wedge(slot: u8, label: &str, key: &str) -> RadialItem {
    RadialItem::new(label, None, vec![]).at(slot).key(key)
}

fn deeper(slot: u8, label: &str, key: &str) -> RadialItem {
    wedge(slot, label, key).goes()
}

fn back() -> RadialItem {
    wedge(7, "Back", "back")
}

fn place(items: Vec<RadialItem>) -> Vec<RadialItem> {
    items.into_iter().filter(|i| i.slot.is_some()).collect()
}

impl App {
    /// the first ring for whatever was clicked
    pub(crate) fn compass_root(&self, on: PieFor) -> Option<(String, String, Vec<RadialItem>)> {
        match on {
            PieFor::Track(tr) => Some(self.track_root(tr)),
            PieFor::NewTrack => Some(self.new_track_root()),
            PieFor::Master => Some((
                "SONG".into(),
                "Master".into(),
                place(vec![
                    wedge(0, "Master…", "master"),
                    wedge(2, "Export…", "export"),
                    wedge(6, "Save", "save"),
                ]),
            )),
            PieFor::Clip(id) => self.clip_root(id),
            PieFor::Lane { track, at } => Some(self.lane_root(track, at)),
            PieFor::Launch(id) => self.launch_root(id),
            PieFor::Drum { pad, .. } => Some((
                "PATTERN".into(),
                format!("pad {}", pad + 1),
                place(vec![
                    wedge(0, "Generate", "drum-gen"),
                    wedge(1, "Copy pattern", "drum-copy"),
                    deeper(2, "Fill", "fill"),
                    wedge(3, "Variation", "drum-var"),
                    wedge(6, "Play", "drum-play"),
                    deeper(7, "Remove…", "remove"),
                ]),
            )),
            PieFor::RackInst(idx) => self.rack_inst_root(idx),
            PieFor::RackFx(idx, i) => self.rack_fx_root(idx, i),
            PieFor::Auto {
                track, lane, point, ..
            } => Some(self.auto_root(track, lane, point)),
        }
    }

    /// the ring a › wedge opens. None: the wedge is not a page
    pub(crate) fn compass_page(
        &self,
        on: PieFor,
        key: &str,
    ) -> Option<(String, String, Vec<RadialItem>)> {
        let track = |on: PieFor| match on {
            PieFor::Track(tr) | PieFor::Lane { track: tr, .. } | PieFor::Auto { track: tr, .. } => {
                Some(tr)
            }
            _ => None,
        };
        match key {
            "instruments" => Some(self.instrument_page(matches!(on, PieFor::NewTrack), track(on))),
            "drums" => Some(self.drum_page(matches!(on, PieFor::NewTrack), track(on))),
            "sounds" => track(on).map(|tr| self.sounds_page(tr)),
            "presets" => match on {
                PieFor::RackInst(idx) => Some(self.preset_page_idx(idx)),
                _ => track(on).map(|tr| self.sounds_page(tr)),
            },
            "lanes" | "add-lane" => track(on).map(|tr| self.lanes_page(tr, None)),
            "fx-lanes" => match on {
                PieFor::RackFx(idx, i) => Some(self.fx_lane_page(idx, i)),
                _ => None,
            },
            "look" => match on {
                PieFor::Clip(_) | PieFor::Launch(_) => Some(self.clip_look_page()),
                _ => track(on).map(|tr| self.look_page(tr)),
            },
            "move" => track(on).map(|tr| self.move_page(tr)),
            "speed" => track(on).map(|tr| self.speed_page(tr)),
            "colour" => Some(self.colour_page(false)),
            "colour2" => Some(self.colour_page(true)),
            "icons" => Some(self.icon_page(0)),
            "icons2" => Some(self.icon_page(1)),
            "icons3" => Some(self.icon_page(2)),
            "repeat" => Some(self.repeat_page()),
            "repeat-track" => track(on).map(|tr| self.repeat_track_page(tr)),
            "paste-how" => Some((
                "PASTE".into(),
                "at this bar".into(),
                place(vec![
                    wedge(0, "Independent copy", "paste:own"),
                    wedge(1, "Linked copy", "paste:link"),
                    back(),
                ]),
            )),
            "new-clip-kind" => Some((
                "NEW CLIP".into(),
                "at this bar".into(),
                place(vec![
                    wedge(0, "Note clip", "new-notes"),
                    wedge(1, "Pattern clip", "new-block"),
                    back(),
                ]),
            )),
            "fx-move" => Some((
                "MOVE".into(),
                "in the row".into(),
                place(vec![
                    wedge(0, "Left", "fx-left"),
                    wedge(1, "Right", "fx-right"),
                    back(),
                ]),
            )),
            "fx-presets" => match on {
                PieFor::RackFx(idx, i) => Some(self.fx_preset_page(idx, i)),
                _ => None,
            },
            "arrange" => Some(self.arrange_page(matches!(on, PieFor::Clip(_)))),
            "pattern" => Some(self.pattern_page(track(on).unwrap_or(0))),
            "remove" => Some(self.remove_page(on)),
            "effects" => match on {
                PieFor::RackInst(idx) | PieFor::RackFx(idx, _) => Some(self.effects_page(idx)),
                _ => None,
            },
            "shape" => Some((
                "SHAPE".into(),
                "segment".into(),
                place(vec![
                    wedge(0, "Linear", "shape:linear"),
                    wedge(1, "Step", "shape:step"),
                    wedge(2, "Smooth", "shape:smooth"),
                    back(),
                ]),
            )),
            "fill" => Some((
                "FILL".into(),
                "this lane".into(),
                place(vec![
                    wedge(0, "Every beat", "fill:beat"),
                    wedge(1, "Every 8th", "fill:8"),
                    wedge(2, "Every 16th", "fill:16"),
                    back(),
                ]),
            )),
            "place" => Some((
                "PLACE".into(),
                "on the timeline".into(),
                place(vec![
                    wedge(0, "At the play marker", "place:play"),
                    wedge(1, "After the last clip", "place:last"),
                    wedge(2, "In the loop region", "place:loop"),
                    wedge(3, "Follow…", "launch-follow"),
                    back(),
                ]),
            )),
            "variations" => Some(self.var_page(on)),
            "edit-block" => Some((
                "EDIT".into(),
                "this clip".into(),
                place(vec![
                    wedge(0, "Piano roll", "edit:roll"),
                    wedge(2, "Pattern editor", "edit:pattern"),
                    back(),
                ]),
            )),
            "clip-lane" => match on {
                PieFor::Auto { .. } => Some(self.clip_lane_page(on)),
                _ => None,
            },
            _ => None,
        }
    }

    fn track_root(&self, tr: usize) -> (String, String, Vec<RadialItem>) {
        let info = self.pie_info(tr);
        let title = format!("{:02}", tr + 1);
        let sub = self
            .with(|s| s.song.channel_names.get(tr).cloned())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Track".into());
        let items = match info.class {
            super::trackpie::TrackClass::Empty => place(vec![
                deeper(0, "Instrument", "instruments"),
                deeper(1, "Drum kit", "drums"),
                wedge(2, "Open Rack", "rack"),
                deeper(4, "Add lane", "add-lane"),
                deeper(5, "Look", "look"),
                wedge(6, "Mute", "mute"),
                deeper(7, "Remove…", "remove"),
            ]),
            super::trackpie::TrackClass::Drum => place(vec![
                wedge(0, "Drum pattern", "drum-pattern"),
                wedge(1, "Duplicate", "dup-track"),
                wedge(2, "Edit steps", "edit-steps"),
                deeper(3, "Swap kit", "drums"),
                deeper(4, "Lanes", "lanes"),
                deeper(5, "Look", "look"),
                wedge(6, "Mute", "mute"),
                deeper(7, "Remove…", "remove"),
            ]),
            super::trackpie::TrackClass::Melodic => {
                let pitched = !info.percussive;
                let mut v = vec![
                    if pitched {
                        deeper(0, "New pattern", "pattern")
                    } else {
                        wedge(0, "New pattern", "new-pattern")
                    },
                    deeper(1, "Repeat", "repeat-track"),
                    wedge(2, "Edit notes", "edit-notes"),
                ];
                if info.sounds.is_empty() {
                    v.push(wedge(3, "Browse…", "browse"));
                } else {
                    v.push(deeper(3, "Sounds", "sounds"));
                }
                v.push(deeper(4, "Lanes", "lanes"));
                v.push(deeper(5, "Look", "look"));
                v.push(wedge(6, "Mute", "mute"));
                v.push(deeper(7, "Remove…", "remove"));
                place(v)
            }
        };
        (title, sub, items)
    }

    fn new_track_root(&self) -> (String, String, Vec<RadialItem>) {
        (
            "NEW".into(),
            "track".into(),
            place(vec![
                deeper(0, "Instrument", "instruments"),
                deeper(1, "Drum pattern", "drums"),
                wedge(2, "Audio track", "audio"),
                wedge(3, "Empty track", "empty"),
            ]),
        )
    }

    fn instrument_page(
        &self,
        _new_track: bool,
        _tr: Option<usize>,
    ) -> (String, String, Vec<RadialItem>) {
        // Six common melodic instruments, then Browse… for Voice, Wavetable,
        // Grid, Noise, plugins and library sounds. Nothing is only on this ring.
        let names = [
            Builtin::WaveForge,
            Builtin::StringWave,
            Builtin::Fm4,
            Builtin::Analog,
            Builtin::Synth,
            Builtin::ChipFx,
        ];
        let mut v = vec![];
        for (i, b) in names.into_iter().enumerate() {
            v.push(wedge(i as u8, b.label(), &format!("inst:{}", b.label())));
        }
        v.push(wedge(6, "Browse…", "browse"));
        v.push(back());
        ("INSTRUMENT".into(), "then Back".into(), place(v))
    }

    fn drum_page(&self, _new_track: bool, _tr: Option<usize>) -> (String, String, Vec<RadialItem>) {
        let kits = ["909 Kit", "Psy Kit", "Techno Kit", "Trap Kit", "Lo-fi Kit"];
        let mut v = vec![];
        for (i, k) in kits.into_iter().enumerate() {
            v.push(wedge(i as u8, k, &format!("kit:{k}")));
        }
        v.push(wedge(5, "Drum Synth", "inst:Drum Synth"));
        v.push(wedge(6, "More kits…", "more-kits"));
        v.push(back());
        (
            "DRUMS".into(),
            "Acoustic-ish is under More kits…".into(),
            place(v),
        )
    }

    fn sounds_page(&self, tr: usize) -> (String, String, Vec<RadialItem>) {
        let info = self.pie_info(tr);
        self.preset_names(&info.sounds)
    }

    fn preset_page_idx(&self, idx: usize) -> (String, String, Vec<RadialItem>) {
        let names = self.with(|s| {
            s.song
                .instruments
                .get(idx)
                .map(|i| {
                    super::instpanel::instcard::preset_list(i)
                        .into_iter()
                        .map(|p| p.1)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        self.preset_names(&names)
    }

    fn preset_names(&self, names: &[String]) -> (String, String, Vec<RadialItem>) {
        let mut v = vec![];
        for (i, n) in names.iter().take(6).enumerate() {
            v.push(wedge(i as u8, n, &format!("sound:{i}")));
        }
        v.push(wedge(6, "Browse…", "browse"));
        v.push(back());
        ("SOUNDS".into(), "name and effects stay".into(), place(v))
    }

    fn lanes_page(
        &self,
        tr: usize,
        only_fx: Option<(usize, usize)>,
    ) -> (String, String, Vec<RadialItem>) {
        if let Some((idx, dev)) = only_fx {
            return self.fx_lane_page(idx, dev);
        }
        let mut v = vec![wedge(0, "Show or hide", "lanes-toggle")];
        let shorts = self.lane_shortcuts(tr);
        for (i, (label, _)) in shorts.iter().take(4).enumerate() {
            v.push(wedge((i + 1) as u8, label, &format!("lane-short:{i}")));
        }
        v.push(wedge(5, "All parameters…", "lanes-all"));
        v.push(back());
        ("LANES".into(), "this track".into(), place(v))
    }

    /// parameters worth offering first on this track: its mixer volume, its
    /// instrument cutoff when it has one, and the mix of its first insert.
    /// Not the master limiter and not another instrument
    pub(crate) fn lane_shortcuts(&self, tr: usize) -> Vec<(String, Target)> {
        let slot = self.track_slot(tr);
        let idx = (slot as usize).wrapping_sub(1);
        let mut out = vec![("Track volume".into(), Target::ChanVolume(tr))];
        let extra = self.with(|s| {
            let inst = s.song.instruments.get(idx)?;
            let mut v = vec![];
            if inst.kind.patch().is_some() {
                v.push(("Cutoff".into(), Target::Cutoff(idx)));
                v.push(("Resonance".into(), Target::Resonance(idx)));
            } else {
                v.push(("Level".into(), Target::Level(idx)));
            }
            if let Some(dev) = inst.fx.first() {
                if let Some(p) = mix_index(dev.kind) {
                    v.push((format!("{} mix", dev.kind.name()), Target::Fx(idx, 0, p)));
                }
            }
            Some(v)
        });
        if let Some(v) = extra {
            out.extend(v);
        }
        out
    }

    fn fx_lane_page(&self, idx: usize, dev: usize) -> (String, String, Vec<RadialItem>) {
        let params = self.with(|s| {
            s.song
                .instruments
                .get(idx)
                .and_then(|i| i.fx.get(dev))
                .map(|d| {
                    (
                        d.title(),
                        crate::fx::params(d.kind)
                            .iter()
                            .map(|p| p.name.to_string())
                            .collect::<Vec<_>>(),
                    )
                })
                .unwrap_or_default()
        });
        let mut v = vec![];
        for (i, name) in params.1.iter().take(6).enumerate() {
            v.push(wedge(i as u8, name, &format!("fxparam:{i}")));
        }
        if params.1.len() > 6 {
            v.push(wedge(6, "All parameters…", "lanes-all"));
        }
        v.push(back());
        (params.0, "this effect".into(), place(v))
    }

    fn clip_look_page(&self) -> (String, String, Vec<RadialItem>) {
        (
            "LOOK".into(),
            "this clip".into(),
            place(vec![
                wedge(0, "Rename", "rename"),
                deeper(1, "Colour", "colour"),
                back(),
            ]),
        )
    }

    fn look_page(&self, _tr: usize) -> (String, String, Vec<RadialItem>) {
        (
            "LOOK".into(),
            "not the sound".into(),
            place(vec![
                wedge(0, "Rename", "rename"),
                deeper(1, "Colour", "colour"),
                deeper(2, "Icon", "icons"),
                deeper(3, "Row speed", "speed"),
                deeper(4, "Move", "move"),
                back(),
            ]),
        )
    }

    fn move_page(&self, _tr: usize) -> (String, String, Vec<RadialItem>) {
        (
            "MOVE".into(),
            "the whole track".into(),
            place(vec![
                wedge(0, "Up", "move:up"),
                wedge(1, "Down", "move:down"),
                wedge(2, "To top", "move:top"),
                wedge(3, "To bottom", "move:bottom"),
                back(),
            ]),
        )
    }

    fn speed_page(&self, _tr: usize) -> (String, String, Vec<RadialItem>) {
        (
            "ROW SPEED".into(),
            "pattern commands".into(),
            place(vec![
                wedge(0, "Follow song", "speed:0"),
                wedge(1, "2", "speed:2"),
                wedge(2, "3", "speed:3"),
                wedge(3, "4", "speed:4"),
                wedge(4, "6", "speed:6"),
                wedge(5, "8", "speed:8"),
                wedge(6, "12", "speed:12"),
                back(),
            ]),
        )
    }

    fn colour_page(&self, more: bool) -> (String, String, Vec<RadialItem>) {
        // Blue is on the first ring (a common choice). The rest follow.
        let first = ["Auto", "Red", "Orange", "Yellow", "Green", "Blue"];
        let rest = ["Teal", "Violet", "Pink"];
        let slice: &[&str] = if more { &rest } else { &first };
        let mut v = vec![];
        for (i, name) in slice.iter().enumerate() {
            let key = if *name == "Auto" {
                "colour:auto".into()
            } else {
                format!("colour:{name}")
            };
            v.push(wedge(i as u8, name, &key));
        }
        if !more {
            v.push(deeper(6, "More", "colour2"));
        }
        v.push(back());
        ("COLOUR".into(), "clip or track".into(), place(v))
    }

    fn icon_page(&self, page: usize) -> (String, String, Vec<RadialItem>) {
        let mut icons: Vec<TrackIcon> = std::iter::once(TrackIcon::Auto)
            .chain(TrackIcon::ALL)
            .collect();
        let start = page * 6;
        if start >= icons.len() {
            icons.clear();
        } else {
            icons = icons.split_off(start);
        }
        let more = icons.len() > 6;
        let mut v = vec![];
        for (i, ic) in icons.iter().take(6).enumerate() {
            let label = if *ic == TrackIcon::Auto {
                "Auto"
            } else {
                ic.label()
            };
            v.push(wedge(i as u8, label, &format!("icon:{}", ic.key())));
        }
        if more {
            let key = if page == 0 { "icons2" } else { "icons3" };
            v.push(deeper(6, "More", key));
        }
        v.push(back());
        ("ICON".into(), "header".into(), place(v))
    }

    fn repeat_page(&self) -> (String, String, Vec<RadialItem>) {
        (
            "REPEAT".into(),
            "one undo".into(),
            place(vec![
                wedge(0, "Linked copy", "rep:linked"),
                wedge(1, "Own copy", "rep:own"),
                wedge(2, "Repeat ×2", "rep:2"),
                wedge(3, "Repeat ×4", "rep:4"),
                wedge(4, "Copy", "rep:copy"),
                wedge(5, "Cut", "rep:cut"),
                wedge(6, "Make unique", "rep:unique"),
                back(),
            ]),
        )
    }

    fn arrange_page(&self, note: bool) -> (String, String, Vec<RadialItem>) {
        let mut v = vec![wedge(0, "Loop this clip", "loop-clip")];
        if note {
            v.push(wedge(1, "To launcher", "to-launcher"));
        }
        v.push(wedge(2, "Glue", "glue"));
        v.push(wedge(3, "Use track instrument", "use-instr"));
        v.push(back());
        ("ARRANGE".into(), "this clip".into(), place(v))
    }

    fn pattern_page(&self, _tr: usize) -> (String, String, Vec<RadialItem>) {
        (
            "PATTERN".into(),
            "at the play marker".into(),
            place(vec![
                wedge(0, "Empty 1 bar", "new-pattern"),
                wedge(1, "Chords…", "chords"),
                wedge(2, "Melody…", "melody"),
                wedge(3, "Bass…", "bass"),
                back(),
            ]),
        )
    }

    fn effects_page(&self, _idx: usize) -> (String, String, Vec<RadialItem>) {
        let kinds = [
            FxKind::Eq,
            FxKind::Compressor,
            FxKind::Delay,
            FxKind::Reverb,
            FxKind::Filter,
            FxKind::Chorus,
        ];
        let mut v = vec![];
        for (i, k) in kinds.into_iter().enumerate() {
            v.push(wedge(i as u8, k.name(), &format!("fx:{}", k.name())));
        }
        v.push(wedge(6, "Browse…", "fx-browse"));
        v.push(back());
        ("INSERT".into(), "on this instrument".into(), place(v))
    }

    fn remove_page(&self, on: PieFor) -> (String, String, Vec<RadialItem>) {
        let (title, label, key) = match on {
            PieFor::Track(_) => ("REMOVE", "Delete track", "delete-track"),
            PieFor::Clip(_) | PieFor::Launch(_) => ("REMOVE", "Delete clip", "delete-clip"),
            PieFor::RackFx(_, _) => ("REMOVE", "Remove effect", "delete-fx"),
            PieFor::Auto { point: Some(_), .. } => ("REMOVE", "Delete point", "delete-point"),
            PieFor::Auto { .. } => ("REMOVE", "Remove lane", "delete-lane"),
            PieFor::Drum { .. } => ("CLEAR", "This lane", "clear-lane"),
            _ => ("REMOVE", "Delete", "delete-clip"),
        };
        let mut v = vec![wedge(0, label, key)];
        if matches!(on, PieFor::Drum { .. }) {
            v.push(wedge(1, "All lanes", "clear-all"));
        }
        v.push(back());
        (title.into(), "Back cancels".into(), place(v))
    }

    fn var_page(&self, on: PieFor) -> (String, String, Vec<RadialItem>) {
        let n = match on {
            PieFor::Clip(id) => self
                .tlv
                .clip_label(id)
                .and_then(|(c, _)| c.notes())
                .map(|d| self.var_count(d))
                .unwrap_or(0),
            _ => 0,
        };
        let mut v = vec![];
        if n < crate::timeline::rr::MAX_VARS {
            v.push(wedge(0, "Add variation", "var:add"));
        }
        if n > 1 {
            v.push(wedge(1, "Order…", "var:order"));
        }
        v.push(back());
        ("VARIATIONS".into(), "this clip".into(), place(v))
    }

    fn clip_lane_page(&self, on: PieFor) -> (String, String, Vec<RadialItem>) {
        let PieFor::Auto {
            clip,
            has_clip_lane,
            ..
        } = on
        else {
            return ("CLIP".into(), "lane".into(), vec![back()]);
        };
        let mut v = vec![];
        match (clip, has_clip_lane) {
            (Some(_), false) => v.push(wedge(0, "Add clip lane", "clip-lane-add")),
            (Some(_), true) => {
                v.push(wedge(0, "Clear clip lane", "clip-lane-clear"));
                v.push(wedge(1, "Remove clip lane", "clip-lane-remove"));
                v.push(wedge(2, "Absolute", "clip-mode:abs"));
                v.push(wedge(3, "Add", "clip-mode:add"));
                v.push(wedge(4, "Multiply", "clip-mode:mul"));
            }
            _ => v.push(wedge(0, "No clip here", "noop").dimmed()),
        }
        v.push(back());
        ("CLIP LANE".into(), "moves with the clip".into(), place(v))
    }

    fn clip_root(&self, id: u32) -> Option<(String, String, Vec<RadialItem>)> {
        let (c, name) = self.tlv.clip_label(id)?;
        let group = self.tlv.sels.contains(&id) && self.tlv.sels.len() > 1;
        let title = if group {
            format!("{} clips", self.tlv.sels.len())
        } else {
            name
        };
        let sub = format!("track {:02}", c.track + 1);
        let block = c.block().is_some();
        let note = c.notes().is_some();
        let edit = if block && self.tracker_on() {
            deeper(0, "Edit notes", "edit-block")
        } else {
            wedge(0, "Edit notes", "edit:roll")
        };
        let mut v = vec![
            edit,
            deeper(1, "Repeat", "repeat"),
            wedge(2, "Split…", "split"),
        ];
        if note {
            v.push(deeper(3, "Variations", "variations"));
        }
        v.push(deeper(4, "Arrange", "arrange"));
        v.push(deeper(5, "Look", "look"));
        v.push(wedge(6, "Mute", "mute-clip"));
        v.push(deeper(7, "Remove…", "remove"));
        Some((title, sub, place(v)))
    }

    fn lane_root(
        &self,
        track: usize,
        at: crate::timeline::Tick,
    ) -> (String, String, Vec<RadialItem>) {
        let name = self
            .with(|s| {
                s.song
                    .timeline
                    .as_deref()
                    .and_then(|t| t.tracks.get(track))
                    .map(|t| t.name.clone())
            })
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("Track {:02}", track + 1));
        let place_s = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .map(|t| t.bar_pos(at))
                .map(|p| format!("bar {}.{}", p.bar, p.beat))
                .unwrap_or_default()
        });
        let paste = if self.tlv.clipboard.is_empty() {
            wedge(1, "Nothing copied", "paste").dimmed()
        } else {
            deeper(1, "Paste here", "paste-how")
        };
        let new_clip = if self.tracker_on() {
            deeper(0, "New clip", "new-clip-kind")
        } else {
            wedge(0, "New clip", "new-notes")
        };
        let mut v = vec![
            new_clip,
            paste,
            wedge(3, "Import audio…", "import-audio"),
            wedge(4, "Add marker", "marker"),
        ];
        if !self.noterec.recent.is_empty() {
            v.push(wedge(2, "Capture MIDI", "capture"));
        }
        (name, place_s, place(v))
    }

    fn launch_root(&self, id: u32) -> Option<(String, String, Vec<RadialItem>)> {
        let c = self.with(|s| {
            let t = s.song.timeline.as_deref()?;
            let l = t.launch_clip(id)?;
            Some((l.clip.track, super::objpie::launch_name(t, &l.clip)))
        })?;
        Some((
            c.1,
            format!("launcher · track {:02}", c.0 + 1),
            place(vec![
                wedge(0, "Play", "launch-play"),
                wedge(1, "Variation", "launch-var"),
                wedge(2, "Edit notes", "launch-edit"),
                wedge(3, "Instrument", "browse"),
                deeper(4, "Place", "place"),
                deeper(5, "Look", "look"),
                wedge(6, "Stop", "launch-stop"),
                deeper(7, "Remove…", "remove"),
            ]),
        ))
    }

    fn rack_inst_root(&self, idx: usize) -> Option<(String, String, Vec<RadialItem>)> {
        let name = self.with(|s| s.song.instruments.get(idx).map(|i| i.name.clone()))?;
        let has_presets = self.with(|s| {
            s.song
                .instruments
                .get(idx)
                .is_some_and(|i| !super::instpanel::instcard::preset_list(i).is_empty())
        });
        let presets = if has_presets {
            deeper(0, "Presets", "presets")
        } else {
            wedge(0, "Browse…", "browse")
        };
        Some((
            "RACK".into(),
            name,
            place(vec![
                presets,
                wedge(2, "Advanced", "advanced"),
                wedge(3, "Replace…", "replace"),
                deeper(4, "Add effect", "effects"),
                wedge(5, "Save rack…", "save-rack"),
            ]),
        ))
    }

    fn rack_fx_root(&self, idx: usize, i: usize) -> Option<(String, String, Vec<RadialItem>)> {
        let (title, has_pre) = self.with(|s| {
            let d = s.song.instruments.get(idx)?.fx.get(i)?;
            Some((d.title(), !crate::fx::presets(d.kind).is_empty()))
        })?;
        let pre = if has_pre {
            deeper(0, "Presets", "fx-presets")
        } else {
            wedge(0, "Parameters", "fx-params-toggle")
        };
        Some((
            "FX".into(),
            title,
            place(vec![
                pre,
                wedge(1, "Duplicate", "fx-dup"),
                wedge(2, "Parameters", "fx-params-toggle"),
                deeper(3, "Move", "fx-move"),
                deeper(4, "Add lane", "fx-lanes"),
                wedge(6, "Bypass", "fx-bypass"),
                deeper(7, "Remove…", "remove"),
            ]),
        ))
    }

    fn auto_root(
        &self,
        track: usize,
        lane: usize,
        point: Option<usize>,
    ) -> (String, String, Vec<RadialItem>) {
        let label = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.tracks.get(track))
                .and_then(|t| t.auto.get(lane))
                .map(|l| {
                    if l.label.is_empty() {
                        l.target.clone()
                    } else {
                        l.label.clone()
                    }
                })
                .unwrap_or_else(|| "Lane".into())
        });
        if point.is_some() {
            (
                "POINT".into(),
                label,
                place(vec![
                    wedge(0, "Linear", "shape:linear"),
                    wedge(1, "Step", "shape:step"),
                    wedge(2, "Smooth", "shape:smooth"),
                    wedge(3, "Straighten", "straighten"),
                    deeper(7, "Remove…", "remove"),
                ]),
            )
        } else {
            (
                "LANE".into(),
                label,
                place(vec![
                    wedge(0, "Add point", "add-point"),
                    deeper(1, "Clip lane", "clip-lane"),
                    wedge(2, "Pencil", "pencil"),
                    deeper(3, "Shape", "shape"),
                    wedge(6, "Lane on", "lane-on"),
                    deeper(7, "Remove…", "remove"),
                ]),
            )
        }
    }

    fn repeat_track_page(&self, tr: usize) -> (String, String, Vec<RadialItem>) {
        let paste = if self.tlv.clipboard.is_empty() {
            wedge(1, "Nothing copied", "paste-play").dimmed()
        } else {
            wedge(1, "Paste at playhead", "paste-play")
        };
        (
            "REPEAT".into(),
            format!("track {:02}", tr + 1),
            place(vec![
                wedge(0, "Duplicate track", "dup-track"),
                paste,
                back(),
            ]),
        )
    }

    fn fx_preset_page(&self, idx: usize, i: usize) -> (String, String, Vec<RadialItem>) {
        let names = self.with(|s| {
            s.song
                .instruments
                .get(idx)
                .and_then(|inst| inst.fx.get(i))
                .map(|d| {
                    crate::fx::presets(d.kind)
                        .iter()
                        .map(|(n, _)| (*n).to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        let mut v = vec![];
        for (k, n) in names.iter().take(7).enumerate() {
            v.push(wedge(k as u8, n, &format!("fxpre:{k}")));
        }
        v.push(back());
        ("PRESET".into(), "this effect".into(), place(v))
    }

    /// run a compass choice. The clicked object is `on`, not whatever was
    /// selected before the menu opened
    pub(crate) fn compass_act(&mut self, on: PieFor, key: &str) {
        if key == "noop" || key == "back" {
            return;
        }
        match on {
            PieFor::Track(tr) => self.act_track(tr, key, false),
            PieFor::NewTrack => self.act_new_track(key),
            PieFor::Master => self.act_master(key),
            PieFor::Clip(id) => self.act_clip(id, key),
            PieFor::Lane { track, at } => self.act_lane(track, at, key),
            PieFor::Launch(id) => self.act_launch(id, key),
            PieFor::Drum {
                clip,
                data,
                track,
                pad,
            } => self.act_drum(clip, data, track, pad, key),
            PieFor::RackInst(idx) => self.act_rack_inst(idx, key),
            PieFor::RackFx(idx, i) => self.act_rack_fx(idx, i, key),
            PieFor::Auto {
                track,
                lane,
                point,
                clip,
                tick,
                ..
            } => self.act_auto(track, lane, point, clip, tick, key),
        }
    }

    fn act_track(&mut self, tr: usize, key: &str, _new_track: bool) {
        self.tl_select_track(tr, true);
        self.ch = tr;
        if let Some(rest) = key.strip_prefix("inst:") {
            if let Some(b) = builtin_named(rest) {
                self.pie_apply(tr, PieAct::NewInstrument(b), &egui_ctx_placeholder());
            }
            return;
        }
        if let Some(kit) = key.strip_prefix("kit:") {
            self.pie_apply(tr, PieAct::DrumKitHere(leak(kit)), &egui_ctx_placeholder());
            return;
        }
        if key == "inst:Drum Synth" {
            self.pie_apply(
                tr,
                PieAct::DrumHere(Builtin::DrumSynth),
                &egui_ctx_placeholder(),
            );
            return;
        }
        match key {
            "rack" => self.pie_apply(tr, PieAct::Devices, &egui_ctx_placeholder()),
            "browse" | "more-kits" => {
                if key == "more-kits" {
                    self.pie_apply(tr, PieAct::MoreKitsHere, &egui_ctx_placeholder());
                } else {
                    self.pie_apply(tr, PieAct::ChangeInstrument, &egui_ctx_placeholder());
                }
            }
            "mute" => self.pie_apply(tr, PieAct::Mute, &egui_ctx_placeholder()),
            "dup-track" => self.pie_apply(tr, PieAct::Duplicate, &egui_ctx_placeholder()),
            "delete-track" => self.pie_apply(tr, PieAct::Delete, &egui_ctx_placeholder()),
            "rename" => self.pie_apply(tr, PieAct::Rename, &egui_ctx_placeholder()),
            "new-pattern" | "drum-pattern" => {
                self.pie_apply(tr, PieAct::NewPattern, &egui_ctx_placeholder())
            }
            "edit-notes" | "edit-steps" => self.edit_track_notes(tr, key == "edit-steps"),
            "chords" => self.pie_apply(tr, PieAct::Chords, &egui_ctx_placeholder()),
            "melody" => self.pie_apply(tr, PieAct::Melody, &egui_ctx_placeholder()),
            "bass" => self.pie_apply(tr, PieAct::Bass, &egui_ctx_placeholder()),
            "paste-play" => self.pie_apply(tr, PieAct::PasteHere, &egui_ctx_placeholder()),
            "lanes-toggle" => self.pie_apply(tr, PieAct::AutoToggle, &egui_ctx_placeholder()),
            "lanes-all" => {
                let at = Pos2::new(200.0, 200.0);
                self.track_auto_open(tr, at);
            }
            "move:up" => self.pie_apply(tr, PieAct::Move(MoveTo::Up), &egui_ctx_placeholder()),
            "move:down" => self.pie_apply(tr, PieAct::Move(MoveTo::Down), &egui_ctx_placeholder()),
            "move:top" => self.pie_apply(tr, PieAct::Move(MoveTo::Top), &egui_ctx_placeholder()),
            "move:bottom" => {
                self.pie_apply(tr, PieAct::Move(MoveTo::Bottom), &egui_ctx_placeholder())
            }
            _ => {}
        }
        if let Some(n) = key.strip_prefix("speed:") {
            if let Ok(v) = n.parse::<u8>() {
                self.pie_apply(tr, PieAct::Speed(v), &egui_ctx_placeholder());
            }
        }
        if let Some(name) = key.strip_prefix("colour:") {
            let c = if name == "auto" {
                None
            } else {
                super::objpie::COLOURS
                    .iter()
                    .find(|c| c.0 == name)
                    .map(|c| c.1)
            };
            self.pie_apply(tr, PieAct::Colour(c), &egui_ctx_placeholder());
        }
        if let Some(k) = key.strip_prefix("icon:") {
            let ic = TrackIcon::from_key(k);
            self.pie_apply(tr, PieAct::Icon(ic), &egui_ctx_placeholder());
        }
        if let Some(i) = key.strip_prefix("sound:") {
            if let Ok(k) = i.parse::<usize>() {
                self.pie_apply(tr, PieAct::Sound(k as u8), &egui_ctx_placeholder());
            }
        }
        if let Some(i) = key.strip_prefix("lane-short:") {
            if let Ok(k) = i.parse::<usize>() {
                if let Some((_, tg)) = self.lane_shortcuts(tr).get(k) {
                    self.auto_op(super::timeline::autolanes::AutoOp::AddLane(tr, *tg));
                    self.with(|s| {
                        if let Some(t) = s.song.timeline.as_deref_mut() {
                            if tr < 64 {
                                t.view.auto_open |= 1 << tr;
                            }
                        }
                    });
                    self.status = format!("Lane on track {:02}", tr + 1);
                }
            }
        }
    }

    fn act_new_track(&mut self, key: &str) {
        if let Some(rest) = key.strip_prefix("inst:") {
            if rest == "Drum Synth" {
                self.pie_apply_new(PieAct::DrumTrack(Builtin::DrumSynth));
            } else if let Some(b) = builtin_named(rest) {
                self.pie_apply_new(PieAct::NewTrackWith(b));
            }
            return;
        }
        if let Some(kit) = key.strip_prefix("kit:") {
            self.pie_apply_new(PieAct::DrumKitTrack(leak(kit)));
            return;
        }
        match key {
            "audio" => self.pie_apply_new(PieAct::AddAudio),
            "empty" => self.pie_apply_new(PieAct::AddEmpty(false)),
            "browse" => self.pie_apply_new(PieAct::AddInstrument),
            "more-kits" => self.pie_apply_new(PieAct::MoreKits),
            _ => {}
        }
    }

    fn act_master(&mut self, key: &str) {
        match key {
            "master" => self.lcd.master_open = true,
            "export" => self.open_export(),
            "save" => self.save(false),
            _ => {}
        }
    }

    fn act_clip(&mut self, id: u32, key: &str) {
        self.clip_scope(id);
        match key {
            "edit:roll" => {
                self.tlv.sel = Some(id);
                self.timeline_open_roll();
            }
            "edit:pattern" => {
                if let Some((c, _)) = self.tlv.clip_label(id) {
                    if let Some((b, col)) = c.block() {
                        let row = (c.offset / crate::timeline::ROW) as usize;
                        self.timeline_open(b, col, row, usize::MAX);
                    }
                }
            }
            "split" => self.tl_apply(super::timeline::TlOp::SplitArm),
            "rep:linked" => self.tl_apply(super::timeline::TlOp::Duplicate),
            "rep:own" => {
                self.snapshot();
                self.undo_hold += 1;
                self.tl_apply(super::timeline::TlOp::Duplicate);
                self.tl_apply(super::timeline::TlOp::MakeUnique);
                self.undo_hold -= 1;
                self.status = "Independent copy placed after it (its own notes)".into();
            }
            "rep:2" => self.repeat_n(1),
            "rep:4" => self.repeat_n(3),
            "rep:copy" => self.tl_copy(false),
            "rep:cut" => self.tl_copy(true),
            "rep:unique" => self.tl_apply(super::timeline::TlOp::MakeUnique),
            "loop-clip" => self.tl_apply(super::timeline::TlOp::LoopClip(id)),
            "to-launcher" => self.clip_to_launcher(id),
            "glue" => self.tl_apply(super::timeline::TlOp::Glue),
            "use-instr" => self.tl_apply(super::timeline::TlOp::UseTrackInstr),
            "mute-clip" => self.tl_apply(super::timeline::TlOp::Mute),
            "delete-clip" => self.tl_apply(super::timeline::TlOp::Delete),
            "rename" => self.start_clip_rename(id),
            "var:add" | "var:order" => {
                let child = if key == "var:order" {
                    Some("Order…")
                } else {
                    Some("Add variation")
                };
                self.var_pie_pick(Some(id), child);
            }
            _ => {}
        }
        if let Some(name) = key.strip_prefix("colour:") {
            let c = if name == "auto" {
                Some("Auto")
            } else {
                Some(name)
            };
            // set_clip_colour wants the display name, including Auto
            let label = if name == "auto" { "Auto" } else { name };
            let _ = c;
            self.set_clip_colour(&self.tlv.sels.clone(), Some(label));
        }
    }

    fn repeat_n(&mut self, n: usize) {
        self.snapshot();
        self.undo_hold += 1;
        for _ in 0..n {
            self.tl_apply(super::timeline::TlOp::Duplicate);
        }
        self.undo_hold -= 1;
        self.status = format!("Repeated: {n} linked copies after it");
    }

    fn clip_scope(&mut self, id: u32) {
        let group = self.tlv.sels.contains(&id) && self.tlv.sels.len() > 1;
        if !group {
            self.tlv.sel = Some(id);
            self.tlv.sels = vec![id];
        }
    }

    fn edit_track_notes(&mut self, tr: usize, steps: bool) {
        let id = self.with(|s| {
            let t = s.song.timeline.as_deref()?;
            t.clips
                .iter()
                .find(|c| {
                    c.track as usize == tr
                        && c.audio().is_none()
                        && (c.notes().is_some() || c.block().is_some())
                })
                .map(|c| c.id)
        });
        if let Some(id) = id {
            self.track_open_pattern(tr, id);
        } else if steps {
            self.pie_apply(tr, PieAct::NewPattern, &egui_ctx_placeholder());
        } else {
            self.status = "This track has no note clip yet. New pattern makes one.".into();
        }
    }

    fn act_lane(&mut self, track: usize, at: crate::timeline::Tick, key: &str) {
        self.tl_select_track(track, true);
        let bar = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .map_or(4 * crate::timeline::PPQ, |t| t.sig_at(0).bar_ticks())
        });
        let on_bar = at / bar * bar;
        match key {
            "new-notes" => self.tl_apply(super::timeline::TlOp::NewNotes(track, on_bar)),
            "new-block" => self.tl_apply(super::timeline::TlOp::NewClip(track, on_bar)),
            "paste:own" => self.tl_paste(Some((track, at)), false),
            "paste:link" => self.tl_paste(Some((track, at)), true),
            "capture" => self.tl_apply(super::timeline::TlOp::Capture(track, on_bar)),
            "import-audio" => self.audio_op(super::timeline::audioclip::AudioOp::Import(
                track,
                crate::timeline::snap(at, crate::timeline::ROW),
            )),
            "marker" => self.tl_apply(super::timeline::TlOp::AddMarker(crate::timeline::snap(
                at,
                crate::timeline::ROW,
            ))),
            "paste" => {
                self.status = "Nothing copied: select clips and press Ctrl+C first".into();
            }
            _ => {}
        }
    }

    fn act_launch(&mut self, id: u32, key: &str) {
        let Some((tr, k)) = self.launch_where(id) else {
            return;
        };
        match key {
            "launch-play" => self.ca_slot(tr, k),
            "launch-stop" => self.ca_stop(tr),
            "launch-edit" => self.ca_open_launch(tr, k),
            "browse" => self.track_change_instrument(tr),
            "launch-follow" => {
                self.launch_follow_open(id);
            }
            "place:play" => self.launch_place(id, "At the play marker"),
            "place:last" => self.launch_place(id, "After the last clip"),
            "place:loop" => self.launch_place(id, "In the loop region"),
            "delete-clip" => self.ca_delete(id),
            "rename" => self.start_clip_rename(id),
            "launch-var" => self.ca_variation(id),
            _ => {}
        }
        if let Some(name) = key.strip_prefix("colour:") {
            let label = if name == "auto" { "Auto" } else { name };
            self.set_clip_colour(&[id], Some(label));
        }
    }

    fn launch_follow_open(&mut self, id: u32) {
        let at = self.opie_at.unwrap_or(Pos2::new(240.0, 240.0));
        self.follow_open(id, at);
    }

    fn act_drum(&mut self, clip: u32, data: usize, track: usize, pad: usize, key: &str) {
        let (label, child) = match key {
            "drum-gen" => ("Generate", None),
            "drum-copy" => ("Copy pattern", None),
            "drum-var" => ("Variation", None),
            "drum-play" => ("Play", None),
            "fill:beat" => ("Fill", Some("Every beat")),
            "fill:8" => ("Fill", Some("Every 8th")),
            "fill:16" => ("Fill", Some("Every 16th")),
            "clear-lane" => ("Clear", Some("This lane")),
            "clear-all" => ("Clear", Some("All lanes")),
            _ => return,
        };
        self.dk_pie(clip, data, track, pad, label, child);
    }

    fn act_rack_inst(&mut self, idx: usize, key: &str) {
        match key {
            "advanced" => {
                self.instr = (idx + 1) as u8;
                self.ip.advanced = true;
                self.set_lower(1);
            }
            "replace" | "browse" => {
                self.instr = (idx + 1) as u8;
                self.track_change_instrument(self.ch);
            }
            "save-rack" => {
                if let Some(r) = self.chains_root() {
                    self.open_save_rack(r);
                } else {
                    self.open_save_rack(idx);
                }
            }
            "fx-browse" => self.devices_open_effects(),
            _ => {}
        }
        if let Some(name) = key.strip_prefix("fx:") {
            if let Some(kind) = fx_named(name) {
                self.apply_fx_ops(
                    idx,
                    vec![super::fxview::FxOp::Insert(
                        usize::MAX,
                        Box::new(crate::fx::Device::new(kind)),
                    )],
                );
                self.set_lower(4);
                self.status = format!("Added {name} on this instrument");
            }
        }
        if let Some(i) = key.strip_prefix("sound:") {
            if let Ok(k) = i.parse::<usize>() {
                self.apply_preset_idx(idx, k);
            }
        }
    }

    fn apply_preset_idx(&mut self, idx: usize, k: usize) {
        let tr = {
            let n = self.with(|s| s.song.channels);
            (0..n).find(|&tr| self.track_slot(tr) as usize == idx + 1)
        };
        if let Some(tr) = tr {
            self.track_sound(tr, k);
        }
    }

    fn act_rack_fx(&mut self, idx: usize, i: usize, key: &str) {
        let op = match key {
            "fx-dup" => Some(super::fxview::FxOp::Dup(i)),
            "fx-bypass" => Some(super::fxview::FxOp::Toggle(i)),
            "fx-left" => Some(super::fxview::FxOp::Move(i, i.saturating_sub(1))),
            "fx-right" => {
                let n = self.with(|s| s.song.instruments.get(idx).map(|x| x.fx.len()).unwrap_or(0));
                Some(super::fxview::FxOp::Move(i, (i + 2).min(n)))
            }
            "delete-fx" => Some(super::fxview::FxOp::Remove(i)),
            "fx-params-toggle" => {
                let key = (
                    idx,
                    super::devcards::open_key(
                        &self.with(|s| s.song.instruments[idx].fx.clone()),
                        i,
                    ),
                );
                if !self.dev.open.remove(&key) {
                    self.dev.open.insert(key);
                }
                None
            }
            _ => None,
        };
        if let Some(op) = op {
            self.apply_fx_ops(idx, vec![op]);
        }
        if let Some(k) = key.strip_prefix("fxpre:") {
            if let Ok(k) = k.parse::<usize>() {
                self.apply_fx_ops(idx, vec![super::fxview::FxOp::Preset(i, k)]);
            }
        }
        if let Some(p) = key.strip_prefix("fxparam:") {
            if let Ok(p) = p.parse::<usize>() {
                if let Some(tg) = self.with(|s| {
                    let kind = s.song.instruments.get(idx)?.fx.get(i)?.kind;
                    let _ = kind;
                    Some(Target::Fx(idx, i, p))
                }) {
                    let tr = self.ch;
                    self.auto_op(super::timeline::autolanes::AutoOp::AddLane(tr, tg));
                    self.with(|s| {
                        if let Some(t) = s.song.timeline.as_deref_mut() {
                            if tr < 64 {
                                t.view.auto_open |= 1 << tr;
                            }
                        }
                    });
                }
            }
        }
        if key == "lanes-all" {
            self.track_auto_open(self.ch, Pos2::new(200.0, 200.0));
        }
    }

    fn act_auto(
        &mut self,
        track: usize,
        lane: usize,
        point: Option<usize>,
        clip: Option<u32>,
        tick: i64,
        key: &str,
    ) {
        use super::timeline::autolanes::{AutoOp, LaneRef};
        use crate::automation::{ClipMode, Shape};
        let target = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.tracks.get(track))
                .and_then(|t| t.auto.get(lane))
                .map(|l| l.target.clone())
                .unwrap_or_default()
        });
        let lane_now = |target: &str| LaneRef::Track(track, target.to_string());
        match key {
            "add-point" => {
                let pos = (tick as f32 / crate::timeline::ROW as f32).max(0.0);
                self.auto_op(AutoOp::Add(lane_now(&target), pos, 0.5, None));
            }
            "pencil" => self.auto_tool = crate::automation::AutoTool::Pencil,
            "lane-on" => {
                let on = self.with(|s| {
                    s.song
                        .timeline
                        .as_deref()
                        .and_then(|t| t.tracks.get(track))
                        .and_then(|t| t.auto.get(lane))
                        .map(|l| l.enabled)
                        .unwrap_or(true)
                });
                self.auto_op(AutoOp::Enable(track, target.clone(), !on));
            }
            "delete-lane" => {
                self.auto_op(AutoOp::Remove(track, target.clone()));
            }
            "delete-point" => {
                if let Some(i) = point {
                    self.auto_op(AutoOp::Delete(lane_now(&target), i));
                }
            }
            "straighten" => {
                if let Some(i) = point {
                    self.auto_op(AutoOp::Curve(lane_now(&target), i, 0.0));
                }
            }
            "clip-lane-add" => {
                if let Some(id) = clip {
                    self.auto_op(AutoOp::AddClipLane(id, target.clone()));
                }
            }
            "clip-lane-clear" => {
                if let Some(id) = clip {
                    self.auto_op(AutoOp::Clear(LaneRef::Clip(id, target.clone())));
                }
            }
            "clip-lane-remove" => {
                if let Some(id) = clip {
                    self.auto_op(AutoOp::RemoveClipLane(id, target.clone()));
                }
            }
            "clip-mode:abs" | "clip-mode:add" | "clip-mode:mul" => {
                if let Some(id) = clip {
                    let mode = match key {
                        "clip-mode:add" => ClipMode::RelAdd,
                        "clip-mode:mul" => ClipMode::RelMul,
                        _ => ClipMode::Absolute,
                    };
                    self.auto_op(AutoOp::SetClipMode(id, target.clone(), mode));
                }
            }
            _ => {}
        }
        if let Some(name) = key.strip_prefix("shape:") {
            let sh = match name {
                "step" => Shape::Step,
                "smooth" => Shape::Smooth,
                _ => Shape::Linear,
            };
            if let Some(i) = point {
                self.auto_op(AutoOp::Shape(lane_now(&target), i, sh));
            }
        }
    }
}

fn builtin_named(name: &str) -> Option<Builtin> {
    Builtin::ALL.into_iter().find(|b| b.label() == name)
}

fn fx_named(name: &str) -> Option<FxKind> {
    FxKind::ALL.into_iter().find(|k| k.name() == name)
}

/// PieAct stores a `&'static str` for kit names. The menu's labels are
/// already `'static` literals, so this only accepts those
fn leak(s: &str) -> &'static str {
    match s {
        "909 Kit" => "909 Kit",
        "Psy Kit" => "Psy Kit",
        "Techno Kit" => "Techno Kit",
        "Acoustic-ish Kit" => "Acoustic-ish Kit",
        "Trap Kit" => "Trap Kit",
        "Lo-fi Kit" => "Lo-fi Kit",
        _ => "909 Kit",
    }
}

fn egui_ctx_placeholder() -> egui::Context {
    // pie_apply only uses ctx to request_repaint. A short-lived context
    // is enough; the real frame repaints anyway
    egui::Context::default()
}

fn mix_index(kind: FxKind) -> Option<usize> {
    crate::fx::params(kind)
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case("mix"))
}

/// parameter names of one insert, for tests. Not another device's names
#[cfg(test)]
pub(crate) fn effect_param_names(kind: FxKind) -> Vec<String> {
    crate::fx::params(kind)
        .iter()
        .map(|p| p.name.to_string())
        .collect()
}
