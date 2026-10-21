# Unified workflow: plan for Stages 2–6 (for approval)

Based on `AUDIT.md`. Each numbered item below is **one small commit with its own tests**, in the order I propose to land them. Nothing here has been built yet. Every stage ends with the full gate (fmt, clippy on Linux and Windows, `cargo test`, render fingerprints), screenshots at 1920×1080 and 1366×768, and a test installer for you.

## Order of work (biggest wins first)
1. **Stage 2**: quick layout fixes that stop the timeline from disappearing (small, low risk).
2. **Stage 4a**: one drop controller with placement preview, snap and no silent overwrite, used by the launcher drag (note-clip slots included), Browser drops and timeline moves.
3. **Stage 4b**: launcher clips become real clips of their own (the staging area), with per-track launching.
4. **Stage 3**: instrument workflow (track select → full editor, one "track's instrument" rule, effects inside the editor).
5. **Stage 5**: pies per object, clip rename and colour, active-object readout.
6. **Stage 6**: workflow regression suite (scenarios A/B/C, save/load, old songs).

4a comes before 4b because it fixes the drag you can't do today (note-clip slots) without waiting for the new data model, and 4b then reuses it.

---

## Data-model decisions (please confirm)

**D1. Launcher clips are their own clips, stored next to the arrangement.**
`Timeline::launcher: Vec<LaunchClip>` with `LaunchClip { scene: u16, clip: Clip }`, plus `Timeline::scenes: Vec<Scene { name, colour }>`.
- They reuse the existing `Clip` struct (track, length, loop, offset, name, colour, mute, gain, seed, clip automation) and the **same pools** (`Timeline::notes`, `Timeline::audio`) and the **same id space** (`Timeline::new_id`). The piano roll, Drum Kit editor, chord lane and Make unique therefore work on a launcher clip with no second editor.
- They are not on the timeline, so the plan, render and export ignore them.
- Saved in `timeline.toml` only when non-empty (`skip_serializing_if`). Songs without launcher clips save **byte-identically**, and `min_reader_version` is bumped only for songs that use them.

**D2. What the launcher grid shows.** The grid shows launcher clips. In a timeline song without any, it keeps today's view, scenes derived from the arrangement (block scenes for Tracker clips, `sections()` for note clips), labelled **"From the arrangement"**, plus a **Capture into the launcher** button. The button turns those scenes into launcher clips, linked to the arrangement clips. Nothing that works today is lost, and there is still one launcher (no second version of the feature). Tracker-mode songs are unchanged.

**D3. Shared vs independent after a drag.** Dragging a launcher clip onto the timeline creates a **linked** clip (same note data: what the brief asks for, "same clip data where possible"). **Hold Alt** for an independent copy. Launcher **Duplicate (variation)** always makes an **independent** copy, because variations are meant to differ.

**D4. The linked-edit rule and how it's shown.**
- The **notes or audio are shared**: editing them in any linked clip changes every clip that shares them, in the launcher and in the arrangement.
- **Per clip**: position, length, loop, trim, name, colour, mute, gain and clip automation.
- **Shown in three places**:
  - a **×N badge** on every linked clip (timeline and launcher);
  - the piano roll or Drum Kit header: "Linked: 3 clips (2 in the arrangement, 1 in the launcher) · **Make unique**";
  - the clip pie centre, "linked ×N".
- Make unique (Ctrl+Shift+D) is offered everywhere and is one undo step.

**D5. No silent overwrite.**
- A drop from the launcher or Browser that **overlaps** a clip turns the preview red, names the clip and bars it overlaps, and asks on release: **Replace**, **Place after it**, or **Cancel**.
- Timeline **moves** and **Ctrl+D** show the same red highlight and a status line, but don't prompt (a prompt on every nudge would get in the way). Undo still covers them. *Your call if you'd rather have the prompt there too.*

**D6. "The track's instrument" has one rule:** `Track::instr`, else the slot most used by its clips (today's `track_slot`), else none. It is computed, not written, so old songs don't change on save.

---

## Stage 2: persistent layout
| # | Commit | Test |
|---|---|---|
| 2.1 | The sample editor opens in the lower panel (Instrument tab › Sample) instead of `View::Sample`; Ctrl+5 still gives the full view on request | `view == Timeline` after Act::OpenSample; the sample editor still edits (existing `sampleview` tests) |
| 2.2 | "Open in Tracker" is shown only with the Tracker on and only for Tracker clips; with the Tracker off, launcher double-click and Enter open the piano roll | menu items per clip kind; `view` stays Timeline |
| 2.3 | Instruments list › "Open in FX view" and the Mixer's FX button open the **Devices** tab; the FX view stays in View › FX | `view` stays Timeline |
| 2.4 | The selected track scrolls into view when the lower panel opens or resizes | track row rect is inside the lanes rect after `set_lower(1)` at 1366×768 |
| 2.5 | Laptop height: the lower panel keeps at least about 3 track rows of timeline visible (it shrinks before the timeline drops below that) | `narrow_tests` at 1366×768: ≥ 3 rows; screenshots |
| 2.6 | Lower-panel header breadcrumb: "Track 07 Pad 2 › 07 Supersaw Pad (WaveForge) › Synth" | text from `ch` / `instr` / tab |

## Stage 4a: one drop controller (preview, snap, overlap)
| # | Commit | Test |
|---|---|---|
| 4a.1 | `DropPlan { track, start, len, overlaps, action }` computed by one function: uses the **timeline snap setting** (Shift = off), the Master row and audio-vs-note checks, and the overlap list | unit tests: snap Bar/Beat/Off, overlap detection, invalid rows |
| 4a.2 | One preview painter (ghost on the target row, snap line through the ruler, card with clip · track · bar · instrument · linked/independent) used by `ca_drag` | screenshot view `launcher-drag` |
| 4a.3 | Note-clip slots (section mode) can be dragged: drop = linked copy (Alt independent), same length, offset and loop | renders: the placed clip plays the same notes a bar later |
| 4a.4 | Overlap prompt (D5) for launcher and Browser drops | the earlier clip is untouched after Cancel and after Place after it |
| 4a.5 | An incompatible target (audio on a note track, the Master row) offers **New track for this clip**, reusing the `DropAt::NewTrack` path from drum patterns, or cancels with a clear hint | no clip on the Master row; new track gets the instrument |
| 4a.6 | Browser drum-pattern drops and timeline moves/Ctrl+D use the same preview and overlap highlight | existing `drumpat` / `timeline_m2` tests still pass |

## Stage 4b: launcher clips as a staging area
| # | Commit | Test |
|---|---|---|
| 4b.1 | Model D1 + serde + reader-version rule | round-trip; a song without launcher clips saves byte-identically; old fixture songs render bit-identically |
| 4b.2 | **Engine spike (investigation commit, no UI)**: per-track launch of an off-timeline clip. `Cue` gets per-track launcher indices; the render thread reads that track's events from the launched clip, looped from the boundary, while other tracks keep playing. No locks or allocation on the audio path | render tests: the launched clip's notes land on the boundary; it loops; the other tracks are unchanged; songs without launches are bit-identical |
| 4b.3 | Grid shows launcher clips; empty slot: double-click = new clip on the track's instrument, opened in the piano roll (or the Drum Kit editor on a kit track) | workflow test |
| 4b.4 | Slot ▶ / scene ▶ launch launcher clips; you can mix clips from different scenes | engine + UI tests |
| 4b.5 | "Capture into the launcher" from arrangement scenes (D2) | linked counts correct |
| 4b.6 | Generators target the open clip: Drum Kit New pattern/Generate and chord Generate write into a **launcher** clip when one is open | scenario-A test |
| 4b.7 | Linked badge ×N and the piano roll/Drum Kit header line (D4) | counts across launcher + timeline |
| 4b.8 | Record into a launcher slot (armed track, quantised start) | capture test |

## Stage 3: instrument workflow
| # | Commit | Test |
|---|---|---|
| 3.1 | One resolver `track_instrument(tr)` (D6); `HeadOp::Select`, `typing.rs` and `browserview.rs` use it | the three paths agree, including converted songs |
| 3.2 | Clicking a **track header** shows its instrument's full editor in the lower panel. Clicking a **clip** shows the clip editor (piano roll or Drum Kit). This is one clear rule, with a Settings switch to turn it off | tab after header click / clip click |
| 3.3 | A track with no instrument shows "Choose an instrument" in the lower panel (the track pie's Add › built-ins and Browse…, the same list) | assigning from it opens the editor |
| 3.4 | Effects inside the editor: the instrument editor gets an **Effects** tab that is the Devices row component (same code, not a copy) | inserting a device there shows it in Devices |
| 3.5 | Shared instance notice: "Also used by track 03 · Make a separate copy for this track" (reuses `instruments.rs` `duplicate_instrument`) | the edit affects only this track after copying |
| 3.6 | The Instruments list click and Inspector › Edit go through 3.2 | — |
| 3.7 | Track pie: **Edit instrument** and **Automation** segments (Organize absorbs Save CPU so the ring stays at 9) | `trackpie_tests` |

## Stage 5: context and navigation
| # | Commit | Test |
|---|---|---|
| 5.1 | Timeline **clip pie** on `radial.rs` (the reusable widget): Edit ›, Split…, Duplicate › (Linked copy / Independent copy / Repeat ×2 / ×4), Make unique, Colour ›, Rename, Mute, Delete, To launcher, Notes › (Use the track's instrument, Quantise, Glue). A list-menu choice in Settings, like Settings › Chord menus › List for the chord pie | each wedge acts on the clicked clip |
| 5.2 | **Launcher clip pie**: Play/Stop, Edit notes, Instrument, Duplicate (variation), Place in arrangement › (At the playhead / After the last clip / In the loop region / Replace selected), Record into, Rename, Colour ›, Delete | placement uses the DropPlan of 4a |
| 5.3 | **Drum pattern pie** on the step grid: Generate, Fill, Variation, Clear, Duplicate as variation | — |
| 5.4 | Clip Rename / Colour in the Inspector too (model fields exist) | saved and shown |
| 5.5 | Wording without the Tracker: "Clip 00 Intro · Track 03" instead of "Block 00 Intro · ch 3", and "clip" instead of "Tracker clip" in hints | text tests |
| 5.6 | Active object readout (track · clip · instrument) in the lower-panel header (2.6) and highlight of the active clip in both timeline and launcher | — |

The track pie keeps its own painter for now (it has grids that `radial.rs` lacks). Merging the two pie widgets is a later, separate refactor, not part of this work.

## Stage 6: regression
- Headless workflow tests (`workflow_tests.rs` style) for scenario A (Drum Kit → pattern → launcher → drag → Ctrl+D → render), B (WaveForge → edit → record into launcher → drag → render), and C (chords → clip → two launcher versions → place one).
- Save/load round trips with launcher clips, linked clips and Make unique, plus every fixture song bit-identical.
- Screenshot pass of every new view at 1920×1080 and 1366×768, looked at one by one.
- Test installer on your PC.

## Risks
- **Engine (4b.2):** per-track launching changes `advance_row`. This is why it's a spike with render tests before any UI, and why songs without launches must stay bit-identical.
- **The parallel selection-linking work on `titlebar`** touches `app/timeline.rs` and `HeadOp::Select`. Stage 3.1/3.2 must start **after** that merge lands, to avoid conflicts and two rules for selection.
- **The auto-open editor** can steal the piano roll while you edit notes. Mitigated by the header-vs-clip rule and the setting.
- **1366×768:** another header line (breadcrumb) costs height. It goes into the existing tab row, not a new row.
- **File format:** older builds can't open songs that use launcher clips. They get the existing "please update" message, and songs without launcher clips are unaffected.
- **Overlap prompt fatigue:** limited to drops from outside the timeline (D5).

## What stays unchanged
Tracker mode, blocks, the Order list and its arranger; Tracker-mode Clip Arranger block scenes; the engine path for songs without launcher clips (bit-identical renders); the file format of songs without launcher clips; existing shortcuts; the track pie layout (apart from 3.7); the chord pie; no new dependencies; no Tracker code removed.

## Mockups (`/workspace/shots/unified/`)
| File | What it is |
|---|---|
| `01-default-layout-REAL-1920x1080.png`, `…-1366x768.png` | **Real render** of the current build: timeline on top, launcher on the left, piano roll in the lower panel. This is the layout the plan keeps. |
| `02-instrument-in-lower-panel-REAL-1920x1080.png`, `…-1366x768.png` | **Real render**: the instrument editor in the lower panel. At 1366×768 only two tracks stay visible (Stage 2.4/2.5). |
| `03-launcher-to-timeline-drag-1920x1080.png`, `…-1366x768.png` | **Mockup** over a real render: launcher clip dragged to a track, with ghost, snap line, target card and the overlap rule (Stage 4a). |
| `04-launcher-clip-pie-1920x1080.png` | **Mockup**: launcher clip pie with Place in arrangement › (Stage 5.2). |
| `05-timeline-clip-pie-1920x1080.png` | **Mockup**: timeline clip pie with Duplicate › Linked / Independent and the linked count (Stage 5.1, D4). |
| `raw/` | Unmodified harness renders used as evidence and backgrounds. |
| `src/mock.py` | The script that draws the mockups (Pillow, box-only; not part of the app). |
