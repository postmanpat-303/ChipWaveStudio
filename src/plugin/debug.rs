//! Plugin-host debug log (off by default). Set `CHIPWAVE_PLUGIN_DEBUG=1` to
//! append timestamped lines to `plugin-debug.log` in the config folder (and
//! stderr): plugin calls that could block the UI (params flush under the
//! audio lock, editor create / scale / parent / show, thumbnail capture) are
//! timed, and a watchdog thread notes any such call still running after
//! [`WATCHDOG_AFTER`], so a freeze leaves a trace of where it is stuck.
//! Main thread / UI only: never call from the audio thread.
use std::io::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// a timed call still running after this is logged by the watchdog
pub const WATCHDOG_AFTER: Duration = Duration::from_secs(2);
/// a timed call that took longer than this is logged when it returns
pub const SLOW: Duration = Duration::from_millis(250);

/// is the debug log on (`CHIPWAVE_PLUGIN_DEBUG` set to anything but 0/empty)?
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("CHIPWAVE_PLUGIN_DEBUG").is_ok_and(|v| !v.is_empty() && v != "0")
    })
}

fn start() -> Instant {
    static T0: OnceLock<Instant> = OnceLock::new();
    *T0.get_or_init(Instant::now)
}

/// where the log goes (None in tests: stderr only)
pub fn log_path() -> Option<std::path::PathBuf> {
    if cfg!(test) {
        return None;
    }
    crate::settings::config_dir().map(|d| d.join("plugin-debug.log"))
}

/// write one line if the debug log is on (`f` only runs then)
pub fn log(f: impl FnOnce() -> String) {
    if enabled() {
        write_line(&f());
    }
}

fn write_line(msg: &str) {
    let line = format!(
        "[{:>9.3}s pid {}] {msg}",
        start().elapsed().as_secs_f64(),
        std::process::id()
    );
    eprintln!("plugin-debug: {line}");
    static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
    let Ok(mut g) = FILE.lock() else { return };
    if g.is_none() {
        *g = log_path().and_then(|p| {
            let _ = std::fs::create_dir_all(p.parent()?);
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .ok()
        });
    }
    if let Some(f) = g.as_mut() {
        let _ = writeln!(f, "{line}");
        let _ = f.flush();
    }
}

/// the timed call in progress (what, start, serial), read by the watchdog
static CURRENT: Mutex<Vec<(String, Instant, u64)>> = Mutex::new(Vec::new());
static SERIAL: AtomicU64 = AtomicU64::new(1);

fn watchdog() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("plugin-watchdog".into())
            .spawn(|| {
                // serials already reported, so each stuck call is logged once
                // per WATCHDOG_AFTER step rather than every tick
                let mut told: Vec<(u64, u64)> = vec![];
                loop {
                    std::thread::sleep(Duration::from_millis(500));
                    let now: Vec<(String, Instant, u64)> =
                        CURRENT.lock().map(|v| v.clone()).unwrap_or_default();
                    told.retain(|t| now.iter().any(|c| c.2 == t.0));
                    for (what, t0, id) in now {
                        let steps = t0.elapsed().as_secs() / WATCHDOG_AFTER.as_secs().max(1);
                        if steps == 0 {
                            continue;
                        }
                        match told.iter_mut().find(|t| t.0 == id) {
                            Some(t) if t.1 >= steps => continue,
                            Some(t) => t.1 = steps,
                            None => told.push((id, steps)),
                        }
                        write_line(&format!(
                            "WATCHDOG: still inside {what} after {:.1}s (UI thread blocked?)",
                            t0.elapsed().as_secs_f64()
                        ));
                    }
                }
            });
    });
}

/// Times a plugin call; logs its start (debug), its end when slow, and lets
/// the watchdog report it while it runs. No-op unless the debug log is on.
pub struct Timed {
    what: Option<(String, Instant, u64)>,
}

impl Timed {
    pub fn new(what: impl FnOnce() -> String) -> Timed {
        if !enabled() {
            return Timed { what: None };
        }
        watchdog();
        let w = what();
        let id = SERIAL.fetch_add(1, Ordering::Relaxed);
        write_line(&format!("begin {w}"));
        let t0 = Instant::now();
        if let Ok(mut v) = CURRENT.lock() {
            v.push((w.clone(), t0, id));
        }
        Timed {
            what: Some((w, t0, id)),
        }
    }
}

impl Drop for Timed {
    fn drop(&mut self) {
        let Some((w, t0, id)) = self.what.take() else {
            return;
        };
        if let Ok(mut v) = CURRENT.lock() {
            v.retain(|c| c.2 != id);
        }
        let dt = t0.elapsed();
        write_line(&format!(
            "end   {w} ({:.1} ms){}",
            dt.as_secs_f64() * 1000.0,
            if dt >= SLOW { "  SLOW" } else { "" }
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_is_free_when_off() {
        // the test environment never sets the variable
        if enabled() {
            return;
        }
        let mut ran = false;
        let t = Timed::new(|| {
            ran = true;
            "x".into()
        });
        drop(t);
        assert!(!ran, "the label is not even built while the log is off");
        assert!(CURRENT.lock().unwrap().is_empty());
        assert_eq!(log_path(), None, "tests never write a log file");
    }
}
