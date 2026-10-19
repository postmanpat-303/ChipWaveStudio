# The Browser (Unreleased)

The Browser is a long panel docked on the **right** of the window. It lists everything ChipWave can load, with search, tags and filters, and it is the quickest way to get a sound into a song: **double-click** an item, or **drag** it onto a track, an instrument slot or the FX chain.

![The Browser next to the Timeline (dark theme)](screenshots/dark-browser.png)

## Showing it

| Do | How |
|---|---|
| Show / hide | **Ctrl+B**, the **Browser** button in the toolbar's VIEW group, or **×** in the panel's header |
| Width | drag the panel's left edge (260–720 pt) |

Whether it is open and how wide it is are saved in `settings.toml` (`browser_open`, `browser_w`; see [SETTINGS.md](SETTINGS.md)). It is open by default and works in every theme.

## Layout

From top to bottom (this is mockup B, `docs/mockups/src/browser_b.html`):

1. **Header:** the BROWSER chip, the tab and how many items it has, and three buttons: **♥** (favourites only), **rescan** (library and locations, in the background) and **•••** (Add folder…, Rescan locations, Find plugins…, Auto-play on select, remove a location).
2. **Category tabs** (4 per row), each in its format's colour:

   | Tab | Shows |
   |---|---|
   | **Instruments** | blank Synth and Voice, every generator preset (Drum Synth, kits, Chip FX, Analog, FM-4, Noise, Wave, Grid), your `.cwi` instrument library, `.cwi` files in your locations, and the open song's instruments (Location *This song*) |
   | **Effects** | ChipWave's built-in effects (EQ, Delay, Reverb, Compressor, Filter …, each with a one-line description), the Grid FX patches and every effect plugin from the scan. **Adds to: …** at the top says which track / instrument a double-click adds to; drag into the lower panel's **Devices** row to insert between two devices ([FX.md › Devices](FX.md#devices-tab-unreleased)) |
   | **Plugins** | CLAP and VST3 plugins from the plugin scan, instruments and effects (the subtitle says which). **Find plugins…** and **Rescan plugins** are on this tab |
   | **Samples** | WAV, FLAC, MP3 and Ogg files in your locations |
   | **Songs** | the demo songs, `.cwt` / `.cwt.toml` songs in your locations, and MOD / MED files (listed; import is planned) |
   | **Presets** | the factory presets and built-in patches ([STARTER_SOUNDS.md](STARTER_SOUNDS.md), Voice presets) |
   | **Grid** | the Grid starter patches: Grid instruments and Grid FX ([GRID.md](GRID.md)) |
   | **Wavetables** | the Wave generator's 10 built-in tables and wavetable WAVs in your locations (files in a `wavetable` / `tables` folder, or with a `clm ` chunk) |
   | **MIDI** | `.mid` files in your locations (listed; MIDI file import is planned) |

3. **Search:** every word must match the name, category, creator, details or tags (`bass chip`).
4. **Tag chips:** Bass, Lead, Pad, Keys, Drums, FX, Chip, Analog, FM, Voice, Warm, Bright, Plucky, Evolving, Showcase. Only the tags the tab has are shown; several chips narrow the list (all must match). Tags come from the names, categories and kinds.
5. **Category, Creator and Location columns** with counts. Click a value to filter, **All** to clear. Location is *All*, *Library* (built in, your library, plugins), *This song*, *Favourites* and each folder you added.
6. **Results**, sortable by **Name**, **Type** or **Rating**. Each row has a coloured stripe and an outlined **text tag** for its format (CWI, CLAP, VST3, WAV …), the name, a second line (kind, category, size…), its **stars** and a **heart**.
7. **Preview bar:** ▶ / ■, "Preview on C-3 · plays through track NN …" and the **Auto** (auto-play) toggle.
8. **Colour legend:** CWI instrument, VST3 plugin, CLAP plugin, WAV / FLAC / MP3 / OGG sample, CWT song, MOD/MED*, WT wavetable, GRID patch, MIDI* (* = import planned).

## Showcase plugins

The Plugins tab enriches a small built-in catalogue of well-known open-source instruments (and Surge XT Effects) once they appear in the plugin scan. Matching is by CLAP/VST3 plugin id when known, otherwise by display name. Matched plugins get:

- a **category** such as Hybrid, FM, Analog, Pad, Bass, Drums, Sampler or Chip (instead of plain "Instrument")
- a credited **creator**
- the **Showcase** tag chip (plus related tags such as FM / Analog / Drums)

Current catalogue (not bundled with ChipWave; install them yourself or with **Get showcase instruments…**, below):

| Plugin | Creator | Licence | Formats | Category |
|---|---|---|---|---|
| Surge XT | Surge Synth Team | GPL-3.0 | CLAP+VST3 | Hybrid |
| Dexed | Digital Suburban | GPL-3.0 | CLAP+VST3 | FM |
| OB-Xf | Surge Synth Team | GPL-3.0 | CLAP+VST3 | Analog |
| Odin 2 | The Wave Warden | GPL-3.0 | CLAP+VST3 | Pad |
| Monique | Thomas Arndt / Surge Synth Team | GPL-3.0 / MIT | CLAP+VST3 | Bass |
| Geonkick | Quamplex | GPL-3.0+ | VST3 | Drums |
| sfizz | SFZTools | BSD-2 (engine) | VST3 | Sampler |
| ADLplug | Jean Pierre Cimalando | BSL-1.0 | VST3 | Chip |
| AyMidi | Bernat Arlandis (berarma) | GPL-2.0 | CLAP+VST3 (source only) | Chip (AY-3-8910 / YM2149) |
| YMulator Synth | Hiroaki (hiroaki0923) | GPL-3.0 | VST3 | Chip (YM2151 / OPM FM) |
| Mukkemann RetroTrax | Mukkemann | GPL-3.0 | CLAP+VST3 | Chip (SID 6581; tracker-style instrument) |

### Get showcase instruments…

Browser › **Plugins** tab › **Get showcase instruments…** (also in the Browser's ••• menu) opens a dialog listing the catalogue above with creator, category, licence, download size and a link to each project's source code. Tick what you want and press **Download & install**:

1. ChipWave downloads each instrument from its **creator's own GitHub release** (URLs, versions, sizes and SHA-256 sums are pinned in `src/showcase/mod.rs`; nothing else is fetched).
2. The **SHA-256** of every download is checked; a mismatch (or a missing file) stops that item with a clear message and an **Open download page** button. Nothing is installed from a bad download.
3. Only the `.vst3` / `.clap` bundles are extracted (plus the plugin's own factory-data folder where it needs one: Surge XT's patches / wavetables, OB-Xf's patches), into the **per-user** plugin folders — no admin rights:
   - Windows: `%LOCALAPPDATA%\Programs\Common\VST3` and `…\CLAP` (data: `%LOCALAPPDATA%\Surge XT`, `%LOCALAPPDATA%\Surge Synth Team\OB-Xf`)
   - macOS: `~/Library/Audio/Plug-Ins/VST3` and `CLAP`
   - Linux: `~/.vst3` and `~/.clap` (data under `$XDG_DATA_HOME` or `~/.local/share`)
4. The plugin's licence (`<Name>-LICENSE.txt`) and a source note (`<Name>-SOURCE.txt`: version, source-code link, download URL and SHA-256) are written next to it.
5. The folders are added to ChipWave's plugin folders, the plugin scan runs, and the Browser jumps to Plugins filtered to **Showcase**.

Each item has its own progress bar; **Cancel** stops cleanly (partly extracted files are removed). An instrument that is already installed at the same version is skipped (`showcase-installed.json` in the ChipWave settings folder records what was installed). Standalone apps, VST2 and LV2 builds inside the archives are never copied.

**Installer-only releases are never run.** When a creator ships only an `.exe` / `.pkg` / `.dmg` installer, or no build at all, the row shows **Open download page** instead of a checkbox.

| Instrument | Windows | macOS | Linux |
|---|---|---|---|
| Surge XT 1.3.4 | auto (≈302 MB incl. factory data; unticked by default) | download page | download page |
| Dexed 1.0.1 | auto | auto | auto |
| OB-Xf 1.0.3 | auto (+ patches) | download page | auto (+ patches) |
| Odin 2 2.4.1 | download page (installer only) | download page | auto |
| Monique (nightly 2024-08-07) | auto | download page | auto |
| Geonkick 3.9.0 basic | download page (installer only) | download page (no build) | auto |
| sfizz 1.2.3 | download page (installer only) | download page | download page |
| ADLplug 1.0.0 | auto (.7z) | auto (.7z) | download page (.deb only) |
| AyMidi | download page (no builds published) | download page | download page |
| YMulator Synth 0.1.3 | auto | auto | auto |
| Mukkemann RetroTrax 0.92.0 | auto | auto | auto |

Developer / test flag: `chipwave --install-showcase <id,id|all> [--root DIR]` runs the same installer without the window (ids: `surge-xt, dexed, ob-xf, odin2, monique, geonkick, sfizz, adlplug, aymidi, ymulator, retrotrax`).

Libraries: `ureq` (HTTPS), `zip`, `tar` + `flate2` (.tar.gz), `sevenz-rust2` (.7z, Apache-2.0), `sha2` — all GPL-3-compatible, credited in Help › About.

The mapping lives in `src/browser/catalogue.rs` and is generic: any scanned plugin whose id or name matches is enriched. Binaries are **not** in the git repo.

## Plugin thumbnails

On the **Plugins** tab each row can show a small screenshot of the plugin's own editor:

1. When a plugin first appears, ChipWave draws a tidy **card** (category colour, name, creator, format tag) so the list is never empty.
2. In the background a crash-guarded child (`chipwave --capture-thumb …`, one plugin at a time, killed after 25 s) opens the editor, briefly, in the top-left corner of the screen, and captures it: `PrintWindow` of the editor window first, then a copy of its area of the screen for OpenGL editors that `PrintWindow` can't see. A blank capture counts as failed. The image is downscaled to 256 px on its long side and cached as a PNG (plus a small `.json` saying captured / card / failed) in `<config>/plugin-thumbs/`, keyed by format + plugin id + version, so an update captures again.
3. Hover a row for a larger preview; right-click › **Refresh thumbnail** forces a new capture. **Rescan plugins** does not wipe the cache; only Refresh does.
4. ••• › **Plugin thumbnails** turns the feature off (list only, as before). Setting: `browser_thumbs` in `settings.toml` (default on).

Plugins with no editor, a failed or timed-out capture, or a non-Windows host keep the generated card (and are not retried until **Refresh thumbnail**). The editor window flashes up for about 3 s on the first capture. Showcase catalogue entries do not ship credited screenshots (licence-clear capture at install time is enough). Capture is one plugin at a time so the UI stays responsive.

## Drum Patterns

The **Drum Patterns** tab (grid icon) holds 67 ready-made grooves in 14 style folders: Psytrance / Dark psy (6), Techno (5), House (5), Trance (4), DnB / Jungle (5), Breakbeat (4), Hip-hop / Lo-fi (6), Trap (4), Disco / Funk (4), Synthwave / 80s (4), Reggaeton / Dembow (4), Amiga / Chip (4), Afro / Latin (5) and Fills (7). Each pattern has a name, a length in bars (1, 2 or 4), a suggested BPM range and a paired kit.

- **Rows:** a step-grid icon that turns into **Play** on hover, the name, a **step thumbnail** of the first bar (hats on top, snares and claps in the middle, the kick at the bottom), the heart, and a **PATTERN** tag. The pattern playing has an accent bar, a **Stop** icon and a progress line under the row.
- **Navigation:** click a style folder to open it, **Up** or the breadcrumb (`Drum Patterns › Techno`) to go back. Searching looks through every style at once (name, style, kit, BPM). **Favourites** shows only hearted patterns.
- **Kits (13):** Psy, 808, 909, Techno, Trap, Lo-fi, Chip, Acoustic-ish, Disco, Latin, DnB, Synthwave and Dembow. Every kit is a ChipWave **Drum Synth** in kit mode (the note picks the drum: C kick, D snare, D# clap, F# closed hat, A# open hat, toms and percs in between), so no samples are shipped and every kit is ours to ship under the GPL. The drum recipes follow the analog-drum models credited in About › Credits (sc808, Plaits / DaisySP).

### Preview (synced to the song)

Click the Play icon on a row, select a row while **Auto** is on, or press Space.

- **While the song plays**, the preview waits for the **next bar** and then plays at the song's tempo, locked to the song position. A jump or loop in the song re-aligns it.
- **While stopped**, it loops at the song's tempo.
- Click it again, press **Esc**, or preview anything else to stop. The bar at the bottom shows what is playing and whether it is waiting for the bar. It also has **Play / Stop** and a **Volume** slider (saved as `pattern_vol`). **Auto** previews each row as you select it.
- The song is never changed by a preview. The preview is mixed after the song, so it doesn't go through track effects.

### Placing a pattern

- **Drag a row below the last track** of the Timeline: a new drum track named after the kit is made, the kit is added as an instrument, and the pattern becomes a note clip starting on the bar under the mouse.
- **Drag onto an existing track:** the clip goes there, on the bar. If that track already plays a drum kit, the notes play on it. Otherwise the pattern's kit is used (an instrument of the same name in the song is reused).
- **Double-click / Load** puts it on a new drum track at the play marker.
- The notes are stored in ticks, so the clip follows the song's tempo and any tempo changes. A tracker-mode song asks to switch to timeline mode first, as other clip edits do. One undo step removes the track, clip and instrument together.
- The clip is a normal note clip: edit it in the piano roll, and **Ctrl/Cmd+D** duplicates it after itself (as for any clip).

## Files and drives

The **Files** tab is a small file browser:

- **Quick locations:** Home, Music, Desktop, Downloads (OneDrive folders are used when Windows moved them there).
- **Drives** opens a list of every drive: local, USB / removable, and **mapped network drives** (tagged NET, with their `\\server\share` path). Drives are read in the background with a 2.5 s timeout, so a sleeping NAS never freezes the app; one that doesn't answer shows *not responding*, and a disconnected mapping shows *not connected*.
- **Back / Forward / Up**, a clickable **breadcrumb**, **Refresh**, **Sort** (name, type, size) and **Favourites** (heart a folder to list it there).
- **Typing a path:** click the breadcrumb to type a path and press Enter (Esc cancels). Normal paths, `~`, `Z:` and UNC paths such as `\\nas\music\loops` all work.
- Folders come first. Double-click a folder to open it and a song to load it. Samples, instruments and other files can be previewed and dragged like in the other tabs. **+ Locations** adds the current folder to the scanned Locations.

### Mapped network drives (Windows)

Mapped drives (such as Z:) belong to your normal, unelevated Windows sign-in. A program running **as administrator** can't see them. Up to 0.2.4.0 the setup's *Start ChipWave Studio* checkbox started ChipWave with the setup's administrator rights, so that copy saw no network drives in its folder dialogs. The setup now starts ChipWave as you, unelevated, through Explorer. ChipWave also lists drives itself (`GetLogicalDrives` / `GetDriveTypeW`, with `WNetGetConnectionW` for the UNC path). If it ever runs elevated, the Drives list warns about this and you can still type the UNC path. If drives are still missing, close ChipWave and start it from the Start menu.

## Favourites and ratings

Click the **heart** at the end of a row (or right-click › Add to favourites) and the **stars** (1–5; click the same number again to clear). Both are saved in `settings.toml` (`browser_favs`, `browser_ratings`) under the item's key: a file's path, or a name for built-in items. The heart in the header, the **Favourites · n** chip (first in the tag row, with the count for this tab) and the *Favourites* location show only favourites; **Sort: Rating** puts the best first.

## Locations (your folders)

**+ Add folder…** (in the Location column) or **••• › Add folder…** adds a folder. It is scanned **in the background** (the header shows a spinner and the file count; the UI never waits): samples, songs, `.cwi` files, wavetables, MIDI and module files, down to 12 folders deep, skipping hidden folders, symbolic links and plugin bundles, at most 50 000 files per folder. The result is cached in `browser-index.json` next to the settings, so the Browser shows the last scan at once on the next start and rescans quietly. Right-click a location for **Rescan** or **Remove this location** (the files stay where they are). The folder list is `browser_dirs` in `settings.toml`. Category is a file's folder; Creator is "User".

The instrument library folder ([INSTRUMENT_LIBRARY.md](INSTRUMENT_LIBRARY.md)) is read by the same background scan.

## Loading

**Double-click** or **Enter** (when the Browser has the keyboard) does the natural thing:

| Item | Double-click / Enter |
|---|---|
| Instrument, generator preset, `.cwi`, Grid instrument | a **new track** playing it, and the instrument editor opens |
| Preset (Presets tab) | **replaces the current instrument's sound**; its notes are kept |
| Plugin instrument | a **new track** with it loaded, and its **editor window** opens (Windows; elsewhere the parameter list) |
| Plugin effect, Grid FX | added to the **current instrument's FX chain** (max. 16 devices) and its **editor** opens |
| Sample | Timeline view: an **audio clip** on the current track at the play marker. Elsewhere: a new sample instrument |
| Wavetable | the current instrument is a Wave generator: oscillator A plays it. Otherwise a new Wave instrument with that table |
| Song, demo | opens it, with the usual "save changes?" prompt |
| MOD / MED, MIDI | a message: import is planned |

Files (`.cwi`, samples, wavetable WAVs) are read on a worker thread; the status line says "Loading…" until they arrive. Every load is one undo step.

**Right-click** a row for the other choices: *Add as a new instrument*, *Replace instrument NN's sound*, *Load onto track NN* (the current track), *New track with it*, *Audio clip on track NN at the play marker*, *Preview*, favourites, rating and *Copy path*.

## Drag and drop

Drag a row out of the Browser. A ghost shows the item and what letting go there would do ("Track 03 plays 808 Boom", "New track: 808 Boom", "can't drop here"), and the target lights up.

| Drop on | Instrument, preset, `.cwi`, plugin instrument | Plugin effect, Grid FX | Sample |
|---|---|---|---|
| **A timeline track header** (or its row) | the track **plays it**: the instrument is added and every note on that track switches to it (see below) | added to the FX chain of the instrument the track plays | over the lanes: an **audio clip** at the drop point (snapped); on the header: the track plays it as a sample instrument |
| **Below the last track** (the empty area under the headers or lanes) | a **new track**, named after it, playing it | – | a new track with it as a sample instrument |
| **An instrument slot** in the sidebar list | replaces that slot's sound (notes kept) | added to that slot's FX chain | replaces that slot's sound |
| **The FX view** | – | added to the current instrument's chain | – |
| **The Devices row** (lower panel) | replaces the track's instrument, **keeping its effects** | inserted where the marker shows (between two cards; on the instrument card = first) | replaces the track's instrument |
| **The pattern editor** (or any other view) | a new instrument slot, made current, ready to type with | added to the current instrument's chain | a new sample instrument |

The drop indicator on the timeline is an outlined header and a tinted row, or an **insertion line** under the last track with **"New track: <name>"**. A full song (32 tracks) says "All 32 tracks are used" instead.

### What "the track plays it" means

Tracks are mixer channels (one per pattern column). Assigning an instrument to a track rewrites the instrument number of **every note on that track**: in timeline mode, the notes of the track's block clips (the track's column of each block) and of its note clips; in tracker mode, the channel's column in every block. Effects and volumes stay. The track also remembers the instrument (`instr` in `timeline.toml`), so an empty track's header shows it and clicking the header makes it the current instrument. The whole drop (new slot, notes, new track) is **one undo step**.

## Preview

Selecting an instrument, preset, `.cwi`, sample or wavetable with **Auto** on plays it once on **C-3** (samples at their root note) **through a copy of the current track's mixer channel** (volume, pan, sends) plus the master and the return effects, so you hear it about as it will sound in the song. ▶ in the preview bar (or **Space** while the Browser has the keyboard) plays the selection; ■, **Esc** or a load stops it. The preview stops the song if it was playing, like the library's audition. Plugins are not previewed (they'd have to be loaded first), and songs, Grid FX and MIDI files don't preview.

## Keyboard

Click into the Browser and it has the keyboard: **↑ / ↓**, **Home / End** move the selection (and preview with Auto on), **Enter** loads, **Space** previews, **Esc** gives the keyboard back. Ctrl shortcuts work as everywhere else. **Ctrl+B** shows and hides the panel (remappable in Settings › Keys as "Show / hide the Browser").

## Find plugins

On the **first start** (and once for users of earlier versions) ChipWave asks how to find your VST3 and CLAP plugins:

1. **Scan automatically:** checks the standard folders first (Windows `C:\Program Files\Common Files\VST3` and `\CLAP`, `%LOCALAPPDATA%\Programs\Common\VST3` / `CLAP`; macOS `/Library/Audio/Plug-Ins/VST3|CLAP` and `~/Library/…`; Linux `~/.vst3`, `~/.clap`, `/usr/lib/vst3`, `/usr/lib/clap`, `/usr/local/lib/…`, plus `VST3_PATH` / `CLAP_PATH`). Tick **Also search the whole drive** for a deeper search for folders that hold `.vst3` / `.clap` files (every drive on Windows; home, `/usr`, `/opt` on Linux; `/Applications`, `/Library` and home on macOS).
2. **Choose folders myself:** a folder picker with an add / remove list.
3. **Skip for now.**

The search runs **in the background** with a live count and the folder it is in, and **Stop** ends it. It then lists every folder it found with its plugin count (untick any you don't want) and **Add N folders** puts them in the plugin folders (**Settings › Plugins**) **and** the Browser's locations, then runs the plugin scan. It is never shown again unless you ask: **Settings › Plugins › Find plugins…** and the **Plugins** tab's **Find plugins…** open it any time. The answer is saved as `plugin_setup_done`. Details on scanning: [PLUGINS.md](PLUGINS.md#finding-plugins).

## Tracks from the timeline headers

The Browser works together with the track headers ([TIMELINE.md › Tracks](TIMELINE.md#tracks-add-reorder-instruments)): **+ Add track** under the last header, right-click a header › **Add track above / below / with instrument…** (the last opens the Browser on Instruments; the next instrument you load there makes the track), and **drag a header up or down** to reorder.

## Old songs

The Browser only reads. Opening it changes nothing in a song (tested: an old song saves byte-identically after browsing every tab), and songs only change when you load something. `Track::instr` is written only for tracks that were given an instrument by a drop.

## Limits in this version

- MIDI files and MOD / MED modules are listed but can't be imported yet.
- Plugins aren't previewed, and plugin presets (`.vstpreset`, CLAP preset files) aren't listed.
- Plugin editor **thumbnails** capture on Windows (and Wine) only; other OSes show generated cards. VST3 editors themselves still open on Windows only; CLAP floating editors work where the plugin offers them.
- No waveform display or tempo-synced preview of samples; previews are one note, about 2.4 s.
- Ratings and favourites of files are keyed by path: moving a file forgets them.
- Dropping onto the mixer view or a specific FX slot position isn't supported (effects go to the end of the chain).
- Drum Patterns has no step editor yet, so there are no Euclid / Rotate / Random tools. Edit placed patterns in the piano roll.
- The pattern preview plays the pattern's own kit, not the instrument on a track, and isn't heard through track effects.
- The Files tab shows one folder at a time (no tree), and you can't rename, move or delete files there.
- Tag chips are inferred from names and categories; you can't add your own tags yet.

## Under the hood (for developers)

- `src/plugin/thumb.rs`: thumbnail cache, generated cards, capture child and queue; `src/app/browserview.rs` / `browserpanel.rs` show them and open editors on double-click.
- `src/browser/catalogue.rs`: Showcase plugin metadata (id/name → category, creator, tags).
- `src/showcase/mod.rs`: the Get showcase instruments manifest and installer; `src/app/showcase.rs`: its dialog.
- `src/browser.rs`: the item model (`Item`, `Tab`, `Fmt` with badge colours, `Loc`, `Src`), tags, the built-in / library / song / plugin item builders, the folder scanner (`scan_folder`, `classify`), the cached `Index` (`browser-index.json`), `filter` / `column_values`, and loading helpers (`instrument_of`, `sample_instrument`, `wave_instrument`, `grid_fx_device`, `preview_song`). Unit tests in `src/browser/tests.rs`.
- `src/app/browserview.rs`: state (`BrowserUi`), the background scan and load jobs, actions (`br_load`, `br_instrument` with `Act`, `br_drop` with `DropAt`), preview and keys. `src/app/browserpanel.rs`: painting and input, the drag ghost and drop indicators. `src/app/plugsetup.rs`: Find plugins. `src/chanops.rs`: channel remapping (reorder, insert, assign). UI tests: `src/app/browser_tests.rs`.
- `src/drumpat.rs` (+ `drumpat/library.rs`): the pattern model, step strings, kits; `src/engine/patprev.rs`: the synced preview voice; `src/app/drumpat.rs`: the Drum Patterns and Files tabs, placement (`TlOp::DrumPattern`); `src/drives.rs`: drive list, UNC, quick folders. Tests: `src/drumpat/tests.rs`, `src/app/drumpat_tests.rs`.
- Screenshots: `--screenshot out.png --view browser|browser-samples|browser-patterns|browser-patterns-styles|browser-patterns-drop|browser-files|plugin-setup --theme dark|amiga`.
