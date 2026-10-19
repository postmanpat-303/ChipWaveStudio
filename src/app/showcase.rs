//! "Get showcase instruments…" dialog (docs/BROWSER.md): pick from the nine
//! free, open-source instruments, download them from their creators' own
//! release pages (SHA-256 checked) into the per-user plugin folders, then
//! rescan so they show in the Browser's Plugins tab with the Showcase tag.
//! The installer itself lives in crate::showcase.
use super::*;
use crate::showcase::{self, Get, Roots, State, ITEMS};

pub(crate) struct ShowcaseDlg {
    /// one per showcase::ITEMS entry (nine)
    pub(crate) ticked: Vec<bool>,
    pub(crate) job: Option<showcase::Job>,
    pub(crate) roots: Option<Roots>,
    /// the finished job's folders were added and the rescan started
    pub(crate) applied: bool,
    pub(crate) msg: String,
    /// tests swap in a canned downloader
    pub(crate) fetch: Arc<dyn showcase::Fetch>,
}

impl ShowcaseDlg {
    pub(crate) fn new(roots: Option<Roots>) -> Self {
        let ticked = ITEMS
            .iter()
            .map(|it| {
                it.auto()
                    && !it.big
                    && !roots.as_ref().is_some_and(|r| showcase::is_installed(r, it))
            })
            .collect();
        Self {
            ticked,
            job: None,
            roots,
            applied: false,
            msg: String::new(),
            fetch: Arc::new(showcase::Http),
        }
    }
    fn running(&self) -> bool {
        self.job.as_ref().is_some_and(|j| !j.finished())
    }
}

impl App {
    pub(super) fn open_showcase(&mut self) {
        if self.showcase.is_none() {
            self.showcase = Some(ShowcaseDlg::new(Roots::user()));
        }
    }

    /// start downloading the ticked items
    pub(super) fn showcase_start(&mut self) {
        let Some(d) = self.showcase.as_mut() else { return };
        let Some(roots) = d.roots.clone() else {
            d.msg = "Couldn't find your user folder (LOCALAPPDATA / HOME is not set).".into();
            return;
        };
        let ids: Vec<&'static str> = ITEMS
            .iter()
            .zip(&d.ticked)
            .filter(|(it, on)| **on && it.auto())
            .map(|(it, _)| it.id)
            .collect();
        if ids.is_empty() {
            return;
        }
        d.applied = false;
        d.msg.clear();
        d.job = Some(showcase::spawn(ids, roots, d.fetch.clone()));
    }

    /// once the job is done: add the folders, rescan, show them in the Browser
    pub(super) fn showcase_poll(&mut self) {
        let Some(d) = self.showcase.as_mut() else { return };
        let Some(j) = &d.job else { return };
        if !j.finished() || d.applied {
            return;
        }
        d.applied = true;
        let prog = j.progress.lock().unwrap().clone();
        let ok = prog.iter().filter(|p| p.state == State::Done).count();
        let skip = prog.iter().filter(|p| p.state == State::Skipped).count();
        let bad = prog.iter().filter(|p| matches!(p.state, State::Failed(_))).count();
        let roots = d.roots.clone();
        let summary;
        for p in prog.iter().filter(|p| matches!(p.state, State::Done | State::Skipped)) {
            if let Some(k) = ITEMS.iter().position(|i| i.id == p.id) {
                d.ticked[k] = false;
            }
        }
        d.msg = format!(
            "{ok} installed{}{} · scanning plugins…",
            if skip > 0 { format!(", {skip} already there") } else { String::new() },
            if bad > 0 { format!(", {bad} failed") } else { String::new() }
        );
        if ok + skip == 0 {
            d.msg = if bad > 0 { "Nothing was installed (see the errors above).".into() } else { "Cancelled.".into() };
            return;
        }
        summary = d.msg.trim_end_matches(" · scanning plugins…").to_string();
        if let Some(r) = roots {
            for dir in [&r.vst3, &r.clap] {
                let s = dir.display().to_string();
                if dir.exists() && !self.extra.plugin_dirs.contains(&s) {
                    self.extra.plugin_dirs.push(s);
                }
            }
        }
        self.save_settings();
        self.status = format!("Showcase instruments: {summary} · scanning plugins in the background");
        self.br.rev += 1;
        self.start_plugin_scan(true);
        self.showcase_reveal();
    }

    /// the Browser on Plugins, filtered to the Showcase tag
    fn showcase_reveal(&mut self) {
        self.open_browser_tab(crate::browser::Tab::Plugins);
        self.br.filter.tags = vec!["Showcase"];
        self.br_refresh();
    }

    pub(super) fn showcase_window(&mut self, ctx: &egui::Context) {
        self.showcase_poll();
        let Some(d) = self.showcase.as_ref() else { return };
        let t = th();
        let running = d.running();
        let prog: Vec<showcase::Progress> = d
            .job
            .as_ref()
            .map(|j| j.progress.lock().unwrap().clone())
            .unwrap_or_default();
        let mut ticked = d.ticked.clone();
        let installed: Vec<bool> = ITEMS
            .iter()
            .map(|it| d.roots.as_ref().is_some_and(|r| showcase::is_installed(r, it)))
            .collect();
        let msg = d.msg.clone();
        let roots = d.roots.clone();
        let mut open = true;
        // 1 start, 2 cancel, 3 close, 4 reveal
        let mut act: Option<u8> = None;
        let mut url: Option<&'static str> = None;
        egui::Window::new("Get showcase instruments")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(820.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Nine free, open-source instruments, made by their creators and downloaded from their own release pages. ChipWave checks each download's SHA-256, copies only the plugin into your user plugin folders (no admin needed) and puts the licence and a source-code note next to it.");
                ui.label(egui::RichText::new("They are not part of ChipWave: thanks to the creators below. Instruments whose creators only ship an installer program get an Open download page button instead, so nothing is run behind your back.").small().color(t.ink2));
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    egui::Grid::new("showcase-grid")
                        .striped(true)
                        .num_columns(7)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            for h in ["", "Instrument", "Category", "Creator", "Licence", "Size", ""] {
                                ui.label(egui::RichText::new(h).small().strong().color(t.ink2));
                            }
                            ui.end_row();
                            for (k, it) in ITEMS.iter().enumerate() {
                                let p = prog.iter().find(|p| p.id == it.id);
                                match it.get() {
                                    Get::Auto(_) => {
                                        ui.add_enabled(!running, egui::Checkbox::without_text(&mut ticked[k]));
                                    }
                                    Get::Page(_) => {
                                        ui.label("");
                                    }
                                }
                                ui.vertical(|ui| {
                                    ui.hyperlink_to(egui::RichText::new(it.name).strong(), it.source)
                                        .on_hover_text(format!("source code: {}", it.source));
                                    let mut sub = format!("{} · {}", it.version, it.formats);
                                    if !it.note.is_empty() {
                                        sub += &format!(" · {}", it.note);
                                    }
                                    ui.label(egui::RichText::new(sub).small().color(t.ink2));
                                });
                                ui.label(it.category);
                                ui.label(it.creator);
                                ui.label(it.licence);
                                ui.label(match it.get() {
                                    Get::Auto(_) => showcase::mb(it.size()),
                                    Get::Page(_) => "—".into(),
                                });
                                // status column
                                ui.vertical(|ui| {
                                    ui.set_min_width(190.0);
                                    match (it.get(), p) {
                                        (Get::Page(why), _) => {
                                            if ui.button("Open download page").on_hover_text(it.page).clicked() {
                                                url = Some(it.page);
                                            }
                                            ui.label(egui::RichText::new(why).small().color(t.ink2));
                                        }
                                        (Get::Auto(_), Some(p)) => match &p.state {
                                            State::Waiting => {
                                                ui.label(egui::RichText::new("waiting…").small().color(t.ink2));
                                            }
                                            State::Downloading | State::Verifying | State::Installing => {
                                                let what = match p.state {
                                                    State::Downloading => format!("{} / {}", showcase::mb(p.done), showcase::mb(p.total)),
                                                    State::Verifying => "checking SHA-256…".into(),
                                                    _ => "installing…".into(),
                                                };
                                                ui.add(egui::ProgressBar::new(p.fraction()).desired_width(180.0).text(what));
                                            }
                                            State::Done => {
                                                ui.label(egui::RichText::new("Installed").color(t.accent));
                                            }
                                            State::Skipped => {
                                                ui.label(egui::RichText::new("Already installed").color(t.accent));
                                            }
                                            State::Cancelled => {
                                                ui.label(egui::RichText::new("cancelled").small().color(t.ink2));
                                            }
                                            State::Failed(e) => {
                                                ui.label(egui::RichText::new(format!("Failed: {e}")).small().color(egui::Color32::from_rgb(0xe0, 0x60, 0x50)));
                                                if ui.small_button("Open download page").clicked() {
                                                    url = Some(it.page);
                                                }
                                            }
                                        },
                                        (Get::Auto(_), None) => {
                                            if installed[k] {
                                                ui.label(egui::RichText::new(format!("{} installed", it.version)).small().color(t.accent));
                                            } else if it.big {
                                                ui.label(egui::RichText::new("large download").small().color(t.ink2));
                                            }
                                        }
                                    }
                                });
                                ui.end_row();
                            }
                        });
                });
                ui.add_space(4.0);
                let n = ITEMS.iter().zip(&ticked).filter(|(i, on)| **on && i.auto()).count();
                let bytes: u64 = ITEMS.iter().zip(&ticked).filter(|(i, on)| **on && i.auto()).map(|(i, _)| i.size()).sum();
                if let Some(r) = &roots {
                    ui.label(egui::RichText::new(format!("Installs to {}  and  {}", r.vst3.display(), r.clap.display())).small().color(t.ink2));
                }
                if !msg.is_empty() {
                    ui.label(egui::RichText::new(&msg).color(t.accent));
                }
                ui.horizontal(|ui| {
                    if running {
                        ui.spinner();
                        if ui.button("Cancel").clicked() {
                            act = Some(2);
                        }
                    } else {
                        let lbl = if n == 0 {
                            "Download & install".to_string()
                        } else {
                            format!("Download & install {n} ({})", showcase::mb(bytes))
                        };
                        if ui.add_enabled(n > 0 && roots.is_some(), egui::Button::new(lbl)).clicked() {
                            act = Some(1);
                        }
                        if ui.button("Show in Browser").on_hover_text("the Plugins tab, filtered to Showcase").clicked() {
                            act = Some(4);
                        }
                        if ui.button("Close").clicked() {
                            act = Some(3);
                        }
                    }
                });
                if running {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
            });
        if let Some(u) = url {
            ctx.open_url(egui::OpenUrl::new_tab(u));
        }
        if let Some(d) = self.showcase.as_mut() {
            d.ticked = ticked;
        }
        if !open {
            act = Some(if running { 2 } else { 3 });
        }
        match act {
            Some(1) => self.showcase_start(),
            Some(2) => {
                if let Some(j) = self.showcase.as_ref().and_then(|d| d.job.as_ref()) {
                    j.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Some(3) => self.showcase = None,
            Some(4) => self.showcase_reveal(),
            _ => {}
        }
    }
}

/// `--install-showcase <id,id|all> [--root DIR]`: the same installer without
/// the window (Wine / CI tests). Prints one line per item.
pub fn cli_install(args: &[String], i: usize) -> i32 {
    let want = args.get(i + 1).cloned().unwrap_or_default();
    let roots = match args.iter().position(|a| a == "--root") {
        Some(k) => args.get(k + 1).map(|d| Roots::under(std::path::Path::new(d))),
        None => Roots::user(),
    };
    let Some(roots) = roots else {
        eprintln!("no user folder (LOCALAPPDATA / HOME unset)");
        return 2;
    };
    let ids: Vec<&'static str> = if want == "all" {
        ITEMS.iter().filter(|i| i.auto()).map(|i| i.id).collect()
    } else {
        want.split(',')
            .filter_map(|w| ITEMS.iter().find(|i| i.id == w.trim()).map(|i| i.id))
            .collect()
    };
    if ids.is_empty() {
        eprintln!(
            "usage: --install-showcase <{}|all> [--root DIR]",
            ITEMS.iter().map(|i| i.id).collect::<Vec<_>>().join(",")
        );
        return 2;
    }
    println!("VST3 → {}\nCLAP → {}", roots.vst3.display(), roots.clap.display());
    let mut code = 0;
    for id in ids {
        let it = showcase::item(id).unwrap();
        if let Get::Page(why) = it.get() {
            println!("{id}: open {} ({why})", it.page);
            continue;
        }
        let mut last = 0u64;
        let r = showcase::install(it, &roots, &showcase::Http, &std::sync::atomic::AtomicBool::new(false), &mut |s, d| {
            if let Some(d) = d {
                if d >= last + 5_000_000 || s != State::Downloading {
                    last = d;
                    println!("  {id}: {s:?} {}", showcase::mb(d));
                }
            }
        });
        match r {
            Ok(true) => println!("{id}: installed {}", it.version),
            Ok(false) => println!("{id}: already installed {}", it.version),
            Err(e) => {
                println!("{id}: FAILED: {e}");
                code = 1;
            }
        }
    }
    let mut ext = crate::settings::load();
    let mut changed = false;
    for d in [&roots.vst3, &roots.clap] {
        let s = d.display().to_string();
        if d.exists() && !ext.extra.plugin_dirs.contains(&s) {
            ext.extra.plugin_dirs.push(s);
            changed = true;
        }
    }
    if changed && args.iter().all(|a| a != "--root") {
        crate::settings::save(&ext);
    }
    code
}
