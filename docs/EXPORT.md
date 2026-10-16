# ChipWave Tracker: Audio Export (WAV and MP3)

**Status:** design. No code yet.

## 1. What can be exported
| Scope | Description |
|---|---|
| **Whole song** | The full section list from start to end (or to the song-end command) |
| **Selected range / section** | A loop range from the arranger ruler, or one section/playing sequence (see [ARRANGER.md](ARRANGER.md)) |
| **Single block** | One block as it sounds at the current tempo, with an optional loop count (handy for making loops) |
| **Per-track stems** | One file per track (or per instrument), all the same length and start point so they line up in a DAW. Options: include master FX, skip silent tracks |

Other export routes stay as they are: **MOD/MMD** module export ([OCTAMED_SPEC.md](OCTAMED_SPEC.md)), **render-to-sample** for synth patches ([SOUND_DESIGN.md §7](SOUND_DESIGN.md)), and the native `.cwt` song ([FILE_FORMAT.md](FILE_FORMAT.md)).

## 2. Common options
- **Loop count:** play the song (or range) 1–16 times. Songs that loop back with a position-jump command are detected so the export doesn't run forever (hard cap: 60 minutes).
- **Tail:** *Cut*, *Let ring* (until output stays below −90 dBFS for 200 ms, max 30 s), or *Fade out* over N seconds (linear or equal-power), optionally only on the last loop.
- **Normalise:** off, peak (target e.g. −0.3 dBFS), or loudness (EBU R128 integrated LUFS target, e.g. −14 LUFS, with a true-peak ceiling). Stems use one shared gain so their mix stays correct.
- **Paula mode:** export with or without the Amiga output emulation.
- **File naming template:** `{song}-{section}-{track}.{ext}`, with an output folder (by default the song's folder, or `Documents\GrokPrograms`-style user setting).

## 3. WAV options
- Bit depth: **16-bit int**, **24-bit int**, **32-bit float**.
- Sample rate: **44.1 kHz** or **48 kHz** (the engine renders at the chosen rate; it isn't resampled afterwards).
- Channels: stereo or mono.
- **Dither** (int formats only): off, TPDF, or TPDF with simple noise shaping. On by default for 16-bit, off for 24-bit, not used for float.
- Metadata: a RIFF `LIST/INFO` chunk with title (`INAM`), artist (`IART`), comment (`ICMT`) and software (`ISFT`) from song info.
- Writer: `hound`. It writes int and float WAV but not INFO chunks, so we append that small chunk ourselves after closing the file (or write the RIFF header manually; it's simple).

## 4. MP3 options
- Mode: **CBR** or **VBR** (LAME quality V0–V9). Default **VBR V2** (~190 kbps).
- CBR bitrate presets: 128, 192, 256, **320 kbps**.
- Sample rate: 44.1 or 48 kHz (same as render). Joint stereo, mono or stereo.
- Encoder quality: LAME `-q` 2 (high) by default.
- **ID3v2.4 tags** from song info: title, artist, album, year, genre, comment ("Made with ChipWave Tracker"), track number, and optional cover art (PNG/JPEG from song info). Written with the `id3` crate after encoding.
- MP3 always takes 32-bit float from the engine (no dither needed; LAME does its own quantisation).

## 5. Offline render engine
- It uses **the same engine** as playback (same `Song`, voices, synth, FX and Paula code), just driven by a render loop instead of the cpal callback, so the export sounds exactly like playback.
- It runs **faster than real time** on a worker thread, in blocks of 512–4096 frames, without real-time throttling. Expect around 20–100× real time for typical songs; heavy synth patches are slower.
- Deterministic: random sources (noise, S&H LFOs) are seeded from the song, so two exports of the same song are identical. Golden-file tests rely on this.
- Stems: one render pass with per-track buses tapped (faster than N passes). If master FX is chosen, each stem is rendered separately with the master chain.
- Normalising or loudness targets need two passes: render to a temporary 32-bit float file, measure, then apply gain plus dither and encode. Single-pass is used when normalise is off.
- Audio playback is paused while exporting (or, as an option, keeps playing on a separate engine instance).

## 6. Progress UI
- A modal export dialog: scope, format tabs (WAV/MP3), options, output path with the naming preview, and *Export*.
- While rendering: a progress bar (song position ÷ estimated length, from a pre-scan of tempo and jump commands), current section/block name, elapsed and ETA, speed ("38× real time"), a mini level meter, and **Cancel** (deletes partial files).
- For stems, a per-file list with ticks.
- When finished: "Show in folder", "Play", and a clipping warning if any sample exceeded 0 dBFS before normalising.
- Settings are remembered per song (stored in `.cwt` settings) and as global defaults.

## 7. Rust crates (checked on crates.io, 16 Oct 2026)
| Crate | Version | Licence | Use |
|---|---|---|---|
| `hound` | 3.5.1 | Apache-2.0 | WAV writing (16/24 int, 32 float) |
| `mp3lame-encoder` | 0.2.5 | LGPL-3.0 | Safe Rust wrapper over LAME (CBR/VBR, quality, ID3 off) |
| `mp3lame-sys` | 0.1.11 | LGPL-3.0 | Bundles **LAME 3.100** source and builds it **statically**: autotools on Unix, `cc` with MSVC config on Windows |
| `id3` | 1.17.2 | MIT | ID3v2 tags |
| `rubato` | 5.0.1 | MIT OR Apache-2.0 | only if a resample step is ever needed |

Alternatives looked at: `lame-sys` 0.1.2 (last updated 2017, links to a system LAME), and `shine-rs` 0.1.5 (LGPL-2.0, a fixed-point encoder with lower quality). Neither beats `mp3lame-encoder`.

### Licensing and building notes
- **MP3 patents:** the last MP3 patents expired in 2017, and Fraunhofer ended its licensing programme, so MP3 encoding no longer needs a patent licence.
- **LAME is LGPL** (the `mp3lame-*` crates are published as LGPL-3.0). ChipWave's own licence hasn't been chosen yet. LGPL lets non-GPL apps use the library, but because `mp3lame-sys` links LAME **statically**, the LGPL requires that users can relink the app against a modified LAME. Options:
  1. **License ChipWave under a GPL-compatible open-source licence** (GPL-3.0, or MIT/Apache with sources published). Since the repo is public, publishing the source meets the relink requirement. This is the simplest route and the recommendation.
  2. If ChipWave were ever closed-source: load LAME **dynamically** (`libmp3lame.so` / `libmp3lame.dll`, e.g. via `libloading`) and ship LAME's source/licence alongside, or provide object files for relinking.
- Either way: ship LAME's licence text and a "uses LAME" credit in About, and **don't** enable the `decoder` feature (that pulls in GPL mpglib).
- **Linux build:** `mp3lame-sys` needs autotools (`autoconf`, `automake`, `libtool`) and a C compiler at build time. On CachyOS/Arch that's `base-devel`. Nothing extra at runtime since it's static. Flatpak builds work the same.
- **Windows build:** compiled with `cc` using the MSVC toolchain (Visual Studio Build Tools needed, already required for Rust's MSVC target). The single `.exe` keeps working with no DLL to ship.
- **Feature flag:** `mp3` cargo feature (on by default) so a WAV-only build stays possible on odd targets.

## 8. Roadmap placement
- **WAV:** MVP (as before).
- **MP3:** **MVP** too. It reuses the exact same offline renderer as WAV; the extra work is an encoder call, tags and a format tab (a day or two). Sharing tracks online is the main reason people export, and MP3 is what Del wants to hand to people. The only risk is the build dependency, isolated behind the `mp3` feature.
- Stems, loudness normalisation and cover art: **phase 1.5**.
