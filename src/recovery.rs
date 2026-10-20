//! Autosave + crash recovery (docs/SAFETY.md §1-3, simplified: no journal).
//! Layout: <config>/recovery/<session-id>/{session.toml, heartbeat, autosave.cwt,
//! autosave.1.cwt, autosave.2.cwt, panic.cwt, crash.txt}, and a copy of every
//! crash report in <config>/crashes/crash-<unix time>.txt. A clean exit removes
//! the session folder; a folder left behind (stale heartbeat) is offered for
//! recovery on the next start.
use crate::model::Song;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const HEARTBEAT_EVERY: Duration = Duration::from_secs(20);
/// a session whose heartbeat is older than this is not running any more
const STALE: Duration = Duration::from_secs(90);

pub fn root() -> Option<PathBuf> {
    crate::settings::config_dir().map(|d| d.join("recovery"))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SessionInfo {
    pub pid: u32,
    pub started: u64,
    pub version: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub song_path: Option<String>,
}

/// Save atomically: write `<path>.tmp`, fsync, keep the old file as `.bak`, rename.
pub fn atomic_save(song: &Song, path: &Path, backups: u32) -> Result<(), String> {
    let tmp = path.with_extension("cwt.tmp");
    // save_cwt flushes to disk through its own writable handle
    if let Err(e) = crate::fileio::save_cwt(song, &tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    // sanity: the temp file must load back before it replaces anything
    if let Err(e) = crate::fileio::load_cwt(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("verify failed: {e}"));
    }
    if backups > 0 && path.exists() {
        // song.cwt.bak (newest), song.cwt.bak2 ... bakN
        let bak = |i: u32| {
            if i == 1 {
                path.with_extension("cwt.bak")
            } else {
                path.with_extension(format!("cwt.bak{i}"))
            }
        };
        let _ = std::fs::remove_file(bak(backups));
        for i in (1..backups).rev() {
            let _ = std::fs::rename(bak(i), bak(i + 1));
        }
        let _ = std::fs::copy(path, bak(1));
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

pub struct Session {
    pub dir: PathBuf,
    /// an autosave is running (autosaves never overlap)
    busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Session {
    pub fn start(root: &Path, title: &str, song_path: Option<&Path>) -> Option<Session> {
        let id = format!("{}-{}", now(), std::process::id());
        let dir = root.join(id);
        std::fs::create_dir_all(&dir).ok()?;
        let s = Session {
            dir,
            busy: Default::default(),
        };
        s.write_info(title, song_path);
        s.heartbeat();
        Some(s)
    }
    pub fn write_info(&self, title: &str, song_path: Option<&Path>) {
        let info = SessionInfo {
            pid: std::process::id(),
            started: now(),
            version: crate::VERSION.into(),
            title: title.into(),
            song_path: song_path.map(|p| p.display().to_string()),
        };
        if let Ok(t) = toml::to_string(&info) {
            let _ = std::fs::write(self.dir.join("session.toml"), t);
        }
    }
    pub fn heartbeat(&self) {
        let _ = std::fs::write(self.dir.join("heartbeat"), now().to_string());
    }
    /// Autosave (keeps the last 3). Blocking; call from a worker thread.
    pub fn autosave_blocking(dir: &Path, song: &Song) -> Result<(), String> {
        let a = dir.join("autosave.cwt");
        if a.exists() {
            let _ = std::fs::rename(dir.join("autosave.1.cwt"), dir.join("autosave.2.cwt"));
            let _ = std::fs::copy(&a, dir.join("autosave.1.cwt"));
        }
        atomic_save(song, &a, 0)
    }
    /// Autosave on a background thread with a snapshot of the song.
    /// Returns false (and does nothing) while a previous autosave still runs.
    pub fn autosave(&self, song: Song) -> bool {
        use std::sync::atomic::Ordering;
        if self.busy.swap(true, Ordering::AcqRel) {
            return false;
        }
        let dir = self.dir.clone();
        let busy = self.busy.clone();
        std::thread::spawn(move || {
            let _ = Session::autosave_blocking(&dir, &song);
            busy.store(false, Ordering::Release);
        });
        true
    }
    /// wait (up to `max`) for a running autosave to finish
    pub fn wait_idle(&self, max: std::time::Duration) {
        let t0 = std::time::Instant::now();
        while self.busy.load(std::sync::atomic::Ordering::Acquire) && t0.elapsed() < max {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    /// clean exit: nothing to recover
    pub fn close(&self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Best-effort emergency snapshot (panic hook).
pub fn emergency(dir: &Path, song: Option<&Song>, msg: &str) {
    let _ = std::fs::create_dir_all(dir);
    let mut txt = format!(
        "ChipWave {}{} crashed\n{}\n\n{}\n",
        crate::VERSION,
        option_env!("CW_BUILD").map_or(String::new(), |b| format!(" ({b})")),
        msg,
        std::backtrace::Backtrace::force_capture()
    );
    if let Some(s) = song {
        match crate::fileio::save_cwt(s, &dir.join("panic.cwt")) {
            Ok(()) => txt.push_str("\nemergency snapshot: panic.cwt\n"),
            Err(e) => txt.push_str(&format!("\nemergency snapshot failed: {e}\n")),
        }
    }
    let _ = std::fs::write(dir.join("heartbeat"), "0");
    let _ = std::fs::write(dir.join("crash.txt"), &txt);
    // a copy the recovery clean-up never deletes (discarding the recovered
    // song, or a session with nothing saved yet, removes the session folder)
    if let Some(logs) = crash_log_dir(dir) {
        keep_crash_log(&logs, &txt);
    }
}

/// `<config>/crashes` for a session folder `<config>/recovery/<id>`
fn crash_log_dir(session: &Path) -> Option<PathBuf> {
    Some(session.parent()?.parent()?.join("crashes"))
}

/// write `crash-<unix time>.txt` into `logs`, keeping the newest 20
fn keep_crash_log(logs: &Path, txt: &str) {
    let _ = std::fs::create_dir_all(logs);
    let _ = std::fs::write(logs.join(format!("crash-{}.txt", now())), txt);
    let mut old: Vec<PathBuf> = std::fs::read_dir(logs)
        .map(|r| {
            r.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("crash-") && n.ends_with(".txt"))
                })
                .collect()
        })
        .unwrap_or_default();
    old.sort();
    let n = old.len().saturating_sub(20);
    for p in &old[..n] {
        let _ = std::fs::remove_file(p);
    }
}

#[derive(Clone, Debug)]
pub struct Orphan {
    pub dir: PathBuf,
    pub file: PathBuf,
    pub info: SessionInfo,
    /// "emergency snapshot" or "autosave"
    pub source: &'static str,
    pub modified: SystemTime,
    pub crashed: bool,
}

/// Session folders left behind by a crash (not `own`, heartbeat stale).
pub fn find_orphans(root: &Path, own: Option<&Path>) -> Vec<Orphan> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(root) else {
        return out;
    };
    for e in rd.flatten() {
        let dir = e.path();
        if Some(dir.as_path()) == own || !dir.is_dir() {
            continue;
        }
        let hb: u64 = std::fs::read_to_string(dir.join("heartbeat"))
            .ok()
            .and_then(|t| t.trim().parse().ok())
            .unwrap_or(0);
        if now().saturating_sub(hb) < STALE.as_secs() {
            continue; // another instance is running
        }
        let info: SessionInfo = std::fs::read_to_string(dir.join("session.toml"))
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default();
        let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let cands = [
            (dir.join("panic.cwt"), "emergency snapshot"),
            (dir.join("autosave.cwt"), "autosave"),
        ];
        let best = cands
            .iter()
            .filter_map(|(p, s)| mtime(p).map(|t| (p.clone(), *s, t)))
            .max_by_key(|x| x.2);
        let crashed = dir.join("crash.txt").exists();
        match best {
            Some((file, source, modified)) => out.push(Orphan {
                dir,
                file,
                info,
                source,
                modified,
                crashed,
            }),
            // nothing saved yet: just clean the folder up
            None => {
                let _ = std::fs::remove_dir_all(&dir);
            }
        }
    }
    out.sort_by_key(|o| std::cmp::Reverse(o.modified));
    out
}

pub fn discard(o: &Orphan) {
    let _ = std::fs::remove_dir_all(&o.dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_root(n: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("chipwave_rec_{n}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn autosave_is_atomic_rotates_and_recovers() {
        let root = tmp_root("a");
        let mut song = crate::fileio::demo_song();
        let s = Session::start(&root, &song.title, None).unwrap();
        for k in 0..4 {
            song.bpm = 120.0 + k as f32;
            Session::autosave_blocking(&s.dir, &song).unwrap();
        }
        assert!(s.dir.join("autosave.1.cwt").exists() && s.dir.join("autosave.2.cwt").exists());
        assert!(!s.dir.join("autosave.cwt.tmp").exists());
        // live session (fresh heartbeat) is not an orphan
        assert!(find_orphans(&root, None).is_empty());
        // simulate a crash: stale heartbeat, folder left behind
        std::fs::write(s.dir.join("heartbeat"), "1").unwrap();
        let o = find_orphans(&root, None);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].source, "autosave");
        let rec = crate::fileio::load_cwt(&o[0].file).unwrap();
        assert_eq!(rec.bpm, 123.0);
        assert_eq!(rec.patterns[0].cells, song.patterns[0].cells);
        discard(&o[0]);
        assert!(find_orphans(&root, None).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn emergency_snapshot_and_clean_exit() {
        let root = tmp_root("b");
        let song = crate::fileio::demo_song();
        let s = Session::start(&root, "x", None).unwrap();
        emergency(&s.dir, Some(&song), "test panic");
        let o = find_orphans(&root, None);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].source, "emergency snapshot");
        assert!(o[0].crashed);
        assert!(crate::fileio::load_cwt(&o[0].file).is_ok());
        // a clean exit leaves nothing behind
        let s2 = Session::start(&root, "y", None).unwrap();
        s2.close();
        assert!(!s2.dir.exists());
        // own session is never offered
        assert!(find_orphans(&root, Some(&o[0].dir)).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn atomic_save_keeps_backup() {
        let root = tmp_root("c");
        let p = root.join("song.cwt");
        let mut song = crate::fileio::demo_song();
        atomic_save(&song, &p, 3).unwrap();
        song.bpm = 99.0;
        atomic_save(&song, &p, 3).unwrap();
        song.bpm = 98.0;
        atomic_save(&song, &p, 3).unwrap();
        assert!(p.with_extension("cwt.bak2").exists());
        song.bpm = 99.0;
        atomic_save(&song, &p, 3).unwrap();
        assert_eq!(crate::fileio::load_cwt(&p).unwrap().bpm, 99.0);
        assert!(
            crate::fileio::load_cwt(&p.with_extension("cwt.bak"))
                .unwrap()
                .bpm
                != 99.0
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn atomic_save_leaves_no_stray_files() {
        let root = tmp_root("d");
        let p = root.join("song.cwt");
        let song = crate::fileio::demo_song();
        atomic_save(&song, &p, 1).unwrap();
        atomic_save(&song, &p, 1).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["song.cwt", "song.cwt.bak"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn autosaves_never_overlap() {
        let root = tmp_root("e");
        let s = Session::start(&root, "t", None).unwrap();
        let song = crate::fileio::demo_song();
        assert!(s.autosave(song.clone()));
        // second request while the first still runs is refused
        let second = s.autosave(song.clone());
        s.wait_idle(std::time::Duration::from_secs(20));
        assert!(!s.busy.load(std::sync::atomic::Ordering::Acquire));
        assert!(s.autosave(song) || second);
        s.wait_idle(std::time::Duration::from_secs(20));
        assert!(crate::fileio::load_cwt(&s.dir.join("autosave.cwt")).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn crash_reports_outlive_the_recovery_folder() {
        let root = tmp_root("crashlog");
        let dir = root.join("recovery").join("123-1");
        emergency(&dir, None, "panicked at src/app/chordlane.rs:1:1: boom");
        // the recovery prompt's Discard, or "nothing saved yet": folder gone
        let _ = std::fs::remove_dir_all(&dir);
        let logs: Vec<_> = std::fs::read_dir(root.join("crashes"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(logs.len(), 1);
        let t = std::fs::read_to_string(logs[0].path()).unwrap();
        assert!(t.contains("chordlane.rs:1:1: boom"), "{t}");
        // only the newest 20 are kept
        for i in 0..30 {
            std::fs::write(root.join("crashes").join(format!("crash-{i:04}.txt")), "x").unwrap();
        }
        keep_crash_log(&root.join("crashes"), "y");
        assert_eq!(std::fs::read_dir(root.join("crashes")).unwrap().count(), 20);
        let _ = std::fs::remove_dir_all(&root);
    }
}
