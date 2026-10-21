//! Stage 5 (docs/unified/PLAN.md): the pies for the objects you work on,
//! on the reusable `radial.rs` widget: a Timeline clip, a launcher clip and
//! the Drum Kit step grid. Each wedge reuses the operation its list menu,
//! shortcut or button already has (one undo step each). Settings › Clip
//! menus › List keeps the plain list menus instead. Also the clip Rename
//! window and the clip colours (5.4).
use super::radial::{RIcon, Radial, RadialEv, RadialItem};
use super::timeline::TlOp;
use super::*;
use crate::timeline as tl;

/// what a pie is open on
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PieFor {
    /// a Timeline clip (note or Tracker clip)
    Clip(u32),
    /// a Clip Arranger launcher clip
    Launch(u32),
    /// the Drum Kit step grid: the pattern clip, its notes and a pad lane
    Drum {
        clip: u32,
        data: usize,
        track: usize,
        pad: usize,
    },
}

/// clip colours offered (the track palette's first eight)
pub(crate) const COLOURS: [(&str, u32); 8] = [
    ("Red", 0xB0605A),
    ("Orange", 0xC0864A),
    ("Yellow", 0xB9A04E),
    ("Green", 0x6E9A58),
    ("Teal", 0x4F9A96),
    ("Blue", 0x5A86B4),
    ("Violet", 0x7A6CB4),
    ("Pink", 0xA8679A),
];

fn colour_children() -> Vec<String> {
    std::iter::once("Auto".to_string())
        .chain(COLOURS.iter().map(|c| c.0.to_string()))
        .collect()
}

/// the clip Rename window: (clip id, text, focus it next frame)
#[derive(Clone, Debug)]
pub(crate) struct ClipRename {
    pub id: u32,
    pub text: String,
    pub focus: bool,
}

/// Timeline clip pie wedges (with `block`: a Tracker clip, no launcher)
pub(crate) fn clip_items(block: bool, tracker: bool) -> Vec<RadialItem> {
    let mut edit = vec!["Piano roll".to_string()];
    if block && tracker {
        edit.push("Tracker".into());
    }
    let mut v = vec![
        RadialItem::new("Edit", Some(RIcon::Edit), edit),
        RadialItem::new("Split…", Some(RIcon::Select), vec![]),
        RadialItem::new(
            "Duplicate",
            Some(RIcon::Add),
            vec![
                "Linked copy".into(),
                "Independent copy".into(),
                "Repeat ×2".into(),
                "Repeat ×4".into(),
            ],
        ),
        RadialItem::new("Make unique", Some(RIcon::Generate), vec![]),
        RadialItem::new("Colour", Some(RIcon::Bulb), colour_children()),
        RadialItem::new("Rename", Some(RIcon::Type), vec![]),
        RadialItem::new("Mute", None, vec![]),
        RadialItem::new("Delete", Some(RIcon::Delete), vec![]),
    ];
    if !block {
        v.push(RadialItem::new("To launcher", Some(RIcon::Play), vec![]));
    }
    v.push(RadialItem::new(
        "Notes",
        Some(RIcon::Notes),
        vec![
            "Use the track's instrument".into(),
            "Glue".into(),
            "Loop around".into(),
        ],
    ));
    v
}

/// launcher clip pie wedges
pub(crate) fn launch_items() -> Vec<RadialItem> {
    vec![
        RadialItem::new("Play", Some(RIcon::Play), vec![]),
        RadialItem::new("Stop", None, vec![]),
        RadialItem::new("Edit notes", Some(RIcon::Edit), vec![]),
        RadialItem::new("Instrument", Some(RIcon::Bulb), vec![]),
        RadialItem::new("Duplicate (variation)", Some(RIcon::Add), vec![]),
        RadialItem::new(
            "Place in arrangement",
            Some(RIcon::Notes),
            vec![
                "At the play marker".into(),
                "After the last clip".into(),
                "In the loop region".into(),
            ],
        ),
        RadialItem::new("Rename", Some(RIcon::Type), vec![]),
        RadialItem::new("Colour", None, colour_children()),
        RadialItem::new("Delete", Some(RIcon::Delete), vec![]),
    ]
}

/// Drum Kit step grid pie wedges
pub(crate) fn drum_items() -> Vec<RadialItem> {
    vec![
        RadialItem::new("Play", Some(RIcon::Play), vec![]),
        RadialItem::new("Generate", Some(RIcon::Generate), vec![]),
        RadialItem::new(
            "Fill",
            Some(RIcon::Notes),
            vec!["Every beat".into(), "Every 8th".into(), "Every 16th".into()],
        ),
        RadialItem::new("Variation", Some(RIcon::Bulb), vec![]),
        RadialItem::new(
            "Clear",
            Some(RIcon::Delete),
            vec!["This lane".into(), "All lanes".into()],
        ),
        RadialItem::new("Duplicate as variation", Some(RIcon::Add), vec![]),
    ]
}

impl App {
    /// open a pie at `at`
    pub(crate) fn open_obj_pie(&mut self, on: PieFor, at: Pos2, ctx: &egui::Context) {
        let (items, title, sub) = match on {
            PieFor::Clip(id) => {
                let Some((c, name)) = self.tlv.clip_label(id) else {
                    return;
                };
                let (a, b) = self.with(|s| {
                    s.song
                        .timeline
                        .as_deref()
                        .zip(c.notes())
                        .map_or((0, 0), |(t, d)| tl::linked(t, d))
                });
                let sub = if a + b > 1 {
                    format!("track {:02} · linked ×{}", c.track + 1, a + b)
                } else {
                    format!("track {:02}", c.track + 1)
                };
                (
                    clip_items(c.block().is_some(), self.tracker_on()),
                    name,
                    sub,
                )
            }
            PieFor::Launch(id) => {
                let Some(c) = self.with(|s| {
                    s.song
                        .timeline
                        .as_deref()
                        .and_then(|t| t.launch_clip(id))
                        .map(|l| (l.clip.track, launch_name(t_of(s), &l.clip)))
                }) else {
                    return;
                };
                (
                    launch_items(),
                    c.1,
                    format!("launcher · track {:02}", c.0 + 1),
                )
            }
            PieFor::Drum { pad, .. } => (
                drum_items(),
                "PATTERN".to_string(),
                format!("pad lane {}", pad + 1),
            ),
        };
        let mut pie = Radial::new(at, title, sub, items);
        pie.clamp_to(ctx.content_rect());
        self.opie = Some((pie, on));
    }

    /// every frame: the open pie, the Rename window
    pub(crate) fn obj_pie_frame(&mut self, ctx: &egui::Context) {
        if let Some((on, at)) = self.opie_req.take() {
            self.open_obj_pie(on, at, ctx);
        }
        if let Some((mut pie, on)) = self.opie.take() {
            match pie.show(ctx) {
                RadialEv::Cancel => {}
                RadialEv::Pick(i, c) => {
                    let label = pie
                        .items
                        .get(i)
                        .map(|x| x.label.clone())
                        .unwrap_or_default();
                    let child = c.and_then(|c| pie.items.get(i)?.children.get(c).cloned());
                    self.obj_pie_pick(on, &label, child.as_deref());
                }
                _ => self.opie = Some((pie, on)),
            }
        }
        self.clip_rename_window(ctx);
    }

    /// Esc while a pie or the Rename window is open (`modal_key`)
    pub(crate) fn obj_pie_escape(&mut self) -> bool {
        if self.opie.take().is_some() {
            return true;
        }
        if self.clip_rename.take().is_some() {
            self.status = "Rename cancelled".into();
            return true;
        }
        false
    }

    fn obj_pie_pick(&mut self, on: PieFor, label: &str, child: Option<&str>) {
        match on {
            PieFor::Clip(id) => self.clip_pie_pick(id, label, child),
            PieFor::Launch(id) => self.launch_pie_pick(id, label, child),
            PieFor::Drum {
                clip,
                data,
                track,
                pad,
            } => self.drum_pie_pick(clip, data, track, pad, label, child),
        }
    }

    fn clip_pie_pick(&mut self, id: u32, label: &str, child: Option<&str>) {
        if !self.tlv.sels.contains(&id) {
            self.tlv.sels = vec![id];
            self.tlv.sel = Some(id);
        }
        match (label, child) {
            ("Edit", Some("Tracker")) => {
                let blk = self
                    .tlv
                    .clip_label(id)
                    .and_then(|(c, _)| c.block().map(|b| (b, c)));
                if let Some(((b, col), c)) = blk {
                    let row = (c.offset / tl::ROW) as usize;
                    self.timeline_open(b, col, row, usize::MAX);
                }
            }
            ("Edit", _) => {
                self.tlv.sel = Some(id);
                self.timeline_open_roll();
            }
            ("Split…", _) => self.tl_apply(TlOp::SplitArm),
            ("Duplicate", Some("Independent copy")) => {
                self.snapshot();
                self.undo_hold += 1;
                self.tl_apply(TlOp::Duplicate);
                self.tl_apply(TlOp::MakeUnique);
                self.undo_hold -= 1;
                self.status = "Independent copy placed after it (its own notes)".into();
            }
            ("Duplicate", Some(r)) if r.starts_with("Repeat") => {
                let n = if r.ends_with('4') { 3 } else { 1 };
                self.snapshot();
                self.undo_hold += 1;
                for _ in 0..n {
                    self.tl_apply(TlOp::Duplicate);
                }
                self.undo_hold -= 1;
                self.status = format!("Repeated: {} linked copies after it", n);
            }
            ("Duplicate", _) => self.tl_apply(TlOp::Duplicate),
            ("Make unique", _) => self.tl_apply(TlOp::MakeUnique),
            ("Colour", c) => self.set_clip_colour(&self.tlv.sels.clone(), c),
            ("Rename", _) => self.start_clip_rename(id),
            ("Mute", _) => self.tl_apply(TlOp::Mute),
            ("Delete", _) => self.tl_apply(TlOp::Delete),
            ("To launcher", _) => self.clip_to_launcher(id),
            ("Notes", Some("Glue")) => self.tl_apply(TlOp::Glue),
            ("Notes", Some("Loop around")) => self.tl_apply(TlOp::LoopFromSelection),
            ("Notes", Some(_)) => self.tl_apply(TlOp::UseTrackInstr),
            ("Notes", None) => self.status = "Notes: choose from the outer ring".into(),
            _ => {}
        }
    }

    /// track and scene of a launcher clip
    fn launch_where(&self, id: u32) -> Option<(usize, usize)> {
        self.with(|s| {
            let l = s.song.timeline.as_deref()?.launch_clip(id)?;
            Some((l.clip.track as usize, l.scene as usize))
        })
    }

    fn launch_pie_pick(&mut self, id: u32, label: &str, child: Option<&str>) {
        let Some((tr, k)) = self.launch_where(id) else {
            return;
        };
        match (label, child) {
            ("Play", _) => self.ca_slot(tr, k),
            ("Stop", _) => self.ca_stop(tr),
            ("Edit notes", _) => self.ca_open_launch(tr, k),
            ("Instrument", _) => {
                self.tl_select_track(tr, true);
                self.show_instrument_editor();
            }
            ("Duplicate (variation)", _) => self.ca_variation(id),
            ("Place in arrangement", w) => self.launch_place(id, w.unwrap_or("At the play marker")),
            ("Rename", _) => self.start_clip_rename(id),
            ("Colour", c) => self.set_clip_colour(&[id], c),
            ("Delete", _) => self.ca_delete(id),
            _ => {}
        }
    }

    /// Place in arrangement ›: the same plan and question as a drag (D5)
    pub(crate) fn launch_place(&mut self, id: u32, how: &str) {
        let Some((tr, _)) = self.launch_where(id) else {
            return;
        };
        let src = super::dropplan::DropSrc::Clip(id);
        let Some(info) = self.drop_src_info(src) else {
            return;
        };
        let cue = self.tlv.cue;
        let (start, bar) = self.with(|s| {
            let t = s.song.timeline.as_deref();
            let bar = t.map_or(4 * tl::PPQ, |t| t.sig_at(0).bar_ticks().max(1));
            let last = t.map_or(0, |t| {
                t.clips
                    .iter()
                    .filter(|c| c.track as usize == tr)
                    .map(|c| c.end())
                    .max()
                    .unwrap_or(0)
            });
            let lp = t.and_then(|t| t.loop_range).map(|r| r.0);
            let at = match how {
                "After the last clip" => last,
                "In the loop region" => lp.unwrap_or(cue),
                _ => cue,
            };
            (at, bar)
        });
        let start = (start + bar - 1) / bar * bar;
        let start = if how == "At the play marker" {
            cue / bar * bar
        } else {
            start
        };
        let (overlaps, layer_ok) = self.with(|s| {
            let Some(t) = s.song.timeline.as_deref() else {
                return (vec![], false);
            };
            let ov = tl::overlaps(t, tr, start, start + info.len, &[]);
            let blocks = t
                .clips
                .iter()
                .any(|c| ov.contains(&c.id) && c.block().is_some());
            (ov, !blocks && !info.block)
        });
        let plan = super::dropplan::DropPlan {
            track: tr,
            new_track: false,
            start,
            len: info.len,
            overlaps,
            layer_ok,
            bad: None,
            grid: bar,
            snap: "Bar",
        };
        self.drop_release(src, Some(plan), false);
    }

    /// a note clip's linked copy in the launcher: the track's first free scene
    pub(crate) fn clip_to_launcher(&mut self, id: u32) {
        let r = self.with(|s| {
            let t = s.song.timeline.as_deref_mut()?;
            let c = t.clips.iter().find(|c| c.id == id)?.clone();
            c.notes()?;
            let tr = c.track as usize;
            let k = (0..).find(|&k| tl::launch_at(t, tr, k).is_none())?;
            tl::launch_copy(t, id, tr, k, false)?;
            Some((tr, k))
        });
        match r {
            Some((tr, k)) => {
                self.dirty = true;
                self.status = format!(
                    "In the launcher: scene {} on track {:02}, linked to this clip (Clip Arranger, Ctrl+8)",
                    k + 1,
                    tr + 1
                );
            }
            None => self.status = "Only note clips go to the launcher".into(),
        }
    }

    /// clip colour (timeline or launcher clips); "Auto" = the track's
    pub(crate) fn set_clip_colour(&mut self, ids: &[u32], name: Option<&str>) {
        let col = name
            .and_then(|n| COLOURS.iter().find(|c| c.0 == n))
            .map(|c| c.1);
        let n = self.with(|s| {
            let Some(t) = s.song.timeline.as_deref_mut() else {
                return 0;
            };
            let mut n = 0;
            for c in t
                .clips
                .iter_mut()
                .chain(t.launcher.iter_mut().map(|l| &mut l.clip))
                .filter(|c| ids.contains(&c.id))
            {
                c.colour = col;
                n += 1;
            }
            n
        });
        if n > 0 {
            self.dirty = true;
            self.status = match name {
                Some(x) if col.is_some() => format!("Clip colour: {x}"),
                _ => "Clip colour: the track's (Auto)".into(),
            };
        }
    }

    pub(crate) fn start_clip_rename(&mut self, id: u32) {
        let name = self.with(|s| {
            let t = s.song.timeline.as_deref()?;
            let c = t.any_clip(id)?;
            Some(launch_name(t, c))
        });
        if let Some(text) = name {
            self.clip_rename = Some(ClipRename {
                id,
                text,
                focus: true,
            });
        }
    }

    /// set (or with "" clear) a clip's own name; linked clips keep theirs
    pub(crate) fn rename_clip(&mut self, id: u32, text: &str) {
        let v = text.trim();
        let ok = self.with(|s| {
            let Some(t) = s.song.timeline.as_deref_mut() else {
                return false;
            };
            match t
                .clips
                .iter_mut()
                .chain(t.launcher.iter_mut().map(|l| &mut l.clip))
                .find(|c| c.id == id)
            {
                Some(c) => {
                    c.name = (!v.is_empty()).then(|| v.to_string());
                    true
                }
                None => false,
            }
        });
        if ok {
            self.dirty = true;
            self.status = if v.is_empty() {
                "Clip name cleared: it shows its notes' name again".into()
            } else {
                format!("Clip renamed to {v} (linked copies keep their own names)")
            };
        }
    }

    fn clip_rename_window(&mut self, ctx: &egui::Context) {
        let Some(mut r) = self.clip_rename.take() else {
            return;
        };
        let (mut ok, mut cancel) = (false, false);
        egui::Window::new("Rename clip")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -80.0])
            .show(ctx, |ui| {
                ui.label(
                    "The clip's own name (empty: its notes' name). Linked copies keep theirs.",
                );
                let te = ui.add(egui::TextEdit::singleline(&mut r.text).desired_width(260.0));
                if r.focus {
                    te.request_focus();
                    r.focus = false;
                }
                if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    ok = true;
                }
                ui.horizontal(|ui| {
                    if ui.button("Rename").clicked() {
                        ok = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if ok {
            self.rename_clip(r.id, &r.text.clone());
        } else if !cancel {
            self.clip_rename = Some(r);
        }
    }

    fn drum_pie_pick(
        &mut self,
        clip: u32,
        data: usize,
        track: usize,
        pad: usize,
        label: &str,
        child: Option<&str>,
    ) {
        self.dk_pie(clip, data, track, pad, label, child);
    }
}

fn t_of(s: &Shared) -> &tl::Timeline {
    s.song.timeline.as_deref().expect("timeline")
}

/// a clip's shown name: its own, else its notes', else the kind
pub(crate) fn launch_name(t: &tl::Timeline, c: &tl::Clip) -> String {
    if let Some(n) = c.name.clone().filter(|n| !n.is_empty()) {
        return n;
    }
    match c.notes().and_then(|d| t.notes.get(d)) {
        Some(d) if !d.name.is_empty() => d.name.clone(),
        _ => "Clip".into(),
    }
}
