# Where does this go?

A plain-English map of ChipWave Studio for anyone adding a feature. ChipWave
is one Rust program, but it is cut into four parts with one job each. Keep
each change inside the part it belongs to and most features "just link up".

| Part | Its one job | Where it lives |
|---|---|---|
| **The window** | draws panels, turns clicks and keys into edits | `src/app.rs`, `src/app/` |
| **The song** | what a song *is*: data, edits, undo, the file | `src/model.rs`, `src/timeline.rs`, `src/automation/`, `src/modulation.rs`, `src/fileio.rs`, `src/app/undo.rs` |
| **The sound** | turns the song into samples before the deadline | `src/engine.rs`, `src/engine/`, `src/synth.rs`, `src/fx.rs`, `src/gen*.rs`, `src/audio.rs` |
| **Things described as data** | instruments, effects, presets, knobs, settings | `src/fx.rs` (`params()`), `src/modulation.rs` (`PATCH_PARAMS`), `src/gen.rs` (`gen_knobs`), `src/app/params.rs`, `src/instfile.rs`, `src/presets.rs`, `src/settings.rs` |

The window and the sound only meet through the song: the window edits
`audio::Shared { song, engine }` under its lock (`App::with`), and the
engine reads the same song when it renders. The window never runs DSP
itself, and the engine never reads files or talks to egui.

## The five rules

1. **Something the user does** becomes an edit of the song (an action).
2. **Something that changes the sound** is read by the engine from the song
   (or sent as a live event), never computed in the window.
3. **Something with knobs and a face** is described by a table, not
   hand-wired into each feature.
4. **Something that talks to hardware** (MIDI) turns messages into the same
   edits the window makes.
5. **Something that must be remembered** is a field in the song file (or
   the settings file), with a version so older files still load.

The sections below say exactly which files each kind of change touches.

---

## 1. A new knob

A *knob* is any continuous value that can be automated, MIDI-learned or
modulated. Every knob has a stable **id** (text, saved in songs and settings)
and a `automation::Target` (the same knob as a small enum, used at run time).

- The id ⇄ target mapping is `automation::Target::parse` / `Target::id` /
  `Target::parse_in` in `src/automation/mod.rs`. **Never rename an existing
  id**: old songs, lanes, modulation routes and MIDI learn maps store it.
  Examples: `instrument.1.cutoff`, `instrument.2.patch.amp-a`,
  `instrument.3.gen.4`, `instrument.1.fx.2.p0`, `mixer.ch.1.volume`,
  `mixer.return_a.time`, `mod.1.amount`, `tempo`.
- Where the knob is described depends on what it belongs to:
  - a synth patch value (`synth::Patch`): add a row to
    `modulation::PATCH_PARAMS` (`src/modulation.rs`) **and** the knob in the
    instrument panel (`src/app/params.rs`, same key and range). The panel
    gives it `Target::Patch` automatically when the key matches.
  - a generator knob: the engine's `MacroDef` table (`src/gen.rs`,
    `src/gen2.rs`, `src/gen_wave.rs`, `src/poly/`, `src/strings/`,
    `src/chipfx.rs`); `gen::gen_knobs` lists it.
  - an effect parameter: the device's `ParamInfo` table in `src/fx.rs`
    (`params()`).
  - a mixer value: `src/model.rs` (`Mixer`) plus a `Target` variant.
- **The knob list** (`src/knobs.rs`) builds every knob of a song from those
  tables, gives each one a name and range, and writes a value into the song.
  MIDI learn, controller maps, automation lane names, modulation names and generator knob
  names all ask it. Its test (`knobs::tests`) fails if two knobs share an
  id or an id does not read back to its knob, and
  `params::tests::panel_knobs_agree_with_the_knob_list` fails if the
  instrument panel and the knob list disagree about a range — so a knob
  added to one table but not the other is caught.
- Ranges: a knob's value is stored in real units (Hz, seconds) or as 0..1
  (effects, generators); automation and modulation always work in 0..1 and
  convert with `Target::to_real` / `Target::to_norm`.

## 2. A new effect

1. `src/fx.rs`: add the `FxKind` variant, its `name()`, blurb, colour, the
   `ParamInfo` table and its entry in `params()`, the text key in the
   `FxKind` parser, and the DSP state (a `Dsp` variant, its constructor arm and its processing arm).
   Bigger DSP goes in its own file (see `src/fxmod.rs` for Phaser and
   FreqShift).
2. Its knobs are automatable, learnable and modulatable at once through
   `Target::Fx` — no other list to edit.
3. A device kind an older reader cannot parse needs a reader-version bump
   (section 5).
4. Docs: `docs/FX.md`.

## 3. A new instrument kind

- Synth-like voices: `InstrumentKind` in `src/model.rs`.
- Generators (most new sounds): a `GenEngine` variant in `src/gen.rs` with a
  `MacroDef` table for its knobs, its `gen_knobs` / `gen_knob_def` /
  `gen_knob_norm` / `set_gen_knob_norm` arms, its voice in the engine.
- The panel: a builder function in `src/app/params.rs` (cards and knobs from
  the same table), its tag in `params::type_tag`.
- Saving: instrument files are `src/instfile.rs` (`.cwi`); presets are
  `src/presets.rs`. A new engine in a song is a reader-version bump.
- Tests: add it to `params::tests::all_types()` so the panel and knob-list
  tests cover it.

## 4. Something from hardware (MIDI)

- The MIDI thread is `src/midi.rs`: it parses bytes, sends notes straight to
  the engine (lock-free `rtrb` queue) and everything else to the window
  (`UiEv`). It never touches the song.
- In the window, `src/app/midiview.rs` turns a control change into a knob
  edit: MIDI learn maps a CC number to a knob id
  (`MidiSettings::learn`, saved in `settings.toml`), then
  `knobs::set_norm` writes the value — the same write a dragged knob makes.
- Controller maps (one small TOML file per controller) save and load those
  learn lists; see `docs/MIDI.md`. They contain only knob ids, so they are
  data, not code — no scripts.

## 5. Something that must be saved

- Songs are a zip (`.cwt`) written and read in `src/fileio.rs`: a
  `manifest.toml` with `min_reader_version`, the song TOML, the timeline
  TOML, samples.
- **Add a field with `#[serde(default)]`** so older files load without it.
  Prefer leaving the field out when it has its default, so songs that don't
  use the feature stay byte-identical.
- **Bump the reader version only when an older ChipWave would play the song
  wrongly or fail to parse it** (a new enum variant such as a new
  `GenEngine` or `FxKind`, a new clip kind). Add a `needs_reader_N(song)`
  function, add it to the `min_reader` chain in `src/fileio.rs` (where the song is written), and raise `READER_VERSION`. Songs that
  don't use the feature keep writing their old version.
- Old songs must render bit-identically: the fingerprint test
  `render_hashes` (`src/render_hashes.rs`) renders every demo and fixture
  (`tests/fixtures/`); its hashes must not change.
- App preferences (not songs) live in `src/settings.rs` (`settings.toml`);
  keyboard shortcuts in `src/keymap.rs` (`keymap.toml`).

## 6. Something the user does (an edit)

- Edits run in the window: `App::with(|s| ...)` changes `s.song` under the
  shared lock.
- Undo is a snapshot history (`src/app/undo.rs`): after each frame the live
  song is compared with the last step and a new step is made if it changed.
  So **every edit is undoable without extra code**. Call `self.snapshot()`
  before a discrete edit so it becomes its own step.

## 7. The audio thread rules

The engine renders on its own thread (`src/audio.rs`), and the sound card
callback only copies samples out of a ring buffer. Inside
`Engine::render` and everything it calls:

- **No allocation.** Buffers are made before playback or on first use and
  reused. `engine::tests::render_is_allocation_free_after_warmup` counts
  allocations and fails on any.
- **No new locks, no waiting.** The only lock is the one `audio.rs` already
  takes around a render; never lock anything else, never block, never sleep.
- **No logging, printing, file or network access.** Report through atomics
  (`audio::Stats`) or a lock-free queue and let the window show it.
- **No panics on bad data** — but if one happens, `audio.rs` catches it and
  goes quiet instead of taking the window down.
- Live input (MIDI notes) arrives through `rtrb` queues; a full queue drops
  events rather than block.

## 8. What we deliberately don't do

- No separate engine program: one process keeps install and debugging
  simple; plugin files are already checked in a separate process before
  they load (`src/plugin/scan.rs`), and an engine panic is caught
  (`src/audio.rs`).
- No scripted controllers: controller maps are plain data.
- No split into many crates: modules give the same separation without the
  build overhead.
