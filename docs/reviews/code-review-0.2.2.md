# ChipWave Tracker – Code review at 0.2.2 (commit ee4abee)

Review date: 2026-10-16. Review only; no code was changed.

## Tooling results
- `cargo test`: **51 passed, 0 failed** (toolchain 1.99.1 at ~/.cargo/bin; /usr/bin/cargo 1.85 is too old for egui 0.37 and fails, so put ~/.cargo/bin first on PATH).
- `cargo clippy --all-targets`: **0 warnings** (default lints).
- `cargo clippy -- -W clippy::pedantic` (advisory): ~500 warnings, almost all cosmetic: numeric casts (usize→f32, f32→usize), unseparated long literals, `map().unwrap_or()`, redundant closures, single-character names. None point to a real bug. Do not chase them wholesale; the f32→usize casts are worth a look wherever they index arrays.
- `cargo audit` (cargo-audit 0.22.1, 1329 advisories, 341 crates): **no vulnerabilities or warnings found**.
- `unsafe`: 2 blocks, both small Win32 FFI (`audio.rs:286` timeBeginPeriod/SetThreadPriority, `main.rs:173` AttachConsole). Sound.

## Architecture note (important for reading the findings)
The cpal callback (`audio.rs:201-236`) only pops from an rtrb ring: no lock and no allocation. That part is good. The real-time risk sits one level up: the **engine thread** (`audio.rs:164-191`) renders under `Arc<Mutex<Shared>>`, and the UI takes that same mutex (≈110 `self.with(..)` / `lock()` sites). Anything slow the UI does while holding the lock drains the ring and causes an underrun. The ring absorbs about 20–40 ms by default.

---

## HIGH

### H1. Save holds the audio mutex during the whole save and verify
`app.rs:539`: `self.with(|s| recovery::atomic_save(&s.song, &path, nb))`. atomic_save (`recovery.rs:38-65`) zips everything, encodes every sample as 32-bit WAV, fsyncs, **re-loads the file to verify** and rotates backups, all while the mutex is locked. With samples loaded this takes tens to hundreds of ms, so playback glitches or drops out on every Ctrl+S.
**Fix:** clone the song under the lock (`let song = self.with(|s| s.song.clone())`), then save outside it. Better still, save on a worker thread the way autosave does, and report the result through a channel.

### H2. UI panels hold the audio mutex while drawing
`app.rs:1335`: `let mut s = shared.lock().unwrap();` stays held for the whole instrument/patch editor (sliders and all), so the engine thread can't render while egui lays out that panel. Similar long-held guards probably exist in the sampleview, fxview and mixer code (worth an audit).
**Fix:** copy the patch out, edit the copy, write it back under a short lock when it changes. Longer term, move to a message/command queue (UI → engine) or a double-buffered `Arc<Song>` swap (e.g. `arc-swap`), so the engine never waits on the UI.

### H3. Poisoned mutex cascades into a full crash
`lock().unwrap()` is used everywhere: engine thread `audio.rs:177`, `App::with` `app.rs:410`, `app.rs:440`, `app.rs:1335`, `audio.rs:144`. A panic in any lock holder poisons the mutex. The next `unwrap` panics on the other thread too, so audio dies and the UI crashes. The panic hook does handle poison for its snapshot, which helps, but the app still goes down.
**Fix:** a helper `fn lock(&self) -> MutexGuard<..> { self.shared.lock().unwrap_or_else(|p| p.into_inner()) }`, used everywhere. In the engine thread, also catch a render panic (`catch_unwind`) and output silence rather than killing the thread.

## MEDIUM

### M1. Pasting crafted or odd clipboard text can panic (slicing a UTF-8 string by bytes)
`fileio.rs:61-63`: `if f.len() == 4 { hex2(&f[0..2]) .. hex2(&f[2..4]) }`. `len()` counts bytes, so a token like `aé1` (4 bytes) panics with "byte index 2 is not a char boundary" (reproduced). This is reachable from clipboard paste (`input.rs:137`, after the CHIPWAVE header line) and from a corrupt or hand-edited `.cwt` (`rows_from_text`, `fileio.rs:83`).
**Fix:** `if f.len()==4 && f.is_ascii()`, or use `f.get(0..2)`. Add a fuzz/proptest over `cell_from_text`.

### M2. "Quick export WAV" (Ctrl+E) renders the entire song on the UI thread
`app.rs:671-689` (called from `app.rs:962` and `topbar.rs:259`): `engine::render_song` runs synchronously, buffering the whole song plus a 3 s tail in a Vec. A 7-minute track freezes the window for seconds ("Not responding" on Windows), and memory grows by about 150 MB for 7 min stereo f32. The proper export dialog (`export::spawn`, a threaded job with progress) already exists.
**Fix:** route Ctrl+E to `export::spawn` with WAV defaults and the last path, then delete `export_wav`.

### M3. Undo doesn't cover everything the user can edit
`snapshot()` (`app.rs:414-430`) stores patterns, order, automation, per-instrument FX and returns only. Changes to instrument patch/volume, samples, mixer channel volume/pan/mute, bpm/speed, channel count/names, title and instrument add/delete aren't undoable, or worse, undo lands in a mismatched state. Example: undo after deleting an instrument zips `p.3` (FX lists) onto the wrong instruments (`app.rs:446-450`), so effect chains shift onto the wrong sounds.
**Fix:** snapshot a whole `SongState` (everything except sample PCM, which should be held in `Arc<[f32]>` so clones are cheap), or move to command-based undo. Add tests: edit → undo → equals the original, for each edit type.

### M4. Undo cost grows with the song (full clones on every edit, O(n) eviction)
Every edit clones all patterns and all automation (lanes allow up to 100k points), keeping up to 500 copies, and `undo.remove(0)` is O(n) on a Vec (`app.rs:425`). A big song means a lot of memory and per-keystroke stalls, and the clone happens **under the audio mutex**.
**Fix:** `VecDeque` with `pop_front`; `Arc`-share unchanged patterns (`Vec<Arc<Pattern>>`, copy-on-write) or store per-edit diffs; cap by memory rather than count.

### M5. Allocation and string parsing on the engine thread under the lock
`engine.rs:494`: `Target::parse(&lane.target)` runs for every enabled lane **every 32 frames**, and `parse` allocates (`automation.rs:112` `split('.').collect::<Vec<_>>()`). Also `engine.rs:514` clones a whole synth patch the first time per instrument, and `fx.rs:943-944` rebuilds FX states (delay/reverb buffers up to ~2 s) when a chain's structure changes, all under the mutex. None of this runs in the cpal callback itself, but it lengthens lock hold time and allocator contention with the UI.
**Fix:** parse targets once (store `Target` in `Lane`, or cache `Vec<Option<Target>>` keyed on lane count/generation). Prebuild FX states on the UI side and hand them over.

### M6. The fsync before rename does nothing on Windows; nested temp/bak names
`recovery.rs:41-42` opens the temp file **read-only** and calls `sync_all`. On Windows `FlushFileBuffers` needs write access, so it fails, and the error is ignored. A power cut right after save can leave a truncated `song.cwt`. Also, `atomic_save` writes to `song.cwt.tmp`, while `save_cwt` independently writes to `with_extension("cwt.tmp")` of *that* path and does its own rename-to-`.bak` (`fileio.rs:134, 228-231`), producing `song.cwt.cwt.tmp`/`.cwt.cwt.bak` artefacts and two different backup schemes.
**Fix:** make `save_cwt` write to a `Write` (or a given path) with no rename logic, call `file.sync_all()` on the writable handle before dropping it, keep all rename/backup logic in `atomic_save`, and remove `fileio.rs:228-231`.

### M7. Silent data loss limits on load and save
- `load_cwt` stops at 1000 blocks (`fileio.rs:258`) and 256 automation lanes (`fileio.rs:303`), with no error, while save writes as many as exist. Block 1000+ quietly vanishes after reload (and `{:03}` formatting grows to 4 digits).
- An undecodable sample (`fileio.rs:286-291`) silently becomes empty.
- An invalid automation lane is skipped silently (`fileio.rs:306`).
**Fix:** enforce the same limits in the UI, or loop until the entry is missing. Collect load warnings and show them ("2 items could not be loaded").

### M8. Concurrent autosaves can overlap
`recovery.rs:106-110` spawns a fresh thread for each autosave, and the slot rotation + `atomic_save` to the same `autosave.cwt.tmp` isn't serialised. With a 1-minute interval and a big song on a slow disk (or the on_exit blocking autosave racing a background one, `app.rs:3339`), two writers can clobber the temp file. The verify step makes a corrupt result unlikely to replace the good copy, but rotation can still drop a generation.
**Fix:** one long-lived autosave worker fed by a channel (latest snapshot wins), or an `AtomicBool` in-progress guard. Join it in `on_exit`.

## LOW

- **L1** `audio.rs:230`: `eprintln!` inside the cpal callback (stderr I/O on the RT thread). Instead, count the underrun and log it from the UI/engine thread.
- **L2** `audio.rs:287`: `timeBeginPeriod(1)` is never paired with `timeEndPeriod` when the engine thread stops (each device change starts a new one, and the count leaks). `THREAD_PRIORITY_TIME_CRITICAL` on a thread that also takes the UI-shared mutex risks priority inversion; `HIGHEST`, or MMCSS (`AvSetMmThreadCharacteristicsW("Pro Audio")`), is the usual choice.
- **L3** `audio.rs:145-148`: dead code (`was_playing` computed and discarded). A device change always stops playback; either restore it or remove the variable.
- **L4** `audio.rs:174`: `RING_FRAMES * 2 - prod.slots()` is fine, but `target_frames` can reach `RING_FRAMES - CHUNK*2` while the prefill uses `target*2` samples. That's consistent, just add a comment or an assert.
- **L5** `app.rs:459-460, 891, 913`: `n - 1` / `rows() - 1` assume at least one pattern or row. True today (the loader guarantees it), but it's fragile; use `saturating_sub`.
- **L6** `engine.rs:329`: direct index `song.instruments[c.instr]` is guarded by the check above it. OK.
- **L7** `fileio.rs:410`: `expect("demo toml")` on embedded data. Acceptable, and covered by the `demo_loads_and_saves` test.
- **L8** `load_cwt`: `lines_per_beat` is only `max(1)`, with no upper clamp; clamp it to something like 1..=32 like speed.
- **L9** Fill-to-length generation (`fillview.rs:87`) clones the song and runs the DP plus annealing on the UI thread. Fine for typical sizes; move it to a worker if large targets ever stall.
- **L10** Repaint: `app.rs:3594` repaints continuously while playing (expected). Make sure idle doesn't repaint every frame (topbar.rs:210 calls `request_repaint` unconditionally; check its condition).

## Dependencies and licensing
- GPL-3.0-or-later app + `mp3lame-encoder`/`mp3lame-sys` (LGPL, LAME 3.100 statically linked): **compatible**. LGPL code may be combined into a GPL-3 work, and since the whole program's source is published, relinking requirements are satisfied.
- **Gap:** the release zip (`dist/ChipWave-0.2.2-windows-x64.zip`) contains only `ChipWave.exe` and `README.txt`. GPL-3 §4/6 expects the licence text to ship with binaries, along with the source location (README has a URL, good) and third-party notices. Add `LICENSE.txt` (GPL-3), `THIRD_PARTY_LICENSES.txt` (LAME LGPL, plus the MIT/Apache notices of egui/cpal/zip/etc., generated with `cargo about` or `cargo-bundle-licenses`), and an About box line.
- `cargo audit` is clean. Consider adding `cargo deny check licenses advisories` to CI.
- `symphonia` is dev-only (MP3 round-trip test). Fine.

## Test gaps
Covered: arrange (11), input/clipboard (9), engine (5), sample (5), fileio (4), fx (4), recovery (3), export (3).
Missing:
1. Undo/redo round-trip for each kind of edit (see M3).
2. Fuzz/property tests for `cell_from_text`, `clip_from_text` and `load_cwt` on corrupt zips (would have caught M1).
3. Load/save round-trip of a song *with samples*, mixer settings, roles and derived blocks, compared field by field (not only cells).
4. Limits: >1000 blocks, >256 lanes (M7).
5. Engine: rendering a song with all FX kinds and automation for N seconds without NaN/Inf, and no allocation in `render` (a counting allocator test, e.g. `assert_no_alloc`).
6. Audio thread: a stress test holding the UI lock for X ms to measure underruns. There's a `--play-test`, but it isn't run in CI.
7. No UI tests; consider `egui_kittest` for the key-handling smoke paths (Space/Shift+Space, Ctrl+C/V).

## Structure and maintainability
- `src/app.rs` is **3,598 lines / 64 fns**, plus `arrange.rs` (1,675), `fx.rs` (1,074) and `fillview.rs` (1,015). Split `app.rs` into `app/pattern.rs` (editor + keys), `app/arranger.rs`, `app/instruments.rs`, `app/undo.rs`, `app/file.rs` (open/save/recent/autosave). The `app/` submodule pattern already exists.
- The lock helper `self.with(..)` is called ~110 times; each is a potential long hold. Centralising state access (e.g. `UiSong` working copy + `commit()`) would make H1/H2/M4 easier to fix and test.
- Duplication: two WAV export paths (`engine::write_wav` vs `export.rs`), two save/backup schemes (`fileio::save_cwt` vs `recovery::atomic_save`), and repeated TOML-entry write blocks in `save_cwt` (a small `write_toml(z, name, &value)` helper would remove about 40 lines).
- `Target` lives as a `String` in `Lane` and is re-parsed on the hot path; store the enum (serde can keep the string form).

## Recommended fix order
1. H1 save off the lock → H2 short lock scopes in panels → H3 poison-tolerant lock helper (one small PR; big stability win).
2. M1 UTF-8 slice panic (one-line fix + fuzz test).
3. M2 route Ctrl+E to the threaded exporter.
4. M6 + M8 save pipeline cleanup (fsync, single backup scheme, serialised autosave) and M7 load warnings.
5. M3/M4 undo redesign (whole-state snapshot with Arc-shared samples/patterns, VecDeque) + tests.
6. M5 pre-parsed automation targets; L1/L2 audio thread polish.
7. Licence files in the release zip; cargo-deny in CI.
8. Split app.rs before MIDI (0.2.3) and VST work make it bigger.

---

## Status after 0.2.2.1 (fix release)
Fixed: H1 (save runs on a worker thread from a snapshot), H2 (patch panel edits a copy, commits under a short lock; no other UI code holds the lock across drawing), H3 (`audio::lock` recovers poisoned mutexes; engine render wrapped in `catch_unwind` → silence + engine reset), M1 (ASCII check + fuzz test), M2 (Ctrl+E uses the threaded exporter), M5 (allocation-free `Target::parse`, cached parsed targets, allocation-free patch copy; test proves `render` makes 0 allocations after warm-up), M6 (`save_cwt` writes + fsyncs through the writable handle, single tmp/.bak scheme in `atomic_save`, temp removed on failure), M7 (limits 10 000 blocks / 4096 lanes enforced on save, load warnings window), M8 (autosaves never overlap; exit waits for a running one), L1, L2 (timeEndPeriod via guard, THREAD_PRIORITY_HIGHEST), L3, L8, decode_wav shift overflow on 0-bit WAVs. Licence + third-party notices in the zip (`scripts/package-windows.sh`, `scripts/third_party_licenses.py`), `cargo deny check` in CI (`deny.toml`).

Still open (next): M3/M4 undo redesign, app.rs split, FX state rebuild when a channel switches instrument (allocates on the engine thread mid-song), L5, L9, L10.
