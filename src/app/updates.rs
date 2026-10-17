//! Help > Check for updates… and the quiet startup check (0.2.3.5).
//! The HTTP request runs on a background thread (crate::update); the UI only
//! polls a channel, so it never freezes. Nothing opens unless the user clicks.
use super::*;
use crate::update::{self, Line, Outcome, Release};
use std::sync::mpsc;

#[derive(Default)]
pub(super) struct UpdateUi {
    rx: Option<mpsc::Receiver<Outcome>>,
    /// the user asked (show the result) vs the quiet startup check
    manual: bool,
    started: bool,
    pub(super) win: Option<UpdWin>,
    /// newer, non-skipped version found by the startup check (toolbar badge)
    pub(super) badge: Option<Release>,
    /// UI tests deliver the "network" result through this
    #[cfg(test)]
    pub(super) test_tx: Option<mpsc::Sender<Outcome>>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum UpdWin {
    Checking,
    Newer(Release),
    Latest,
    Failed(String),
}

fn open(ctx: &egui::Context, url: &str) {
    ctx.open_url(egui::OpenUrl::new_tab(url));
}

impl App {
    /// start a check (no-op if one is already running); `manual` shows the result
    pub(super) fn check_updates(&mut self, manual: bool) {
        if manual {
            self.upd.manual = true;
            self.upd.win = Some(UpdWin::Checking);
        }
        if self.upd.rx.is_some() {
            return;
        }
        #[cfg(test)]
        {
            let (tx, rx) = mpsc::channel();
            self.upd.test_tx = Some(tx);
            self.upd.rx = Some(rx);
        }
        #[cfg(not(test))]
        {
            self.upd.rx = Some(update::spawn_check());
        }
    }

    /// once per run: the quiet startup check, if enabled and due (24 h)
    fn startup_update_check(&mut self) {
        if self.upd.started {
            return;
        }
        self.upd.started = true;
        let now = update::now_secs();
        if cfg!(test) || !update::startup_due(self.extra.update_check, self.extra.update_last, now)
        {
            return;
        }
        self.extra.update_last = now;
        self.save_settings();
        self.check_updates(false);
    }

    fn is_skipped(&self, r: &Release) -> bool {
        let s = self.extra.update_skip.trim();
        !s.is_empty() && (s == r.version || s == r.tag)
    }

    /// called every frame: startup check, deliver a finished result
    pub(super) fn poll_updates(&mut self, ctx: &egui::Context) {
        self.startup_update_check();
        let Some(rx) = &self.upd.rx else { return };
        let got = match rx.try_recv() {
            Ok(o) => o,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                Outcome::Failed("the check stopped unexpectedly".into())
            }
        };
        self.upd.rx = None;
        if self.upd.manual {
            self.upd.manual = false;
            self.upd.win = Some(match got {
                Outcome::Newer(r) => UpdWin::Newer(r),
                Outcome::Latest(_) => UpdWin::Latest,
                Outcome::Failed(e) => UpdWin::Failed(e),
            });
        } else if let Outcome::Newer(r) = got {
            if !self.is_skipped(&r) {
                self.status = format!(
                    "ChipWave {} is available (Help > Check for updates…)",
                    r.version
                );
                self.upd.badge = Some(r);
            }
        }
    }

    /// toolbar badge (right side of the menu bar) when a newer version exists
    pub(super) fn update_badge(&mut self, ui: &mut egui::Ui) {
        let Some(r) = self.upd.badge.clone() else {
            return;
        };
        let b = egui::Button::new(
            egui::RichText::new(format!("Update {} available", r.version))
                .small()
                .color(th().accent),
        );
        if ui
            .add(b)
            .on_hover_text("a newer ChipWave is out: click for details")
            .clicked()
        {
            self.upd.win = Some(UpdWin::Newer(r));
        }
    }

    pub(super) fn update_window(&mut self, ctx: &egui::Context) {
        let Some(win) = self.upd.win.clone() else {
            return;
        };
        let mut open_flag = true;
        let mut close = false;
        egui::Window::new("Check for updates")
            .open(&mut open_flag)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| match &win {
                UpdWin::Checking => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Checking for updates…");
                    });
                    ui.add_space(6.0);
                    if ui.button("Close").clicked() {
                        // keep checking quietly; a newer version shows as a badge
                        self.upd.manual = false;
                        close = true;
                    }
                }
                UpdWin::Latest => {
                    ui.label(format!("You're on the latest version ({})", crate::VERSION));
                    ui.add_space(6.0);
                    if ui.button("OK").clicked() {
                        close = true;
                    }
                }
                UpdWin::Failed(e) => {
                    ui.label("Couldn't check for updates right now.");
                    ui.label(
                        egui::RichText::new(format!(
                            "{e}. Check your internet connection and try again later, or look on the releases page."
                        ))
                        .small()
                        .color(th().ink2),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Try again").clicked() {
                            self.check_updates(true);
                        }
                        if ui.button("Open releases page").clicked() {
                            open(ctx, update::RELEASES_PAGE);
                        }
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                }
                UpdWin::Newer(r) => {
                    ui.label(
                        egui::RichText::new(format!(
                            "You have {}, latest is {}",
                            crate::VERSION,
                            r.version
                        ))
                        .strong(),
                    );
                    if !r.name.is_empty() {
                        ui.label(egui::RichText::new(&r.name).color(th().ink2));
                    }
                    ui.add_space(4.0);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(260.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.set_width(430.0);
                                let lines = update::notes_lines(&r.body);
                                if lines.is_empty() {
                                    ui.label(
                                        egui::RichText::new("(no release notes)").color(th().ink2),
                                    );
                                }
                                for l in lines {
                                    match l {
                                        Line::Heading(t) => {
                                            ui.label(egui::RichText::new(t).strong());
                                        }
                                        Line::Bullet(t) => {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.label("•");
                                                ui.label(t);
                                            });
                                        }
                                        Line::Text(t) => {
                                            ui.label(t);
                                        }
                                        Line::Blank => ui.add_space(4.0),
                                    }
                                }
                            });
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let dl = ui.button("Download installer").on_hover_text(r.download_url());
                        if dl.clicked() {
                            open(ctx, r.download_url());
                            close = true;
                        }
                        if ui.button("Open release page").on_hover_text(&r.page).clicked() {
                            open(ctx, &r.page);
                            close = true;
                        }
                        if ui.button("Not now").clicked() {
                            close = true;
                        }
                        if ui
                            .button("Skip this version")
                            .on_hover_text("don't remind me about this version at startup")
                            .clicked()
                        {
                            self.extra.update_skip = r.version.clone();
                            self.upd.badge = None;
                            self.save_settings();
                            close = true;
                        }
                    });
                }
            });
        if !open_flag || close {
            if self.upd.win == Some(UpdWin::Checking) {
                self.upd.manual = false;
            }
            self.upd.win = None;
        }
    }
}
