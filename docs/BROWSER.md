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
2. **Category tabs** (4 × 2), each in its format's colour:

   | Tab | Shows |
   |---|---|
   | **Instruments** | blank Synth and Voice, every generator preset (Drum Synth, kits, Chip FX, Analog, FM-4, Noise, Wave, Grid), your `.cwi` instrument library, `.cwi` files in your locations, and the open song's instruments (Location *This song*) |
   | **Plugins** | CLAP and VST3 plugins from the plugin scan, instruments and effects (the subtitle says which). **Find plugins…** and **Rescan plugins** are on this tab |
   | **Samples** | WAV, FLAC, MP3 and Ogg files in your locations |
   | **Songs** | the demo songs, `.cwt` / `.cwt.toml` songs in your locations, and MOD / MED files (listed; import is planned) |
   | **Presets** | the factory presets and built-in patches ([STARTER_SOUNDS.md](STARTER_SOUNDS.md), Voice presets) |
   | **Grid** | the Grid starter patches: Grid instruments and Grid FX ([GRID.md](GRID.md)) |
   | **Wavetables** | the Wave generator's 10 built-in tables and wavetable WAVs in your locations (files in a `wavetable` / `tables` folder, or with a `clm ` chunk) |
   | **MIDI** | `.mid` files in your locations (listed; MIDI file import is planned) |

3. **Search:** every word must match the name, category, creator, details or tags (`bass chip`).
4. **Tag chips:** Bass, Lead, Pad, Keys, Drums, FX, Chip, Analog, FM, Voice, Warm, Bright, Plucky, Evolving. Only the tags the tab has are shown; several chips narrow the list (all must match). Tags come from the names, categories and kinds.
5. **Category, Creator and Location columns** with counts. Click a value to filter, **All** to clear. Location is *All*, *Library* (built in, your library, plugins), *This song*, *Favourites* and each folder you added.
6. **Results**, sortable by **Name**, **Type** or **Rating**. Each row has a coloured stripe and a **badge** for its format, the name, a second line (kind, category, size…), its **stars** and a **heart**.
7. **Preview bar:** ▶ / ■, "Preview on C-3 · plays through track NN …" and the **Auto** (auto-play) toggle.
8. **Colour legend:** CWI instrument, VST3 plugin, CLAP plugin, WAV / FLAC / MP3 / OGG sample, CWT song, MOD/MED*, WT wavetable, GRID patch, MIDI* (* = import planned).

## Favourites and ratings

Click the **heart** at the end of a row (or right-click › Add to favourites) and the **stars** (1–5; click the same number again to clear). Both are saved in `settings.toml` (`browser_favs`, `browser_ratings`) under the item's key: a file's path, or a name for built-in items. **♥** in the header and the *Favourites* location show only favourites; **Sort: Rating** puts the best first.

## Locations (your folders)

**+ Add folder…** (in the Location column) or **••• › Add folder…** adds a folder. It is scanned **in the background** (the header shows a spinner and the file count; the UI never waits): samples, songs, `.cwi` files, wavetables, MIDI and module files, down to 12 folders deep, skipping hidden folders, symbolic links and plugin bundles, at most 50 000 files per folder. The result is cached in `browser-index.json` next to the settings, so the Browser shows the last scan at once on the next start and rescans quietly. Right-click a location for **Rescan** or **Remove this location** (the files stay where they are). The folder list is `browser_dirs` in `settings.toml`. Category is a file's folder; Creator is "User".

The instrument library folder ([INSTRUMENT_LIBRARY.md](INSTRUMENT_LIBRARY.md)) is read by the same background scan.

## Loading

**Double-click** or **Enter** (when the Browser has the keyboard) does the natural thing:

| Item | Double-click / Enter |
|---|---|
| Instrument, generator preset, `.cwi`, Grid instrument | a **new instrument slot** (it becomes the current instrument) |
| Preset (Presets tab) | **replaces the current instrument's sound**; its notes are kept |
| Plugin instrument | a new plugin instrument, loaded at once |
| Plugin effect, Grid FX | added to the **current instrument's FX chain** (max. 16 devices) |
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
- No waveform display or tempo-synced preview of samples; previews are one note, about 2.4 s.
- Ratings and favourites of files are keyed by path: moving a file forgets them.
- Dropping onto the mixer view or a specific FX slot position isn't supported (effects go to the end of the chain).
- Tag chips are inferred from names and categories; you can't add your own tags yet.

## Under the hood (for developers)

- `src/browser.rs`: the item model (`Item`, `Tab`, `Fmt` with badge colours, `Loc`, `Src`), tags, the built-in / library / song / plugin item builders, the folder scanner (`scan_folder`, `classify`), the cached `Index` (`browser-index.json`), `filter` / `column_values`, and loading helpers (`instrument_of`, `sample_instrument`, `wave_instrument`, `grid_fx_device`, `preview_song`). Unit tests in `src/browser/tests.rs`.
- `src/app/browserview.rs`: state (`BrowserUi`), the background scan and load jobs, actions (`br_load`, `br_instrument` with `Act`, `br_drop` with `DropAt`), preview and keys. `src/app/browserpanel.rs`: painting and input, the drag ghost and drop indicators. `src/app/plugsetup.rs`: Find plugins. `src/chanops.rs`: channel remapping (reorder, insert, assign). UI tests: `src/app/browser_tests.rs`.
- Screenshots: `--screenshot out.png --view browser|browser-samples|plugin-setup --theme dark|amiga`.
