//! Help > About: header with the cicada, then About | Credits | Licences tabs.
//! The credits come from `crate::credits::CREDITS` (one structured list).
use super::topbar::{DONATE_URL, PAYPAL_URL, YOUTUBE_URL};
use super::*;
use crate::credits::{self, Credit, Group, CREDITS};

pub(super) const WEBSITE_URL: &str = "https://postmanpat-303.github.io/ChipWaveStudio/";
pub(super) const GITHUB_URL: &str = "https://github.com/postmanpat-303/ChipWaveStudio";
pub(super) const LICENCE_URL: &str = "https://www.gnu.org/licenses/gpl-3.0.html";
pub(super) const TAGLINE: &str = "A DAW and tracker for chip and psytrance sounds";
const DEFAULT_SIZE: egui::Vec2 = egui::vec2(640.0, 560.0);

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(super) enum AboutTab {
    #[default]
    About,
    Credits,
    Licences,
}

#[derive(Default)]
pub(super) struct AboutState {
    pub tab: AboutTab,
    pub filter: String,
    logo: Option<egui::TextureHandle>,
    /// frames left in which the window is (re)centred: set when it opens
    centre_frames: u8,
    /// where it was last centred (still there = not moved by the user)
    centred_at: Option<egui::Pos2>,
    was_open: bool,
    /// shown under the notices button when the file isn't there
    msg: String,
}

fn muted() -> Color32 {
    th().ink2
}

/// licence badge colour: copyleft warm, permissive green
fn badge_colour(lic: &str) -> Color32 {
    let t = th();
    if lic.contains("GPL") {
        if t.light {
            t.ochre
        } else {
            t.fx
        }
    } else {
        t.sage
    }
}

fn badge(ui: &mut egui::Ui, text: &str) {
    let col = badge_colour(text);
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        crate::theme::prop(crate::theme::ts::SMALL),
        col,
    );
    let size = galley.size() + egui::vec2(10.0, 3.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect(
        rect,
        4.0,
        col.gamma_multiply(0.12),
        Stroke::new(1.0, col.gamma_multiply(0.8)),
        egui::StrokeKind::Inside,
    );
    p.galley(rect.center() - galley.size() / 2.0, galley, col);
}

/// where THIRD_PARTY_LICENSES.txt sits next to the exe (Windows zip / installer,
/// Linux), or in the Mac bundle's Resources
fn notices_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [
        dir.join("THIRD_PARTY_LICENSES.txt"),
        dir.join("../Resources/THIRD_PARTY_LICENSES.txt"),
        dir.join("../share/doc/chipwave/THIRD_PARTY_LICENSES.txt"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

fn open_file(p: &std::path::Path) {
    #[cfg(windows)]
    let cmd = "explorer";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = "xdg-open";
    let _ = std::process::Command::new(cmd).arg(p).spawn();
}

/// a link-coloured, clickable label that may wrap
fn link(ui: &mut egui::Ui, text: egui::RichText, url: &str) {
    let r = ui.add(egui::Hyperlink::from_label_and_url(
        text.color(th().accent),
        url,
    ));
    r.on_hover_text(url);
}

fn credit_row(ui: &mut egui::Ui, c: &Credit, left_w: f32) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.allocate_ui_with_layout(
            egui::vec2(left_w, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(left_w);
                ui.spacing_mut().item_spacing.y = 1.0;
                let name = egui::RichText::new(c.name)
                    .strong()
                    .size(crate::theme::ts::px(crate::theme::ts::BODY));
                if c.url.is_empty() {
                    ui.add(egui::Label::new(name.color(th().ink)).wrap());
                } else {
                    link(ui, name, c.url);
                }
                if !c.by.is_empty() {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(c.by)
                                .size(crate::theme::ts::px(crate::theme::ts::LABEL))
                                .color(muted()),
                        )
                        .wrap(),
                    );
                }
            },
        );
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.add(
                egui::Label::new(
                    egui::RichText::new(c.what).size(crate::theme::ts::px(crate::theme::ts::BODY)),
                )
                .wrap(),
            );
            if !c.licence.is_empty() || !c.note.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if !c.licence.is_empty() {
                        badge(ui, c.licence);
                    }
                    if !c.note.is_empty() {
                        ui.label(
                            egui::RichText::new(c.note)
                                .size(crate::theme::ts::px(crate::theme::ts::LABEL))
                                .italics()
                                .color(muted()),
                        );
                    }
                });
            }
        });
    });
}

impl App {
    fn about_logo(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.about.logo.is_none() {
            let icon = eframe::icon_data::from_png_bytes(include_bytes!(
                "../../assets/icon/chipwave_studio_256.png"
            ))
            .ok()?;
            let img = egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            );
            self.about.logo =
                Some(ctx.load_texture("about_logo", img, egui::TextureOptions::LINEAR));
        }
        self.about.logo.clone()
    }

    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            self.about.was_open = false;
            return;
        }
        if !self.about.was_open {
            // centre in the main window every time it opens (the first frame
            // only guesses the size, so do it again once egui has measured it)
            self.about.was_open = true;
            self.about.centre_frames = 3;
            self.about.msg.clear();
        }
        let id = egui::Id::unique("about_chipwave_studio");
        let mut win = egui::Window::new("About ChipWave Studio")
            .id(id)
            .collapsible(false)
            .resizable(true)
            .default_size(DEFAULT_SIZE)
            .min_size([520.0, 420.0]);
        // re-centre while it opens and for as long as the user hasn't moved it
        // (egui only knows the real size after a frame or two)
        let st = egui::AreaState::load(ctx, id);
        let untouched = match (self.about.centred_at, st.and_then(|s| s.pivot_pos)) {
            (Some(a), Some(p)) => (a - p).length() < 0.5,
            _ => false,
        };
        if self.about.centre_frames > 0 || untouched {
            self.about.centre_frames = self.about.centre_frames.saturating_sub(1);
            let size = st
                .and_then(|s| s.size)
                .unwrap_or(DEFAULT_SIZE + egui::vec2(14.0, 44.0));
            let pos = (ctx.content_rect().center() - size / 2.0).round();
            win = win.current_pos(pos);
            // egui 0.37 windows with a title-bar drag ignore current_pos once
            // they have a stored position, so move the stored one too
            if let Some(mut st) = st {
                st.pivot_pos = Some(pos);
                st.pivot = egui::Align2::LEFT_TOP;
                ctx.memory_mut(|m| {
                    m.areas_mut()
                        .set_state(egui::LayerId::new(egui::Order::Middle, id), st)
                });
            }
            self.about.centred_at = Some(pos);
        }
        let logo = self.about_logo(ctx);
        let mut open = true;
        win.open(&mut open).show(ctx, |ui| {
            ui.set_min_size(DEFAULT_SIZE - egui::vec2(0.0, 30.0));
            self.about_header(ui, logo);
            ui.add_space(10.0);
            self.about_tabs(ui);
            ui.add_space(2.0);
            ui.separator();
            ui.add_space(4.0);
            // footer first (bottom-up), so the scrolling body takes exactly the
            // rest of the window and never pushes it taller
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.add_space(2.0);
                self.about_footer(ui);
                ui.separator();
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(("about_body", self.about.tab as u8))
                        .auto_shrink([false, false])
                        .show(ui, |ui| match self.about.tab {
                            AboutTab::About => self.about_tab_about(ui),
                            AboutTab::Credits => self.about_tab_credits(ui),
                            AboutTab::Licences => self.about_tab_licences(ui),
                        });
                });
            });
        });
        if !open {
            self.show_about = false;
            self.about.was_open = false;
        }
    }

    fn about_footer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .button("Full notices: THIRD_PARTY_LICENSES.txt")
                .on_hover_text("Open the full licence notices that ship next to the app")
                .clicked()
            {
                match notices_path() {
                    Some(p) => open_file(&p),
                    None => {
                        self.about.msg = "Not found next to the app; it's in every download.".into()
                    }
                }
            }
            if !self.about.msg.is_empty() {
                ui.label(
                    egui::RichText::new(&self.about.msg)
                        .size(crate::theme::ts::px(crate::theme::ts::COMPACT))
                        .color(muted()),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("{} credits · thank you!", CREDITS.len()))
                        .size(crate::theme::ts::px(crate::theme::ts::COMPACT))
                        .color(muted()),
                );
            });
        });
    }

    fn about_header(&mut self, ui: &mut egui::Ui, logo: Option<egui::TextureHandle>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            if let Some(t) = &logo {
                ui.add(egui::Image::new((t.id(), egui::vec2(84.0, 84.0))));
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("ChipWave Studio")
                            .size(crate::theme::ts::px(crate::theme::ts::DISPLAY))
                            .strong()
                            .color(th().ink),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(format!("v{}", crate::version_label()))
                            .size(crate::theme::ts::px(crate::theme::ts::BODY))
                            .color(muted()),
                    )
                    .on_hover_text("Version");
                });
                ui.label(
                    egui::RichText::new(TAGLINE)
                        .size(crate::theme::ts::px(crate::theme::ts::LARGE)),
                );
                ui.label(
                    egui::RichText::new("Created by Del (postmanpat)")
                        .size(crate::theme::ts::px(crate::theme::ts::BODY))
                        .color(muted()),
                );
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    link(
                        ui,
                        egui::RichText::new("Website")
                            .size(crate::theme::ts::px(crate::theme::ts::BODY)),
                        WEBSITE_URL,
                    );
                    ui.label(egui::RichText::new("·").color(muted()));
                    link(
                        ui,
                        egui::RichText::new("GitHub")
                            .size(crate::theme::ts::px(crate::theme::ts::BODY)),
                        GITHUB_URL,
                    );
                    ui.label(egui::RichText::new("·").color(muted()));
                    link(
                        ui,
                        egui::RichText::new("GPL-3.0-or-later")
                            .size(crate::theme::ts::px(crate::theme::ts::BODY)),
                        LICENCE_URL,
                    );
                });
            });
        });
    }

    fn about_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.spacing_mut().button_padding = egui::vec2(14.0, 5.0);
            for (t, label) in [
                (AboutTab::About, "About"),
                (AboutTab::Credits, "Credits"),
                (AboutTab::Licences, "Licences"),
            ] {
                let on = self.about.tab == t;
                let txt =
                    egui::RichText::new(label).size(crate::theme::ts::px(crate::theme::ts::LARGE));
                if ui
                    .add(egui::Button::selectable(
                        on,
                        if on { txt.strong() } else { txt },
                    ))
                    .clicked()
                {
                    self.about.tab = t;
                }
            }
        });
    }

    fn about_tab_about(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        let fill = ui.visuals().faint_bg_color;
        egui::Frame::new()
            .fill(fill)
            .stroke(Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    egui::RichText::new("Support ChipWave")
                        .size(crate::theme::ts::px(crate::theme::ts::LARGE))
                        .strong(),
                );
                ui.add_space(2.0);
                ui.label("To keep the software going I need Coffee :)");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for (label, url) in [
                        ("Buy me a coffee", DONATE_URL),
                        ("PayPal", PAYPAL_URL),
                        ("YouTube: demos & promos", YOUTUBE_URL),
                    ] {
                        let b = egui::Button::new(
                            egui::RichText::new(label)
                                .size(crate::theme::ts::px(crate::theme::ts::BODY)),
                        )
                        .min_size(egui::vec2(0.0, 30.0));
                        if ui.add(b).on_hover_text(url).clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                        }
                    }
                });
            });
        ui.add_space(12.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(
                    "ChipWave Studio is a free music DAW in the tradition of OctaMED: a pattern \
                     tracker with an arranger, mixer, master limiter and loudness meter, synths, \
                     Voice, generators, samples and CLAP / VST3 plugins.",
                )
                .size(crate::theme::ts::px(crate::theme::ts::BODY)),
            )
            .wrap(),
        );
        ui.add_space(10.0);
        egui::Grid::new("about_facts")
            .num_columns(2)
            .spacing([18.0, 7.0])
            .show(ui, |ui| {
                let key = |ui: &mut egui::Ui, k: &str| {
                    ui.label(
                        egui::RichText::new(k)
                            .size(crate::theme::ts::px(crate::theme::ts::BODY))
                            .color(muted()),
                    );
                };
                key(ui, "Version");
                ui.label(
                    egui::RichText::new(crate::version_label())
                        .size(crate::theme::ts::px(crate::theme::ts::BODY)),
                );
                ui.end_row();
                key(ui, "Licence");
                link(
                    ui,
                    egui::RichText::new("GNU GPL 3.0 or later")
                        .size(crate::theme::ts::px(crate::theme::ts::BODY)),
                    LICENCE_URL,
                );
                ui.end_row();
                key(ui, "Website");
                link(
                    ui,
                    egui::RichText::new(WEBSITE_URL)
                        .size(crate::theme::ts::px(crate::theme::ts::BODY)),
                    WEBSITE_URL,
                );
                ui.end_row();
                key(ui, "Source code");
                link(
                    ui,
                    egui::RichText::new(GITHUB_URL)
                        .size(crate::theme::ts::px(crate::theme::ts::BODY)),
                    GITHUB_URL,
                );
                ui.end_row();
                key(ui, "Built with");
                ui.label(
                    egui::RichText::new("Rust, egui and open-source work (see Credits)")
                        .size(crate::theme::ts::px(crate::theme::ts::BODY)),
                );
                ui.end_row();
            });
    }

    fn about_tab_credits(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let t = th();
            let mut edit = egui::TextEdit::singleline(&mut self.about.filter)
                .hint_text("Filter credits…")
                .desired_width(240.0);
            if t.light {
                // the Amiga theme's dark CRT fields would hide the hint here
                edit = edit.background_color(t.pill).text_color(t.ink);
                let v = ui.visuals_mut();
                v.extreme_bg_color = t.pill;
                v.text_edit_bg_color = Some(t.pill);
            }
            ui.add(edit);
            if !self.about.filter.is_empty() && ui.small_button("Clear").clicked() {
                self.about.filter.clear();
            }
        });
        ui.add_space(6.0);
        let q = self.about.filter.trim().to_lowercase();
        let left_w = (ui.available_width() * 0.36).clamp(150.0, 230.0);
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let mut shown = 0;
        for g in Group::ALL {
            let rows: Vec<&Credit> = CREDITS
                .iter()
                .filter(|c| c.group == g)
                .filter(|c| q.is_empty() || credits::haystack(c).to_lowercase().contains(&q))
                .collect();
            if rows.is_empty() {
                continue;
            }
            shown += rows.len();
            let head = egui::RichText::new(g.title())
                .size(crate::theme::ts::px(crate::theme::ts::LARGE))
                .strong()
                .color(th().ink);
            let mut h = egui::CollapsingHeader::new(head)
                .id_salt(("credits_group", g as u8))
                .default_open(true);
            if !q.is_empty() {
                h = h.open(Some(true));
            }
            h.show(ui, |ui| {
                ui.spacing_mut().indent = 4.0;
                for (i, c) in rows.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(4.0);
                        let r = ui.available_rect_before_wrap();
                        ui.painter().hline(
                            r.left()..=r.right(),
                            r.top(),
                            Stroke::new(1.0, line.gamma_multiply(0.6)),
                        );
                        ui.add_space(5.0);
                    }
                    credit_row(ui, c, left_w);
                }
                ui.add_space(6.0);
            });
            ui.add_space(2.0);
        }
        if shown == 0 {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("No credits match.").color(muted()));
        }
    }

    fn about_tab_licences(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("ChipWave Studio")
                .size(crate::theme::ts::px(crate::theme::ts::LARGE))
                .strong(),
        );
        ui.add_space(2.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(
                    "ChipWave Studio is free software: you can redistribute it and/or modify it \
                     under the terms of the GNU General Public License, version 3 or (at your \
                     option) any later version. It comes with ABSOLUTELY NO WARRANTY. The licence \
                     text is installed next to the app as LICENSE.txt.",
                )
                .size(crate::theme::ts::px(crate::theme::ts::BODY)),
            )
            .wrap(),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            badge(ui, "GPL-3.0-or-later");
            link(
                ui,
                egui::RichText::new("Read the GNU GPL v3")
                    .size(crate::theme::ts::px(crate::theme::ts::BODY)),
                LICENCE_URL,
            );
        });
        ui.add_space(14.0);
        ui.label(
            egui::RichText::new("Licences of credited work")
                .size(crate::theme::ts::px(crate::theme::ts::LARGE))
                .strong(),
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(
                "Projects whose code, data or design ChipWave uses or credits. Papers, \
                 products and ideas have no licence badge.",
            )
            .size(crate::theme::ts::px(crate::theme::ts::COMPACT))
            .color(muted()),
        );
        ui.add_space(8.0);
        let mut lics: Vec<&str> = CREDITS
            .iter()
            .map(|c| c.licence)
            .filter(|l| !l.is_empty())
            .collect();
        lics.sort();
        lics.dedup();
        let left_w = 130.0;
        for (i, lic) in lics.iter().enumerate() {
            if i > 0 {
                ui.add_space(6.0);
            }
            let names: Vec<&str> = CREDITS
                .iter()
                .filter(|c| c.licence == *lic)
                .map(|c| c.name)
                .collect();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(left_w, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(left_w);
                        badge(ui, lic);
                    },
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(names.join(", "))
                            .size(crate::theme::ts::px(crate::theme::ts::BODY)),
                    )
                    .wrap(),
                );
            });
        }
    }
}
