# Unified workflow: Stage 1 audit

Audited **read-only** on branch `unified` (`/workspace/cw-unified`): `origin/titlebar` (20a210d) with `origin/pianoplay` (07ffc6c) merged locally. The merge had no conflicts. Nothing in the app was changed.

**How each finding was checked.** **[code]** means I read the code at the reference given. **[shot]** means I saw it in a render from the real app made with the `shot.rs` harness (`--screenshot`, Lumen Drift demo, Slate theme, 1920×1080 and 1366×768; the images are in `raw/`). **[test]** means an existing automated test covers it. Nothing below was clicked through by hand in a live window, so a [code] finding is how the code reads, not a confirmed run.

---

## 1. Architecture map

### 1.1 Layout and panels
| Piece | Where | What it does today |
|---|---|---|
| Central views | `src/app.rs` `enum View` (Pattern, Arranger, Timeline, Mixer, Fx, Sample, Settings) | Only one is shown at a time. Mixer, Fx, Sample and Settings **replace** the Timeline. |
| Default layout | `instpanel.rs` `default_layout()` | Timeline on top, **piano roll** in the lower panel (about 40 %). Used on start, on File › New and when a timeline song opens. Ctrl+6 restores it. **[shot]** `01-default-layout-REAL-*.png` |
| Lower panel | `instpanel.rs` `set_lower(m)` / `lower_mode()` / `lower_switch_bar()` | One tab at a time: Piano roll (3), Devices (4), Instrument (1), Automation (2), or the Order list (0, Tracker only). The flags `extra.lower_inst`, `lower_dev`, `pr.open` and `show_auto` are saved in settings. |
| Full height | `instpanel.rs` `toggle_full_height()` (F11) | Hides the lower panel; the timeline keeps its place. |
| Clip Arranger (launcher) | `cliparr.rs` `clip_arranger()` / `ca_grid()` | A side panel to the **left** of the Timeline, with its rows lined up with the track rows. It is on by default (`settings.rs` `clip_arranger: true`), toggled with Ctrl+8, and has the Master panel under it. |
| Inspector, Browser | `inspector.rs`, `browserpanel.rs` / `browserview.rs` | Side panels on the right. |
| Sample editor | `sampleview.rs`, opened from `instpanel.rs:879` `Act::OpenSample => self.view = View::Sample` | Opens as a **full view**, so the Timeline disappears. |
| FX view | `fxview.rs`, opened from `instruments.rs:250` and `mixer.rs:680` | A **full view** that edits the same insert chain as the Devices tab. |

### 1.2 Instruments and track assignment
- Instruments are **song-level slots**: `Song::instruments`, 1-based ids, `InstrumentKind` with `Gen(GenEngine::{Poly (WaveForge), Drums (Drum Kit), Analog, Fm, Noise, Wave, Grid, ChipFx})`, plus Synth, Sample, Voice and Plugin.
- A track points at a slot with `timeline::Track::instr` (0 = none). Notes also carry their own `tl::Note::instr`; a note with 0 plays whatever the channel last played. **Two tracks that point at the same slot share one instance**, including its insert chain.
- There are **three different answers to "the track's instrument"**:
  1. `Track::instr` alone: `timeline.rs:920` `track_instr()`. Track selection uses this (`app/timeline.rs:3115` `HeadOp::Select`).
  2. `Track::instr`, otherwise the slot most used by the track's clips: `browserview.rs:1096` `track_slot()`.
  3. `Track::instr`, otherwise the globally selected `self.instr`: `typing.rs:115` `track_instr()` (live typing and recording).

  A song converted from the Tracker (`timeline/edit.rs:25` `enter_timeline_mode`) leaves every `Track::instr` at 0. In such a song, selecting a track does **not** change the instrument editor.
- Insert FX belong to the **instrument** (`devices.rs` header comment: "The chain is the instrument's existing fx list"). The Devices tab presents them as the track's chain.

### 1.3 Lower-panel editors
- Instrument editor: `instpanel.rs:510` `instrument_view()`. Tabs come from `params.rs` `Tab` (Grid, Synth = WaveForge, Main, Modulation, FX, Advanced). The modulator strip is `modview::strip`. WaveForge opens on its Synth tab (`polyview.rs:165` `poly_panel`). For Drum Kit, the Main tab is `drumkitview.rs:208` `drumkit_view()`, which has pads, Sound, the step sequencer (`dk_sequencer`), the generator (`dk_generate`, including fills) and a pattern switcher. **[shot]** `raw/timeline-drumkit-*.png`, `02-instrument-in-lower-panel-REAL-*.png`
- Devices: `devices.rs:231` `devices_view()`. It shows the instrument card followed by the inserts.
- Piano roll: `pianoview.rs`, with the chord lane (`chordlane.rs`, which has its own pie on `radial.rs`).
- Automation: `lanes.rs`, `timeline/autolanes.rs`, `trackauto.rs`.

### 1.4 Clip data model (including linked clips)
- `timeline.rs:476` `Clip { id, track, start, len, offset, loop_len, kind, name, colour, muted, gain, fades, seed, auto }`.
- `timeline.rs:336` `ClipKind::{Block{pattern,col}, Notes{data}, Audio{asset}}`.
- Note data is pooled: `Timeline::notes: Vec<Shared<NoteClipData>>` (an `Arc`). **Linked clips** are clips with the same `data` index. Ctrl+D (`timeline/edit.rs:186` `duplicate` → `copies_at`) and Alt-drag create them. Ctrl+Shift+D (`make_unique` / `make_unique_notes`, `edit.rs:236/258`) splits one off. Block clips link through the same block. Clip automation (`Clip::auto`) is **per clip**, even for linked copies.
- **The UI does not show linking.** No badge or count is drawn on a clip, and the only hint is the status text after Ctrl+D (`app/timeline/edit.rs:217`). **[code]**
- `Clip::name` and `Clip::colour` are in the model, but **no UI sets them**. The Inspector clip section has Mute, Gain, Fades and Shape; the clip menu has no Rename and no Colour. **[code]** `inspector.rs:213`, `app/timeline.rs:2806-2858`
- Overlaps: for note and block clips the **later clip wins** (`timeline/plan.rs:230`); audio clips mix. Nothing blocks or flags an overlap.

### 1.5 Launcher (Clip Arranger) and its relationship to arrangement clips
- **The launcher has no clips of its own.** It is a *view of the arrangement*:
  - In a Tracker-clip song, scene N = block N, and a slot = (block, track column). See `cliparr.rs` `previews()`.
  - In a song with note or audio clips (pianoplay), scenes are **clip start times** and a slot = the timeline clip that starts there. See `cliparr.rs:152` `sections()`, `ca_clip()`.
- **Data relationship:** a launcher slot is literally the arrangement's block or clip. Editing a slot edits the arrangement. There is nowhere to keep an idea that is not already in the song timeline. **[code]**
- **Engine:** `engine/launch.rs` `Cue { scene: block to loop | span: timeline rows to loop, on/off track masks, quantise }`. There is **one global play position**. Launching a slot plays *that* scene's span and holds every other track silent: `cliparr.rs:408` `ca_slot_sec` sets `off = all & !bit(tr)`. **You cannot play track 1 from scene A against track 2 from scene B.** **[code]**
- **Slot → timeline drag**: `cliparr.rs:1194` `ca_drag` → `TlOp::PlaceBlock` → `timeline/edit.rs:648` `place_block`. It works **only for block slots**; it is disabled in section mode (`cliparr.rs:967` `&& secs.is_none()`), so **note-clip slots can't be dragged to the timeline**. The block drag snaps to **bars only** (ignoring the snap setting) and shows a tinted row with a ghost and the hint "Tracker clip on track NN, bar B" **[shot]** `raw/cliparranger-drag-*.png`. `place_block` pushes the clip **without checking for overlap**, so a drop onto an existing clip silently covers it, because the later clip wins.

### 1.6 Playback engine
- `engine.rs` plays tracker songs exactly as before. Timeline songs play from the resolved plan (`timeline/plan.rs`, `Engine::ensure_plan`). Clip Arranger cues switch at quantise boundaries without locks (`launch.rs`). Clip-scene loops use `Engine::tl_span` (pianoplay). The piano roll playhead comes from `app/timeline.rs` `tl_head()` (pianoplay).

### 1.7 Selection state (several independent fields)
| Field | Meaning |
|---|---|
| `self.ch` | current track / channel (also the Tracker cursor column) |
| `self.instr` | current instrument slot (drives the Instrument tab and live notes) |
| `self.tlv.sel` / `sels` | selected timeline clips |
| `self.pr.*` | piano roll note selection and focus |
| `self.dk.clip` | the Drum Kit editor's current pattern clip |
| `self.arr_sel` / `arr_multi` | Order list entries (Tracker) |
| `self.pattern` / `row` / `col` | Tracker cursor |

Nothing gives one "active track / clip / instrument" readout in a single place. Another worker is adding two-way track ↔ timeline selection linking on `titlebar`; this audit does not cover that work.

### 1.8 Drag and drop (three separate mechanisms)
1. **Browser drags**: `browserview.rs:48` `DropAt::{Slot, Track(tr, tick), NewTrack(tick), Fx, Device, Central}` with `*_drop_hint` texts. Drum patterns (`drumpat.rs:393-470` `pat_place` / `pat_drop` / `pat_place_now`) already create a **new track with a suitable kit** when dropped below the tracks. That is the "incompatible target → create a track" behaviour the brief asks for.
2. **Clip Arranger slot drag**: `cliparr.rs` `ca_drag`, its own painter and hit test (`tlv.drop_at`).
3. **Timeline clip move / Alt-copy**: `app/timeline.rs` (`TlDrag`, about line 2591) → `edit.rs:94` `move_clips` / `copies_at`. **No overlap check.**

### 1.9 Serialization
`.cwt` zip (`fileio.rs`): `song.toml`, plus `timeline.toml` for timeline songs, with `min_reader_version`; this build reads up to **5** (`fileio.rs:423`). Every timeline field is `serde(default)` and is skipped when empty or default, so songs without a feature save byte-identically. Undo stores whole-song snapshots (`undo.rs`), which stay cheap because note data is `Arc`-pooled.

### 1.10 Menus and pies
| Menu | Implementation |
|---|---|
| Track pie | `trackpie.rs`, its own painter. Segments: Rename, Colour, Icon, Add › (built-in instruments, Browse…, tracks), Devices, Routing, Organize, Save CPU, Delete. **It has no "Edit instrument" or "Automation" segment.** |
| Chord pie | `chordlane.rs` on the reusable `radial.rs` widget, with a "List" fallback in Settings. |
| Timeline clip | egui list context menu (`app/timeline.rs:2790-2915`). |
| Launcher slot | **none** (click = play, double-click = open). |
| Drum pattern | none; its actions are buttons in the Drum Kit header. |

**There are two separate pie implementations** (track pie and `radial.rs`).

---

## 2. Scenarios in the current build

### Scenario A: drums
| Step | What happens today | Issue |
|---|---|---|
| Open a Drum Kit track | In a new song: track pie › Add › Drum Kit creates the kit, assigns it and opens the Instrument tab (`trackops.rs:100` `track_new_instrument` → `show_instrument_panel`). For an *existing* drum track, a header click only sets `ch`/`instr` and the lower panel stays on the piano roll (`app/timeline.rs:3115`). | **Extra step:** Instrument tab or Ctrl+I. **Hidden context:** a converted song has `Track::instr = 0`, so the click doesn't even switch the editor. |
| Edit the kit, generate a pattern | Pads, Sound and Generate are in `drumkit_view`. **[shot]** | OK. The generator writes into the current pattern **clip on the timeline** (`dk_new_pattern` places it after the track's last clip, `drumkitview.rs:861`). |
| Put it in a launcher clip | Not possible as a separate step. The pattern *is* an arrangement clip, and the launcher shows it as a section slot. | **Break:** no staging area. The idea is committed to the arrangement the moment it exists. |
| Trigger it with other clips | Scene ▶ loops that clip-start span on all tracks. Slot ▶ plays only that track and silences the rest (`ca_slot_sec`). | **Break:** you can't mix clips from different scenes, and "with others" only works if they start at the same time. |
| Drag it to the timeline | Disabled for note-clip slots (`cliparr.rs:967`). | **Break:** this is the core missing feature. |
| Duplicate, fills, arrange | Ctrl+D on the timeline creates a linked copy after the selection with no overlap check. Fills come from the generator's Fill amount. The separate "Fill to length…" dialog (`fillview.rs`) is Tracker/Order-list only. | **Duplicate feature names:** generator Fill vs Song › Fill to length. **Silent overlap** on duplicate. **Linking isn't shown.** |

### Scenario B: synth
| Step | What happens today | Issue |
|---|---|---|
| Select a WaveForge track | Sets `ch` and `instr` (when `Track::instr` > 0); status "Track NN selected (pattern cursor channel)". | **Extra step** to open the editor. **Tracker jargon** in the status text. |
| Full editor | Instrument tab › Synth (`poly_panel`), Modulation tab, FX tab. Inserts are in the **Devices** tab, and the full **FX view** hides the timeline. **[shot]** | **Separated controls:** effects are split across three places. At 1366×768 only **two tracks** stay visible above the editor, and the selected track can be scrolled out of view. **[shot]** `02-…-REAL-1366x768.png` |
| Adjust, audition | Knobs and MIDI/typing play `typing.rs` `track_instr()`. | OK. |
| Record or create a clip | Record arms → note clip on the timeline. "New clip here" / double-click empty lane → note clip. | Goes straight into the arrangement. |
| Experiment in the launcher | Only by looping spans of the arrangement. | **Break:** no independent variations. |
| Drag to the timeline | n/a: the clip is already there. | |

### Scenario C: idea from scratch
| Step | What happens today | Issue |
|---|---|---|
| Select an instrument | Browser, Instruments list (Ctrl+8 off), Inspector › Instrument, track pie › Add. | **Four places** that select or assign an instrument, with different results (assign vs select vs create). |
| Chords, bass, melody | Chord lane + chord pie + Generate dialog in the piano roll (note clips, "Bass only" voicing). The **Chord helper** window (Song › Chord helper…) has Chords / Arp / Bass but writes only into Tracker **blocks**. There is no melody generator. | **Duplicated feature:** two chord tools with different targets. The bass and arp writers can't reach a note clip. |
| Preview | Generate › Preview, and pointing at a chord pie item. | OK. |
| Make a clip | The chord lane needs a note clip open first. | **Order inverted:** you must create the clip before the idea. |
| Versions in the launcher | Not possible (see A). | **Break.** |
| Arrange the preferred one | Already in the arrangement. | |

### Other breaks found along the way
- **"Open in Tracker"** is always in the clip menu. On a note clip it does **nothing** (`menu_open` is None, `app/timeline.rs:2917`). On a block clip it switches to `View::Pattern`, which hides the timeline even when the Tracker is turned off (`timeline_open`, `app/timeline.rs:3166`). The launcher slot double-click does the same for block slots (`cliparr.rs:1144`).
- **Sample editing** and the **FX view** replace the timeline (see 1.1).
- **Tracker wording in the default (non-Tracker) UI**: "Block 00 Intro · ch 3" in the piano roll header and "Tracker clip on track 05" in the slot drag. **[shot]**
- **1366×768**: with the lower panel open, the timeline shows about 5 tracks, or 2 with the instrument editor. The launcher's 4 scene columns are cut to "02 Drop…". **[shot]**

---

## 3. Acceptance criteria
| # | Criterion | Status | Evidence |
|---|---|---|---|
| 1 | Timeline visible at the top by default | **Met** | `default_layout()`; **[shot]** `01-default-layout-REAL-*`. Sample editor, FX view and Open in Tracker still hide it. |
| 2 | Selecting an instrument opens its full editor | **Partly** | Track pie › Add, Browser and Inspector › Edit open it. A track header click and an Instruments-list click do not. Converted songs: no switch at all. Effects are in a separate tab or view. |
| 3 | Instrument editing keeps track and arrangement context | **Partly** | The Instrument tab keeps the timeline **[shot]**, but the selected track can scroll out of view (2 rows at 1366×768), and Sample/FX views replace the timeline. |
| 4 | Create and audition ideas in the launcher without committing | **Not met** | The launcher has no clips of its own. Launch = loop a span of the arrangement with track mutes. |
| 5 | Drag launcher → timeline with placement feedback | **Partly** | Block slots only, with a bar-snapped ghost and hint. Note-clip slots: disabled. No snap setting, no overlap feedback. |
| 6 | Clips keep content and play correctly after placement | **Partly** | `place_block` keeps the block content (the clip length is the whole block). Note-clip drop doesn't exist. An overlap silently masks the earlier clip. |
| 7 | Move, duplicate and arrange consistently | **Partly** | Move, Alt-copy, Ctrl+D, split, glue and loop work for all clip types **[test]** (`m2_tests`, `timeline_m2_tests`). None of them checks for overlap. Clip rename and colour have no UI. |
| 8 | Predictable linked vs independent edits | **Partly** | The model is clear (`Shared` data, Make unique), but linking is invisible and copies are linked by default with no choice at copy time. |
| 9 | Instrument, drum, pattern and automation workflows connect to the timeline | **Partly** | Drum Kit patterns, chord lane, recording and automation lanes all write timeline clips. The Chord helper and Fill to length are Tracker-only. Generators bypass any staging. |
| 10 | Common actions don't need unrelated panels | **Partly** | Effects: Devices / FX view / instrument FX tab. Instrument choice: four places. Track edit instrument: not in the track pie. |
| 11 | Existing projects and features still work | **Met on the merged tree (tests)** | See the test run below. Old-song bit-identity is covered by the existing render fingerprint tests. |
| 12 | Builds and passes full workflow regression tests | **Partly** | It builds and the existing suite runs (below). There is no cross-workflow suite for scenarios A/B/C yet; Stage 6 adds one. |



## 4. Duplicated or fragmented features (consolidation list)
1. Effects editing: Devices tab, FX view (full screen) and the instrument's own FX tab.
2. Instrument selection or assignment: Browser, Instruments list, Inspector, track pie › Add, and Drum Kit "Kits…".
3. Chord tools: Chord helper window (blocks) and chord lane + Generate (note clips).
4. Arrangement surfaces: Order list (Tracker), Timeline, and the Clip Arranger, which loops Timeline spans.
5. Pies: `trackpie.rs` (own painter) and `radial.rs` (chord pie).
6. "The track's instrument": three resolvers (1.2).
7. Drag and drop: three hit-test and preview paths (1.8).
8. Fill: generator Fill (Drum Kit) and Song › Fill to length (Tracker).

## 5. Build and test evidence
Run on the merged `unified` tree (20a210d + 07ffc6c, merge commit not changed), 20 Oct 2026, 23:49–23:57 Irish time:
- `cargo fmt --check`: clean.
- `cargo build --release`: OK.
- `cargo test --release`: **814 passed, 0 failed, 17 ignored** (app/unit, including `render_hashes.rs`, the old-song render fingerprints) and **3 passed** (`tests/plugins.rs`).
- Clippy (Linux/Windows) and the Windows installer were **not** run. This stage changes no code.
- Screenshots: `shot.rs` views `timeline-layout`, `timeline-instrument`, `timeline-drumkit`, `cliparranger-drag`, `cliparranger-notes` and `trackpie`, at 1920×1080 and 1366×768 (`raw/`).

