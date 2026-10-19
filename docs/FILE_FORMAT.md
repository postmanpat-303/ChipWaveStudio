# ChipWave Studio: Native Song Format (`.cwt`)

**Status (0.2.3.14):** implemented: the `.cwt` ZIP container (TOML + WAV), the single-file text song `.cwt.toml` (used by the demos; `patch = "gen:<preset>"` / `"voice:<preset>"` load factory sounds), `.cwi` instrument files ([INSTRUMENT_LIBRARY.md](INSTRUMENT_LIBRARY.md)), autosave and backups. Up to 32 channels per song (0.2.3.8). New features add fields with defaults, so older songs load unchanged; a song that uses a newer instrument type does not open in an older version. Render-to-sample links, MED/MOD import and file association below are still plans.

`.cwt` is ChipWave's main save format. It holds everything MMD/MOD can't: arranger layout, synth patches and FX chains, render links, settings. MMD/MOD stay **import/export** formats ([OCTAMED_SPEC.md](OCTAMED_SPEC.md)).

## 1. Recommendation: a ZIP container with TOML text plus binary samples

A `.cwt` file is a standard **ZIP** archive, so anyone (or any AI) can open it with a normal unzip tool, read the text files, diff them in git, and fix them by hand.

```
song.cwt (zip)
├── mimetype                    "application/x-chipwave-song" (first entry, stored uncompressed)
├── manifest.toml               format id, versions, file list + checksums
├── song.toml                   song info, tempo, settings, track count/pans/volumes
├── sequences.toml              playing sequences + section list (== MMD2 structure)
├── arranger.toml               colours, repeats, lanes, markers, zoom, loop range
├── blocks/
│   ├── 000.toml                one block: name, lines, tracks, rows (compact text)
│   └── 001.toml
├── instruments/
│   ├── 01.toml                 instrument header (type, volume, finetune, transpose, loop…)
│   ├── 01.wav                  embedded sample (mono/stereo, 8/16-bit or float)
│   ├── 02.toml                 synth instrument → points at a patch
│   └── 03.toml                 OctaMED synth/hybrid (wave tables + vol/wf sequences)
├── plugins/
│   └── 07.toml + 07.bin        CLAP/VST3 plugin id/version + saved state (MIDI_VST.md)
├── patches/
│   └── acid-paula-bass.cwpatch same TOML as standalone presets (SOUND_DESIGN.md §8)
├── export.toml                 remembered export settings (EXPORT.md)
└── thumbnails/arranger.png     optional preview for file managers
```

Why this layout:
- **Inspectable:** text for everything structural; samples as plain WAV that any editor can open.
- **Patch reuse:** `patches/*.cwpatch` is byte-for-byte the preset format, so drag a patch out of a song and it's a preset, drop one in and it's an instrument.
- **Robust:** one damaged sample doesn't take out the song; the loader can skip it and warn.
- **Proven:** the same approach as OpenDocument (`mimetype` first), EPUB and Bitwig-style project bundles.
- **Compression:** Deflate for TOML (high ratio), WAV samples **stored** or Deflate (lightweight; FLAC-in-zip is an option for later). Zstd is avoided in v1 because many unzip tools can't read it.

### Block text encoding
Rows use a fixed-width, tracker-looking line so it's readable and diffs nicely:
```toml
name = "Chorus"
lines = 64
tracks = 8
# row: note ins cmd-arg [cmd-arg...] | per track
rows = """
00 | C-3 01 0C40 | --- .. .... | E-2 03 0000 | ...
01 | --- .. .... | --- .. .... | --- .. .... | ...
"""
```
(Empty trailing rows can be omitted; a binary `rows.bin` alternative is reserved for very large songs if parsing speed ever matters.)

### Render-to-sample links
`instruments/05.toml`:
```toml
type = "sample"
sample = "05.wav"
[source]
patch = "acid-paula-bass.cwpatch"
note = "C-3"
param_hash = "blake3:9f1c…"   # hash of the patch's parameters when rendered
keep_live = true
```
If the hash no longer matches, the sample shows as stale (SOUND_DESIGN.md §7).

## 2. Versioning and forward compatibility
`manifest.toml`:
```toml
format = "chipwave-song"
format_version = 1            # bump on breaking change
min_reader_version = 1        # oldest reader that can still open it safely
app = "ChipWave Studio 0.3.0"
created = 2026-10-16T13:45:00+01:00

[[file]]
path = "song.toml"
size = 812
crc32 = "8a1f22c0"
blake3 = "…"
```
- Readers open any file with `min_reader_version <= their version`. **Enforced since the Timeline M0 work (Unreleased):** this version reads version 1. A `.cwt` whose `manifest.toml`, or a `.cwt.toml` whose top level or `[song]` table, asks for more is refused with "made with a newer ChipWave Studio, please update" and a Check for updates button, instead of loading half the song. **This version reads version 2:** timeline-mode songs (Timeline M2, Unreleased) add `timeline.toml` to the `.cwt` (a `[timeline]` table in a `.cwt.toml`) and write `min_reader_version = 2`; tracker-mode songs still write `1` and are unchanged byte for byte ([TIMELINE.md](TIMELINE.md#file-format-and-older-versions)). File › Export tracker-compatible copy saves a version 1 song. Newer optional data goes in new keys or files, which older readers **keep as-is when re-saving** (unknown keys and files are preserved, not dropped).
- Each loader has migration steps `v1 → v2 → …`, tested with stored example files.

## 3. Integrity
- ZIP already has a CRC-32 per entry; the manifest adds a **BLAKE3** hash per file for stronger checks and to detect edits made outside the app (shown as a note, not an error).
- **Safe saving:** write to `song.cwt.tmp`, fsync, then atomically rename; keep the previous version as `song.cwt.bak`. Autosave every few minutes to a separate recovery file.

## 4. Safe loading (don't repeat Octopus-tracker)
Our Octopus-tracker review found its song loader trusts the file without checks, so a damaged file can crash it. ChipWave must treat every file, including its own `.cwt`, as untrusted:
- Limits checked **before** allocating: max file size (e.g. 2 GB), max zip entries (10 000), max uncompressed size per entry and in total (**zip-bomb guard**, plus compression ratio cap), max lines per block (3 200, as in OctaMED), tracks 1–64, max samples, sample length.
- Reject absolute paths and `..` in entry names (**zip-slip**), and never extract to disk; read entries into memory.
- All indices (block numbers in sequences, instrument numbers in rows, patch references) are validated after parsing; bad ones become empty/default with a warning list shown to the user.
- No panics: parsing returns `Result`, with fuzz tests (`cargo fuzz`) on the `.cwt`, MMD and MOD loaders.
- The same rules apply to MMD/MOD import (pointer offsets checked against file length).

## 5. Crates (checked on crates.io, 16 Oct 2026)
| Crate | Version | Licence |
|---|---|---|
| `zip` | 9.0.4 | MIT |
| `toml` + `serde` | 1.1.8 / 1.0.229 | MIT OR Apache-2.0 |
| `hound` | 3.5.1 | Apache-2.0 |
| `crc32fast` | 1.5.2 | MIT OR Apache-2.0 |
| `blake3` | 1.8.7 | CC0-1.0 OR Apache-2.0 (+LLVM exception) |
| `flate2` | 1.1.10 | MIT OR Apache-2.0 |

## 6. Related file types
| Extension | MIME | What |
|---|---|---|
| `.cwt` | `application/x-chipwave-song` | song (this doc) |
| `.cwi` | `application/x-chipwave-instrument` | one instrument, ZIP of `instrument.toml` (+ `sample.wav`) (INSTRUMENT_LIBRARY.md) |
| `.cwpatch` | `application/x-chipwave-patch` | synth patch preset, plain TOML (SOUND_DESIGN.md §8) |

## 7. File association
**Linux** (also inside the Flatpak):
- `share/mime/packages/chipwave.xml` (shared-mime-info):
```xml
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-chipwave-song">
    <comment>ChipWave Studio song</comment>
    <sub-class-of type="application/zip"/>
    <glob pattern="*.cwt"/>
    <magic priority="60"><match type="string" offset="30" value="mimetypeapplication/x-chipwave-song"/></magic>
  </mime-type>
  <mime-type type="application/x-chipwave-patch">
    <comment>ChipWave synth patch</comment>
    <sub-class-of type="text/plain"/>
    <glob pattern="*.cwpatch"/>
  </mime-type>
</mime-info>
```
- The `.desktop` file gets `MimeType=application/x-chipwave-song;application/x-chipwave-patch;` and `Exec=chipwave %f`. The package runs `update-mime-database` and `update-desktop-database` (pacman hooks do this automatically on Arch/CachyOS).
- Icons: `share/icons/hicolor/*/mimetypes/application-x-chipwave-song.png`.

**Windows 11:**
- The installer (MSI via `cargo-wix`, or Inno Setup) registers a ProgID under `HKCU\Software\Classes` (per-user, no admin needed): `.cwt` → `ChipWave.Song`, with `DefaultIcon` and `shell\open\command = "…\chipwave.exe" "%1"`; the same for `.cwpatch` → `ChipWave.Patch`.
- Also add `OpenWithProgids` and an `Applications\chipwave.exe\SupportedTypes` entry. Windows 11 won't let an app make itself the default without asking, so the user picks it in "Open with" the first time.
- The portable `.exe` (no installer) offers a "Register file types" button in Settings that writes the same per-user keys.

## 8. Roadmap placement
**MVP:** you can't make songs without saving them. v1 holds blocks, sequences, sample instruments, song info and settings. Patches, render links and arranger data arrive with phase 1.5 as new files/keys, as the forward-compatibility rules above allow.
