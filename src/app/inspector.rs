//! The left Inspector (0.2.4 "inspired" layout, Del's pick E): a
//! collapsible panel that shows what is selected. A track (or the pattern
//! cursor channel): Track (colour, name, volume, pan, mute / solo), Sends
//! (delay / reverb returns, sidechain duck) and Instrument (the slot it
//! plays, Edit / Browse). A timeline clip: its own section first (name,
//! where it sits, mute; audio clips: gain, fades and their shape) with
//! Piano roll / Pattern editor buttons. Every control binds to a field the
//! song already has, so nothing here is decoration; edits go through the
//! song-wide undo like any other (slider drags are one step on release),
//! and the name field commits on click-away / Enter and Esc cancels.
//!
//! I toggles it (View > Inspector; in the pattern editor, where I types a
//! note, Alt+I). See docs/INSPECTOR.md.
use super::*;
use crate::timeline::{self as tl, PPQ};

/// what the Inspector shows: the last thing clicked
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Focus {
    #[default]
    Track,
    Clip,
}

#[derive(Default)]
pub(crate) struct InspectorUi {
    pub(crate) focus: Focus,
    /// last seen selection / cursor channel (focus follows the newer one)
    seen_sel: Option<u32>,
    seen_ch: usize,
    /// the track name being typed (track, text); None = not editing
    name_edit: Option<(usize, String)>,
    /// screen rect as last drawn (tests, screenshots)
    pub(crate) rect: Option<Rect>,
    /// the track name field as last drawn (tests)
    pub(crate) name_rect: Option<Rect>,
}

pub(crate) const INSPECTOR_W: f32 = 252.0;

fn section(ui: &mut egui::Ui, id: &str, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    let t = th();
    egui::CollapsingHeader::new(
        egui::RichText::new(title.to_uppercase())
            .size(11.0)
            .strong()
            .color(t.ink2),
    )
    .id_salt(("insp", id))
    .default_open(true)
    .show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        add(ui)
    });
    ui.add_space(2.0);
}

/// "label ........ control" rows
fn row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(62.0, 18.0), Sense::hover());
        ui.painter().text(
            r.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            FontId::proportional(11.5),
            th().ink2,
        );
        add(ui)
    });
}

fn pan_text(v: f64) -> String {
    let p = (v * 100.0).round() as i32;
    match p {
        0 => "C".into(),
        p if p < 0 => format!("L {}", -p),
        p => format!("R {p}"),
    }
}

fn beats(t: tl::Tick) -> f64 {
    t as f64 / PPQ as f64
}

impl App {
    pub(super) fn toggle_inspector(&mut self) {
        self.extra.inspector_open = !self.extra.inspector_open;
        self.save_settings();
        self.status = if self.extra.inspector_open {
            "Inspector shown (I hides it; Alt+I in the Tracker)".into()
        } else {
            "Inspector hidden (I shows it)".into()
        };
    }

    /// the panel itself (called from the main layout when open)
    pub(super) fn inspector_side(&mut self, ui: &mut egui::Ui) {
        let resp = egui::Panel::left("inspector")
            .exact_size(INSPECTOR_W)
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(th().cart_bot)
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 6,
                        top: 6,
                        bottom: 6,
                    }),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("insp_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.inspector_ui(ui));
            });
        self.insp.rect = Some(resp.response.rect);
    }

    /// screenshots: show the selected clip (or the cursor track)
    pub(super) fn inspector_shot(&mut self, clip: bool) {
        self.insp.seen_ch = self.ch;
        self.insp.seen_sel = self.tlv.sel;
        self.insp.focus = if clip && self.tlv.sel.is_some() {
            Focus::Clip
        } else {
            Focus::Track
        };
    }

    fn inspector_focus(&mut self) -> (Option<u32>, usize) {
        // focus follows whichever changed last: a clicked clip or the track
        let sel = if self.view == View::Timeline {
            self.tlv.sel
        } else {
            None
        };
        if sel != self.insp.seen_sel {
            self.insp.seen_sel = sel;
            if sel.is_some() {
                self.insp.focus = Focus::Clip;
            }
        }
        if self.ch != self.insp.seen_ch {
            self.insp.seen_ch = self.ch;
            self.insp.focus = Focus::Track;
        }
        let clip = sel.filter(|_| self.insp.focus == Focus::Clip);
        (clip, self.ch)
    }

    fn inspector_ui(&mut self, ui: &mut egui::Ui) {
        let t = th();
        ui.set_max_width(INSPECTOR_W - 22.0);
        ui.spacing_mut().slider_width = 92.0;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(" INSPECTOR ")
                    .strong()
                    .monospace()
                    .size(10.5)
                    .color(Color32::from_rgb(0x1A, 0x1C, 0x1F))
                    .background_color(t.accent),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("×")
                    .on_hover_text("hide the Inspector (I)")
                    .clicked()
                {
                    self.toggle_inspector();
                }
            });
        });
        ui.add_space(4.0);
        let (clip_id, ch) = self.inspector_focus();
        let mut track = ch;
        if let Some((clip, name)) = clip_id.and_then(|id| self.tlv.clip_label(id)) {
            track = clip.track as usize;
            self.inspector_clip(ui, &clip, &name);
        }
        let nch = self.with(|s| s.song.channels);
        if track < nch {
            self.inspector_track(ui, track);
            self.inspector_sends(ui, track);
            self.inspector_instrument(ui, track);
        } else {
            section(ui, "track", "Track", |ui| {
                ui.label(
                    egui::RichText::new(
                        "Song / Master track: tempo and master lanes (Mixer has the master level)",
                    )
                    .small()
                    .color(t.ink2),
                );
            });
        }
    }

    // ------------------------------------------------------------- clip

    fn inspector_clip(&mut self, ui: &mut egui::Ui, clip: &tl::Clip, name: &str) {
        let t = th();
        let mode = self.with(|s| s.song.timeline.is_some());
        let (title, kind) = match clip.kind {
            tl::ClipKind::Audio { .. } => ("Audio clip", "audio"),
            tl::ClipKind::Notes { .. } => ("Note clip", "notes"),
            _ => ("Tracker clip", "block"),
        };
        let id = clip.id;
        let mut muted = clip.muted;
        let mut gain_db = 20.0 * clip.gain.max(1e-4).log10();
        let (mut fi, mut fo) = (beats(clip.fade_in), beats(clip.fade_out));
        let mut shape = clip.fade_shape;
        let (pos, end, bpm) = self.with(|s| {
            let tlr = s.song.timeline.as_deref();
            let bp = |x| tlr.map(|t| t.bar_pos(x));
            (bp(clip.start), bp(clip.end()), s.song.bpm)
        });
        let file = if let tl::ClipKind::Audio { asset } = clip.kind {
            self.tlv.cache.as_ref().and_then(|c| {
                c.tl.audio.get(asset as usize).map(|a| {
                    format!(
                        "{:.1} s · {} Hz{}",
                        a.seconds(),
                        a.rate,
                        if a.missing() { " · missing" } else { "" }
                    )
                })
            })
        } else {
            None
        };
        let mut changed = false;
        let mut open_roll = false;
        let mut open_pat = false;
        section(ui, "clip", title, |ui| {
            ui.label(egui::RichText::new(name).strong().color(t.ink));
            if let Some(f) = &file {
                ui.label(egui::RichText::new(f).small().color(t.ink2));
            }
            let len_beats = beats(clip.len);
            let at = match (pos, end) {
                (Some(a), Some(b)) => format!(
                    "bar {}.{} – {}.{} · {:.2} beats",
                    a.bar, a.beat, b.bar, b.beat, len_beats
                ),
                _ => format!("{len_beats:.2} beats · {bpm:.0} BPM"),
            };
            ui.label(egui::RichText::new(at).size(10.5).color(t.ink2));
            if let Some(l) = clip.loop_len {
                ui.label(
                    egui::RichText::new(format!("loops every {:.2} beats", beats(l)))
                        .small()
                        .color(t.ink2),
                );
            }
            if !mode {
                ui.label(
                    egui::RichText::new(
                        "Tracker mode: this clip comes from the order list. Switch the song to timeline mode to edit clips here.",
                    )
                    .small()
                    .color(t.ochre),
                );
            }
            ui.add_enabled_ui(mode, |ui| {
                row(ui, "Mute", |ui| {
                    if ui
                        .checkbox(&mut muted, "Muted")
                        .on_hover_text("mute this clip (M)")
                        .changed()
                    {
                        changed = true;
                    }
                });
                if kind == "audio" {
                    row(ui, "Gain", |ui| {
                        if ui
                            .add(
                                egui::Slider::new(&mut gain_db, -48.0..=12.0)
                                    .suffix(" dB")
                                    .fixed_decimals(1),
                            )
                            .on_hover_text("clip gain (also: right-click the clip)")
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    let max = len_beats;
                    row(ui, "Fade in", |ui| {
                        if ui
                            .add(
                                egui::DragValue::new(&mut fi)
                                    .range(0.0..=max)
                                    .speed(0.05)
                                    .fixed_decimals(2)
                                    .suffix(" beats"),
                            )
                            .on_hover_text("or drag the clip's top-left corner")
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    row(ui, "Fade out", |ui| {
                        if ui
                            .add(
                                egui::DragValue::new(&mut fo)
                                    .range(0.0..=max)
                                    .speed(0.05)
                                    .fixed_decimals(2)
                                    .suffix(" beats"),
                            )
                            .on_hover_text("or drag the clip's top-right corner")
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    row(ui, "Shape", |ui| {
                        egui::ComboBox::from_id_salt("insp_fade_shape")
                            .selected_text(shape.name())
                            .width(110.0)
                            .show_ui(ui, |ui| {
                                for f in tl::FadeShape::ALL {
                                    if ui.selectable_value(&mut shape, f, f.name()).changed() {
                                        changed = true;
                                    }
                                }
                            });
                    });
                }
            });
            if kind != "audio" {
                ui.horizontal(|ui| {
                    if ui
                        .button("Piano roll")
                        .on_hover_text(
                            "edit this clip's notes in the lower panel (double-click the clip)",
                        )
                        .clicked()
                    {
                        open_roll = true;
                    }
                    if kind == "block"
                        && ui
                            .button("Open in Tracker")
                            .on_hover_text("this clip's block in the Tracker (Enter)")
                            .clicked()
                    {
                        open_pat = true;
                    }
                });
            }
        });
        if changed && mode {
            let to_tick = |b: f64| (b * PPQ as f64).round() as tl::Tick;
            self.with(|s| {
                if let Some(tlm) = s.song.timeline.as_deref_mut() {
                    if let Some(c) = tlm.clips.iter_mut().find(|c| c.id == id) {
                        c.muted = muted;
                        if c.audio().is_some() {
                            c.gain = 10f32.powf(gain_db / 20.0);
                            c.fade_shape = shape;
                        }
                    }
                    if clip.audio().is_some() {
                        tl::set_fades(tlm, id, to_tick(fi), to_tick(fo));
                    }
                }
                let Shared { song, engine, .. } = s;
                engine.ensure_plan(song);
            });
            self.dirty = true;
        }
        if open_roll {
            self.tlv.sel = Some(id);
            self.timeline_open_roll();
        }
        if open_pat {
            if let Some((b, col)) = clip.block() {
                let entry = self.tlv.entry_at(clip.start);
                self.timeline_open(b, col, (clip.offset / tl::ROW) as usize, entry);
            }
        }
    }

    // ------------------------------------------------------------ track

    fn inspector_track(&mut self, ui: &mut egui::Ui, tr: usize) {
        let t = th();
        let (name, mut m, colour, mode, icon) = self.with(|s| {
            (
                s.song.channel_names.get(tr).cloned().unwrap_or_default(),
                s.song.mixer.channels.get(tr).copied().unwrap_or_default(),
                s.song
                    .timeline
                    .as_deref()
                    .and_then(|x| x.tracks.get(tr))
                    .and_then(|x| x.colour),
                s.song.timeline.is_some(),
                s.song
                    .timeline
                    .as_deref()
                    .and_then(|x| x.tracks.get(tr))
                    .map_or(tl::TrackIcon::Auto, |x| x.icon),
            )
        });
        let slot = self.track_slot(tr);
        let tag = self.with(|s| {
            s.song
                .instruments
                .get((slot as usize).wrapping_sub(1))
                .map(|i| super::params::type_tag(&i.kind))
                .unwrap_or("")
        });
        let mut new_icon: Option<tl::TrackIcon> = None;
        let cur_col = match colour {
            Some(c) => hex(c & 0xFF_FFFF),
            None => super::timeline::track_colour(tr),
        };
        let m0 = m;
        let mut new_colour: Option<Option<u32>> = None;
        let mut commit_name: Option<String> = None;
        section(ui, "track", &format!("Track {:02}", tr + 1), |ui| {
            // name: commits on click-away / Enter, Esc cancels
            row(ui, "Name", |ui| {
                let editing = self.insp.name_edit.as_ref().is_some_and(|(k, _)| *k == tr);
                let mut text = if editing {
                    self.insp.name_edit.as_ref().unwrap().1.clone()
                } else {
                    name.clone()
                };
                // the field's own fill decides the ink (Amiga's text fields are navy)
                let bg = ui.visuals().extreme_bg_color;
                let ink = if crate::theme::luma(bg) > 0.5 {
                    Color32::from_rgb(0x10, 0x12, 0x16)
                } else {
                    Color32::from_rgb(0xE8, 0xEC, 0xEF)
                };
                let r = ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .text_color(ink)
                        .id_salt(("insp_name", tr))
                        .desired_width(150.0)
                        .char_limit(32),
                );
                self.insp.name_rect = Some(r.rect);
                if r.has_focus() || r.changed() {
                    self.insp.name_edit = Some((tr, text.clone()));
                }
                if r.lost_focus() {
                    let esc = ui.input(|i| i.key_pressed(Key::Escape));
                    if let Some((_, s)) = self.insp.name_edit.take() {
                        if !esc && s.trim() != name && !s.trim().is_empty() {
                            commit_name = Some(s.trim().to_string());
                        }
                    }
                }
            });
            row(ui, "Colour", |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let (sw, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), Sense::hover());
                ui.painter().rect_filled(sw, 3, cur_col);
                ui.add_space(4.0);
                ui.add_enabled_ui(mode, |ui| {
                    ui.menu_button("Pick…", |ui| {
                        ui.label(egui::RichText::new("track colour").small());
                        ui.horizontal_wrapped(|ui| {
                            ui.set_max_width(150.0);
                            for c in super::timeline::track_palette() {
                                let (r, resp) =
                                    ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::click());
                                ui.painter().rect_filled(r.shrink(1.0), 3, hex(c));
                                if colour == Some(c) {
                                    ui.painter().rect_stroke(
                                        r,
                                        3,
                                        Stroke::new(2.0, t.selection_outline),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                if resp.clicked() {
                                    new_colour = Some(Some(c));
                                    ui.close();
                                }
                            }
                        });
                        if ui.button("Automatic").clicked() {
                            new_colour = Some(None);
                            ui.close();
                        }
                    })
                    .response
                    .on_hover_text("pick this track's colour")
                    .on_disabled_hover_text("track colours need timeline mode");
                });
            });
            row(ui, "Icon", |ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::hover());
                super::glyphs::tile(
                    ui.painter(),
                    r,
                    super::glyphs::track_glyph(icon, tag),
                    cur_col,
                    Color32::from_rgb(0x16, 0x18, 0x1B),
                    theme::polished(),
                );
                ui.add_space(4.0);
                ui.add_enabled_ui(mode, |ui| {
                    ui.menu_button("Pick…", |ui| {
                        ui.label(egui::RichText::new("track icon").small());
                        if ui
                            .selectable_label(icon == tl::TrackIcon::Auto, "Auto")
                            .on_hover_text("picked from the instrument type")
                            .clicked()
                        {
                            new_icon = Some(tl::TrackIcon::Auto);
                            ui.close();
                        }
                        egui::Grid::new(("insp_icons", tr))
                            .spacing([3.0, 3.0])
                            .show(ui, |ui| {
                                for (k, ic) in tl::TrackIcon::ALL.iter().enumerate() {
                                    let (r, resp) = ui.allocate_exact_size(
                                        egui::vec2(28.0, 24.0),
                                        Sense::click(),
                                    );
                                    let on = icon == *ic;
                                    ui.painter().rect_filled(
                                        r,
                                        4,
                                        if on || resp.hovered() {
                                            t.accent.gamma_multiply(0.35)
                                        } else {
                                            theme::shade(&t, 0.1)
                                        },
                                    );
                                    if let Some(g) = super::glyphs::icon_glyph(*ic) {
                                        super::glyphs::paint(
                                            ui.painter(),
                                            Rect::from_center_size(
                                                r.center(),
                                                egui::vec2(14.0, 14.0),
                                            ),
                                            g,
                                            t.ink,
                                        );
                                    }
                                    if resp.on_hover_text(ic.label()).clicked() {
                                        new_icon = Some(*ic);
                                        ui.close();
                                    }
                                    if k % 4 == 3 {
                                        ui.end_row();
                                    }
                                }
                            });
                    })
                    .response
                    .on_hover_text(
                        "this track's header icon (also in the track pie: right-click the header)",
                    )
                    .on_disabled_hover_text("track icons need timeline mode");
                });
            });
            row(ui, "Volume", |ui| {
                ui.add(
                    egui::Slider::new(&mut m.volume, 0.0..=2.0)
                        .custom_formatter(|v, _| crate::model::db_text(v as f32))
                        .show_value(true),
                )
                .on_hover_text("track (mixer channel) volume");
            });
            row(ui, "Pan", |ui| {
                ui.spacing_mut().slider_width = 70.0;
                ui.add(
                    egui::Slider::new(&mut m.pan, -1.0..=1.0).custom_formatter(|v, _| pan_text(v)),
                );
                if ui.small_button("C").on_hover_text("centre").clicked() {
                    m.pan = 0.0;
                }
            });
            row(ui, "", |ui| {
                for (on, l, tip, c) in [
                    (&mut m.mute, "M", "mute (F9)", t.ochre),
                    (&mut m.solo, "S", "solo (F10)", t.accent),
                ] {
                    let txt = egui::RichText::new(l).strong().color(if *on {
                        Color32::from_rgb(0x16, 0x18, 0x1B)
                    } else {
                        t.ink2
                    });
                    let mut b = egui::Button::new(txt).min_size(egui::vec2(30.0, 18.0));
                    if *on {
                        b = b.fill(c);
                    }
                    if ui.add(b).on_hover_text(tip).clicked() {
                        *on = !*on;
                    }
                }
            });
        });
        if m != m0 {
            self.with(|s| {
                if let Some(c) = s.song.mixer.channels.get_mut(tr) {
                    *c = m;
                }
            });
            self.dirty = true;
        }
        if let Some(c) = new_colour {
            self.snapshot();
            self.with(|s| {
                if let Some(x) = s
                    .song
                    .timeline
                    .as_deref_mut()
                    .and_then(|x| x.tracks.get_mut(tr))
                {
                    x.colour = c;
                }
            });
        }
        if let Some(i) = new_icon {
            self.snapshot();
            self.with(|s| {
                if let Some(x) = s
                    .song
                    .timeline
                    .as_deref_mut()
                    .and_then(|x| x.tracks.get_mut(tr))
                {
                    x.icon = i;
                }
            });
        }
        if let Some(n) = commit_name {
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
    }

    fn inspector_sends(&mut self, ui: &mut egui::Ui, tr: usize) {
        let (mut m, names) = self.with(|s| {
            (
                s.song.mixer.channels.get(tr).copied().unwrap_or_default(),
                s.song
                    .mixer
                    .returns
                    .iter()
                    .map(|r| r.name.clone())
                    .collect::<Vec<_>>(),
            )
        });
        let m0 = m;
        section(ui, "sends", "Sends", |ui| {
            for k in 0..2 {
                let label = names
                    .get(k)
                    .cloned()
                    .unwrap_or_else(|| ["Delay", "Reverb"][k].to_string());
                row(ui, &label, |ui| {
                    ui.add(
                        egui::Slider::new(&mut m.send[k], 0.0..=1.0)
                            .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
                    )
                    .on_hover_text(format!(
                        "post-fader send to return {} ({label}; Mixer view has the returns)",
                        ["A", "B"][k]
                    ));
                });
            }
            row(ui, "Duck", |ui| {
                ui.add(
                    egui::Slider::new(&mut m.duck, 0.0..=1.0)
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
                )
                .on_hover_text("sidechain ducking depth (keyed by the Mixer's duck key channel)");
            });
        });
        if m != m0 {
            self.with(|s| {
                if let Some(c) = s.song.mixer.channels.get_mut(tr) {
                    *c = m;
                }
            });
            self.dirty = true;
        }
    }

    fn inspector_instrument(&mut self, ui: &mut egui::Ui, tr: usize) {
        let t = th();
        let slot = self.track_slot(tr);
        let info = self.with(|s| {
            s.song
                .instruments
                .get((slot as usize).wrapping_sub(1))
                .map(|i| (i.name.clone(), super::params::type_tag(&i.kind)))
        });
        let mut edit = false;
        let mut browse = false;
        section(ui, "instrument", "Instrument", |ui| {
            match &info {
                Some((n, tag)) => {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::hover());
                        super::glyphs::tile(
                            ui.painter(),
                            r,
                            super::glyphs::glyph_for(tag),
                            super::instpanel::tag_colour(tag),
                            Color32::from_rgb(0x16, 0x18, 0x1B),
                            theme::polished(),
                        );
                        ui.label(
                            egui::RichText::new(format!("{slot:02X} {n}"))
                                .strong()
                                .color(t.ink),
                        );
                    });
                    ui.horizontal(|ui| {
                        super::instpanel::tag_pill(ui, tag);
                        if ui
                            .button("Edit")
                            .on_hover_text("open it in the instrument editor (lower panel, Ctrl+I)")
                            .clicked()
                        {
                            edit = true;
                        }
                        if ui
                            .button("Browse…")
                            .on_hover_text("the Browser's Instruments tab")
                            .clicked()
                        {
                            browse = true;
                        }
                    });
                }
                None => {
                    ui.label(egui::RichText::new("No instrument yet").color(t.ink2));
                    if ui.button("Browse instruments…").clicked() {
                        browse = true;
                    }
                }
            }
            ui.label(
                egui::RichText::new(
                    "Drop an instrument from the Browser on this track's header to swap it.",
                )
                .small()
                .color(t.ink2),
            );
        });
        if edit && slot > 0 {
            self.instr = slot;
            self.show_instrument_panel();
        }
        if browse {
            self.open_browser_tab(crate::browser::Tab::Instruments);
        }
    }
}
