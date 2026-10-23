//! "Find plugins" (docs/PLUGINS.md): the first-run plugin step, also in
//! Settings › Plugins and the Browser's Plugins tab. Scan the standard
//! VST3 / CLAP folders (optionally the whole drive) in the background, or
//! pick folders by hand, or skip. Found folders go to the plugin folders
//! and the Browser locations, then the plugin scan runs.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Choose,
    Searching,
    Found,
    Pick,
}

pub(crate) struct SearchJob {
    rx: mpsc::Receiver<Vec<(PathBuf, usize)>>,
    pub(crate) visited: Arc<AtomicUsize>,
    now: Arc<Mutex<String>>,
    stop: Arc<AtomicBool>,
}

pub(crate) struct PlugSetup {
    pub(crate) step: Step,
    pub(crate) deep: bool,
    /// first run (or the one-time prompt for existing users)
    pub(crate) first: bool,
    /// folders found (with plugin counts) and whether to add each
    pub(crate) found: Vec<(PathBuf, usize, bool)>,
    /// folders picked by hand
    pub(crate) picked: Vec<PathBuf>,
    pub(crate) job: Option<SearchJob>,
    /// tests: search these instead of the standard folders / drives
    pub(crate) roots: Option<Vec<PathBuf>>,
    pub(crate) msg: String,
}

impl PlugSetup {
    pub(crate) fn new(first: bool) -> Self {
        Self {
            step: Step::Choose,
            deep: false,
            first,
            found: vec![],
            picked: vec![],
            job: None,
            roots: None,
            msg: String::new(),
        }
    }
}

impl App {
    pub(super) fn open_plugin_setup(&mut self, first: bool) {
        if self.psetup.is_none() {
            self.psetup = Some(PlugSetup::new(first));
        }
    }

    /// start the background search (standard folders, then the drive)
    pub(super) fn psetup_search(&mut self) {
        let Some(ps) = self.psetup.as_mut() else {
            return;
        };
        let (std_roots, deep_roots) = match &ps.roots {
            Some(r) => (r.clone(), vec![]),
            None => (
                crate::plugin::scan::default_dirs(),
                if ps.deep {
                    crate::plugin::scan::deep_roots()
                } else {
                    vec![]
                },
            ),
        };
        let (tx, rx) = mpsc::channel();
        let visited = Arc::new(AtomicUsize::new(0));
        let now = Arc::new(Mutex::new(String::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (v2, n2, s2) = (visited.clone(), now.clone(), stop.clone());
        let spawned = std::thread::Builder::new()
            .name("find-plugins".into())
            .spawn(move || {
                use crate::plugin::scan::find_plugin_folders;
                let mut out = find_plugin_folders(&std_roots, 6, 200_000, &v2, &n2, &s2);
                if !deep_roots.is_empty() {
                    for f in find_plugin_folders(&deep_roots, 14, 2_000_000, &v2, &n2, &s2) {
                        if !out.iter().any(|(p, _)| p == &f.0 || f.0.starts_with(p)) {
                            out.push(f);
                        }
                    }
                }
                let _ = tx.send(out);
            });
        match spawned {
            Ok(_) => {
                ps.job = Some(SearchJob {
                    rx,
                    visited,
                    now,
                    stop,
                });
                ps.step = Step::Searching;
            }
            Err(e) => ps.msg = format!("Couldn't start the search: {e}"),
        }
    }

    /// poll the search (also called by tests)
    pub(super) fn psetup_poll(&mut self) {
        let Some(ps) = self.psetup.as_mut() else {
            return;
        };
        let Some(j) = &ps.job else { return };
        match j.rx.try_recv() {
            Ok(v) => {
                ps.found = v.into_iter().map(|(p, n)| (p, n, true)).collect();
                ps.job = None;
                ps.step = Step::Found;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                ps.job = None;
                ps.step = Step::Found;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    /// add folders to the plugin folders and Browser locations, finish
    pub(super) fn psetup_apply(&mut self, dirs: Vec<PathBuf>) {
        let mut added = 0;
        for d in &dirs {
            let s = d.display().to_string();
            if !self.extra.plugin_dirs.contains(&s) {
                self.extra.plugin_dirs.push(s.clone());
                added += 1;
            }
            if !self.extra.browser_dirs.contains(&s) {
                self.extra.browser_dirs.push(s);
            }
        }
        self.extra.plugin_setup_done = true;
        self.save_settings();
        self.psetup = None;
        self.status = format!(
            "{} plugin folder{} added ({} new) · scanning plugins in the background",
            dirs.len(),
            if dirs.len() == 1 { "" } else { "s" },
            added
        );
        self.br.rev += 1;
        if !dirs.is_empty() {
            self.br_rescan();
            self.start_plugin_scan(false);
        }
    }

    fn psetup_skip(&mut self) {
        if let Some(j) = self.psetup.as_ref().and_then(|p| p.job.as_ref()) {
            j.stop.store(true, Ordering::Relaxed);
        }
        self.extra.plugin_setup_done = true;
        self.save_settings();
        self.psetup = None;
        self.status = "Plugin setup skipped: Settings › Plugins › Find plugins… any time".into();
    }

    pub(super) fn plugin_setup_window(&mut self, ctx: &egui::Context) {
        self.psetup_poll();
        let Some(ps) = self.psetup.as_ref() else {
            return;
        };
        let step = ps.step;
        let first = ps.first;
        let t = th();
        let mut open = true;
        let mut act: Option<u8> = None; // 1 search, 2 pick, 3 skip, 4 apply found, 5 apply picked, 6 cancel search, 7 back, 8 add folder
        let mut remove: Option<usize> = None;
        let mut deep = ps.deep;
        let mut found = ps.found.clone();
        egui::Window::new(if first { "Welcome · find your plugins" } else { "Find plugins" })
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| match step {
                Step::Choose => {
                    ui.label("ChipWave can load VST3 and CLAP plugins as instruments and effects. How should it find yours?");
                    ui.add_space(6.0);
                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new("1  Scan automatically").strong());
                        ui.label(egui::RichText::new(std_dirs_text()).small().color(t.ink2));
                        ui.checkbox(&mut deep, "Also search the whole drive for folders with .vst3 / .clap (slower)");
                        if ui.button("Scan now").clicked() {
                            act = Some(1);
                        }
                    });
                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new("2  Choose folders myself").strong());
                        ui.label(egui::RichText::new("pick the folders your plugins are installed in").small().color(t.ink2));
                        if ui.button("Choose folders…").clicked() {
                            act = Some(2);
                        }
                    });
                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new("3  Skip for now").strong());
                        ui.label(egui::RichText::new("later: Settings › Plugins › Find plugins…, or the Browser's VST & CLAP tab").small().color(t.ink2));
                        if ui.button("Skip").clicked() {
                            act = Some(3);
                        }
                    });
                }
                Step::Searching => {
                    let (n, now) = self
                        .psetup
                        .as_ref()
                        .and_then(|p| p.job.as_ref())
                        .map(|j| (j.visited.load(Ordering::Relaxed), j.now.lock().map(|g| g.clone()).unwrap_or_default()))
                        .unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Searching… {n} folders looked at"));
                    });
                    ui.add(egui::Label::new(egui::RichText::new(now).small().color(t.ink2)).truncate());
                    ui.label(egui::RichText::new("This runs in the background: keep working, the result shows here.").small().color(t.ink2));
                    if ui.button("Stop").clicked() {
                        act = Some(6);
                    }
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
                Step::Found => {
                    if found.is_empty() {
                        ui.label("No VST3 or CLAP plugins were found in those places.");
                        ui.label(egui::RichText::new("Try the whole-drive search, or choose the folders yourself.").small().color(t.ink2));
                    } else {
                        let total: usize = found.iter().map(|f| f.1).sum();
                        ui.label(format!("Found {total} plugin{} in {} folder{}:", if total == 1 { "" } else { "s" }, found.len(), if found.len() == 1 { "" } else { "s" }));
                        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                            for (p, n, on) in found.iter_mut() {
                                ui.checkbox(on, format!("{}   ({n})", p.display()));
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        let k = found.iter().filter(|f| f.2).count();
                        if ui.add_enabled(k > 0, egui::Button::new(format!("Add {k} folder{}", if k == 1 { "" } else { "s" }))).clicked() {
                            act = Some(4);
                        }
                        if ui.button("Back").clicked() {
                            act = Some(7);
                        }
                        if ui.button("Close").clicked() {
                            act = Some(3);
                        }
                    });
                }
                Step::Pick => {
                    let picked = self.psetup.as_ref().map(|p| p.picked.clone()).unwrap_or_default();
                    ui.label("Plugin folders:");
                    if picked.is_empty() {
                        ui.label(egui::RichText::new("none yet").small().color(t.ink2));
                    }
                    for (k, p) in picked.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.small_button("×").on_hover_text("remove").clicked() {
                                remove = Some(k);
                            }
                            ui.label(p.display().to_string());
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Add folder…").clicked() {
                            act = Some(8);
                        }
                        if ui.add_enabled(!picked.is_empty(), egui::Button::new("Done")).clicked() {
                            act = Some(5);
                        }
                        if ui.button("Back").clicked() {
                            act = Some(7);
                        }
                    });
                }
            });
        if let Some(ps) = self.psetup.as_mut() {
            ps.deep = deep;
            ps.found = found;
            if let Some(k) = remove {
                ps.picked.remove(k);
            }
        }
        if !open {
            self.psetup_skip();
            return;
        }
        match act {
            Some(1) => self.psetup_search(),
            Some(2) => {
                if let Some(ps) = self.psetup.as_mut() {
                    ps.step = Step::Pick;
                }
            }
            Some(3) => self.psetup_skip(),
            Some(4) => {
                let dirs: Vec<PathBuf> = self
                    .psetup
                    .as_ref()
                    .map(|p| {
                        p.found
                            .iter()
                            .filter(|f| f.2)
                            .map(|f| f.0.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                self.psetup_apply(dirs);
            }
            Some(5) => {
                let dirs = self
                    .psetup
                    .as_ref()
                    .map(|p| p.picked.clone())
                    .unwrap_or_default();
                self.psetup_apply(dirs);
            }
            Some(6) => {
                if let Some(j) = self.psetup.as_ref().and_then(|p| p.job.as_ref()) {
                    j.stop.store(true, Ordering::Relaxed);
                }
            }
            Some(7) => {
                if let Some(ps) = self.psetup.as_mut() {
                    ps.step = Step::Choose;
                }
            }
            Some(8) => {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("Add a plugin folder")
                    .pick_folder()
                {
                    if let Some(ps) = self.psetup.as_mut() {
                        if !ps.picked.contains(&p) {
                            ps.picked.push(p);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// the standard folders, for the dialog
fn std_dirs_text() -> String {
    let v: Vec<String> = crate::plugin::scan::default_dirs()
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    format!("checks the standard folders first: {}", v.join(" · "))
}
