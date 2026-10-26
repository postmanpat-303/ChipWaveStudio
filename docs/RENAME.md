# Rename: ChipWave Tracker → ChipWave Studio (0.2.4.0)

**Why:** an unrelated paid iOS chiptune app called "ChipWave" (Code and DESIGN) appeared on the
App Store in August 2026. "ChipWave Studio" tells this free, open-source desktop app apart from it.
Approved by Del on 18 Oct 2026; shipped in **0.2.4.0** together with the Windows icon fix (37797a8)
and the transpose buttons (2b3ad3f).

## Principles

- **Nothing breaks for existing users.** Settings, recent files, keys, the instrument library,
  styles, the plugin cache and recovery files live in a folder called `ChipWave`
  (`%APPDATA%\ChipWave`, `~/Library/Application Support/ChipWave`, `~/.config/chipwave`).
  That name never contained "Tracker", so it **stays as it is**: no migration, no fallback needed.
  (egui's own persistence feature is not enabled, so eframe's app name stores nothing.)
- **Release asset names stay the same** (required by Wave Hub): `ChipWave-<ver>-setup-x64.exe`, `ChipWave-<ver>-windows-x64.zip`,
  `ChipWave-<ver>-macos-universal.{dmg,zip}`, the versionless `ChipWave-setup-x64.exe`,
  `ChipWave-windows-x64.zip`, `ChipWave-macos-universal.{dmg,zip}` and `SHA256SUMS.txt`.
  Old in-app updaters (they pick the asset whose name contains `setup` and ends in `.exe`, or the
  `.dmg` with `macos`) and Wave Hub keep finding them. No old-name copies are needed.
- **Executables keep their names:** `ChipWave.exe` (Windows), `ChipWave.app` / `Contents/MacOS/ChipWave`
  (macOS), `chipwave` (crate and Linux binary). Shortcuts, pinned taskbar icons and dragged-in
  `.app` copies keep working; a new `ChipWave.app` replaces the old one instead of sitting next to it.
- **Version:** Cargo `0.2.4+fix.0`, displayed `0.2.4.0`, tag `v0.2.4.0`. The updater compares
  numerically part by part (`0.2.4.0` > `0.2.3.19`), semver agrees (`0.2.4` > `0.2.3`), and a unit
  test covers 0.2.3.19 → 0.2.4.0.
- **Never create a new repo named ChipWaveTracker** (it would break GitHub's redirects).
- The app's update check still falls back to the old address on a 404 (harmless now that the repo is renamed).
- **Git history is not rewritten.** Old release notes, tags and the captured
  `src/update_fixture_v0.2.3.4.json` (a verbatim GitHub API reply) keep the old name: they are history.

## What changes

| Area | Old | New |
|---|---|---|
| GitHub repo | postmanpat-303/ChipWaveTracker | postmanpat-303/ChipWaveStudio (GitHub redirects the repo, git, releases and API) |
| Website | postmanpat-303.github.io/ChipWaveTracker/ | postmanpat-303.github.io/ChipWaveStudio/ (no redirect possible for project Pages) |
| Window title, eframe app id | ChipWave Tracker | ChipWave Studio |
| About screen | ChipWave Tracker 0.2.3.19 | ChipWave Studio 0.2.4.0 |
| Updater URL | api.github.com/repos/postmanpat-303/ChipWaveTracker/releases/latest | …/ChipWaveStudio/releases/latest, falling back to the old address on a 404 (works before and after the repo rename) |
| User-Agent | ChipWave-Tracker/<ver> | ChipWave-Studio/<ver> |
| Song/instrument file `app` field, MP3 TSSE tag, CLAP host name | ChipWave Tracker <ver> | ChipWave Studio <ver> (readers never parse it; old files still load) |
| Icon files | assets/icon/chipwave_tracker*.{png,ico} | assets/icon/chipwave_studio*.{png,ico} (same pixels) |
| Windows installer | product, shortcuts and Add/Remove name "ChipWave Tracker" | "ChipWave Studio"; install folder now `Program Files\ChipWave Studio` (the setup moves an old install, see below); **kept**: `ChipWave.exe`, uninstall key `…\Uninstall\ChipWaveTracker`, `HKLM\Software\ChipWaveTracker` (Wave Hub compatibility), plus a junction at the old folder path |
| macOS | CFBundleDisplayName "ChipWave Tracker", CFBundleName "ChipWave" | both "ChipWave Studio"; bundle id **kept** `com.postmanpat303.chipwave` |
| Linux | (no .desktop file) | `packaging/linux/` .desktop + AppStream metainfo, id `io.github.postmanpat_303.ChipWaveStudio` |

### Windows upgrade over an old install (silent with `/S` too)

The installer (`installer/chipwave.nsi`) used to install into `Program Files\ChipWave Tracker`, the name
it had before the rename, so that upgrades landed in the same place. It now installs into
**`Program Files\ChipWave Studio`**:

1. `.onInit` reads `HKLM\Software\ChipWaveTracker\InstallDir` from the 64-bit registry view. If that is
   the old default folder, the setup switches to `Program Files\ChipWave Studio`; any other folder the
   user picked is respected. The old NSIS script only read the key from the 32-bit view
   (InstallDirRegKey runs before `SetRegView 64`), which happened to match the default.
2. Once the new folder is written, the old `ChipWave Tracker` folder is emptied and removed, so there is
   only one copy of the app.
3. A directory junction is left at `Program Files\ChipWave Tracker` pointing at the new folder, so anything
   that still looks for the old path (Wave Hub's published versions look for
   `C:\Program Files\ChipWave Tracker\ChipWave.exe`) keeps finding `ChipWave.exe`.
4. The same uninstall key `…\Uninstall\ChipWaveTracker` is rewritten with DisplayName "ChipWave Studio"
   and the new version, so Add/Remove Programs shows **one** entry. The uninstaller removes the junction as
   well as the install folder.
5. The old Start-menu folder `ChipWave Tracker` (both shortcuts), the old desktop shortcut
   `ChipWave Tracker.lnk` and the old `chipwave_tracker.ico` are removed; new "ChipWave Studio" shortcuts
   are created (desktop only if that section is ticked, as before; on by default and in `/S`).
6. User data (`%APPDATA%\ChipWave`, songs) is never touched. The uninstaller also removes any
   leftover old-name shortcuts.

Tested in Wine (fresh prefix): v0.2.3.19 release installer `/S`, settings seeded, then the 0.2.4.0 setup `/S`.

No `.cwt` file association is registered by any installer (Windows, macOS or Linux) today, before or
after the rename, so nothing changes there; opening a `.cwt` with "Open with → ChipWave.exe" and
`ChipWave.exe song.cwt` keep working.

### macOS

The bundle id stays `com.postmanpat303.chipwave` so macOS keeps treating it as the same app
(Gatekeeper "Open" approval, microphone/MIDI permissions, Dock pin). The bundle folder stays
`ChipWave.app`; the menu bar and Dock show "ChipWave Studio" (CFBundleName, ≤ 15 characters).
The dmg volume is "ChipWave Studio <ver>".

## Release assets (unchanged names)

`ChipWave-<ver>-setup-x64.exe`, `ChipWave-<ver>-windows-x64.zip` (contains `ChipWave.exe`),
`ChipWave-<ver>-macos-universal.dmg`, `ChipWave-<ver>-macos-universal.zip`, `ChipWave-setup-x64.exe`,
`ChipWave-windows-x64.zip`, `ChipWave-macos-universal.dmg`, `ChipWave-macos-universal.zip`, and
`SHA256SUMS.txt` listing all eight. No renamed copies are added.

## Order of work

1. This plan; audit with `rg -i 'chipwave ?tracker|ChipWaveTracker|chipwave_tracker|chipwave-tracker'`.
2. App code, installer, Mac/Linux packaging, scripts, workflows.
3. Updater: new URL, new User-Agent, compare test 0.2.3.19 → 0.2.4.0.
4. Docs: README, docs/*.md, CHANGELOG 0.2.4.0, dist/README.txt, credits generator, Cargo.toml.
5. Verify: cargo test, clippy (Linux + Windows), cargo deny, Wine self-test, Wine upgrade from the
   v0.2.3.19 installer, check-ico, screenshots (About, title).
6. Push to main, then `gh repo rename ChipWaveStudio` (done 18 Oct 2026, after Wave Hub 0.2.5, which follows the redirect); update the remote, description, homepage, topics;
   check the redirects.
7. Done: website on gh-pages at the new address (canonical, og:url, og:image, sitemap, robots, JSON-LD,
   "formerly ChipWave Tracker").
8. Done: release v0.2.4.0 (Windows + Mac, SHA256SUMS.txt), Latest.

## Known limits

- The old website address (postmanpat-303.github.io/ChipWaveTracker/) returns 404 after the rename.
  GitHub has no redirect for project Pages, and a placeholder repo named ChipWaveTracker would take
  over the name and break the repo/API/download redirects that old updaters rely on, so it must not
  be created. Search engines will pick up the new address from the sitemap, the README and links.
- Old release notes and old download files still say "ChipWave Tracker".
