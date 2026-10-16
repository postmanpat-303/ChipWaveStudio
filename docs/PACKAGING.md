# Packaging for Windows, Linux and macOS

Del: *"packaging for the main 3 OSes Mac, Windows, Linux. I do not have a Mac to test anything."*

Every release is built and tested by **GitHub Actions** on GitHub's own Windows, Linux and macOS machines, so the Mac build is compiled *and* run-tested without anyone owning a Mac. The workflow skeleton is [`.github/workflows/ci.yml`](../.github/workflows/ci.yml).

Tool versions below were checked live on 16 Oct 2026 (crates.io / GitHub releases):

| Tool | Version | Licence | Used for |
|---|---|---|---|
| `cargo-packager` | 0.11.8 | MIT/Apache-2.0 | One config → .msi/NSIS .exe, .dmg/.app, AppImage, .deb, from the Tauri team |
| `cargo-bundle` | 0.12.0 | MIT/Apache-2.0 | Alternative for `.app` / `.deb` |
| `cargo-dist` (dist) | 0.32.0 | MIT/Apache-2.0 | Alternative: generates release CI + installers |
| `cargo-wix` | 0.3.9 | MIT/Apache-2.0 | WiX MSI if we want MSI directly |
| `cargo-deb` | 3.8.0 | MIT | `.deb` |
| `cargo-aur` | 1.7.1 | MIT | Generates a PKGBUILD for `-bin` AUR packages |
| `actions/checkout` | v7 | — | CI |
| `actions/upload-artifact` | v7 | — | CI |
| `softprops/action-gh-release` | v3 | — | Attaching files to GitHub Releases |
| `dtolnay/rust-toolchain` | `@stable` | — | Rust install in CI |
| `actions/cache` | v6 | — | Cargo cache |
| `flatpak/flatpak-github-actions` | v6 | — | Flatpak bundle build |

**Plan:** use `cargo-packager` for Windows, macOS, AppImage and .deb (one `[package.metadata.packager]` section in Cargo.toml). Flatpak and the Arch PKGBUILD are hand-written files, which is what those ecosystems expect.

## 0. Priority: Windows first (Del's request)
The **first runnable version targets Windows 11 x86_64**, built on the Linux box and tested under Wine before Del runs it on his PC (`C:\Users\DMAN\Documents\GrokPrograms`).
- **Cross-compile:** `x86_64-pc-windows-gnu` with mingw-w64. That's the route already proven with the Strobe .exe. Or `x86_64-pc-windows-msvc` via **cargo-xwin** (0.23.1, MIT, checked 16 Oct 2026), which downloads the MSVC CRT/SDK. Before using it, the Microsoft licence terms for those downloads need accepting and checking. Start with GNU; switch to MSVC if a dependency needs it (e.g. ASIO or VST3 hosting quirks).
- **Box test:** `wine chipwave.exe --self-test` and `--render` (null audio backend). A short GUI run under Wine on the box desktop takes a screenshot. Wine's audio (WASAPI via PulseAudio) is good enough for a smoke test; the real check is on Del's PC.
- **Deliverable:** portable zip first; the installer comes later. Linux (CachyOS) builds come next, then macOS via CI.

## 1. Windows 11
- **Installer:** NSIS `.exe` setup from cargo-packager (per-user install with no admin needed, Start menu shortcut and icon, `.cwt` file association, uninstaller). An MSI (WiX) is offered as well for people who prefer it.
- **Portable zip:** `ChipWave-x.y.z-windows-x64-portable.zip` with `chipwave.exe` and a `portable.txt` marker. With the marker present, settings live next to the exe instead of `%APPDATA%`.
- **Build:** `x86_64-pc-windows-msvc` on `windows-latest`; Windows on Arm (`aarch64-pc-windows-msvc`) later.
- **SmartScreen:** unsigned installers show "Windows protected your PC → More info → Run anyway" until the app builds reputation. A code-signing certificate costs money; Azure Trusted Signing is a cheaper option to evaluate later. The README documents this honestly.

## 2. Linux (CachyOS first)
- **Arch / CachyOS:** two AUR packages.
  - `chipwave` builds from source with a PKGBUILD in `packaging/arch/`: `depends=(alsa-lib libxkbcommon wayland libx11 lilv)`, `makedepends=(cargo)`.
  - `chipwave-bin` repackages the release tarball (`cargo-aur` can generate it).
  - Same approach as the Strobe app, which already worked on Del's machine.
- **Flatpak / Flathub:** `io.github.postmanpat_303.ChipWave` on the `org.freedesktop.Platform` runtime with the Rust SDK extension. Dependencies are vendored with `flatpak-cargo-generator` (Flathub builds offline).
  - Permissions: `--socket=pulseaudio` (PipeWire's Pulse layer), `--device=all` for MIDI, `--filesystem=xdg-music`, read-only plugin folders, and Wayland/X11.
  - Plugins can't easily be loaded from inside the sandbox (system plugin folders need host paths and matching libraries), so the docs say so and recommend AUR/AppImage for heavy plugin users.
- **AppImage:** universal single file for other distros, built on the oldest supported Ubuntu runner for glibc compatibility.
- **.deb (optional):** for Ubuntu/Mint/Debian, via cargo-packager or `cargo-deb`.
- **Desktop integration everywhere:** `.desktop` file, icons (16–512 px + SVG), AppStream metainfo XML, and a MIME type for `.cwt`.

## 3. macOS (without a Mac)
- **Universal `.app` in a `.dmg`:** build `aarch64-apple-darwin` and `x86_64-apple-darwin` on a macOS runner, join them with `lipo -create`, bundle with cargo-packager (or cargo-bundle), then make a `.dmg`. Minimum macOS 11. `Info.plist` sets `NSMicrophoneUsageDescription` (for audio input) and a `.cwt` document type.
- **Signing and notarisation:** proper Developer ID signing plus Apple notarisation needs a **paid Apple Developer Program membership** (US$99/year at the time of writing; check before buying). Until then:
  - Builds are **ad-hoc signed** (`codesign --force --deep -s - ChipWave.app`). Apple Silicon requires at least an ad-hoc signature for native code to run.
  - Gatekeeper will block the downloaded app the first time. The workaround to document: **right-click (Control-click) → Open → Open**. On recent macOS, if that isn't offered, use **System Settings → Privacy & Security → "Open Anyway"**. Or run `xattr -dr com.apple.quarantine /Applications/ChipWave.app`.
  - Once there's a paid account, CI adds the certificate as an encrypted secret, signs with `codesign --options runtime`, notarises with `xcrun notarytool submit --wait`, and runs `xcrun stapler staple`.
- **Audio/MIDI:** cpal uses CoreAudio and midir uses CoreMIDI. Plugin folders are `~/Library/Audio/Plug-Ins/{CLAP,VST3}` and `/Library/Audio/Plug-Ins/{CLAP,VST3}`, plus `~/Library/Audio/Plug-Ins/LV2` for LV2.
- **Mac testers:** besides CI, ask a friend or the community (e.g. an English Amiga Board or KVR thread) to try pre-releases. Feedback goes in a GitHub issue template that asks for the macOS version and chip type.

## 4. CI: build on every push, release on tag
Matrix: `ubuntu-latest`, `windows-latest`, `macos-latest` (Apple Silicon). Steps per OS:
1. Checkout, install Rust stable (+ the extra macOS target), cache `~/.cargo` and `target`.
2. `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`.
3. Build in release mode.
4. **Headless smoke test** (§5).
5. On `v*` tags: package and attach artifacts to a GitHub Release (draft first, so Del can check before publishing).

Until Cargo.toml exists, the workflow detects that and skips the Rust steps, so it's green today.

## 5. Headless smoke tests (how the Mac build gets tested)
The app gets built-in test modes that need no screen or sound card:
- `chipwave --version` prints the version and exits 0.
- `chipwave --self-test` initialises the engine with a **null audio backend**, loads every built-in preset and template, runs the plugin scanner on an empty folder, round-trips a `.cwt` (save → load → compare), and exits 0 or 1 with a report.
- `chipwave --render tests/songs/smoke.cwt --out smoke.wav --deterministic` renders a test song offline with no audio device, using fixed seeds, no dithering and no wall-clock data.
- CI compares the WAV with `tests/golden/smoke.wav`:
  - Same OS/arch: **exact BLAKE3 hash** match.
  - Across OS/arch: floating-point maths can differ slightly between x86 and ARM, so the cross-platform check compares **samples within a tolerance** (e.g. max abs diff < 1e-6, RMS diff < −120 dB) instead of the hash. Both checks are in `tests/golden.rs`.
- **GUI check:** on Linux, run the app for 5 s under `xvfb-run` with `--screenshot out.png --exit-after 5` and upload the screenshot as an artifact. macOS runners have a real (virtual) display, so the same command runs there and **you can see the Mac build's screenshot in the Actions run** without owning a Mac. Windows runners can do the same.

## 6. Release file names
```
ChipWave-1.0.0-windows-x64-setup.exe
ChipWave-1.0.0-windows-x64.msi
ChipWave-1.0.0-windows-x64-portable.zip
ChipWave-1.0.0-macos-universal.dmg
ChipWave-1.0.0-linux-x86_64.AppImage
ChipWave-1.0.0-linux-x86_64.tar.gz      (used by the AUR -bin package)
chipwave_1.0.0_amd64.deb
io.github.postmanpat_303.ChipWave.flatpak   (Flathub later)
SHA256SUMS.txt
```
