//! Plugin scanner: finds `.clap` and `.vst3` files in the standard
//! and user folders, checks each one in a separate process
//! (`chipwave --scan-plugin <file>`) so a crashing or hanging plugin cannot
//! take ChipWave down, and caches the results by path, size and modification
//! time in `plugin-cache.json` next to the settings. Out-of-process scanning
//! follows the approach of Carla (falkTX) and Ardour (no code copied).
use super::clap::{ClapLib, Desc};
use super::PluginPatch;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// bump when the scan logic changes: every file is checked again
const CACHE_VERSION: u32 = 2;
pub const TIMEOUT: Duration = Duration::from_secs(20);

/// per-file scan timeout (`CHIPWAVE_SCAN_TIMEOUT` seconds overrides, for tests)
pub fn timeout() -> Duration {
    std::env::var("CHIPWAVE_SCAN_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .map(Duration::from_secs_f64)
        .unwrap_or(TIMEOUT)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Found {
    pub format: String,
    pub path: String,
    #[serde(flatten)]
    pub desc: Desc,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub format: String,
    pub mtime: u64,
    pub size: u64,
    /// "ok", "failed", "crashed", "timeout", "unsupported"
    pub status: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub plugins: Vec<Found>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cache {
    pub version: u32,
    pub entries: Vec<Entry>,
}

impl Cache {
    pub fn instruments(&self) -> Vec<Found> {
        let mut v: Vec<Found> = self
            .entries
            .iter()
            .filter(|e| e.status == "ok")
            .flat_map(|e| e.plugins.iter().filter(|p| p.desc.is_instrument()).cloned())
            .collect();
        v.sort_by_key(|f| (f.desc.name.to_lowercase(), f.format.clone()));
        v
    }
}

impl Cache {
    /// scanned audio effects (FX chain menu)
    pub fn effects(&self) -> Vec<Found> {
        let mut v: Vec<Found> = self
            .entries
            .iter()
            .filter(|e| e.status == "ok")
            .flat_map(|e| e.plugins.iter().filter(|p| p.desc.is_effect()).cloned())
            .collect();
        v.sort_by_key(|f| (f.desc.name.to_lowercase(), f.format.clone()));
        v
    }
}

/// letters and digits only, lower case ("Surge XT" == "surge-xt")
fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The bundle's main plugin among the classes one file holds: an
/// instrument first, then the class named like the bundle (exact, then a
/// name that contains it or is contained in it), then the shortest name
/// ("Kontakt 8" before "Kontakt 8 16out"), then file order. MIDI-only and
/// utility classes come last. `None` for an empty list.
pub fn main_class(path: &str, plugins: &[Found]) -> Option<usize> {
    let stem = Path::new(path)
        .file_stem()
        .map(|s| norm(&s.to_string_lossy()))
        .unwrap_or_default();
    (0..plugins.len()).min_by_key(|&i| {
        let d = &plugins[i].desc;
        let n = norm(&d.name);
        let kind = if d.is_instrument() {
            0
        } else if d.is_effect() {
            1
        } else {
            2
        };
        let named = if !stem.is_empty() && n == stem {
            0
        } else if !stem.is_empty() && !n.is_empty() && (n.contains(&stem) || stem.contains(&n)) {
            1
        } else {
            2
        };
        (kind, named, n.len(), i)
    })
}

impl Entry {
    /// this file's main plugin (see [`main_class`])
    pub fn main(&self) -> Option<&Found> {
        main_class(&self.path, &self.plugins).map(|i| &self.plugins[i])
    }
    /// classes listed only with "Show all programs": every class but the
    /// main one, when the file holds an instrument (a file of effects only
    /// keeps them all)
    pub fn is_sub_program(&self, f: &Found) -> bool {
        self.plugins.len() > 1
            && self.plugins.iter().any(|p| p.desc.is_instrument())
            && self.main().is_some_and(|m| m.desc.id != f.desc.id)
    }
}

/// id of the main plugin of the scanned file at `path` (None: not scanned)
pub fn main_id(path: &str, format: &str) -> Option<String> {
    let g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let find = |c: &Cache| {
        c.entries
            .iter()
            .find(|e| e.status == "ok" && e.path == path && e.format == format)
            .and_then(|e| e.main().map(|m| m.desc.id.clone()))
    };
    match g.as_ref() {
        Some(c) => find(c),
        None => {
            drop(g);
            find(&cache())
        }
    }
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

/// `CHIPWAVE_PLUGIN_CACHE` (tests) or <config>/plugin-cache.json
pub fn cache_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CHIPWAVE_PLUGIN_CACHE") {
        return Some(PathBuf::from(p));
    }
    crate::settings::config_dir().map(|d| d.join("plugin-cache.json"))
}

pub fn cache() -> Cache {
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        let c = cache_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Cache>(&t).ok())
            .filter(|c| c.version == CACHE_VERSION)
            .unwrap_or_default();
        *g = Some(c);
    }
    g.clone().unwrap_or_default()
}

pub fn set_cache(c: Cache) {
    if let Some(p) = cache_path() {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(t) = serde_json::to_string_pretty(&c) {
            let _ = std::fs::write(p, t);
        }
    }
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some(c);
}

/// standard plugin folders for this OS (CLAP and VST3)
pub fn default_dirs() -> Vec<PathBuf> {
    let mut v = vec![];
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if cfg!(windows) {
        if let Some(c) = env("COMMONPROGRAMFILES") {
            v.push(c.join("CLAP"));
            v.push(c.join("VST3"));
        }
        if let Some(l) = env("LOCALAPPDATA") {
            v.push(l.join("Programs").join("Common").join("CLAP"));
            v.push(l.join("Programs").join("Common").join("VST3"));
        }
    } else if cfg!(target_os = "macos") {
        v.push("/Library/Audio/Plug-Ins/CLAP".into());
        v.push("/Library/Audio/Plug-Ins/VST3".into());
        if let Some(h) = env("HOME") {
            v.push(h.join("Library/Audio/Plug-Ins/CLAP"));
            v.push(h.join("Library/Audio/Plug-Ins/VST3"));
        }
    } else {
        if let Some(h) = env("HOME") {
            v.push(h.join(".clap"));
            v.push(h.join(".vst3"));
        }
        for d in [
            "/usr/lib/clap",
            "/usr/local/lib/clap",
            "/usr/lib/vst3",
            "/usr/local/lib/vst3",
        ] {
            v.push(d.into());
        }
    }
    for k in ["CLAP_PATH", "VST3_PATH"] {
        if let Some(cp) = std::env::var_os(k) {
            v.extend(std::env::split_paths(&cp));
        }
    }
    v
}

/// every `.clap` / `.vst3` under `dirs` (recursive, bundles count as one)
pub fn find_files(dirs: &[PathBuf]) -> Vec<(PathBuf, &'static str)> {
    fn walk(d: &Path, depth: usize, out: &mut Vec<(PathBuf, &'static str)>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        let mut items: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        items.sort();
        for p in items {
            let ext = p
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            match ext.as_str() {
                "clap" => out.push((p, "clap")),
                "vst3" => out.push((p, "vst3")),
                _ if p.is_dir() && depth < 6 => walk(&p, depth + 1, out),
                _ => {}
            }
        }
    }
    let mut out = vec![];
    for d in dirs {
        walk(d, 0, &mut out);
    }
    out.dedup();
    out
}

fn stamp(p: &Path) -> (u64, u64) {
    // a bundle folder changes when its binary does
    let m = std::fs::metadata(super::clap::binary_path(p)).ok();
    let mtime = m
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (mtime, m.map(|m| m.len()).unwrap_or(0))
}

/// What the `--scan-plugin` child prints (one JSON line).
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub plugins: Vec<Found>,
}

/// In-process check of one file (runs inside the `--scan-plugin` child):
/// load, list the descriptors, create + activate + render a note on each
/// instrument and check the output is finite.
pub fn check_file(path: &Path) -> Report {
    let format = match path.extension().and_then(|e| e.to_str()) {
        Some(e) if e.eq_ignore_ascii_case("clap") => "clap",
        Some(e) if e.eq_ignore_ascii_case("vst3") => "vst3",
        _ => "?",
    };
    let fail = |message: String| Report {
        ok: false,
        message,
        plugins: vec![],
    };
    let descs = match format {
        "clap" => match ClapLib::load(path) {
            Ok(l) => l.descriptors(),
            Err(e) => return fail(e),
        },
        "vst3" => match super::vst3::Vst3Lib::load(path) {
            Ok(l) => l.descriptors(),
            Err(e) => return fail(e),
        },
        _ => return fail("not a CLAP or VST3 plugin".into()),
    };
    let mut plugins = vec![];
    for d in descs {
        if d.is_instrument() {
            use super::Hosted;
            let made: Result<Box<dyn Hosted>, String> = if format == "vst3" {
                super::vst3::Vst3Inst::new(path, &d.id, 48000.0)
                    .map(|i| Box::new(i) as Box<dyn Hosted>)
            } else {
                super::clap::ClapInst::new(path, &d.id, 48000.0)
                    .map(|i| Box::new(i) as Box<dyn Hosted>)
            };
            let mut inst = match made {
                Ok(i) => i,
                Err(e) => {
                    return Report {
                        ok: false,
                        message: format!("{}: {e}", d.name),
                        plugins: vec![],
                    }
                }
            };
            let (mut l, mut r) = (vec![0.0; 512], vec![0.0; 512]);
            // this thread plays the audio thread for the test render
            super::clap::AUDIO_THREAD.with(|a| a.set(true));
            inst.note_on(60, 0.8);
            inst.process(512, &mut l, &mut r);
            inst.note_off(60);
            inst.process(512, &mut l, &mut r);
            super::clap::AUDIO_THREAD.with(|a| a.set(false));
            if l.iter().chain(r.iter()).any(|x| !x.is_finite()) {
                return Report {
                    ok: false,
                    message: format!("{}: output is not finite", d.name),
                    plugins: vec![],
                };
            }
        }
        plugins.push(Found {
            format: format.into(),
            path: path.to_string_lossy().into_owned(),
            desc: d,
        });
    }
    Report {
        ok: true,
        message: String::new(),
        plugins,
    }
}

/// Check one file in a child process (`exe --scan-plugin <file>`).
pub fn scan_file(exe: &Path, path: &Path, format: &str, timeout: Duration) -> Entry {
    let (mtime, size) = stamp(path);
    let mut e = Entry {
        path: path.to_string_lossy().into_owned(),
        format: format.into(),
        mtime,
        size,
        status: "failed".into(),
        message: String::new(),
        plugins: vec![],
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--scan-plugin")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            e.message = format!("could not start the scanner: {err}");
            return e;
        }
    };
    let mut out = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(o) = out.as_mut() {
            use std::io::Read;
            let _ = o.read_to_string(&mut s);
        }
        s
    });
    let t0 = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if t0.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => break None,
        }
    };
    let text = reader.join().unwrap_or_default();
    let Some(status) = status else {
        e.status = "timeout".into();
        e.message = format!("no answer within {} s", timeout.as_secs());
        return e;
    };
    let rep = text
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Report>(l.trim()).ok());
    match rep {
        Some(r) if r.ok => {
            e.status = "ok".into();
            e.plugins = r.plugins;
        }
        Some(r) => e.message = r.message,
        None => {
            e.status = "crashed".into();
            e.message = match status.code() {
                Some(c) => format!("the plugin crashed the scanner (exit code {c})"),
                None => "the plugin crashed the scanner".into(),
            };
        }
    }
    e
}

/// Scan `dirs`, reusing cache entries whose file did not change (all files
/// again with `force`). `progress(done, total, file)` is called per file.
pub fn scan(
    exe: &Path,
    dirs: &[PathBuf],
    old: &Cache,
    force: bool,
    progress: &dyn Fn(usize, usize, &str),
) -> Cache {
    let files = find_files(dirs);
    let mut c = Cache {
        version: CACHE_VERSION,
        entries: vec![],
    };
    for (k, (p, fmt)) in files.iter().enumerate() {
        let ps = p.to_string_lossy().into_owned();
        progress(k, files.len(), &ps);
        let (mtime, size) = stamp(p);
        let reuse = (!force && old.version == CACHE_VERSION)
            .then(|| {
                old.entries
                    .iter()
                    .find(|e| e.path == ps && e.mtime == mtime && e.size == size)
            })
            .flatten();
        c.entries.push(match reuse {
            Some(e) => e.clone(),
            None => scan_file(exe, p, fmt, timeout()),
        });
    }
    progress(files.len(), files.len(), "");
    c
}

/// the file a song's plugin lives in on this computer
pub fn resolve(p: &PluginPatch) -> Option<PathBuf> {
    let direct = PathBuf::from(&p.path);
    if !p.path.is_empty() && direct.exists() {
        return Some(direct);
    }
    cache()
        .entries
        .iter()
        .filter(|e| e.status == "ok" && e.format == p.format)
        .flat_map(|e| e.plugins.iter())
        .find(|f| f.desc.id == p.id && Path::new(&f.path).exists())
        .map(|f| PathBuf::from(&f.path))
}

/// all folders to scan: the standard ones plus the user's
pub fn all_dirs(user: &[String]) -> Vec<PathBuf> {
    let mut v = default_dirs();
    for p in user
        .iter()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
    {
        // the plugin setup may add a standard folder too: scan it once
        if !v.contains(&p) {
            v.push(p);
        }
    }
    v
}

/// First-run plugin setup (docs/PLUGINS.md "Find plugins"): the folders a
/// deep drive search starts from (every drive on Windows; the usual
/// install roots on macOS and Linux)
pub fn deep_roots() -> Vec<PathBuf> {
    let mut v = vec![];
    if cfg!(windows) {
        for c in b'A'..=b'Z' {
            let p = PathBuf::from(format!("{}:\\", c as char));
            if p.exists() {
                v.push(p);
            }
        }
    } else if cfg!(target_os = "macos") {
        v.push("/Applications".into());
        v.push("/Library".into());
        if let Some(h) = std::env::var_os("HOME") {
            v.push(PathBuf::from(h));
        }
    } else {
        if let Some(h) = std::env::var_os("HOME") {
            v.push(PathBuf::from(h));
        }
        for d in ["/usr", "/opt"] {
            v.push(d.into());
        }
    }
    v
}

/// folders a deep search never enters (system, caches, trash, VCS)
fn skip_dir(name: &str) -> bool {
    let n = name.to_lowercase();
    (n.starts_with('.') && n != ".vst3" && n != ".clap")
        || matches!(
            n.as_str(),
            "windows"
                | "$recycle.bin"
                | "system volume information"
                | "winsxs"
                | "proc"
                | "sys"
                | "dev"
                | "node_modules"
                | "target"
                | "cache"
                | "caches"
                | "temp"
                | "tmp"
                | "appdata"
        )
}

/// Search `roots` (down to `max_depth` folders) for folders that hold
/// `.vst3` / `.clap` plugins. Returns each folder with its plugin count,
/// in the order found; bundles count as files and are not entered.
/// `visited` counts folders as they are read and `now` names the current
/// one (progress), `stop` cancels. At most `limit` folders are read.
pub fn find_plugin_folders(
    roots: &[PathBuf],
    max_depth: usize,
    limit: usize,
    visited: &std::sync::atomic::AtomicUsize,
    now: &Mutex<String>,
    stop: &std::sync::atomic::AtomicBool,
) -> Vec<(PathBuf, usize)> {
    use std::sync::atomic::Ordering;
    let mut out: Vec<(PathBuf, usize)> = vec![];
    let mut stack: Vec<(PathBuf, usize)> = roots.iter().rev().map(|r| (r.clone(), 0)).collect();
    while let Some((d, depth)) = stack.pop() {
        if stop.load(Ordering::Relaxed) || visited.load(Ordering::Relaxed) >= limit {
            break;
        }
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        visited.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut g) = now.lock() {
            *g = d.display().to_string();
        }
        let mut n = 0;
        let mut subs = vec![];
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            let ext = p
                .extension()
                .map(|x| x.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if ext == "vst3" || ext == "clap" {
                n += 1;
                continue;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() && !ft.is_symlink() && depth < max_depth && !skip_dir(&name) {
                subs.push(p);
            }
        }
        if n > 0 && !out.iter().any(|(o, _)| o == &d) {
            out.push((d.clone(), n));
        }
        subs.sort();
        for s in subs.into_iter().rev() {
            stack.push((s, depth + 1));
        }
    }
    out
}
