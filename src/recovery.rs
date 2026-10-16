//! Autosave + crash recovery (docs/SAFETY.md §1-3, simplified: no journal).
//! Layout: <config>/recovery/<session-id>/{session.toml, heartbeat, autosave.cwt,
//! autosave.1.cwt, autosave.2.cwt, panic.cwt, crash.txt}. A clean exit removes
//! the session folder; a folder left behind (stale heartbeat) is offered for
//! recovery on the next start.
use crate::model::Song;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const AUTOSAVE_EVERY: Duration = Duration::from_secs(120);
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
pub fn atomic_save(song: &Song, path: &Path, keep_bak: bool) -> Result<(), String> {
    let tmp = path.with_extension("cwt.tmp");
    crate::fileio::save_cwt(song, &tmp)?;
    if let Ok(f) = std::fs::File::open(&tmp) {
        let _ = f.sync_all();
    }
    // sanity: the temp file must load back before it replaces anything
    crate::fileio::load_cwt(&tmp).map_err(|e| format!("verify failed: {e}"))?;
    if keep_bak && path.exists() {
        let _ = std::fs::copy(path, path.with_extension("cwt.bak"));
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

pub struct Session {
    pub dir: PathBuf,
}

impl Session {
    pub fn start(root: &Path, title: &str, song_path: Option<&Path>) -> Option<Session> {
        let id = format!("{}-{}", now(), std::process::id());
        let dir = root.join(id);
        std::fs::create_dir_all(&dir).ok()?;
        let s = Session { dir };
        s.write_info(title, song_path);
        s.heartbeat();
        Some(s)
    }
    pub fn write_info(&self, title: &str, song_path: Option<&Path>) {
        let info = SessionInfo {
            pid: std::process::id(),
            started: now(),
            version: env!("CARGO_PKG_VERSION").into(),
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
        atomic_save(song, &a, false)
    }
    /// Autosave on a background thread with a snapshot of the song.
    pub fn autosave(&self, song: Song) {
        let dir = self.dir.clone();
        std::thread::spawn(move || {
            let _ = Session::autosave_blocking(&dir, &song);
        });
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
        "ChipWave {} crashed\n{}\n\n{}\n",
        env!("CARGO_PKG_VERSION"),
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
    let _ = std::fs::write(dir.join("crash.txt"), txt);
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
        atomic_save(&song, &p, true).unwrap();
        song.bpm = 99.0;
        atomic_save(&song, &p, true).unwrap();
        assert_eq!(crate::fileio::load_cwt(&p).unwrap().bpm, 99.0);
        assert!(
            crate::fileio::load_cwt(&p.with_extension("cwt.bak"))
                .unwrap()
                .bpm
                != 99.0
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
