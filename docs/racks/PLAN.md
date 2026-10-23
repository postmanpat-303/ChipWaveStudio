# Instruments Rack & Nested Device Chains: plan (for approval)

Based on `AUDIT.md`. **Nothing here is built yet.** Each numbered row is one small commit with its own tests. Every stage ends with the full gate (fmt, clippy on Linux and Windows, `cargo test`, check-ico, render fingerprints against `/workspace/fp-base.txt`), screenshots at 1366×768 and 1920×1080, and a test installer in GrokPrograms. Nothing is merged or released without your OK.

Mockups (drawn over real renders of the current build, 1366×768):
1. `01-chains-instrument-selected-1366x768.png`: the panel with a layer's instrument selected
2. `02-chains-nested-device-selected-1366x768.png`: a device inside a Split selected, being dragged to the other path
3. `03-add-device-menu-1366x768.png`: the Add Device menu
4. `04-save-custom-rack-1366x768.png`: the Save Custom Rack dialog
5. `05-my-racks-browser-1366x768.png`: My Racks in the Browser, with a missing-plugin rack

---

## The idea in plain words

- A **rack** is a new kind of instrument slot. It makes no sound of its own. It holds **layers**, and each layer is an ordinary instrument slot (WaveForge, Synth, Drum Kit, a plugin…). When a note plays on the rack, **every layer plays it**. Each layer runs through **its own effects**, the layers are added together, and the sum goes through the **rack's effects**. After that come the track's fader and sends, exactly as today.
- A **Split** is a new device that you put in any chain. It sends the sound down 2 to 4 **paths at once** (each path is its own little chain with a level and pan) and adds them back together. An empty path is the dry sound, which is how parallel compression or a parallel reverb is done.
- The **Nested Device Chains** panel is today's **Devices** tab grown up. Left: the rack as a tree (layers, their effects, the rack effects, Splits and their paths). Right: whatever you clicked, with its real controls.
- **Save Custom Rack** writes the rack slot, its layer slots and every setting into one `.cwr` file in your **Racks** folder. **My Racks** in the Browser lists them.

Why a rack is a slot with layers that are slots: every layer keeps everything instruments already have (full editor, presets, automation, drag-to-modulate, plugin hosting, undo) with no second copy of any of it. The engine changes stay small and only switch on when a song has a rack.

## How it maps onto the engine

| Piece | Today | Change |
|---|---|---|
| Rack instrument | – | `InstrumentKind::Rack(RackP { layers: Vec<Layer> })`, with `Layer { slot, on, level, pan, transpose, key_lo, key_hi }` and at most **4 layers**. Like a plugin instrument, its own voice is silent. |
| Layer voices | 8 note voices per track (`NOTE_VOICES`) | A **separate** layer voice pool per track (8 per layer), used only by rack notes. Tracks without a rack use exactly the old pool, so their voice stealing doesn't change. |
| Layer effects | one `ChainState` per track | Up to 4 extra layer chains per track. They are built on the engine thread when a rack first plays there; the audio thread still never allocates. Each layer's voices (and its plugin instrument's output) go into its own chain, then into the track sum. |
| Rack effects | the owner instrument's `fx` | Unchanged: the rack slot is the track's chain owner, so its `fx` is the rack chain. |
| Split device | – | `FxKind::Split`, with `Device::paths: Vec<Path { devices, on, level, pan }>`: 2–4 paths, at most one Split inside another. Its DSP state holds one `ChainState` per path, the same code that runs every chain today. |
| Where layers play | – | Note clips, launcher clips, MIDI and keyboard jamming, editor note preview, export, and Tracker blocks. In Tracker blocks the layers take the note and its volume, but **not** tracker pitch effects (slides, vibrato, arpeggio) in the first version. The panel says so. |
| Device edits | `apply_fx_ops(slot, FxOp)` with a flat index | The same function, with a **device path** (`[2]` = rack chain device 2, `[2, 1, 0]` = Split at 2 › path B › device 0). There is still one path for every edit, used by the tree, the cards, the Browser drops and the Add Device menu. |
| Undo | snapshot of the song | Nothing new needed. Layer slot numbers are remapped wherever instruments are moved or deleted (the same place notes are remapped today). |
| Song file | `min_reader_version` up to 6 | Songs with a rack or a Split write **7**, so an older ChipWave refuses them with its usual "please update" message instead of losing the rack. Every other song saves byte for byte as before. |

### Old songs stay bit-identical
- No rack and no Split → the engine takes exactly today's code paths. The new pools and chains are not even allocated.
- New fields are `#[serde(default, skip_serializing_if = …)]`, so files without them don't change.
- After every stage: the 88-line render fingerprints must match `/workspace/fp-base.txt` (the same gate as v0.2.6.0), plus a byte-compare save test on the demo songs.

## The rack file (`.cwr`)

A small ZIP, built like `.cwi` and `.cwt` with the same writer (temp file + rename, so a crash never leaves half a file):

| Entry | Content |
|---|---|
| `mimetype` | `application/x-chipwave-rack` (stored, first) |
| `rack.toml` | `format = "chipwave-rack"`, `format_version = 1`, `app = "ChipWave Studio <version>"`, then `[rack]`: `name`, `description`, `sound_type`, `tags`, `created`, `updated`, and `[[instruments]]`. Entry 0 is the rack itself, with its layers pointing at entries 1..n. Each entry has exactly the fields a song instrument has (sound, FX chain with Splits, plugin id + saved state). A `[[plugins]]` summary (format, id, name, vendor, used by) lets the Browser warn without opening every instrument. |
| `samples/NN.wav` | only for sample layers (32-bit float, as in `.cwi`) |

- **TOML**, like every other ChipWave file. It is readable in a text editor and needs no new dependency.
- The file holds **sound only**: nothing about which cards are folded or how wide the tree is (that lives in Settings).
- **Checked before anything is applied** (`racks::validate`): known format, version ≤ 1, 1–4 layers, every layer reference in range, no rack inside a rack, Split depth ≤ 2 with 2–4 paths, every parameter clamped (`Device::fix`), sample WAVs readable, plugin ids present.
- If something fails, nothing is loaded and you get one plain sentence, e.g. "Glass Keys.cwr was made by a newer ChipWave (rack format 2). Please update to load it." or "Reese Wall.cwr is damaged (rack.toml can't be read)."
- **Unknown device kinds** (from a newer build) load as a bypassed "Unknown device" with its data kept, never silently dropped.
- **Missing plugins** load as today: a missing plugin instrument is a silent placeholder, a missing effect is bypassed with its settings kept. My Racks shows the warning **before** you load (mockup 5), with **Find plugins…** and **Load anyway**.
- Documented in `docs/racks/FORMAT.md` (written in Stage 5).

**Where:** the `Racks` folder next to the instrument library, changeable in Settings › Files & Safety:
- Windows: `%APPDATA%\ChipWave\Racks`
- macOS: `~/Library/Application Support/ChipWave/Racks`
- Linux: `~/.config/chipwave/Racks`

---

## Stages (the brief's 8 steps)

One rule throughout: **no control appears before its sound works.** For example, Split is only offered in Add Device in the same stage its audio lands. Steps 3 and 4 of the brief are therefore done together for each device type.

### Stage 1: selecting opens the panel
| # | Commit | Test |
|---|---|---|
| 1.1 | Rename the DEVICES tab to **Nested Device Chains** (view bar, lower-panel switch, View menu, Ctrl+7 kept) | label tests; old settings with `lower_dev` still open it |
| 1.2 | Clicking a track header or an instrument in the Instruments list opens this panel (D2). A Settings choice keeps today's Instrument tab | `follow_select` tests for both choices; Timeline stays the view |
| 1.3 | Panel layout: the tree column (foldable, resizable) and the detail area. Tree width, fold state and open/closed rows are remembered in Settings | settings round-trip; 1366×768 keeps 3 timeline rows (existing `narrow_tests`) |

### Stage 2: the instrument and its existing controls
| # | Commit | Test |
|---|---|---|
| 2.1 | Tree rows for today's sound: instrument, then its effects in order (power, name, "bypassed") | tree built from a song with 0, 1 and 6 effects |
| 2.2 | Detail area: the instrument card (reused), **Open full editor (Ctrl+I)**, and for a device its card plus every parameter (the "…" list, open) | selecting rows switches the detail; params edit the song and undo in one step |
| 2.3 | The current card row stays available as **Cards** (D1) | existing `devices_tests` pass unchanged |

### Stage 3+4a: device editing on the flat chain (real audio already)
| # | Commit | Test |
|---|---|---|
| 3.1 | `apply_fx_ops` takes a device path (flat paths only so far) | every existing FX op test, unchanged results |
| 3.2 | **Add Device** menu (mockup 3): ChipWave effects, plugin effects, search, Enter / Esc, where it lands | inserts at the right index; render differs from the dry render |
| 3.3 | Tree: drag rows to reorder, Bypass, Remove, Duplicate, Ctrl-drop copies | reorder changes the rendered output; automation lanes follow the device (existing test extended) |

### Stage 3+4b: Split (parallel paths, nested chains)
| # | Commit | Test |
|---|---|---|
| 4.1 | `FxKind::Split` data + DSP (2–4 paths, level, pan, on; depth ≤ 2), engine-thread rebuild, no allocation in the callback | 1 empty path at 0 dB = dry (bit-exact); A+B = render(A) + render(B) within float tolerance; bypassed Split = dry; no-alloc test |
| 4.2 | Tree and detail for Split and paths (+ Add path, path level / pan / mute), Split in Add Device | UI tests; mockup 2 screenshot |
| 4.3 | Move between compatible chains: drag or **Move to** buttons, across paths, the rack chain and layer chains (plugins stay out of Splits, D7) | moving a device changes which path processes it (render test) |

### Stage 3+4c: racks and layers
| # | Commit | Test |
|---|---|---|
| 4.4 | `InstrumentKind::Rack` + layers, slot remap on move / delete, reader version 7 only for songs with a rack | serde round-trip; old songs save byte for byte; remap tests |
| 4.5 | Engine: layer voice pool + layer chains (note clips, launcher, export) | a rack of A and B renders as the sum of A alone and B alone; key range and level work; no-alloc test; fingerprints |
| 4.6 | Layers from Tracker blocks, MIDI / keyboard jam and note preview | render tests per source |
| 4.7 | UI: **Make a rack from this instrument** (track pie, Instruments list, panel header), + Add layer (new or existing instrument), layer settings (mockup 1); layer slots indented under their rack in the Instruments list | UI tests; undo is one step each |

### Stage 5: save and load custom racks
| # | Commit | Test |
|---|---|---|
| 5.1 | `racks.rs`: `RackFile`, `validate`, `save_cwr` / `load_cwr` (shared ZIP code with `instfile.rs`), `docs/racks/FORMAT.md` | round-trip; every validation error gives its message; fuzzed/damaged files never panic |
| 5.2 | **Save Custom Rack** dialog (mockup 4): name, description, sound type, tags, what is saved, clash → Update / Save as new | file written; clash never overwrites without asking |
| 5.3 | One `racks::load_into(song, rack, target)` used by every way of loading: new slots, given to the target track only (D9) | other tracks' instruments and renders unchanged; one undo step |

### Stage 6: My Racks
| # | Commit | Test |
|---|---|---|
| 6.1 | **My Racks** category in Browser › Instruments (next to ChipWave Instruments), fed by the library scan; search matches name, tags, sound type, description | browser tests |
| 6.2 | Preview (›) renders a short phrase of the rack off-line, as the library audition does | preview leaves the song untouched |
| 6.3 | Right-click: Load onto track, Add as new instrument, Rename, Duplicate, Update from track's sound (backup kept), Edit description and tags, Show in folder, Delete (D10) | file-system tests in a temp dir |
| 6.4 | Missing-plugin badge and card (mockup 5): Find plugins…, Load anyway | test with a fake plugin id |

### Stage 7: undo and error handling
| # | Commit | Test |
|---|---|---|
| 7.1 | Undo / redo sweep for every rack and device action | scripted: do 20 actions, undo all = the starting song, redo all = the end song |
| 7.2 | Error sweep: damaged / too-new / unknown device / missing sample / missing plugin / read-only folder | each shows a sentence and changes nothing |

### Stage 8: end-to-end
| # | Commit | Test |
|---|---|---|
| 8.1 | Scenario: build a 2-layer rack with a Split → play → save the rack → restart (new App, same config folder) → load into a new song → render matches | `scenario_tests` |
| 8.2 | Your 11 acceptance points as named tests, a docs page (`docs/RACKS.md`), screenshots, test installer | – |

**Size:** Stages 1–2 are small (UI on existing code). Stage 4b (Split) and 4c (layers) are the real engine work and the biggest risk. Stages 5–6 are medium. I'd hand you a test installer after Stage 2, after 4b, after 4c and at the end.

## In scope vs later

**In:** everything above.

**Later (not in this round):**
- plugin effects inside a Split (D7)
- automation and drag-to-modulate on devices inside a Split (D8)
- tracker pitch effects on layers
- per-layer mixer sends or routing a layer to another track
- racks inside racks
- velocity ranges on layers (key ranges are in)
- sharing racks online

---

## Decisions for you (recommended answer first)

- **D1. One panel, not two.** Today's Devices tab becomes Nested Device Chains, so there is no second device view. **Recommended:** yes, with the tree + detail as the default and the current card row kept as a **Cards** toggle, so nothing that works today goes away. *(Update 22 Oct 2026: Del asked for **Cards** as the default view; Tree is one click away and the user's own choice is remembered.)*
  *(Update 23 Oct 2026: Del approved Rack layout A. The Rack is one row of cards (instrument, effects, + drop zone) behind a **Notes | Rack** switch; the tree, Fold tree, the Cards toggle and the signal-flow text are gone from it. The tree survives as the chain editor, opened from a card's **Open in the chain editor**, with **‹ Back to Rack**. The saved `chains_cards` setting is kept but no longer read, so settings files don't change.)*
- **D2. What a click on a track or instrument opens.** **Recommended:** Nested Device Chains, with that instrument selected, so its controls show first and the chains sit beside them, as the brief asks. Settings › Follow the Timeline selection gets a choice to keep today's Instrument tab.
- **D3. Layout.** **Recommended:** tree on the left (about 370 px, foldable, width remembered) and detail on the right (mockups 1–2). On a laptop the tree folds to a slim strip with names.
- **D4. What a rack is.** **Recommended:** a new rack instrument slot whose layers are ordinary instrument slots, at most 4 layers. *Alternative:* layers stored inside one slot. That is tidier in the Instruments list, but every editor, automation and plugin path would need a second "nested" version. I advise against it.
- **D5. Where layers play.** **Recommended:** everywhere notes play, including Tracker blocks, but without tracker slides, vibrato or arpeggio on the layers in the first version.
- **D6. Split limits.** **Recommended:** 2–4 paths, one Split inside another at most, an empty path = dry sound, each path with level, pan and on/off.
- **D7. Plugin effects inside a Split.** **Recommended:** not in the first version (shown greyed with a reason). Plugin effects still go in the rack chain and in layer chains, as today.
- **D8. Automation inside a Split.** **Recommended:** not in the first version. Devices in the rack chain and layer chains keep full automation and modulation. Inside a Split the knobs work by hand, and the automate handles are hidden there so nothing pretends.
- **D9. Loading a rack onto a track.** **Recommended:** its instruments are added as new slots and that track's clips switch to the rack (one undo step). Other tracks, and the slot the track used before, are untouched.
- **D10. Deleting from My Racks.** **Recommended:** a confirm, then the file moves to `Racks/Deleted` (no new dependency for the system recycle bin), so it can be got back by hand. **Update** keeps the previous version in `Racks/.backup` (last 3).
- **D11. File format.** **Recommended:** a `.cwr` ZIP with TOML inside, like `.cwi` and `.cwt`. (The brief suggests JSON or RON. TOML is what every ChipWave file already uses, it is just as versionable, and it needs no new dependency.)
- **D12. Racks folder.** **Recommended:** `<config>/Racks` next to Instruments (paths above), changeable in Settings.
- **D13. Plain instruments.** Saving a rack from an instrument with no layers. **Recommended:** allowed (it becomes a 1-layer rack). The existing **Save to library** (`.cwi`) stays as it is.
- **D14. Piano roll copy/paste branch (`pianocopy`).** **Recommended:** merge it into main first (once you've tried the test installer) so racks starts from the newest code. Nothing in it overlaps this work.
