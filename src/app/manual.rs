//! Help > User Manual: the PDF manual is built into the app (`include_bytes!`
//! of docs/manual/ChipWave-Studio-User-Manual.pdf, rebuilt from the HTML
//! next to it), so every install type has it: setup, portable zip, Wave Hub,
//! macOS bundle and Linux packages. On open it's written once to
//! `<config dir>/Manual/` under a versioned name (rewritten only when its
//! size or SHA-256 differs) and handed to the system PDF viewer with the same
//! `explorer` / `open` / `xdg-open` call the About notices button uses.
//! If that fails, a small window says so and links to the PDF on GitHub.
use super::*;
use sha2::{Digest, Sha256};
use std::path::Path;

/// the manual, inside the exe
pub(crate) const MANUAL_PDF: &[u8] =
    include_bytes!("../../docs/manual/ChipWave-Studio-User-Manual.pdf");

/// the same PDF on GitHub (the fallback link when it can't be opened here)
pub(super) const MANUAL_URL: &str = "https://github.com/postmanpat-303/ChipWaveStudio/blob/main/docs/manual/ChipWave-Studio-User-Manual.pdf";

/// "ChipWave-Studio-User-Manual-0.2.19.0.pdf": one file per app version
pub(crate) fn file_name() -> String {
    format!("ChipWave-Studio-User-Manual-{}.pdf", crate::VERSION)
}

#[derive(Default)]
pub(super) struct ManualState {
    /// Some: opening failed; the reason, shown in the fallback window
    pub err: Option<String>,
    /// times the manual was handed to the viewer (tests)
    pub opened: u32,
}

/// Write `bytes` to `dir/name` unless an identical copy (same size and
/// SHA-256) is already there. Writes to a temp file first, then renames, so a
/// half-written PDF is never left under the real name.
pub(crate) fn write_cached(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let path = dir.join(name);
    if let Ok(m) = std::fs::metadata(&path) {
        if m.is_file()
            && m.len() == bytes.len() as u64
            && std::fs::read(&path)
                .map(|old| Sha256::digest(&old) == Sha256::digest(bytes))
                .unwrap_or(false)
        {
            return Ok(path);
        }
    }
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{name}.part"));
    std::fs::write(&tmp, bytes)?;
    if std::fs::rename(&tmp, &path).is_err() {
        // Windows: the old copy may be open in a viewer; replace in place
        let _ = std::fs::remove_file(&tmp);
        std::fs::write(&path, bytes)?;
    }
    Ok(path)
}

/// where the manual is written: the settings folder, else the temp folder
fn cache_dir() -> PathBuf {
    crate::settings::config_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("ChipWave"))
        .join("Manual")
}

impl App {
    /// Help > User Manual and the About page button
    pub(super) fn open_manual(&mut self) {
        self.manual.err = None;
        // tests never touch the disk or start a viewer
        if cfg!(test) {
            self.manual.opened += 1;
            return;
        }
        let res = write_cached(&cache_dir(), &file_name(), MANUAL_PDF)
            .map_err(|e| format!("couldn't save the manual ({e})"))
            .and_then(|p| {
                super::about::open_file(&p)
                    .map_err(|e| format!("couldn't start the PDF viewer ({e})"))
            });
        match res {
            Ok(()) => self.manual.opened += 1,
            Err(e) => self.manual.err = Some(e),
        }
    }

    /// the "couldn't open the manual" window (only after a failure)
    pub(super) fn manual_window(&mut self, ctx: &egui::Context) {
        let Some(err) = self.manual.err.clone() else {
            return;
        };
        let mut open = true;
        let mut close = false;
        egui::Window::new("User Manual")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(440.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Sorry, the user manual couldn't be opened on this computer.");
                ui.label(
                    egui::RichText::new(format!(
                        "{err}. You can read the same PDF on GitHub instead."
                    ))
                    .size(crate::theme::ts::px(crate::theme::ts::SMALL))
                    .color(th().ink2),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .button("Open the manual on GitHub")
                        .on_hover_text(MANUAL_URL)
                        .clicked()
                    {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(MANUAL_URL));
                        close = true;
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
        if !open || close {
            self.manual.err = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_manual_is_a_pdf() {
        assert!(MANUAL_PDF.starts_with(b"%PDF-"), "starts with %PDF-");
        assert!(MANUAL_PDF.len() > 100_000, "the whole manual, not a stub");
        let tail = &MANUAL_PDF[MANUAL_PDF.len() - 1024..];
        assert!(
            tail.windows(5).any(|w| w == b"%%EOF"),
            "ends with %%EOF (not truncated)"
        );
        assert!(file_name().starts_with("ChipWave-Studio-User-Manual-"));
        assert!(file_name().ends_with(&format!("{}.pdf", crate::VERSION)));
        assert!(MANUAL_URL.ends_with("docs/manual/ChipWave-Studio-User-Manual.pdf"));
    }

    #[test]
    fn the_built_in_manual_is_for_this_version() {
        // docs/manual/build.sh takes the version from Cargo.toml; a stale PDF
        // (built before a version bump) fails here
        let title = format!("/Title (ChipWave Studio {} User Manual)", crate::VERSION);
        assert!(
            MANUAL_PDF
                .windows(title.len())
                .any(|w| w == title.as_bytes()),
            "the manual's title says {}: rerun docs/manual/build.sh",
            crate::VERSION
        );
    }

    #[test]
    fn the_manual_is_written_once_and_refreshed_when_it_differs() {
        let dir = std::env::temp_dir().join(format!("cw-manual-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let sub = dir.join("Manual");
        // first open: creates the folder and the file
        let p = write_cached(&sub, "m.pdf", b"%PDF-one").unwrap();
        assert_eq!(p, sub.join("m.pdf"));
        assert_eq!(std::fs::read(&p).unwrap(), b"%PDF-one");
        assert!(!sub.join("m.pdf.part").exists(), "no temp file left");
        // same bytes: left alone (mtime unchanged)
        let t0 = std::fs::metadata(&p).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_cached(&sub, "m.pdf", b"%PDF-one").unwrap();
        assert_eq!(std::fs::metadata(&p).unwrap().modified().unwrap(), t0);
        // same size, different bytes: rewritten
        write_cached(&sub, "m.pdf", b"%PDF-two").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"%PDF-two");
        // different size: rewritten
        write_cached(&sub, "m.pdf", b"%PDF-three!").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"%PDF-three!");
        // the real manual round-trips byte for byte
        let real = write_cached(&sub, &file_name(), MANUAL_PDF).unwrap();
        assert_eq!(std::fs::read(&real).unwrap(), MANUAL_PDF);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
