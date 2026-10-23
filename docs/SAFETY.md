# Safety: autosave, crash recovery, recent files

> **Implemented in 0.1.9:** atomic saves with `.bak`, autosave every 2 min (only when there are unsaved changes, last 3 kept) to `<config>/recovery/<session>/` (`%APPDATA%\ChipWave\recovery` on Windows), session heartbeat, emergency `panic.cwt` + `crash.txt` from a panic hook, a recovery dialog on the next start (Recover / Discard / Later), and File > Recent (last 20, missing files greyed). Not yet: the edit journal, pinned recents, unsaved-change prompts, and the Preferences > Safety page.

ChipWave should never lose Del's work. This covers saving, recovering after a crash, the recent-files list and unsaved-change prompts. File-level integrity (checksums, safe loading) is in [FILE_FORMAT.md §3–4](FILE_FORMAT.md). Plugin crashes are isolated in a separate process ([MIDI_VST.md](MIDI_VST.md), [PLUGIN_MANAGER.md](PLUGIN_MANAGER.md)), so a bad plugin can't crash the app; you just see "plugin stopped, song kept playing".

**Config / data folders** (from the `directories` crate's conventions):
| OS | Config (settings, keymap, recent files) | Data (autosave, recovery) |
|---|---|---|
| Linux | `~/.config/chipwave/` | `~/.local/share/chipwave/` |
| Windows | `%APPDATA%\ChipWave\` | `%LOCALAPPDATA%\ChipWave\` |
| macOS | `~/Library/Application Support/ChipWave/` | same, `recovery/` subfolder |
The portable Windows build keeps everything next to the exe ([PACKAGING.md](PACKAGING.md)).

## 1. Saving (atomic, never half-written)
1. Serialise the song in memory, off the audio thread.
2. Write `song.cwt.tmp` in the **same folder**, then flush and `fsync` it.
3. Rename the old `song.cwt` → `song.cwt.bak`, then the temp file → `song.cwt` (atomic on the same filesystem; on Windows `ReplaceFileW`/`MoveFileExW` with replace-existing).
4. `fsync` the folder (Linux/macOS).
5. If any step fails: keep the old file, show the error, offer "Save as…".

**Rotating backups:** with "Keep backups" on (default 5), older `.bak` files rotate as `song.cwt.bak1…bak5`, or go to a `ChipWave Backups/` subfolder if you'd rather keep the song folder tidy.

## 2. Autosave
- **Interval:** every 2 minutes by default (1–30 min or off), and only when there are unsaved changes.
- It never overwrites your real file. It writes to `recovery/<session-id>/autosave.cwt` with the same atomic steps.
- It runs in a background thread with a snapshot of the song (copy-on-write data structures, so taking the snapshot is cheap). It doesn't stutter audio.
- The last 3 autosaves per session are kept.

## 3. Crash recovery
- **Session lock:** on start, ChipWave writes `recovery/<session-id>/session.toml` (pid, start time, open song path) and deletes it on clean exit.
- **Journal:** between autosaves, every edit is appended to `journal.log` as a compact serialised command (the same commands the undo system uses), flushed about once per second. Recovery = last autosave + replay of the journal, so at most about 1 s of work is lost.
- **On panic:** a `std::panic::set_hook` handler tries one emergency snapshot (`panic.cwt`) and writes `crash.txt` (message, backtrace, version). On hard crashes (segfault, power loss) the journal and autosave are already on disk.
- **Next launch:** if any session folder is left without a clean-exit marker, a **Recovery dialog** lists each one:
  - The song name, the time of the last change (local time, e.g. "Today 14:32"), and its source (autosave + journal, or emergency snapshot).
  - Buttons: **Recover** (opens as "Untitled (recovered)"; you choose where to save, the original is untouched), **Compare** (info only), **Discard**, and **Later**.
- **Since 0.2.10.0:** every normal exit (window ✕, Alt+F4, File › Quit, after Save or Don't save) deletes the session folder, so unsaved changes you did not keep are gone and nothing is offered next time. Only a crash or a killed process leaves a copy. Copies put off with **Later** move to `recovery-archive/` (newest 10 kept) on the next clean exit; leftovers of older versions' clean exits (heartbeat `0`, no `crash.txt`) are removed without asking.
- Recovery folders older than 30 days are cleaned up after asking.

## 4. Recent files
- **File → Recent**, plus the start screen: the last 20 songs, with **pinned** ones at the top (📌 toggle). Stored in `recent.toml` in the config folder, with path, last opened, pinned flag, and optional Order list thumbnail.
- **Missing files** show greyed out with "not found". Actions: **Locate…** (pick the new path and update the entry), **Remove**, or **Remove all missing**. Paths on unplugged drives or network shares are checked with a short timeout so the menu never hangs.
- It integrates with OS recent lists where available (Windows jump list, macOS Dock recent documents; Linux `recently-used.xbel` is optional).

## 5. Unsaved changes
- A dirty marker `•` in the title and tab shows when there are unsaved changes.
- **Prompts** on close, quit, open/new (single-window mode), and when loading a template over a song: **Save / Don't save / Cancel**, with Save as the default button.
- On macOS, Cmd+Q follows the same prompt.
- If the system is shutting down (logout), ChipWave writes an autosave immediately and quits; recovery offers it next time.

## 6. Undo history
- Undo is command-based (each edit has apply/revert), and the default limit is 500 steps or 256 MB, whichever is reached first.
- **Optional persistence** (off by default): saves the undo stack in the `.cwt` as `undo/history.bin`, so undo works after reopening a song. It's off by default because it grows files and could reveal deleted material if you share the song. "Clear history" and "Save without history" are available.

## 7. Settings
See [SETTINGS.md §2.6](SETTINGS.md).
In **Preferences → Safety**: autosave interval, number of backups and backup location, journal on/off, persistent undo, recovery cleanup age, and "Open recovery folder".
