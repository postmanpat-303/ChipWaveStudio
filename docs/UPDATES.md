# Update check (0.2.3.5)

ChipWave can tell you when a newer version is published on GitHub. It never downloads or installs anything by itself, and it opens nothing unless you click.

## Help > Check for updates…

ChipWave asks `https://api.github.com/repos/postmanpat-303/ChipWaveStudio/releases/latest` (with a `ChipWave-Studio/<version>` User-Agent and a 5 second timeout) on a background thread, so the window never freezes. If that answers 404 (before the repo rename from ChipWaveTracker) it asks the old address `…/repos/postmanpat-303/ChipWaveTracker/releases/latest` instead; other errors are reported straight away. Versions up to 0.2.3.19 only know the old address, which GitHub redirects after the rename. Drafts and prereleases are ignored. The release tag (leading `v` stripped) is compared with the running version; 3- and 4-part versions compare number by number with missing parts as 0 (`0.2.3 < 0.2.3.4 < 0.2.4`).

- **Newer version:** a dialog shows "You have X, latest is Y", the release notes (simple markdown: headings, bullets, text) in a scrollable box and four buttons: **Download installer** (opens the release's `…setup-x64.exe` in your browser, or the release page if there is none), **Open release page**, **Not now** and **Skip this version** (no more startup reminders for that version).
- **Up to date:** "You're on the latest version (X)".
- **No network / GitHub error / rate limit:** a friendly message with **Try again** and **Open releases page**. Nothing crashes or hangs (5 s timeout).

## Startup check

**Settings > Files & Safety > Updates: "Check for updates at startup"** (on by default). At most once every 24 hours ChipWave checks quietly in the background after it starts (startup is never delayed). If a newer, non-skipped version exists, a small **"Update X available"** badge appears at the right of the menu bar and a note in the status bar; clicking the badge shows the dialog above. Up to date, offline or errors stay silent. **Check now** runs a manual check; **Un-skip X** clears a skipped version.

Saved in `settings.toml`: `update_check`, `update_last` (Unix time of the last startup check), `update_skip`.

## Command line and testing

- `ChipWave.exe --check-updates` runs the same check and prints the result (exit code 2 when the check failed).
- `CHIPWAVE_UPDATE_URL=http://127.0.0.1:8765/latest` points the check at a mock server serving a release JSON (used for manual testing).
- Unit tests: version comparison, parsing of a sample and of the real v0.2.3.4 release JSON, prerelease/draft handling, the 24 h rule, the notes renderer, a mock HTTP server (checks the User-Agent / Accept headers) and an unreachable server. UI tests drive the dialog with mocked results: every button, "opens nothing until clicked", skip, badge, failure and retry.

HTTPS uses `ureq` with `rustls` (ring) and the bundled Mozilla root certificates (`webpki-roots`, CDLA-Permissive-2.0); licences are in THIRD_PARTY_LICENSES.txt.
