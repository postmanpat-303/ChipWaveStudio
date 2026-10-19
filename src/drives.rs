//! Drives and quick locations for the Browser's Files tab (docs/BROWSER.md
//! "Files"). On Windows every drive letter is listed with its type, so
//! mapped network drives (DRIVE_REMOTE, shown with their \\server\share) and
//! removable drives appear too, not only fixed disks. Reading a volume label
//! can hang on a slow or disconnected share, so `probe` runs each drive on its
//! own thread with a timeout and marks the ones that don't answer.
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Kind {
    Fixed,
    Removable,
    Network,
    Optical,
    Other,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Fixed => "Local disk",
            Kind::Removable => "Removable",
            Kind::Network => "Network drive",
            Kind::Optical => "CD / DVD",
            Kind::Other => "Drive",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drive {
    /// "C:\", "Z:\", "/", "/media/me/USB"
    pub root: String,
    pub kind: Kind,
    /// volume label (after `probe`)
    pub label: String,
    /// \\server\share of a mapped drive
    pub unc: Option<String>,
    /// None = not probed yet; Some(false) = didn't answer / disconnected
    pub ok: Option<bool>,
}

impl Drive {
    /// "Z: Music (\\nas\music)"
    pub fn title(&self) -> String {
        let mut s = self.root.trim_end_matches(['\\', '/']).to_string();
        if s.is_empty() {
            s = "/".into();
        }
        if !self.label.is_empty() {
            s = format!("{s} {}", self.label);
        }
        if let Some(u) = &self.unc {
            s = format!("{s} ({u})");
        }
        s
    }
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// every drive, without touching the volumes (fast: no network I/O)
#[cfg(windows)]
pub fn list() -> Vec<Drive> {
    use windows_sys::Win32::NetworkManagement::WNet::WNetGetConnectionW;
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    let mask = unsafe { GetLogicalDrives() };
    let mut v = vec![];
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root = format!("{letter}:\\");
        let kind = match unsafe { GetDriveTypeW(wide(&root).as_ptr()) } {
            2 => Kind::Removable,
            3 => Kind::Fixed,
            4 => Kind::Network,
            5 => Kind::Optical,
            _ => Kind::Other,
        };
        // the mapped share (reads the logon session's table, no network I/O)
        let unc = if kind == Kind::Network {
            let mut buf = [0u16; 1024];
            let mut n = buf.len() as u32;
            let r = unsafe {
                WNetGetConnectionW(
                    wide(&format!("{letter}:")).as_ptr(),
                    buf.as_mut_ptr(),
                    &mut n,
                )
            };
            (r == 0).then(|| {
                let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
                String::from_utf16_lossy(&buf[..end])
            })
        } else {
            None
        };
        v.push(Drive {
            root,
            kind,
            label: String::new(),
            unc,
            ok: None,
        });
    }
    v
}

#[cfg(not(windows))]
pub fn list() -> Vec<Drive> {
    let mut v = vec![Drive {
        root: "/".into(),
        kind: Kind::Fixed,
        label: String::new(),
        unc: None,
        ok: None,
    }];
    let user = std::env::var("USER").unwrap_or_default();
    let mut bases: Vec<PathBuf> = vec!["/Volumes".into(), "/mnt".into()];
    if !user.is_empty() {
        bases.push(PathBuf::from("/media").join(&user));
        bases.push(PathBuf::from("/run/media").join(&user));
    }
    for b in bases {
        let Ok(rd) = std::fs::read_dir(&b) else {
            continue;
        };
        let mut found: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        found.sort();
        for p in found {
            v.push(Drive {
                root: p.display().to_string(),
                kind: Kind::Removable,
                label: p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                unc: None,
                ok: None,
            });
        }
    }
    v
}

/// the volume label, or None when the drive isn't ready / doesn't answer
#[cfg(windows)]
fn read_label(root: &str) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;
    let mut name = [0u16; 261];
    let ok = unsafe {
        GetVolumeInformationW(
            wide(root).as_ptr(),
            name.as_mut_ptr(),
            name.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    if ok == 0 {
        return None;
    }
    let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
    Some(String::from_utf16_lossy(&name[..end]))
}

#[cfg(not(windows))]
fn read_label(root: &str) -> Option<String> {
    std::fs::read_dir(root).ok().map(|_| String::new())
}

/// Labels and reachability, each drive on its own thread; drives that
/// haven't answered after `timeout` are marked `ok = Some(false)` (their
/// threads are left to finish on their own).
pub fn probe(mut drives: Vec<Drive>, timeout: Duration) -> Vec<Drive> {
    let (tx, rx) = mpsc::channel();
    for (i, d) in drives.iter().enumerate() {
        let tx = tx.clone();
        let root = d.root.clone();
        let _ = std::thread::Builder::new()
            .name("drive-probe".into())
            .spawn(move || {
                let _ = tx.send((i, read_label(&root)));
            });
    }
    drop(tx);
    let end = Instant::now() + timeout;
    let mut left = drives.len();
    while left > 0 {
        let now = Instant::now();
        if now >= end {
            break;
        }
        match rx.recv_timeout(end - now) {
            Ok((i, r)) => {
                left -= 1;
                if let Some(d) = drives.get_mut(i) {
                    match r {
                        Some(l) => {
                            if d.label.is_empty() {
                                d.label = l;
                            }
                            d.ok = Some(true);
                        }
                        None => d.ok = Some(false),
                    }
                }
            }
            Err(_) => break,
        }
    }
    for d in &mut drives {
        if d.ok.is_none() {
            d.ok = Some(false);
        }
    }
    drives
}

/// Is this process elevated (Run as administrator)? Windows hides the user's
/// mapped network drives from elevated programs (they live in the
/// non-elevated logon session), which is why they could go missing.
#[cfg(windows)]
pub fn elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    const TOKEN_QUERY: u32 = 8;
    let mut tok: HANDLE = std::ptr::null_mut();
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut tok) == 0 {
            return false;
        }
        let mut el: u32 = 0;
        let mut n = 0u32;
        let ok = GetTokenInformation(
            tok,
            TokenElevation,
            &mut el as *mut u32 as *mut core::ffi::c_void,
            4,
            &mut n,
        );
        CloseHandle(tok);
        ok != 0 && el != 0
    }
}

#[cfg(not(windows))]
pub fn elevated() -> bool {
    false
}

/// the user's home folder
pub fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

/// quick locations: (label, folder) for Home, Music, Desktop, Downloads
/// (those that exist; OneDrive-redirected Desktop/Music are found too)
pub fn quick() -> Vec<(&'static str, PathBuf)> {
    let Some(h) = home() else { return vec![] };
    let mut v = vec![("Home", h.clone())];
    for name in ["Music", "Desktop", "Downloads"] {
        let mut c = vec![h.join(name)];
        if let Some(od) = std::env::var_os("OneDrive") {
            c.push(PathBuf::from(od).join(name));
        }
        if let Some(p) = c.into_iter().find(|p| p.is_dir()) {
            v.push((name, p));
        }
    }
    v
}

/// a path typed into the path bar: `~` expanded, quotes trimmed, a bare
/// "Z:" becomes "Z:\"; UNC paths (\\server\share) pass through as they are
pub fn parse_typed(s: &str) -> PathBuf {
    let s = s.trim().trim_matches('"');
    if let Some(rest) = s.strip_prefix('~') {
        if let Some(h) = home() {
            return h.join(rest.trim_start_matches(['/', '\\']));
        }
    }
    let b = s.as_bytes();
    if b.len() == 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        return PathBuf::from(format!("{s}\\"));
    }
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_at_least_one_drive_and_probes_with_timeout() {
        let d = list();
        assert!(!d.is_empty());
        let t = Instant::now();
        let p = probe(d, Duration::from_millis(1500));
        assert!(t.elapsed() < Duration::from_secs(3));
        assert!(p.iter().all(|d| d.ok.is_some()));
        assert!(p.iter().any(|d| d.ok == Some(true)));
    }

    #[test]
    fn typed_paths() {
        assert_eq!(parse_typed("Z:"), PathBuf::from("Z:\\"));
        assert_eq!(
            parse_typed("\\\\nas\\music"),
            PathBuf::from("\\\\nas\\music")
        );
        assert_eq!(parse_typed(" \"/tmp\" "), PathBuf::from("/tmp"));
    }

    #[test]
    fn titles_show_the_share() {
        let d = Drive {
            root: "Z:\\".into(),
            kind: Kind::Network,
            label: "Music".into(),
            unc: Some("\\\\nas\\music".into()),
            ok: Some(true),
        };
        assert_eq!(d.title(), "Z: Music (\\\\nas\\music)");
    }
}
