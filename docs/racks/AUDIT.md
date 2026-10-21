# Instruments Rack & Nested Device Chains: audit (Stage 0)

What ChipWave Studio already has for this brief, checked in the code on `main` a02a181 (v0.2.6.0). Nothing has been built yet; this is the starting point for `PLAN.md`.

**Key:** ✅ works today · 🔧 exists, needs extending · 🆕 new work · ⛔ not feasible without big engine work (or not in this round)

## 1. What exists today (with file paths)

### Instrument selection and the "Instruments Rack"
- There is no panel called "Instruments Rack" in the code. The nearest things are the **Instruments list** in the left sidebar (`src/app/instruments.rs`: one row per instrument slot, click selects, right-click menu, Save to library) and the **track header** on the Timeline (`src/app/timeline.rs`, `HeadOp::Select`). In this audit "selecting an instrument" means either of those.
- Both already open the instrument's editor **below the Timeline** when **Settings › Follow the Timeline selection** is on (the default, `Settings::follow_select`, `show_instrument_editor()`). The Timeline is never replaced (unified workflow, v0.2.6.0).
- One rule decides "the track's instrument" (`track_slot`, D6 of the unified plan).

### The lower panel (`src/app/instpanel.rs`, `src/settings.rs`)
- Timeline on top, lower panel underneath, with a **draggable splitter** (drag to resize, drag to the bottom to hide, double-click for automatic height). F11 hides or shows it.
- The height is **remembered** across restarts (`Settings::lower_h`). So is the chosen tab (`lower_inst`, `lower_dev`, `lower_chosen`) and which cards are folded (`cards_closed`).
- The Timeline keeps at least 3 track rows visible at 1366×768 (laptop rule).
- Tabs in the view bar (`src/app/viewbar.rs`): PIANO · **DEVICES** (Ctrl+7) · **INST** (Ctrl+I) · AUTO.

### The Devices tab (`src/app/devices.rs`, `devcards.rs`, `fxview.rs`)
- Shows the selected track's chain **left to right**: an **instrument card** (the real instrument editor, foldable), then one **card per effect** with its key controls and a graph (EQ curve, compressor curve, filter response, delay taps, reverb decay), and plugin effects with a thumbnail and an **Editor** button.
- Each card has **power (bypass)**, **fold**, **remove**, a **"…"** for every parameter, presets, Duplicate, Move left/right, and a **title you drag to reorder**.
- The **+** card opens Browser › Effects. Effects dragged from the Browser drop **between** cards with an insertion marker.
- Every edit goes through one function, `apply_fx_ops(FxOp)`, which snapshots for undo and keeps automation lanes on the device when it moves.

### Device / FX model (`src/fx.rs`, `src/model.rs`)
- `Instrument { name, volume, transpose, fx: Vec<fx::Device>, kind }`. The insert chain belongs to the **instrument**. It is a **flat list**: no containers, no branches.
- `Device { kind, enabled, collapsed, params (0..1), plugin, uid, grid }`. 14 built-in kinds (EQ, Delay, Reverb, Drive, Chorus/Flanger, Filter, Bitcrusher, Compressor, Metal resonator, Digital distortion, Sub bass, Phaser, Frequency shifter, Grid FX), plus `Plugin` (a CLAP/VST3 effect).
- `enabled = false` is a real bypass with a click-free crossfade (`on_gain`). It is tested (`bypass_is_dry`).
- Most devices have a **Mix** knob (dry/wet inside the device).
- **Grid FX** is a modular patch inside one device (`src/grid.rs`: Mix, VCA, Filter, Delay, Shaper and more). It can already split and recombine audio **inside its own patch**, but it cannot hold other ChipWave devices.

### Audio graph and routing (`src/engine.rs`, `src/engine/notes.rs`, `src/plugin/fxrack.rs`)
- Each track is one channel. **Everything sounding on it** is summed first: the tracker voice and its release tail, up to 8 note-clip voices, plugin instruments and audio clips. That sum then goes through **one insert chain**: the FX of the instrument that played last on that channel (`ChainState::owner`, the ccd5969 fix). After that come fader, pan, ducking, two sends (Return A = delay, Return B = reverb) and the master.
- **Layering:** ❌ not possible today. Each note plays exactly **one** instrument (`NoteSpan::instr` or the tracker cell's instrument). You can only fake a layer with two tracks.
- **Per-layer FX:** ❌. A channel has exactly one `ChainState`.
- **Parallel splits:** ❌ for device chains. What does exist: the per-device Mix knob, the two mixer sends, and a Grid FX patch's own internal cables.
- **Per-device bypass:** ✅ (built-in and plugin effects).
- The chains run **sample by sample** and never allocate on the audio thread. A structure change is rebuilt on the engine thread (`ChainState::sync`, compared by a signature). Plugin effects run in `FxRack`, keyed by (channel, owner instrument, device `uid`), with **plugin delay compensation**.

### Plugin integration (`src/plugin/`)
- CLAP and VST3, as **instruments** (`InstrumentKind::Plugin`) and as **effects** (`FxKind::Plugin`). Out-of-process scanning with a cache (`scan.rs`), editor windows (`gui.rs`).
- `PluginPatch { format, id, name, vendor, path, state (base64), params, macros }`. On another computer the plugin is found **by id** through the scan cache.
- **Missing plugins:** ✅ handled safely. A missing instrument stays a **silent placeholder**. A missing effect is **bypassed with its settings kept**. Both are listed in the "Some parts of this song could not be loaded" window (`fileio.rs` › `missing_warnings`, `fxrack.rs` › `missing_warnings`). Fix: Settings › Plugins › **Find plugins…** (`plugsetup.rs`), then reopen.

### Persistence and versioning
- **Song `.cwt`** (`src/fileio.rs`): a ZIP of TOML files (manifest, song, blocks, instruments, mixer, automation, timeline). `min_reader_version` is only raised for songs that **use** a newer feature (today up to 6: launcher clips). Older songs keep writing the old version byte for byte. Unknown fields are ignored and missing ones take defaults.
- **Instrument `.cwi`** (`src/instfile.rs`, docs/INSTRUMENT_LIBRARY.md): a ZIP with `instrument.toml` (`format = "chipwave-instrument"`, `format_version = 1`): the whole instrument **including its FX chain** and plugin state, plus `sample.wav` for samples. Writes go to a temp file then a rename, so a crash never leaves half a file. A newer `format_version` is refused with a message.
- **User library folder:** `<config>/Instruments`: Windows `%APPDATA%\ChipWave\Instruments`, macOS `~/Library/Application Support/ChipWave/Instruments`, Linux `~/.config/chipwave/Instruments` (`settings::config_dir`). It can be changed in Settings. One level of sub-folders = categories.
- Deps already in `Cargo.toml`: `serde`, `toml`, `serde_json`, `zip`, `sha2`. Nothing new is needed.

### Browser (`src/browser.rs`, `src/app/browserview.rs`, `browserpanel.rs`)
- 11 tabs (Instruments, Effects, Plugins, Samples, Songs, Presets, Grid, Wavetables, MIDI, Drum Patterns, Files), with search, tag chips, a category column, location, favourites and a cached folder scan.
- **"ChipWave Instruments"** is the first category in the Instruments tab (`TOOLS_CAT`). It uses the same code as the track pie's Add.
- Library `.cwi` files are listed as `Loc::Library` items, with the sub-folder as category.
- The **Library window** auditions a sound with **›** (a short phrase rendered off-line, the song untouched). **+** adds it, and **Replace sound** keeps the notes.
- There is **no rename, duplicate or delete** for library files inside the app today.

### Undo (`src/app/undo.rs`)
- Song-wide **snapshot** undo: every history step is a logical copy of the song, with shared parts held in `Arc`. Each change is found by comparing, so **any** change to instruments or their FX chains is undoable without extra code. Knob drags count as one step.
- Library files on disk are **outside** undo (saving or deleting a file is not a song edit).

## 2. Brief item by item

| # | Brief item | Status | Why / what it takes |
|---|---|---|---|
| 1.1 | Timeline unchanged and visible on top | ✅ | Already the layout. No timeline code changes are planned. |
| 1.2 | Selecting an instrument opens a lower panel, no navigating away | ✅ / 🔧 | Opens today, but on the INST editor. It needs to open the new panel when chosen (D2). |
| 1.3 | Resize, collapse, expand, remember the layout | ✅ / 🔧 | Splitter, F11 and the saved height already work. New: a fold strip for the tree column, and remembering which chains are open. |
| 2.1 | Panel called "Nested Device Chains", hierarchical | 🆕 | Today's Devices row is flat. New tree column (D1, D3). |
| 2.2 | Instrument (main source) | ✅ | The instrument card / editor is reused as is. |
| 2.3 | Instrument layers playing together | ⛔ today → 🆕 feasible | The engine plays one instrument per note. New engine path: a note on a rack also starts its layer instruments in the same track's voice pool. Bounded work, but it is real engine work (D4, D5). |
| 2.4 | Effect chains in order | ✅ | `Instrument::fx`, with reorder already working. |
| 2.5 | Nested chains inside containers | 🆕 | New container device (a "Split") whose branches are chains. Built on `ChainState`, which already runs a chain (D6). |
| 2.6 | Parallel processing, split and recombine | 🆕 | The same Split container: 2-4 branches, each with level, pan and mute, summed. No real parallel chains exist today. |
| 2.7 | Select a device to see its full editor | 🔧 | Each card already has its "…" for every parameter, and Grid FX has its own window. New: a detail area beside the tree. |
| 3.1 | Clear Add Device action | 🔧 | The + card opens Browser › Effects. New: an in-place **Add Device** menu per chain (built-ins, plugin effects, Split, Layer). |
| 3.2 | Drag to reorder within a chain | ✅ | Card titles drag today. 🔧 The tree rows get the same thing. |
| 3.3 | Move between compatible chains | 🆕 | No second chain exists today. New: drag between the main chain and the branches, or into a layer's chain. |
| 3.4 | Enable, bypass, remove, configure | ✅ | Power, remove and "…" already exist. Reused in the tree. |
| 3.5 | Expand / collapse chains, clear nesting | 🆕 | The tree column (indents, guide lines, ▸/▾ with text). |
| 3.6 | Text labels, not icons only | ✅ rule | Kept: every control gets a word or a tooltip plus a word. |
| 3.7 | Undo / redo for rack and device changes | ✅ | Snapshot undo covers every song change for free. New code just has to go through the song (it will). |
| 4.1 | Save Custom Rack (name, description, tags / sound type) | 🆕 | New dialog. Writing reuses `instfile.rs`'s ZIP and temp-file code. |
| 4.2 | Own browser category "My Racks" | 🆕 | A new category in the Instruments tab next to "ChipWave Instruments", fed by the library scan. |
| 4.3 | Preserve instruments, chains, order, nesting, params, layers, routing, plugin refs | 🆕 | They all live in `Instrument` (+ the new layer and Split fields), so one serde struct holds everything. |
| 4.4 | Browse, search, preview, rename, duplicate, update, delete | 🔧 / 🆕 | Browse, search and preview: reuse the Browser and the library audition. Rename, duplicate, update and delete: new (the library has none today). |
| 4.5 | Loading must not touch unrelated tracks | ✅ rule | Load = new instrument slots (or replace the selected track's sound) through the existing add/replace code, as one undo step. |
| 4.6 | Missing plugin reported and resolvable, never dropped | ✅ / 🔧 | Placeholder and bypass with state kept already exist for songs. 🔧 Show it in My Racks too (badge + "Find plugins…" + "Load anyway"). |
| 5.1 | Persistent user library, not per project | ✅ / 🔧 | The `<config>/Instruments` folder exists. New: a `Racks` sub-folder. |
| 5.2 | Versioned, documented format | 🔧 | `.cwi` already has `format` + `format_version`. New: `.cwr` documented in docs/racks/FORMAT.md. |
| 5.3 | Rack definition separate from UI state | 🔧 | Song data and UI state are already separate (`DevUi`, `Settings`). The rack file will hold sound only (fold state lives in Settings). |
| 5.4 | Validate before loading | 🔧 | `Device::fix()` clamps params today. New: `Rack::validate()` (lengths, depth, layer refs, versions) before anything is applied. |
| 5.5 | Safe with missing devices, versions, plugins | 🔧 | An unknown device kind in a newer file becomes a clearly labelled "Unknown device (bypassed)" that is kept, never deleted. |
| 5.6 | One shared rack-loading / device-management path | 🔧 | `apply_fx_ops` is the only device-edit path today. It is extended to address nested devices by path. One `racks::load` is used by the Browser, the Library window and drag-and-drop. |
| 7 | Old projects load and behave as before | ✅ rule | No new fields in old songs → same engine path. Checked with the render fingerprints (`/workspace/fp-base.txt`, 88 lines). |

## 3. Things that are NOT feasible this round (without big engine work)
- **Per-layer sends or mixer routing.** Layers sum inside their track, then share the track's fader and sends. Sending a layer to its own mixer channel would need a new bus system.
- **Plugin effects inside Split branches** (first version). `FxRack` addresses plugin instances as (channel, owner, uid) in one flat chain. Nesting them means teaching the plugin rack and delay compensation about branches. This is doable later, but it is the riskiest part, so built-in devices only at first (D7).
- **Automation and drag-to-modulate on devices inside a Split** (first version). Targets are `instrument.N.fx.D.pK`, i.e. by position in a flat list. Nested devices need path-based targets. They stay hand-editable; the automate/modulate handles are hidden for them so nothing looks like it works when it doesn't (D8).
- **Tracker pitch effects on layers.** A tracker channel is a single monophonic voice with tracker effects (slides, vibrato, arpeggio). Layers can take the note and its volume from Tracker blocks through their own voice pool, but copying every pitch effect onto every layer is deferred (D5).
- **Audio-rate sidechains or feedback between branches.** Not part of the brief, and not planned.
