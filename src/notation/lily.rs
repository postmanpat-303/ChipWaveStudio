//! Finding and running LilyPond (https://lilypond.org), which turns a .ly
//! file into a PDF. LilyPond is a separate free program the user installs
//! themselves; ChipWave never bundles it, it only runs it when found.
use std::path::{Path, PathBuf};

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "lilypond.exe"
    } else {
        "lilypond"
    }
}

/// `p` itself (a lilypond program), or the program inside folder `p`
/// (`p/lilypond`, `p/bin/lilypond`, or a `lilypond-x.y.z` folder in `p`)
fn in_place(p: &Path, depth: u32) -> Option<PathBuf> {
    if p.is_file() {
        return Some(p.to_path_buf());
    }
    if !p.is_dir() {
        return None;
    }
    for c in [p.join(exe_name()), p.join("bin").join(exe_name())] {
        if c.is_file() {
            return Some(c);
        }
    }
    if depth == 0 {
        return None;
    }
    // newest lilypond-* folder first
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(p)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|d| {
            d.is_dir()
                && d.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.to_lowercase().starts_with("lilypond"))
        })
        .collect();
    dirs.sort();
    dirs.reverse();
    dirs.iter().find_map(|d| in_place(d, depth - 1))
}

/// where LilyPond is: `custom` (a program or a folder, from Settings), the
/// CW_LILYPOND variable, the PATH, then the usual install folders
pub fn find(custom: &str) -> Option<PathBuf> {
    let custom = custom.trim();
    if !custom.is_empty() {
        if let Some(p) = in_place(Path::new(custom), 1) {
            return Some(p);
        }
    }
    if let Some(p) = std::env::var_os("CW_LILYPOND") {
        if let Some(p) = in_place(Path::new(&p), 1) {
            return Some(p);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&path) {
            let c = d.join(exe_name());
            if c.is_file() {
                return Some(c);
            }
        }
    }
    let mut roots: Vec<PathBuf> = vec![];
    for v in [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "LOCALAPPDATA",
        "USERPROFILE",
        "HOME",
    ] {
        if let Some(x) = std::env::var_os(v) {
            let x = PathBuf::from(x);
            roots.push(x.join("Programs"));
            roots.push(x.join("Downloads"));
            roots.push(x.join("Documents"));
            roots.push(x);
        }
    }
    if cfg!(windows) {
        roots.push(PathBuf::from("C:\\"));
    } else {
        roots.extend(
            [
                "/usr/local/bin",
                "/usr/bin",
                "/opt/homebrew/bin",
                "/opt",
                "/Applications/LilyPond.app/Contents/Resources",
            ]
            .map(PathBuf::from),
        );
    }
    roots.iter().find_map(|r| in_place(r, 1))
}

/// run LilyPond on `ly`, writing `<pdf_stem>.pdf`; Err = what it said
pub fn run(exe: &Path, ly: &Path, pdf_stem: &Path) -> Result<(PathBuf, String), String> {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--pdf")
        .arg("-dno-point-and-click")
        .arg("-o")
        .arg(pdf_stem)
        .arg(ly)
        .stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console window pops up
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("couldn't start LilyPond ({e})"))?;
    let log = String::from_utf8_lossy(&out.stderr).to_string();
    let pdf = PathBuf::from(format!("{}.pdf", pdf_stem.display()));
    if !out.status.success() || !pdf.is_file() {
        let first = log
            .lines()
            .find(|l| l.contains("error"))
            .unwrap_or_else(|| log.lines().last().unwrap_or("no PDF was made"));
        return Err(format!("LilyPond: {}", first.trim()));
    }
    Ok((pdf, log))
}

/// the lines of a LilyPond log that report problems (errors, failed bar
/// checks, programming errors)
pub fn problems(log: &str) -> Vec<String> {
    log.lines()
        .filter(|l| {
            let l = l.to_lowercase();
            l.contains("error") || l.contains("barcheck failed") || l.contains("bar check failed")
        })
        .map(str::to_string)
        .collect()
}
