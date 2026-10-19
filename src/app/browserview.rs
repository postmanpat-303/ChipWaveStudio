//! The Browser (docs/BROWSER.md): a long panel docked on the right that
//! lists everything ChipWave can load (instruments, plugins, samples,
//! songs, presets, Grid patches, wavetables, MIDI files), with search, tag
//! chips, Category / Creator / Location filters, favourites and ratings.
//! Double-click or Enter loads, drag drops onto a timeline track, an
//! instrument slot, the FX view or the pattern editor, and instruments and
//! samples preview on C-3 through the current track.
use super::*;
use crate::browser::{self as br, Filter, Fmt, Item, LocFilter, Src, Tab};
use crate::instfile;
use crate::timeline as tl;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// what loading an instrument-like item does
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Act {
    /// a new instrument slot (becomes the current one)
    New,
    /// replace this slot's sound (its notes are kept)
    Replace(u8),
    /// give timeline track N this instrument
    Track(usize),
    /// a new track at index N playing it, named after it
    NewTrack(usize),
    /// audition on C-3 through the current track
    Preview,
}

/// where a Browser drag would land
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DropAt {
    Slot(u8),
    Track(usize, Option<tl::Tick>),
    NewTrack(Option<tl::Tick>),
    Fx,
    Central,
}

pub(crate) enum ScanMsg {
    Library(Vec<instfile::Entry>),
    Folder(String, Vec<br::FileHit>),
}

pub(crate) struct ScanJob {
    rx: mpsc::Receiver<ScanMsg>,
    count: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
}

pub(crate) struct LoadJob {
    rx: mpsc::Receiver<Result<Instrument, String>>,
    act: Act,
    name: String,
}

pub(crate) struct BrowserUi {
    pub(crate) filter: Filter,
    pub(crate) builtin: Vec<Item>,
    pub(crate) library: Vec<instfile::Entry>,
    pub(crate) index: Option<br::Index>,
    pub(crate) items: Vec<Item>,
    pub(crate) items_key: u64,
    pub(crate) results: Vec<usize>,
    pub(crate) results_key: u64,
    /// the selected item (its key)
    pub(crate) sel: Option<String>,
    pub(crate) scan: Option<ScanJob>,
    pub(crate) scanned: bool,
    pub(crate) plug_scanning: bool,
    pub(crate) rev: u64,
    /// the keyboard goes to the Browser (clicked into last)
    pub(crate) focus: bool,
    /// screen rects as last drawn: the panel, instrument slots, the view
    pub(crate) rect: Rect,
    pub(crate) slot_rects: Vec<(u8, Rect)>,
    pub(crate) central: Rect,
    pub(crate) preview_until: Option<Instant>,
    pub(crate) load: Option<LoadJob>,
    /// "Add track with instrument…": the next load makes a track here
    pub(crate) new_track_at: Option<usize>,
    /// the item being dragged out of the Browser
    pub(crate) drag: Option<Item>,
    pub(crate) scroll_sel: bool,
    pub(crate) search_focus: bool,
    /// the result list's scroll offset as last drawn
    pub(crate) list_off: f32,
    /// result rows as last drawn (item key, rect), for tests
    pub(crate) row_rects: Vec<(String, Rect)>,
    /// screenshot mode: hold the drag below the last track
    pub(crate) shot_drop: bool,
    /// Drum Patterns and Files tabs (app/drumpat.rs)
    pub(crate) pat: super::drumpat::PatUi,
    pub(crate) files: super::drumpat::FilesUi,
}

impl Default for BrowserUi {
    fn default() -> Self {
        Self {
            filter: Filter::default(),
            builtin: vec![],
            library: vec![],
            index: None,
            items: vec![],
            items_key: u64::MAX,
            results: vec![],
            results_key: u64::MAX,
            sel: None,
            scan: None,
            scanned: false,
            plug_scanning: false,
            rev: 0,
            focus: false,
            rect: Rect::NOTHING,
            slot_rects: vec![],
            central: Rect::NOTHING,
            preview_until: None,
            load: None,
            new_track_at: None,
            drag: None,
            scroll_sel: false,
            search_focus: false,
            list_off: 0.0,
            row_rects: vec![],
            shot_drop: false,
            pat: Default::default(),
            files: Default::default(),
        }
    }
}

fn hash_of<T: std::hash::Hash>(v: &T) -> u64 {
    use std::hash::{BuildHasher, BuildHasherDefault};
    BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(v)
}

/// a plugin instrument for a scanned plugin
pub(crate) fn plugin_instrument(f: &crate::plugin::scan::Found) -> Instrument {
    Instrument {
        name: f.desc.name.clone(),
        volume: 0.8,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Plugin(fxplugview::patch_of(f.clone())),
    }
}

/// instrument-like: loads as an instrument (not songs, effects, MIDI)
fn is_instrumentish(it: &Item) -> bool {
    match &it.src {
        Src::Plugin(f) => f.desc.is_instrument(),
        Src::GridFx(_) | Src::Demo(_) | Src::Pattern(_) => false,
        Src::File(_) => it.fmt == Fmt::Cwi || it.fmt.is_sample() || it.fmt == Fmt::Wavetable,
        _ => true,
    }
}

impl BrowserUi {
    /// files counted so far while a location scan runs
    pub(crate) fn scan_count(&self) -> Option<usize> {
        self.scan.as_ref().map(|j| j.count.load(Ordering::Relaxed))
    }
    pub(crate) fn preview_active(&self) -> bool {
        self.preview_until.is_some()
    }
}

impl App {
    pub(super) fn toggle_browser(&mut self) {
        self.extra.browser_open = !self.extra.browser_open;
        if !self.extra.browser_open {
            self.br.focus = false;
            self.br_stop_preview();
        }
        self.save_settings();
        self.status = if self.extra.browser_open {
            "Browser shown (Ctrl+B hides it)".into()
        } else {
            "Browser hidden (Ctrl+B shows it)".into()
        };
    }

    /// open the Browser on a tab (Add track with instrument…, Find plugins)
    pub(super) fn open_browser_tab(&mut self, tab: Tab) {
        if !self.extra.browser_open {
            self.extra.browser_open = true;
            self.save_settings();
        }
        self.br.filter.tab = tab;
        self.br.filter.category = None;
        self.br.filter.creator = None;
        self.br.focus = true;
        self.br.search_focus = true;
    }

    pub(super) fn br_active(&self) -> bool {
        self.extra.browser_open && self.br.focus
    }

    // ------------------------------------------------------------ data

    /// start the background scan of the library folder and the Browser
    /// locations (the cached index shows until it is done)
    pub(super) fn br_rescan(&mut self) {
        if let Some(j) = &self.br.scan {
            j.stop.store(true, Ordering::Relaxed);
        }
        let lib = instfile::library_dir(&self.extra.library_dir);
        let dirs = self.extra.browser_dirs.clone();
        let (tx, rx) = mpsc::channel();
        let count = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (c2, s2) = (count.clone(), stop.clone());
        let spawned = std::thread::Builder::new()
            .name("browser-scan".into())
            .spawn(move || {
                if let Some(d) = lib {
                    let _ = tx.send(ScanMsg::Library(instfile::scan(&d)));
                }
                for d in dirs {
                    if s2.load(Ordering::Relaxed) {
                        return;
                    }
                    let hits = br::scan_folder(std::path::Path::new(&d), &c2, &s2);
                    if tx.send(ScanMsg::Folder(d, hits)).is_err() {
                        return;
                    }
                }
            });
        if spawned.is_ok() {
            self.br.scan = Some(ScanJob { rx, count, stop });
        }
        self.br.scanned = true;
    }

    pub(super) fn br_poll(&mut self, ctx: &egui::Context) {
        if self.br.index.is_none() {
            let mut idx = br::load_index();
            idx.folders
                .retain(|k, _| self.extra.browser_dirs.iter().any(|d| d == k));
            self.br.index = Some(idx);
            self.br.rev += 1;
        }
        if !self.br.scanned && self.extra.browser_open {
            self.br_rescan();
        }
        let mut done = false;
        let mut changed = false;
        if let Some(j) = &self.br.scan {
            loop {
                match j.rx.try_recv() {
                    Ok(ScanMsg::Library(v)) => {
                        self.br.library = v;
                        changed = true;
                    }
                    Ok(ScanMsg::Folder(d, hits)) => {
                        if let Some(i) = self.br.index.as_mut() {
                            i.folders.insert(d, hits);
                        }
                        changed = true;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        done = true;
                        break;
                    }
                }
            }
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if done {
            self.br.scan = None;
            if let Some(i) = &self.br.index {
                br::save_index(i);
            }
        }
        if changed {
            self.br.rev += 1;
        }
        // the plugin scan finished: its results show
        let ps = self.plug.scan.is_some();
        if self.br.plug_scanning && !ps {
            self.br.rev += 1;
        }
        self.br.plug_scanning = ps;
        // decoded loads
        if let Some(j) = &self.br.load {
            match j.rx.try_recv() {
                Ok(r) => {
                    let j = self.br.load.take().unwrap();
                    match r {
                        Ok(inst) => self.br_place(inst, j.act),
                        Err(e) => self.status = format!("Couldn't load {}: {e}", j.name),
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(30));
                }
                Err(mpsc::TryRecvError::Disconnected) => self.br.load = None,
            }
        }
        if self.br.preview_until.is_some_and(|t| Instant::now() >= t) {
            self.br_stop_preview();
        }
        if self.br.preview_until.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// rebuild the item list when its sources changed
    fn br_items(&mut self) {
        let song_sig = self.with(|s| {
            hash_of(
                &s.song
                    .instruments
                    .iter()
                    .map(|i| (i.name.clone(), crate::instfile::kind_name(&i.kind)))
                    .collect::<Vec<_>>(),
            )
        });
        let key = hash_of(&(self.br.rev, song_sig, &self.extra.browser_dirs));
        if key == self.br.items_key {
            return;
        }
        self.br.items_key = key;
        if self.br.builtin.is_empty() {
            self.br.builtin = br::builtin_items();
        }
        let mut v = self.br.builtin.clone();
        v.extend(br::library_items(&self.br.library));
        v.extend(self.with(|s| br::song_items(&s.song)));
        let mut plugs = br::plugin_items(&crate::plugin::scan::cache());
        for p in &mut plugs {
            if let Src::Plugin(f) = &p.src {
                if let Some(d) = self
                    .extra
                    .browser_dirs
                    .iter()
                    .find(|d| f.path.starts_with(d.as_str()))
                {
                    p.loc = br::Loc::Folder(d.clone());
                }
            }
        }
        v.extend(plugs);
        if let Some(i) = &self.br.index {
            for (root, files) in &i.folders {
                v.extend(br::file_items(root, files));
            }
        }
        self.br.items = v;
        self.br.results_key = u64::MAX;
    }

    fn br_results(&mut self) {
        let key = hash_of(&(
            self.br.items_key,
            format!("{:?}", self.br.filter),
            &self.extra.browser_favs,
            &self.extra.browser_ratings,
        ));
        if key == self.br.results_key {
            return;
        }
        self.br.results_key = key;
        self.br.results = br::filter(
            &self.br.items,
            &self.br.filter,
            &self.extra.browser_favs,
            &self.extra.browser_ratings,
        );
    }

    /// the selected item
    pub(crate) fn br_selected(&self) -> Option<Item> {
        let k = self.br.sel.as_ref()?;
        self.br.items.iter().find(|i| &i.key == k).cloned()
    }

    /// refresh items and results (also called by tests)
    pub(crate) fn br_refresh(&mut self) {
        self.br_items();
        self.br_results();
    }

    pub(crate) fn br_result_items(&self) -> Vec<&Item> {
        self.br.results.iter().map(|&i| &self.br.items[i]).collect()
    }

    // ------------------------------------------------------------ actions

    pub(super) fn br_toggle_fav(&mut self, key: &str) {
        let f = &mut self.extra.browser_favs;
        if let Some(i) = f.iter().position(|k| k == key) {
            f.remove(i);
        } else {
            f.push(key.to_string());
        }
        self.save_settings();
    }

    pub(super) fn br_rate(&mut self, key: &str, stars: u8) {
        if stars == 0 || self.extra.browser_ratings.get(key) == Some(&stars) {
            self.extra.browser_ratings.remove(key);
        } else {
            self.extra
                .browser_ratings
                .insert(key.to_string(), stars.min(5));
        }
        self.save_settings();
    }

    pub(super) fn br_add_folder(&mut self, p: PathBuf) {
        let s = p.display().to_string();
        if !self.extra.browser_dirs.contains(&s) {
            self.extra.browser_dirs.push(s.clone());
            self.save_settings();
        }
        self.status = format!("Scanning {s} in the background…");
        self.br_rescan();
    }

    pub(super) fn br_remove_folder(&mut self, d: &str) {
        self.extra.browser_dirs.retain(|x| x != d);
        if let Some(i) = self.br.index.as_mut() {
            i.folders.remove(d);
            br::save_index(i);
        }
        if self.br.filter.loc == LocFilter::Folder(d.to_string()) {
            self.br.filter.loc = LocFilter::All;
        }
        self.save_settings();
        self.br.rev += 1;
    }

    /// the instrument an item stands for, then `act` with it (files are
    /// decoded on a worker thread)
    pub(crate) fn br_instrument(&mut self, it: &Item, act: Act) {
        match &it.src {
            Src::Plugin(f) if f.desc.is_instrument() => {
                if act == Act::Preview {
                    self.status = "Plugins aren't previewed: drop or double-click to load".into();
                    return;
                }
                self.br_place(plugin_instrument(f), act);
            }
            Src::SongInst(i) if matches!(act, Act::Track(_) | Act::NewTrack(_)) => {
                // the song's own instrument: no copy
                self.br_assign_slot((*i + 1) as u8, &it.name, act);
            }
            Src::File(_) => {
                let (tx, rx) = mpsc::channel();
                let it2 = it.clone();
                let spawned = std::thread::Builder::new()
                    .name("browser-load".into())
                    .spawn(move || {
                        let _ = tx.send(br::instrument_of(&it2, &Song::empty()));
                    });
                if let Err(e) = spawned {
                    self.status = format!("Couldn't load {}: {e}", it.name);
                    return;
                }
                if act != Act::Preview {
                    self.status = format!("Loading {}…", it.name);
                }
                self.br.load = Some(LoadJob {
                    rx,
                    act,
                    name: it.name.clone(),
                });
            }
            _ => match self.with(|s| br::instrument_of(it, &s.song)) {
                Ok(inst) => self.br_place(inst, act),
                Err(e) => self.status = format!("{}: {e}", it.name),
            },
        }
    }

    fn br_place(&mut self, inst: Instrument, act: Act) {
        let plugin = matches!(inst.kind, InstrumentKind::Plugin(_));
        match act {
            Act::Preview => {
                self.br_preview_inst(&inst);
                return;
            }
            Act::New => self.add_library_instrument(inst),
            Act::Replace(slot) => self.replace_instrument_sound(slot, inst),
            Act::Track(_) | Act::NewTrack(_) => {
                let name = inst.name.clone();
                self.snapshot();
                self.undo_hold += 1;
                let slot = self.with(|s| s.song.add_instrument(inst));
                match slot {
                    Some(i) => self.br_assign_slot((i + 1) as u8, &name, act),
                    None => self.status = "All 255 instrument slots are used".into(),
                }
                self.undo_hold -= 1;
            }
        }
        if plugin {
            let extra = (self.instr as usize).checked_sub(1).map(|i| (i, self.ch));
            crate::plugin::sync(&self.shared, extra);
            if matches!(act, Act::New) {
                self.default_macros();
            }
        }
    }

    /// give track `tr` (Act::Track) or a new track (Act::NewTrack) slot `slot`
    fn br_assign_slot(&mut self, slot: u8, name: &str, act: Act) {
        self.snapshot();
        self.undo_hold += 1;
        let tr = match act {
            Act::NewTrack(at) => {
                let tname: String = name.chars().take(24).collect();
                if !self.track_insert(at, &tname) {
                    self.undo_hold -= 1;
                    return;
                }
                at
            }
            Act::Track(tr) => tr,
            _ => {
                self.undo_hold -= 1;
                return;
            }
        };
        let n = self.with(|s| crate::chanops::assign_track_instrument(&mut s.song, tr, slot));
        self.undo_hold -= 1;
        self.instr = slot;
        self.ch = tr;
        self.dirty = true;
        crate::plugin::sync(&self.shared, Some(((slot - 1) as usize, tr)));
        self.status = match act {
            Act::NewTrack(_) => format!("New track {:02} {name} (instrument {slot:02X})", tr + 1),
            _ => format!(
                "Track {:02} now plays {slot:02X} {name} ({n} note{} changed)",
                tr + 1,
                if n == 1 { "" } else { "s" }
            ),
        };
    }

    fn br_preview_inst(&mut self, inst: &Instrument) {
        self.pat_stop();
        let ch = self.ch;
        self.with(|s| {
            let song = br::preview_song(inst, &s.song, ch);
            s.engine.stop();
            s.engine.start(&song, 0, 0, false, 0);
            s.preview = Some(song);
        });
        self.br.preview_until = Some(Instant::now() + Duration::from_millis(2400));
        self.status = format!("Preview: {} on C-3 through track {:02}", inst.name, ch + 1);
    }

    pub(crate) fn br_stop_preview(&mut self) {
        self.pat_stop();
        if self.br.preview_until.take().is_some() {
            self.with(|s| {
                if s.preview.is_some() {
                    s.preview = None;
                    s.engine.stop();
                }
            });
        }
    }

    pub(crate) fn br_preview(&mut self, it: &Item) {
        if let Src::Pattern(i) = it.src {
            self.pat_toggle(i);
            return;
        }
        if it.previewable() {
            self.br_instrument(it, Act::Preview);
        }
    }

    /// add an effect item to instrument `slot`'s FX chain
    fn br_effect(&mut self, it: &Item, slot: u8) {
        if slot == 0 || self.with(|s| s.song.instruments.len()) < slot as usize {
            self.status = "Pick an instrument first: effects go into its FX chain".into();
            return;
        }
        match &it.src {
            Src::Plugin(f) => self.add_plugin_fx(slot, fxplugview::patch_of(f.clone())),
            Src::GridFx(n) => {
                let Some(d) = br::grid_fx_device(n) else {
                    return;
                };
                self.snapshot();
                let idx = slot as usize - 1;
                let ok = self.with(|s| {
                    let i = &mut s.song.instruments[idx];
                    if i.fx.len() >= super::instruments::MAX_DEVICES {
                        return false;
                    }
                    i.fx.push(d);
                    true
                });
                self.status = if ok {
                    format!("Added Grid FX '{n}' to instrument {slot:02X}")
                } else {
                    "Chain is full (16 devices)".into()
                };
            }
            _ => {}
        }
    }

    /// Double-click / Enter: the item's natural load
    pub(crate) fn br_load(&mut self, it: &Item, ctx: &egui::Context) {
        self.br_stop_preview();
        if let Some(at) = self.br.new_track_at.take() {
            if is_instrumentish(it) {
                self.br_instrument(it, Act::NewTrack(at));
                return;
            }
        }
        if let Src::Pattern(i) = it.src {
            // a new drum track at the play marker
            self.pat_place(i, None, None);
            return;
        }
        if it.fmt.planned() {
            self.status = format!(
                "{}: {} import is planned (listed only)",
                it.name,
                it.fmt.what()
            );
            return;
        }
        if it.is_effect() {
            self.br_effect(it, self.instr);
            return;
        }
        match &it.src {
            Src::Demo(i) => self.request(crate::prompt::Pending::Demo(*i), ctx),
            Src::File(p) if it.fmt == Fmt::Song => {
                self.request(crate::prompt::Pending::OpenPath(p.clone()), ctx)
            }
            Src::File(p) if it.fmt.is_sample() && self.view == View::Timeline => {
                let (ch, cue) = (self.ch, self.tlv.cue);
                self.import_audio(p.clone(), ch, cue);
            }
            Src::Factory(_) | Src::Gen(_) if it.tab == Tab::Presets => {
                let has = self.with(|s| s.song.instruments.len()) >= self.instr as usize
                    && self.instr > 0;
                self.br_instrument(
                    it,
                    if has {
                        Act::Replace(self.instr)
                    } else {
                        Act::New
                    },
                );
            }
            Src::Table(i) => {
                if !self.br_set_table(Some(*i)) {
                    self.br_instrument(it, Act::New);
                }
            }
            _ => self.br_instrument(it, Act::New),
        }
    }

    /// the current instrument is a Wave generator: oscillator A plays
    /// built-in table `t`
    fn br_set_table(&mut self, t: Option<u8>) -> bool {
        let Some(t) = t else { return false };
        let idx = (self.instr as usize).wrapping_sub(1);
        let is_wave = self.with(|s| {
            matches!(
                s.song.instruments.get(idx).map(|i| &i.kind),
                Some(InstrumentKind::Gen(g)) if matches!(g.engine, crate::gen::GenEngine::Wave(_))
            )
        });
        if !is_wave {
            return false;
        }
        self.snapshot();
        self.with(|s| {
            if let Some(InstrumentKind::Gen(g)) =
                s.song.instruments.get_mut(idx).map(|i| &mut i.kind)
            {
                if let crate::gen::GenEngine::Wave(w) = &mut g.engine {
                    w.tables[0] = t;
                }
            }
        });
        let name = crate::wavetable::BUILTIN
            .get(t as usize)
            .copied()
            .unwrap_or("?");
        self.status = format!("Instrument {:02X} oscillator A: {name}", self.instr);
        true
    }

    /// where a drop at `p` lands
    pub(crate) fn br_drop_at(&self, p: Pos2) -> Option<DropAt> {
        if self.br.rect.contains(p) {
            return None;
        }
        if let Some((s, _)) = self.br.slot_rects.iter().find(|(_, r)| r.contains(p)) {
            return Some(DropAt::Slot(*s));
        }
        if !self.br.central.contains(p) {
            return None;
        }
        if self.view == View::Timeline {
            if let Some((r, tick)) = self.tlv.drop_at(p) {
                return Some(match r {
                    Ok(tr) => DropAt::Track(tr, tick),
                    Err(_) => DropAt::NewTrack(tick),
                });
            }
        }
        if self.view == View::Fx {
            return Some(DropAt::Fx);
        }
        Some(DropAt::Central)
    }

    /// what dropping `it` at `d` does (hint text; None = nothing)
    pub(super) fn br_drop_hint(&self, it: &Item, d: &DropAt) -> Option<String> {
        let inst = is_instrumentish(it);
        let n = self.with(|s| s.song.channels);
        if let Src::Pattern(i) = it.src {
            return self.pat_drop_hint(i, d);
        }
        Some(match d {
            DropAt::Slot(s) if inst => format!("Replace instrument {s:02X}"),
            DropAt::Slot(s) if it.is_effect() => format!("Add to instrument {s:02X}'s FX chain"),
            DropAt::Track(tr, Some(_)) if it.fmt.is_sample() && matches!(it.src, Src::File(_)) => {
                format!("Audio clip on track {:02}", tr + 1)
            }
            DropAt::Track(tr, _) if inst => format!("Track {:02} plays {}", tr + 1, it.name),
            DropAt::Track(tr, _) if it.is_effect() => {
                format!("Add to track {:02}'s instrument FX chain", tr + 1)
            }
            DropAt::NewTrack(_) if inst && n >= crate::model::MAX_CHANNELS => {
                "All 32 tracks are used".into()
            }
            DropAt::NewTrack(_) if inst => format!("New track: {}", it.name),
            DropAt::Fx | DropAt::Central if it.is_effect() => {
                format!("Add to instrument {:02X}'s FX chain", self.instr)
            }
            DropAt::Central if inst => "Add as a new instrument".into(),
            DropAt::Central if matches!(it.src, Src::Demo(_)) || it.fmt == Fmt::Song => {
                "Open this song".into()
            }
            _ => return None,
        })
    }

    /// the instrument track `tr` mostly plays (else the current one)
    pub(super) fn track_slot(&self, tr: usize) -> u8 {
        let from_track = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.tracks.get(tr))
                .map(|t| t.instr)
                .unwrap_or(0)
        });
        if from_track > 0 {
            return from_track;
        }
        let mut count = [0usize; 256];
        self.with(|s| {
            for p in &s.song.patterns {
                for r in &p.cells {
                    if let Some(c) = r.get(tr) {
                        if c.instr > 0 {
                            count[c.instr as usize] += 1;
                        }
                    }
                }
            }
        });
        match (1..256).max_by_key(|&i| count[i]) {
            Some(i) if count[i] > 0 => i as u8,
            _ => self.instr,
        }
    }

    pub(crate) fn br_drop(&mut self, it: &Item, d: DropAt, ctx: &egui::Context) {
        if let Src::Pattern(i) = it.src {
            // the preview keeps playing until the clip is placed
            self.pat_stop();
            self.pat_drop(i, d);
            return;
        }
        self.br_stop_preview();
        let inst = is_instrumentish(it);
        match d {
            DropAt::Slot(s) if inst => self.br_instrument(it, Act::Replace(s)),
            DropAt::Slot(s) if it.is_effect() => self.br_effect(it, s),
            DropAt::Track(tr, Some(tick)) if it.fmt.is_sample() => {
                if let Src::File(p) = &it.src {
                    self.import_audio(p.clone(), tr, tick);
                }
            }
            DropAt::Track(tr, _) if inst => self.br_instrument(it, Act::Track(tr)),
            DropAt::Track(tr, _) if it.is_effect() => {
                let s = self.track_slot(tr);
                self.br_effect(it, s)
            }
            DropAt::NewTrack(_) if inst => {
                let n = self.with(|s| s.song.channels);
                if n >= crate::model::MAX_CHANNELS {
                    self.status =
                        "All 32 tracks are used (the channel limit): drop onto a track instead"
                            .into();
                } else {
                    self.br_instrument(it, Act::NewTrack(n));
                }
            }
            DropAt::Fx | DropAt::Central if it.is_effect() => self.br_effect(it, self.instr),
            DropAt::Central if inst => self.br_instrument(it, Act::New),
            DropAt::Central => self.br_load(it, ctx),
            _ => {}
        }
    }

    // ------------------------------------------------------------ keys

    pub(super) fn br_key(
        &mut self,
        key: egui::Key,
        m: egui::Modifiers,
        ctx: &egui::Context,
    ) -> bool {
        use egui::Key;
        if m.command || m.alt {
            return false;
        }
        let n = self.br.results.len();
        let cur = self.br.sel.as_ref().and_then(|k| {
            self.br
                .results
                .iter()
                .position(|&i| &self.br.items[i].key == k)
        });
        let go = |this: &mut App, k: usize| {
            if let Some(&i) = this.br.results.get(k) {
                let it = this.br.items[i].clone();
                this.br.sel = Some(it.key.clone());
                this.br.scroll_sel = true;
                if this.extra.browser_autoplay {
                    this.br_preview(&it);
                }
            }
        };
        match key {
            Key::ArrowDown if n > 0 => go(self, cur.map(|c| (c + 1).min(n - 1)).unwrap_or(0)),
            Key::ArrowUp if n > 0 => go(self, cur.map(|c| c.saturating_sub(1)).unwrap_or(0)),
            Key::Home if n > 0 => go(self, 0),
            Key::End if n > 0 => go(self, n - 1),
            Key::Enter => {
                if let Some(it) = self.br_selected() {
                    self.br_load(&it, ctx);
                }
            }
            Key::Space if m.shift || self.br_selected().is_some_and(|i| i.previewable()) => {
                if self.br.preview_until.is_some() {
                    self.br_stop_preview();
                } else if let Some(it) = self.br_selected() {
                    self.br_preview(&it);
                }
            }
            Key::Escape => {
                self.br.focus = false;
                self.br_stop_preview();
            }
            _ => return false,
        }
        true
    }
}
