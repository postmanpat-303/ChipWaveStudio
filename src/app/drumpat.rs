//! Browser tabs Drum Patterns and Files (docs/BROWSER.md).
//!
//! Drum Patterns: folders per style, rows with a hover Play button, a step
//! thumbnail and a PATTERN tag; the preview (engine/patprev.rs) waits for the
//! next bar and locks to the song while it plays, and loops at the song tempo
//! while stopped. Dropping a pattern on the timeline makes a note clip on that
//! track, or a new drum track (with the pattern's kit) below the last one.
//!
//! Files: quick locations (Home, Music, Desktop, Downloads), a Drives menu
//! (src/drives.rs: local, removable and mapped network drives), back /
//! forward / up, a breadcrumb path bar you can type into (UNC paths work),
//! refresh, sort and favourite folders. Folders are read on a worker thread,
//! so a slow network share never freezes the window.
use super::browserpanel::{icon, well, Icon};
use super::browserview::DropAt;
use super::timeline::TlOp;
use super::*;
use crate::browser::{self as br, Fmt, Item};
use crate::drives;
use crate::drumpat::{self as dp, PATTERNS, STYLES};
use crate::timeline as tl;
use std::sync::mpsc;
use std::time::Duration;

fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

#[derive(Default)]
pub(crate) struct PatUi {
    /// the style folder shown (None = the list of styles)
    pub(crate) folder: Option<usize>,
    /// the pattern being previewed
    pub(crate) playing: Option<usize>,
    pub(crate) favs_only: bool,
    /// rows as last drawn (pattern index, rect), for tests
    pub(crate) rows: Vec<(usize, Rect)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum FSort {
    #[default]
    Name,
    Type,
    Size,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FEntry {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) dir: bool,
    pub(crate) fmt: Option<Fmt>,
    pub(crate) size: u64,
}

/// a background folder read
type FolderRx = mpsc::Receiver<Result<Vec<FEntry>, String>>;

#[derive(Default)]
pub(crate) struct FilesUi {
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) back: Vec<PathBuf>,
    pub(crate) fwd: Vec<PathBuf>,
    /// the path bar is being typed into
    pub(crate) edit: Option<String>,
    pub(crate) entries: Vec<FEntry>,
    pub(crate) error: Option<String>,
    job: Option<(PathBuf, FolderRx)>,
    pub(crate) drives: Vec<drives::Drive>,
    drives_job: Option<mpsc::Receiver<Vec<drives::Drive>>>,
    drives_asked: bool,
    pub(crate) sort: FSort,
    pub(crate) sel: Option<PathBuf>,
    pub(crate) elevated: Option<bool>,
}

impl FilesUi {
    pub(crate) fn loading(&self) -> bool {
        self.job.is_some()
    }
}

/// read a folder: sub-folders and the files ChipWave can load
pub(crate) fn read_folder(p: &std::path::Path) -> Result<Vec<FEntry>, String> {
    let rd = std::fs::read_dir(p).map_err(|e| e.to_string())?;
    let mut v = vec![];
    for e in rd.flatten().take(20_000) {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        // a symlinked folder still opens (metadata follows the link)
        let dir = ft.is_dir() || (ft.is_symlink() && path.is_dir());
        if dir {
            v.push(FEntry {
                name,
                path,
                dir: true,
                fmt: None,
                size: 0,
            });
        } else if let Some(fmt) = br::classify(&path) {
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            v.push(FEntry {
                name,
                path,
                dir: false,
                fmt: Some(fmt),
                size,
            });
        }
    }
    Ok(v)
}

fn sort_entries(v: &mut [FEntry], s: FSort) {
    v.sort_by(|a, b| {
        b.dir.cmp(&a.dir).then_with(|| match s {
            FSort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            FSort::Type => a
                .fmt
                .map(|f| f.badge())
                .cmp(&b.fmt.map(|f| f.badge()))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            FSort::Size => b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)),
        })
    });
}

/// a file of the Files tab as a Browser item (loads and drags like the others)
pub(crate) fn file_item(e: &FEntry) -> Option<Item> {
    let fmt = e.fmt?;
    let dir = e.path.parent()?.display().to_string();
    br::file_items(
        &dir,
        &[br::FileHit {
            path: e.path.display().to_string(),
            fmt,
            size: e.size,
        }],
    )
    .pop()
}

pub(crate) fn pattern_item(i: usize) -> Option<Item> {
    let mut v = br::pattern_items();
    (i < v.len()).then(|| v.swap_remove(i))
}

/// does pattern `p` match every word of `q`
fn pat_matches(p: &dp::Pat, q: &str) -> bool {
    let hay = format!(
        "{} {} {} {} {}",
        p.name,
        STYLES[p.style],
        p.kit,
        p.bpm_tag(),
        if p.bars == 1 { "1 bar" } else { "bars" }
    )
    .to_lowercase();
    q.split_whitespace()
        .all(|w| hay.contains(&w.to_lowercase()))
}

/// the step thumbnail: hats on top, snares / claps / percs, kick at the bottom
fn thumb(p: &egui::Painter, r: Rect, pat: &dp::Pat, col: Color32, dim: Color32) {
    let t = pat.thumb();
    let row = |set: &[char]| -> u16 {
        t.iter()
            .filter(|(c, _)| set.contains(c))
            .fold(0, |m, (_, b)| m | b)
    };
    let rows = [
        row(&['H', 'O', 'Z']),
        row(&['S', 'C', 'P', 'L', 'M', 'T']),
        row(&['K']),
    ];
    let cw = r.width() / 16.0;
    let rh = r.height() / 3.0;
    for (j, m) in rows.iter().enumerate() {
        for k in 0..16 {
            let on = m & (1 << k) != 0;
            let c = Rect::from_min_size(
                Pos2::new(r.left() + k as f32 * cw, r.top() + j as f32 * rh),
                egui::vec2((cw - 1.0).max(1.0), (rh - 1.0).max(1.0)),
            );
            p.rect_filled(c, 0, if on { col } else { dim });
        }
    }
}

/// the step-grid icon of a pattern row
fn grid_icon(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    for i in 0..3 {
        for j in 0..2 {
            let q = c + egui::vec2((i as f32 - 1.0) * s * 0.32, (j as f32 - 0.5) * s * 0.36);
            let on = (i + j) % 2 == 0;
            p.rect_filled(
                Rect::from_center_size(q, egui::vec2(s * 0.24, s * 0.24)),
                1,
                if on { col } else { col.gamma_multiply(0.45) },
            );
        }
    }
}

pub(super) fn folder_icon(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    let r = Rect::from_center_size(c + egui::vec2(0.0, s * 0.06), egui::vec2(s * 0.9, s * 0.62));
    p.rect_filled(
        Rect::from_min_size(
            r.min - egui::vec2(0.0, s * 0.12),
            egui::vec2(s * 0.4, s * 0.2),
        ),
        1,
        col,
    );
    p.rect_filled(r, 2, col);
}

/// < > ^ arrows for the navigation buttons
fn arrow(p: &egui::Painter, c: Pos2, s: f32, dir: u8, col: Color32) {
    let v = egui::vec2;
    let st = Stroke::new(1.8, col);
    let (a, b, d) = match dir {
        0 => (
            v(s * 0.15, -s * 0.3),
            v(-s * 0.15, 0.0),
            v(s * 0.15, s * 0.3),
        ),
        1 => (
            v(-s * 0.15, -s * 0.3),
            v(s * 0.15, 0.0),
            v(-s * 0.15, s * 0.3),
        ),
        _ => (
            v(-s * 0.3, s * 0.15),
            v(0.0, -s * 0.15),
            v(s * 0.3, s * 0.15),
        ),
    };
    p.line_segment([c + a, c + b], st);
    p.line_segment([c + b, c + d], st);
}

fn nav_button(ui: &mut egui::Ui, dir: u8, enabled: bool, tip: &str) -> egui::Response {
    let t = th();
    let (r, resp) = ui.allocate_exact_size(
        egui::vec2(24.0, 22.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let p = ui.painter();
    p.rect_filled(
        r,
        5,
        if resp.hovered() && enabled {
            well(&t, 1.5)
        } else {
            well(&t, 1.0)
        },
    );
    let col = if enabled {
        t.ink
    } else {
        t.ink2.gamma_multiply(0.4)
    };
    arrow(p, r.center(), 14.0, dir, col);
    resp.on_hover_text(tip)
}

fn pill(ui: &mut egui::Ui, label: &str, on: bool) -> egui::Response {
    let t = th();
    let b = egui::Button::new(egui::RichText::new(label).size(11.5).color(if on {
        t.ink
    } else {
        t.ink2
    }))
    .corner_radius(6)
    .fill(if on { t.sel } else { well(&t, 1.0) })
    .stroke(Stroke::new(1.0, if on { t.accent } else { t.border }));
    ui.add(b)
}

enum POp {
    Open(Option<usize>),
    Toggle(usize),
    Select(usize),
    Place(usize, Option<usize>),
    Drag(usize),
    Fav(String),
}

enum FOp {
    Go(PathBuf),
    Back,
    Fwd,
    Up,
    Refresh,
    Drives,
    Select(FEntry),
    Load(FEntry),
    Drag(FEntry),
    Preview(FEntry),
    FavDir(String),
    AddLocation(PathBuf),
}

impl App {
    // ------------------------------------------------------------ preview

    /// the bar length (ticks) at the play marker (4/4 for tracker songs)
    fn pat_bar(&self) -> tl::Tick {
        let cue = self.tlv.cue;
        self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .map(|t| t.sig_at(cue).bar_ticks())
                .unwrap_or(16 * tl::ROW)
        })
    }

    /// start (or, when it is playing, stop) the preview of pattern `i`
    pub(crate) fn pat_toggle(&mut self, i: usize) {
        if self.br.pat.playing == Some(i) {
            self.pat_stop();
            self.status = "Preview stopped".into();
            return;
        }
        let Some(p) = PATTERNS.get(i) else { return };
        let Some(patch) = dp::kit_patch(p.kit) else {
            return;
        };
        // an instrument preview stops the song: end it first
        if self.br.preview_until.take().is_some() {
            self.with(|s| {
                if s.preview.take().is_some() {
                    s.engine.stop();
                }
            });
        }
        let bar = self.pat_bar();
        let pp = crate::engine::patprev::PatPreview::new(
            patch,
            &p.events(),
            p.len(),
            bar,
            self.extra.pattern_vol,
        );
        let (playing, bpm) = self.with(|s| {
            drop(s.engine.set_pattern_preview(Some(Box::new(pp))));
            (s.engine.playing, s.song.bpm)
        });
        self.br.pat.playing = Some(i);
        self.br.sel = Some(p.key());
        self.status = if playing {
            format!(
                "Preview: {} ({}) starts on the next bar, in time with the song",
                p.name, p.kit
            )
        } else {
            format!(
                "Preview: {} ({}) looping at the song tempo, {:.0} BPM (Esc or click again stops)",
                p.name, p.kit, bpm
            )
        };
    }

    pub(crate) fn pat_stop(&mut self) {
        if self.br.pat.playing.take().is_some() {
            self.with(|s| drop(s.engine.set_pattern_preview(None)));
        }
    }

    /// (playing pattern, progress 0..1 or None while waiting for the bar)
    pub(crate) fn pat_progress(&self) -> Option<(usize, Option<f32>)> {
        let i = self.br.pat.playing?;
        let p = self.with(|s| s.engine.pattern_preview().map(|p| p.progress()));
        p.map(|x| (i, x))
    }

    // ------------------------------------------------------------ placing

    /// place pattern `i` on `track` (None = a new drum track) at `at` (None
    /// = the play marker), snapped to the bar; asks to switch a tracker-mode
    /// song to timeline mode first
    pub(crate) fn pat_place(&mut self, i: usize, track: Option<usize>, at: Option<tl::Tick>) {
        let at = at.unwrap_or(self.tlv.cue).max(0);
        let track = track.map(|t| t.min(254) as u8).unwrap_or(u8::MAX);
        self.tl_apply(TlOp::DrumPattern {
            pat: i as u16,
            track,
            at,
        });
    }

    pub(crate) fn pat_drop_hint(&self, i: usize, d: &DropAt) -> Option<String> {
        let p = PATTERNS.get(i)?;
        let n = self.with(|s| s.song.channels);
        Some(match d {
            DropAt::Track(tr, _) => format!("Note clip on track {:02}", tr + 1),
            DropAt::NewTrack(_) if n >= MAX_CHANNELS => "All 32 tracks are used".into(),
            DropAt::NewTrack(_) => format!("New drum track: {} ({})", p.name, p.kit),
            DropAt::Central | DropAt::Fx => "New drum track at the play marker".into(),
            DropAt::Slot(_) | DropAt::Device(_) => return None,
        })
    }

    pub(crate) fn pat_drop(&mut self, i: usize, d: DropAt) {
        match d {
            DropAt::Track(tr, at) => self.pat_place(i, Some(tr), at),
            DropAt::NewTrack(at) => self.pat_place(i, None, at),
            DropAt::Central | DropAt::Fx => self.pat_place(i, None, None),
            DropAt::Slot(_) | DropAt::Device(_) => {}
        }
    }

    /// (timeline mode) the clip and, for a new track, the track and its kit:
    /// one undo step
    pub(crate) fn pat_place_now(&mut self, i: usize, track: u8, at: tl::Tick) {
        let Some(p) = PATTERNS.get(i) else { return };
        let new = track == u8::MAX;
        let n = self.with(|s| s.song.channels);
        if new && n >= MAX_CHANNELS {
            self.status =
                "All 32 tracks are used (the channel limit): drop onto a track instead".into();
            return;
        }
        // an existing track that already plays a kit keeps it
        let track_kit = if new {
            None
        } else {
            let slot = self.track_slot(track as usize);
            self.with(|s| {
                s.song
                    .instruments
                    .get((slot as usize).wrapping_sub(1))
                    .is_some_and(is_kit)
                    .then_some(slot)
            })
        };
        self.snapshot();
        self.undo_hold += 1;
        let tr = if new {
            if !self.track_insert(n, p.kit) {
                self.undo_hold -= 1;
                return;
            }
            n
        } else {
            track as usize
        };
        let slot = match track_kit {
            Some(s) => Some(s),
            None => self.with(|s| {
                // the same kit already in the song is shared
                if let Some(k) = s
                    .song
                    .instruments
                    .iter()
                    .position(|x| x.name == p.kit && is_kit(x))
                {
                    return Some(k as u8 + 1);
                }
                let inst = dp::kit_instrument(p.kit)?;
                s.song.add_instrument(inst).map(|k| k as u8 + 1)
            }),
        };
        let Some(slot) = slot else {
            self.undo_hold -= 1;
            self.status = "All 255 instrument slots are used".into();
            return;
        };
        let res = self.with(|s| {
            let song = &mut s.song;
            if new {
                crate::chanops::assign_track_instrument(song, tr, slot);
            }
            let t = song.timeline.as_deref()?;
            let sig = t.sig_at(at);
            let bar = sig.bar_ticks().max(1);
            let from = if at >= sig.tick { sig.tick } else { 0 };
            let start = from + (at - from).max(0) / bar * bar;
            let id = tl::new_note_clip(song, tr, start)?;
            let t = song.timeline.as_deref_mut()?;
            let k = t.clips.iter().position(|c| c.id == id)?;
            if let tl::ClipKind::Notes { data } = t.clips[k].kind {
                let d = t.notes[data as usize].make_mut();
                d.name = p.name.to_string();
                d.notes = p.notes(slot);
                d.len = p.len();
            }
            t.clips[k].len = p.len();
            let pos = t.bar_pos(start);
            let crate::audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some((id, pos.bar))
        });
        self.undo_hold -= 1;
        self.dirty = true;
        match res {
            Some((id, bar)) => {
                self.tlv.sels = vec![id];
                self.tlv.sel = Some(id);
                self.ch = tr;
                self.instr = slot;
                self.status = if new {
                    format!(
                        "New drum track {:02} {}: {} at bar {bar} (follows the song tempo)",
                        tr + 1,
                        p.kit,
                        p.name
                    )
                } else {
                    format!("{} on track {:02} at bar {bar}", p.name, tr + 1)
                };
            }
            None => self.status = "No track there".into(),
        }
    }

    // ------------------------------------------------------------ Drum Patterns tab

    pub(super) fn patterns_body(&mut self, ui: &mut egui::Ui) {
        let t = th();
        let mut ops: Vec<POp> = vec![];
        let q = self.br.filter.query.trim().to_string();
        let favs = self.extra.browser_favs.clone();
        let favs_only = self.br.pat.favs_only;
        let flat = !q.is_empty() || favs_only;
        // ---- navigation: up, breadcrumb, favourites
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let inside = self.br.pat.folder.is_some() && !flat;
            if nav_button(ui, 2, inside, "up: all styles").clicked() {
                ops.push(POp::Open(None));
            }
            let crumb = |ui: &mut egui::Ui, s: &str, last: bool| {
                ui.add(
                    egui::Label::new(egui::RichText::new(s).size(12.0).color(if last {
                        t.ink
                    } else {
                        t.ink2
                    }))
                    .truncate()
                    .sense(Sense::click()),
                )
            };
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if pill(ui, "Favourites", favs_only)
                    .on_hover_text("only your favourite patterns (click a row's heart)")
                    .clicked()
                {
                    self.br.pat.favs_only = !favs_only;
                }
                // the crumbs take what is left, truncated with "…" when narrow
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if crumb(ui, "Drum Patterns", !inside).clicked() {
                        ops.push(POp::Open(None));
                    }
                    if flat {
                        ui.label(egui::RichText::new("›").color(t.ink2));
                        crumb(ui, if q.is_empty() { "Favourites" } else { "Search" }, true);
                    } else if let Some(f) = self.br.pat.folder {
                        ui.label(egui::RichText::new("›").color(t.ink2));
                        crumb(ui, STYLES[f], true);
                    }
                });
            });
        });
        // ---- the list
        let bar_h = 58.0;
        let list_h = (ui.available_height() - bar_h).max(80.0);
        let prog = self.pat_progress();
        let sel = self.br.sel.clone();
        let mut rows: Vec<(usize, Rect)> = vec![];
        egui::ScrollArea::vertical()
            .id_salt("br_patterns")
            .max_height(list_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let list: Vec<usize> = if flat {
                    (0..PATTERNS.len())
                        .filter(|&i| {
                            let p = &PATTERNS[i];
                            (q.is_empty() || pat_matches(p, &q))
                                && (!favs_only || favs.contains(&p.key()))
                        })
                        .collect()
                } else if let Some(f) = self.br.pat.folder {
                    dp::in_style(f).map(|(i, _)| i).collect()
                } else {
                    // the style folders
                    for (s, name) in STYLES.iter().enumerate() {
                        let n = dp::in_style(s).count();
                        let w = ui.available_width();
                        let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 30.0), Sense::click());
                        let p = ui.painter();
                        p.rect_filled(r, 6, if resp.hovered() { well(&t, 1.4) } else { well(&t, 0.8) });
                        folder_icon(p, Pos2::new(r.left() + 18.0, r.center().y), 16.0, rgb(Fmt::Pattern.colour()));
                        p.text(
                            Pos2::new(r.left() + 36.0, r.center().y),
                            egui::Align2::LEFT_CENTER,
                            *name,
                            FontId::proportional(12.5),
                            t.ink,
                        );
                        p.text(
                            Pos2::new(r.right() - 10.0, r.center().y),
                            egui::Align2::RIGHT_CENTER,
                            format!("{n} patterns"),
                            FontId::proportional(10.5),
                            t.ink2,
                        );
                        if resp.clicked() {
                            ops.push(POp::Open(Some(s)));
                        }
                    }
                    vec![]
                };
                if flat && list.is_empty() {
                    ui.label(
                        egui::RichText::new(if favs_only && q.is_empty() {
                            "No favourite patterns yet: click a pattern's heart"
                        } else {
                            "No patterns match: try a style, kit or tempo (\"techno\", \"lo-fi\", \"140\")"
                        })
                        .small()
                        .color(t.ink2),
                    );
                }
                for i in list {
                    let p = &PATTERNS[i];
                    let key = p.key();
                    let w = ui.available_width();
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 36.0), Sense::click_and_drag());
                    rows.push((i, r.intersect(ui.clip_rect())));
                    let pt = ui.painter();
                    let playing = prog.map(|x| x.0) == Some(i);
                    let on = sel.as_deref() == Some(key.as_str());
                    let accent = rgb(Fmt::Pattern.colour());
                    let bg = if playing || on {
                        t.sel
                    } else if resp.hovered() {
                        well(&t, 1.4)
                    } else {
                        well(&t, 0.8)
                    };
                    pt.rect_filled(r, 6, bg);
                    if playing {
                        pt.rect_filled(
                            Rect::from_min_size(r.min + egui::vec2(0.0, 4.0), egui::vec2(3.0, r.height() - 8.0)),
                            1,
                            t.accent,
                        );
                    }
                    // play button / step icon
                    let ib = Rect::from_center_size(Pos2::new(r.left() + 20.0, r.center().y), egui::vec2(24.0, 24.0));
                    let hover_ic = resp.hover_pos().is_some_and(|h| ib.contains(h));
                    if playing {
                        pt.rect_filled(ib, 5, t.accent.gamma_multiply(0.25));
                        icon(pt, Icon::Stop, ib.center(), 13.0, t.accent);
                    } else if resp.hovered() {
                        pt.circle_filled(ib.center(), 11.0, if hover_ic { well(&t, 2.2) } else { well(&t, 1.7) });
                        icon(pt, Icon::Play, ib.center() + egui::vec2(1.0, 0.0), 13.0, t.ink);
                    } else {
                        grid_icon(pt, ib.center(), 16.0, accent);
                    }
                    // right side: PATTERN tag, heart, thumbnail
                    // (a narrow panel drops the PATTERN tag to keep room for the name)
                    let tag_w = if w < 280.0 { 8.0 } else { 64.0 };
                    let tag_r = Rect::from_min_max(Pos2::new(r.right() - tag_w, r.top()), Pos2::new(r.right() - 6.0, r.bottom()));
                    if w >= 280.0 {
                        pt.text(tag_r.right_center(), egui::Align2::RIGHT_CENTER, "PATTERN", FontId::proportional(10.0), t.ink2);
                    }
                    let heart = Rect::from_center_size(Pos2::new(tag_r.left() - 10.0, r.center().y), egui::vec2(16.0, 16.0));
                    let fav = favs.contains(&key);
                    let gold = Color32::from_rgb(0xE0, 0xC4, 0x6C);
                    if fav || resp.hovered() {
                        icon(pt, Icon::Heart, heart.center(), 12.0, if fav { gold } else { t.ink2.gamma_multiply(0.4) });
                    }
                    let tw = 64.0f32.min((w - 200.0).max(0.0));
                    let th_r = Rect::from_min_size(Pos2::new(heart.left() - 6.0 - tw, r.center().y - 7.0), egui::vec2(tw, 14.0));
                    if tw > 20.0 {
                        thumb(pt, th_r, p, if playing { t.accent } else { accent.gamma_multiply(0.9) }, well(&t, 1.8));
                    }
                    // name and details
                    let text_r = Rect::from_min_max(Pos2::new(r.left() + 40.0, r.top()), Pos2::new(th_r.left() - 6.0, r.bottom()));
                    let tp = pt.with_clip_rect(text_r);
                    tp.text(
                        Pos2::new(text_r.left(), r.top() + 11.0),
                        egui::Align2::LEFT_CENTER,
                        super::pattern::fit(&tp, p.name, &FontId::proportional(12.5), text_r.width()),
                        FontId::proportional(12.5),
                        if playing { t.accent } else { t.ink },
                    );
                    let detail = if flat {
                        format!("{} · {} · {}", STYLES[p.style], p.kit.trim_end_matches(" Kit"), p.bpm_tag())
                    } else {
                        format!("{} · {} bar{} · {}", p.kit.trim_end_matches(" Kit"), p.bars, if p.bars == 1 { "" } else { "s" }, p.bpm_tag())
                    };
                    let detail = super::pattern::fit(&tp, &detail, &FontId::proportional(10.5), text_r.width());
                    tp.text(Pos2::new(text_r.left(), r.top() + 25.0), egui::Align2::LEFT_CENTER, detail, FontId::proportional(10.5), t.ink2);
                    if playing {
                        // progress underline under the name
                        let x0 = text_r.left();
                        let full = (text_r.width()).min(160.0);
                        let fr = prog.and_then(|x| x.1).unwrap_or(0.0);
                        pt.rect_filled(Rect::from_min_size(Pos2::new(x0, r.bottom() - 4.0), egui::vec2(full, 2.0)), 1, t.accent.gamma_multiply(0.25));
                        pt.rect_filled(Rect::from_min_size(Pos2::new(x0, r.bottom() - 4.0), egui::vec2(full * fr, 2.0)), 1, t.accent);
                    }
                    // input
                    if resp.clicked() {
                        let at = resp.interact_pointer_pos().unwrap_or(r.center());
                        if ib.expand(2.0).contains(at) {
                            ops.push(POp::Toggle(i));
                        } else if heart.expand(2.0).contains(at) {
                            ops.push(POp::Fav(key.clone()));
                        } else {
                            ops.push(POp::Select(i));
                        }
                    }
                    if resp.double_clicked() {
                        ops.push(POp::Place(i, None));
                    }
                    if resp.drag_started() {
                        ops.push(POp::Drag(i));
                    }
                    let resp = if hover_ic {
                        resp.on_hover_text(if playing { "Stop" } else { "Play" })
                    } else {
                        resp.on_hover_text(format!(
                            "{} · {} · {} hits\nkit: {}\nDrag onto the timeline, or double-click for a new drum track",
                            STYLES[p.style],
                            p.bpm_tag(),
                            p.hits(),
                            dp::kit_desc(p.kit)
                        ))
                    };
                    let ch = self.ch;
                    resp.context_menu(|ui| {
                        ui.label(egui::RichText::new(p.name).strong());
                        if ui.button(if playing { "Stop preview" } else { "Preview" }).clicked() {
                            ops.push(POp::Toggle(i));
                            ui.close();
                        }
                        if ui.button("New drum track at the play marker").clicked() {
                            ops.push(POp::Place(i, None));
                            ui.close();
                        }
                        if ui.button(format!("Place on track {:02} at the play marker", ch + 1)).clicked() {
                            ops.push(POp::Place(i, Some(ch)));
                            ui.close();
                        }
                        ui.separator();
                        if ui.button(if fav { "Remove from favourites" } else { "Add to favourites" }).clicked() {
                            ops.push(POp::Fav(key.clone()));
                            ui.close();
                        }
                    });
                }
            });
        self.br.pat.rows = rows;
        // ---- preview bar
        let (bar_r, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), bar_h - 6.0),
            Sense::hover(),
        );
        ui.painter().rect_filled(bar_r, 6, well(&t, 0.9));
        let selected = self
            .br
            .sel
            .as_ref()
            .and_then(|k| PATTERNS.iter().position(|p| &p.key() == k));
        let playing = self.br.pat.playing;
        let (tp, waiting, bpm) = self.with(|s| {
            (
                s.engine.playing,
                s.engine.pattern_preview().is_some_and(|p| p.waiting()),
                s.song.bpm,
            )
        });
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(bar_r.shrink2(egui::vec2(8.0, 4.0))),
            |ui| {
                ui.horizontal(|ui| {
                    let target = playing.or(selected);
                    let r = ui.add_enabled(
                        target.is_some(),
                        egui::Button::new(if playing.is_some() { "Stop" } else { "Play" })
                            .min_size(egui::vec2(64.0, 20.0)),
                    );
                    if r.clicked() {
                        if let Some(i) = target {
                            ops.push(POp::Toggle(i));
                        }
                    }
                    let txt = match (playing, tp) {
                        (Some(_), true) if waiting => "waiting for the next bar…".to_string(),
                        (Some(_), true) => "locked to the song".to_string(),
                        (Some(_), false) => format!("looping at {bpm:.0} BPM"),
                        (None, true) => "starts on the next bar, in time".into(),
                        (None, false) => format!("loops at the song tempo ({bpm:.0} BPM)"),
                    };
                    ui.add(
                        egui::Label::new(egui::RichText::new(&txt).size(11.0).color(t.ink))
                            .truncate(),
                    )
                    .on_hover_text(&txt);
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Volume").size(11.0).color(t.ink2));
                    let mut v = self.extra.pattern_vol;
                    // the slider gives way to "Auto" when the panel is narrow
                    ui.spacing_mut().slider_width =
                        (ui.available_width() - 70.0).clamp(40.0, 100.0);
                    let r = ui.add(egui::Slider::new(&mut v, 0.0..=1.5).show_value(false));
                    if r.changed() {
                        self.extra.pattern_vol = v;
                        self.with(|s| {
                            if let Some(p) = s.engine.pattern_preview_mut() {
                                p.vol = v;
                            }
                        });
                    }
                    if r.drag_stopped() || (r.changed() && !r.dragged()) {
                        self.save_settings();
                    }
                    ui.checkbox(
                        &mut self.extra.browser_autoplay,
                        egui::RichText::new("Auto").size(11.0),
                    )
                    .on_hover_text("preview each pattern when it's selected");
                });
            },
        );
        if playing.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }
        // ---- apply
        for op in ops {
            match op {
                POp::Open(f) => self.br.pat.folder = f,
                POp::Toggle(i) => self.pat_toggle(i),
                POp::Select(i) => {
                    let key = PATTERNS[i].key();
                    let again = self.br.sel.as_deref() == Some(key.as_str());
                    self.br.sel = Some(key);
                    if self.extra.browser_autoplay && !again && self.br.pat.playing != Some(i) {
                        self.pat_toggle(i);
                    }
                }
                POp::Place(i, tr) => {
                    self.pat_stop();
                    self.pat_place(i, tr, None);
                }
                POp::Drag(i) => {
                    self.br.sel = Some(PATTERNS[i].key());
                    self.br.drag = pattern_item(i);
                }
                POp::Fav(k) => self.br_toggle_fav(&k),
            }
        }
    }

    // ------------------------------------------------------------ Files tab

    /// open folder `p` in the Files tab (read on a worker thread)
    pub(crate) fn files_go(&mut self, p: PathBuf, push: bool) {
        if push {
            if let Some(c) = self.br.files.cwd.clone() {
                if c != p {
                    self.br.files.back.push(c);
                    self.br.files.fwd.clear();
                }
            }
        }
        let (tx, rx) = mpsc::channel();
        let q = p.clone();
        let _ = std::thread::Builder::new()
            .name("files-read".into())
            .spawn(move || {
                let _ = tx.send(read_folder(&q));
            });
        self.br.files.cwd = Some(p.clone());
        self.br.files.edit = None;
        self.br.files.job = Some((p.clone(), rx));
        self.extra.files_dir = p.display().to_string();
    }

    /// drives, enumerated and probed in the background (never blocks)
    pub(crate) fn files_drives(&mut self) {
        let (tx, rx) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("drives".into())
            .spawn(move || {
                let d = drives::list();
                let _ = tx.send(d.clone());
                let _ = tx.send(drives::probe(d, Duration::from_millis(2500)));
            });
        self.br.files.drives_job = Some(rx);
        self.br.files.drives_asked = true;
    }

    pub(crate) fn files_poll(&mut self, ctx: &egui::Context) {
        let f = &mut self.br.files;
        if let Some((p, rx)) = &f.job {
            match rx.try_recv() {
                Ok(r) => {
                    let p = p.clone();
                    f.job = None;
                    match r {
                        Ok(mut v) => {
                            sort_entries(&mut v, f.sort);
                            f.entries = v;
                            f.error = None;
                        }
                        Err(e) => {
                            f.entries.clear();
                            f.error = Some(format!("Can't open {}: {e}", p.display()));
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(50))
                }
                Err(_) => f.job = None,
            }
        }
        if let Some(rx) = &f.drives_job {
            loop {
                match rx.try_recv() {
                    Ok(d) => f.drives = d,
                    Err(mpsc::TryRecvError::Empty) => {
                        ctx.request_repaint_after(Duration::from_millis(100));
                        break;
                    }
                    Err(_) => {
                        f.drives_job = None;
                        break;
                    }
                }
            }
        }
    }

    pub(super) fn files_body(&mut self, ui: &mut egui::Ui) {
        let t = th();
        let ctx = ui.ctx().clone();
        if self.br.files.cwd.is_none() && self.br.files.job.is_none() {
            let start = Some(PathBuf::from(&self.extra.files_dir))
                .filter(|p| !self.extra.files_dir.is_empty() && p.is_dir())
                .or_else(drives::home)
                .unwrap_or_else(|| PathBuf::from("/"));
            self.files_go(start, false);
        }
        if !self.br.files.drives_asked {
            self.files_drives();
        }
        if self.br.files.elevated.is_none() {
            self.br.files.elevated = Some(drives::elevated());
        }
        self.files_poll(&ctx);
        let mut ops: Vec<FOp> = vec![];
        let cwd = self.br.files.cwd.clone().unwrap_or_default();
        // ---- quick locations + drives
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for (name, p) in drives::quick() {
                if pill(ui, name, cwd == p).on_hover_text(p.display().to_string()).clicked() {
                    ops.push(FOp::Go(p));
                }
            }
            let drives = self.br.files.drives.clone();
            let elevated = self.br.files.elevated == Some(true);
            let mr = ui.menu_button(egui::RichText::new("Drives").size(11.5), |ui| {
                ui.set_min_width(220.0);
                if drives.is_empty() {
                    ui.label(egui::RichText::new("Looking for drives…").small());
                }
                for d in &drives {
                    let state = match d.ok {
                        None => " · checking…",
                        Some(false) if d.kind == drives::Kind::Network => " · not connected / not responding",
                        Some(false) => " · not ready",
                        Some(true) => "",
                    };
                    let tag = if d.kind == drives::Kind::Network { "NET  " } else { "" };
                    let txt = format!("{tag}{}{state}", d.title());
                    let b = ui.add_enabled(
                        d.ok != Some(false) || d.kind == drives::Kind::Network,
                        egui::Button::new(egui::RichText::new(txt).size(11.5)),
                    );
                    if b.on_hover_text(d.kind.name()).clicked() {
                        ops.push(FOp::Go(PathBuf::from(&d.root)));
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Refresh drives").clicked() {
                    ops.push(FOp::Drives);
                }
                if elevated {
                    ui.label(
                        egui::RichText::new(
                            "ChipWave is running as administrator, so Windows hides your mapped \
                             network drives from it. Start ChipWave normally (Start menu) to see them, \
                             or type the \\\\server\\share path into the path bar.",
                        )
                        .small()
                        .color(t.ink2),
                    );
                }
            });
            let _ = mr.response.on_hover_text("local, removable and network drives");
        });
        // ---- back / forward / up, path bar, refresh, sort
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            let f = &self.br.files;
            if nav_button(ui, 0, !f.back.is_empty(), "back").clicked() {
                ops.push(FOp::Back);
            }
            if nav_button(ui, 1, !f.fwd.is_empty(), "forward").clicked() {
                ops.push(FOp::Fwd);
            }
            if nav_button(ui, 2, cwd.parent().is_some(), "up one folder").clicked() {
                ops.push(FOp::Up);
            }
            // leave room for Refresh and Sort: a bar wider than the space
            // left would widen the panel, and then itself, every frame
            let sort_w = ui.fonts_mut(|f| {
                f.layout_no_wrap("Sort".into(), FontId::proportional(12.5), t.ink).size().x
            });
            let reserve = 24.0 + sort_w + 2.0 * ui.spacing().button_padding.x + 3.0 * 3.0 + 10.0;
            let w = (ui.available_width() - reserve).max(30.0);
            if let Some(mut s) = self.br.files.edit.clone() {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut s)
                        .id_salt("files_path")
                        .desired_width(w - 2.0 * ui.spacing().button_padding.x)
                        .hint_text("C:\\Music, Z:\\, \\\\server\\share, ~/Music"),
                );
                r.request_focus();
                let enter = ui.input(|i| i.key_pressed(Key::Enter));
                let esc = ui.input(|i| i.key_pressed(Key::Escape));
                if enter {
                    ops.push(FOp::Go(drives::parse_typed(&s)));
                    self.br.files.edit = None;
                } else if esc || r.lost_focus() {
                    self.br.files.edit = None;
                } else {
                    self.br.files.edit = Some(s);
                }
            } else {
                // breadcrumb: the last folders, clickable; click the bar to type
                let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), Sense::click());
                let p = ui.painter();
                p.rect_filled(r, 6, if resp.hovered() { well(&t, 1.4) } else { well(&t, 1.0) });
                let comps: Vec<PathBuf> = cwd.ancestors().map(|a| a.to_path_buf()).collect();
                let label = |q: &std::path::Path| {
                    q.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| q.display().to_string())
                };
                let mut x = r.right() - 6.0;
                let mut hits: Vec<(Rect, PathBuf)> = vec![];
                for (k, a) in comps.iter().enumerate() {
                    let txt = label(a);
                    let g = p.layout_no_wrap(txt, FontId::proportional(11.5), if k == 0 { t.ink } else { t.ink2 });
                    let wd = g.size().x;
                    if x - wd < r.left() + 26.0 && k > 0 {
                        p.text(Pos2::new(r.left() + 6.0, r.center().y), egui::Align2::LEFT_CENTER, "…  ›", FontId::proportional(11.5), t.ink2);
                        break;
                    }
                    x -= wd;
                    let gr = Rect::from_min_size(Pos2::new(x, r.center().y - g.size().y / 2.0), g.size());
                    p.galley(gr.min, g, t.ink);
                    hits.push((gr.expand(2.0), a.clone()));
                    if k + 1 < comps.len() {
                        x -= 14.0;
                        p.text(Pos2::new(x + 7.0, r.center().y), egui::Align2::CENTER_CENTER, "›", FontId::proportional(11.5), t.ink2);
                    }
                }
                if resp.clicked() {
                    let at = resp.interact_pointer_pos().unwrap_or(r.center());
                    match hits.iter().find(|h| h.0.contains(at)) {
                        Some((_, a)) if *a != cwd => ops.push(FOp::Go(a.clone())),
                        _ => self.br.files.edit = Some(cwd.display().to_string()),
                    }
                }
                resp.on_hover_text("click a folder to go there, or the bar to type a path (\\\\server\\share works)");
            }
            if super::browserpanel::icon_button(ui, Icon::Rescan, false, true, "refresh").clicked() {
                ops.push(FOp::Refresh);
            }
            let s = self.br.files.sort;
            ui.menu_button("Sort", |ui| {
                for (v, n) in [(FSort::Name, "Name"), (FSort::Type, "Type"), (FSort::Size, "Size")] {
                    if ui.selectable_label(s == v, n).clicked() {
                        self.br.files.sort = v;
                        sort_entries(&mut self.br.files.entries, v);
                        ui.close();
                    }
                }
            });
        });
        // ---- favourite folders, add to locations
        let fav_dirs: Vec<String> = self
            .extra
            .browser_favs
            .iter()
            .filter_map(|k| k.strip_prefix("dir:").map(|s| s.to_string()))
            .collect();
        let here = cwd.display().to_string();
        let here_fav = fav_dirs.contains(&here);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            ui.menu_button(egui::RichText::new("Favourites").size(11.5), |ui| {
                if fav_dirs.is_empty() {
                    ui.label(egui::RichText::new("No favourite folders yet").small());
                }
                for d in &fav_dirs {
                    if ui.button(d).clicked() {
                        ops.push(FOp::Go(PathBuf::from(d)));
                        ui.close();
                    }
                }
            });
            if pill(
                ui,
                if here_fav {
                    "Favourite folder"
                } else {
                    "Add to favourites"
                },
                here_fav,
            )
            .clicked()
            {
                ops.push(FOp::FavDir(here.clone()));
            }
            let is_loc = self.extra.browser_dirs.contains(&here);
            if ui
                .add_enabled(
                    !is_loc,
                    egui::Button::new(
                        egui::RichText::new(if is_loc {
                            "In Locations"
                        } else {
                            "+ Locations"
                        })
                        .size(11.5),
                    ),
                )
                .on_hover_text(
                    "add this folder to the Browser's locations (its files show in the other tabs)",
                )
                .clicked()
            {
                ops.push(FOp::AddLocation(cwd.clone()));
            }
            if self.br.files.loading() {
                ui.spinner();
            }
        });
        // ---- entries
        let list_h = (ui.available_height() - 4.0).max(60.0);
        let entries = &self.br.files.entries;
        let sel = self.br.files.sel.clone();
        let row_h = 26.0;
        if let Some(e) = &self.br.files.error {
            ui.label(egui::RichText::new(e).small().color(t.ink2));
        } else if entries.is_empty() && !self.br.files.loading() {
            ui.label(
                egui::RichText::new("No folders or loadable files here")
                    .small()
                    .color(t.ink2),
            );
        }
        egui::ScrollArea::vertical()
            .id_salt("br_files")
            .max_height(list_h)
            .auto_shrink([false, false])
            .show_rows(ui, row_h, entries.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for k in range {
                    let e = &entries[k];
                    let w = ui.available_width();
                    let (r, resp) =
                        ui.allocate_exact_size(egui::vec2(w, row_h), Sense::click_and_drag());
                    let p = ui.painter();
                    let on = sel.as_ref() == Some(&e.path);
                    if on {
                        p.rect_filled(r, 4, t.sel);
                    } else if resp.hovered() {
                        p.rect_filled(r, 4, well(&t, 1.4));
                    }
                    if e.dir {
                        folder_icon(p, Pos2::new(r.left() + 16.0, r.center().y), 15.0, t.ink2);
                    } else if let Some(f) = e.fmt {
                        let br_ = Rect::from_center_size(
                            Pos2::new(r.left() + 18.0, r.center().y),
                            egui::vec2(30.0, 14.0),
                        );
                        p.rect_filled(br_, 3, rgb(f.colour()));
                        p.text(
                            br_.center(),
                            egui::Align2::CENTER_CENTER,
                            f.badge(),
                            FontId::proportional(9.0),
                            Color32::from_rgb(0x15, 0x17, 0x1a),
                        );
                    }
                    let right = if e.dir {
                        String::new()
                    } else if e.size >= 1 << 20 {
                        format!("{:.1} MB", e.size as f64 / (1u64 << 20) as f64)
                    } else {
                        format!("{} kB", (e.size / 1024).max(1))
                    };
                    let tr = Rect::from_min_max(
                        Pos2::new(r.left() + 40.0, r.top()),
                        Pos2::new(r.right() - 60.0, r.bottom()),
                    );
                    p.with_clip_rect(tr).text(
                        Pos2::new(tr.left(), r.center().y),
                        egui::Align2::LEFT_CENTER,
                        &e.name,
                        FontId::proportional(12.0),
                        t.ink,
                    );
                    p.text(
                        Pos2::new(r.right() - 6.0, r.center().y),
                        egui::Align2::RIGHT_CENTER,
                        right,
                        FontId::proportional(10.0),
                        t.ink2,
                    );
                    if resp.clicked() {
                        ops.push(FOp::Select(e.clone()));
                    }
                    if resp.double_clicked() {
                        ops.push(if e.dir {
                            FOp::Go(e.path.clone())
                        } else {
                            FOp::Load(e.clone())
                        });
                    }
                    if resp.drag_started() && !e.dir {
                        ops.push(FOp::Drag(e.clone()));
                    }
                    let resp = resp.on_hover_text(e.path.display().to_string());
                    resp.context_menu(|ui| {
                        if e.dir {
                            if ui.button("Open").clicked() {
                                ops.push(FOp::Go(e.path.clone()));
                                ui.close();
                            }
                            if ui.button("Add to Locations").clicked() {
                                ops.push(FOp::AddLocation(e.path.clone()));
                                ui.close();
                            }
                            if ui.button("Add to favourite folders").clicked() {
                                ops.push(FOp::FavDir(e.path.display().to_string()));
                                ui.close();
                            }
                        } else {
                            if ui.button("Load").clicked() {
                                ops.push(FOp::Load(e.clone()));
                                ui.close();
                            }
                            if e.fmt.is_some_and(|f| f.is_sample() || f == Fmt::Cwi)
                                && ui.button("Preview").clicked()
                            {
                                ops.push(FOp::Preview(e.clone()));
                                ui.close();
                            }
                        }
                        if ui.button("Copy path").clicked() {
                            ui.ctx().copy_text(e.path.display().to_string());
                            ui.close();
                        }
                    });
                }
            });
        for op in ops {
            match op {
                FOp::Go(p) => self.files_go(p, true),
                FOp::Back => {
                    if let Some(p) = self.br.files.back.pop() {
                        if let Some(c) = self.br.files.cwd.clone() {
                            self.br.files.fwd.push(c);
                        }
                        self.files_go(p, false);
                    }
                }
                FOp::Fwd => {
                    if let Some(p) = self.br.files.fwd.pop() {
                        if let Some(c) = self.br.files.cwd.clone() {
                            self.br.files.back.push(c);
                        }
                        self.files_go(p, false);
                    }
                }
                FOp::Up => {
                    if let Some(p) = cwd.parent() {
                        self.files_go(p.to_path_buf(), true);
                    }
                }
                FOp::Refresh => self.files_go(cwd.clone(), false),
                FOp::Drives => self.files_drives(),
                FOp::Select(e) => {
                    let again = self.br.files.sel.as_ref() == Some(&e.path);
                    self.br.files.sel = Some(e.path.clone());
                    if !e.dir && !again && self.extra.browser_autoplay {
                        if let Some(it) = file_item(&e).filter(|i| i.previewable()) {
                            self.br_preview(&it);
                        }
                    }
                }
                FOp::Load(e) => {
                    if let Some(it) = file_item(&e) {
                        self.br_load(&it, &ctx);
                    }
                }
                FOp::Preview(e) => {
                    if let Some(it) = file_item(&e) {
                        self.br_preview(&it);
                    }
                }
                FOp::Drag(e) => {
                    self.br.files.sel = Some(e.path.clone());
                    self.br.drag = file_item(&e);
                }
                FOp::FavDir(d) => self.br_toggle_fav(&format!("dir:{d}")),
                FOp::AddLocation(p) => self.br_add_folder(p),
            }
        }
    }
}

fn is_kit(i: &Instrument) -> bool {
    matches!(&i.kind, InstrumentKind::Gen(g) if matches!(g.engine, crate::gen::GenEngine::Kit(_)))
}
