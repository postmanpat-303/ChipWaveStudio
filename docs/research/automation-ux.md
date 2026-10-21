# Automation & filter-control UX: research and redesign

Status: **research + design only** (no app code). Written 19 Oct 2026 after Del's
screenshot of the Arranger's automation view ("Strings - LP sweep 500.0 Hz",
"Strings - Flanger 0 %", "Strings - Flanger sweep 3.0 ms") with the note that the
filter rows "are unusable as they are".

Mockups: [`../mockups/automation-ux-a.png`](../mockups/automation-ux-a.png) (recommended)
and [`../mockups/automation-ux-b.png`](../mockups/automation-ux-b.png) (alternative).

---

## 1. Why the current lanes are hard to use

Read from `src/app/arranger.rs` (song lanes), `src/app/lanes.rs` (bar, menu, labels),
`src/app/timeline/autolanes.rs` (track/clip lanes), `src/app/instpanel.rs` (knob menus),
`src/automation.rs`, `docs/AUTOMATION.md`, `docs/TIMELINE.md`.

### 1.1 Where lanes live
1. **Song lanes are detached from tracks.** In the Arranger they are full-width strips
   under the block strip with no header column. Nothing ties "Strings - LP sweep" to the
   Strings channel visually, and with 6+ lanes you scroll away from the blocks you're
   automating.
2. **Two systems.** Arranger/lower-panel *song lanes* and timeline *track lanes* (the
   **A** button) look and behave differently. You "pin" or "unpin" lanes between them.
   It's hard to know which one you're editing, or which one wins ("innermost lane wins":
   clip, then track, then song).
3. **The knob's "Add automation lane" always makes a song lane**, even in timeline mode
   (`Act::Lane` → `add_lane` pushes to `song.automation`). It also switches the lower panel
   from the Instrument editor to the Automation tab and **saves that** as the new default
   (`extra.lower_inst = false`). The knob you were touching disappears, and the lane isn't
   under the track you're looking at.
4. **Duplicates are allowed.** `add_lane` doesn't check for an existing lane on the same
   target, so a second "Add lane" on the same knob makes a second, competing lane.

### 1.2 Naming and reading values
5. **Labels hide the parameter.** Song lanes show a free-text label that is saved in the song.
   In the disco demo, `instrument.8.fx.4.mix` reads "Strings - Flanger 0 %", so it's not
   obvious that this is the Chorus/Flanger *Mix*. `fx.4.delay` reads "Flanger sweep 3.0 ms",
   and `fx.2.cutoff` reads "LP sweep 500 Hz" without saying *cutoff* or *FX slot 2*. The
   label goes stale when you rename an instrument or move a device.
6. **No value scale.** Lanes have three faint grid lines (25/50/75 %) and no units on the
   Y axis. A log-scaled cutoff (20 Hz–20 kHz) looks the same as a linear 0–100 % mix. You
   can only read a value by hovering.
7. **Values are only shown at the playhead or under the mouse**, as text crammed into the
   top-left corner of the lane in the curve colour (11.5 px).
8. **Colour comes from the lane index** (`LANE_COLOURS[k % n]`), so deleting a lane recolours
   every lane below it. Colour means nothing: cutoff isn't always the same colour.

### 1.3 Editing
9. **Small targets.** Points are 3.5 px (5 px selected) on a 64 px lane, or a 44 px lane on
   the timeline. The hit radius is 6–8 px, and the lane height can't be changed.
10. **A plain click anywhere adds a point.** In Select mode, clicking to focus a lane drops
    an unwanted point. Right-click deletes with no menu, and there's no double-click
    convention.
11. **No range or rubber-band selection, copy/paste, or scale/offset of a section**
    (listed as deferred in AUTOMATION.md). An 8-bar filter sweep can't be duplicated to the
    next chorus.
12. **The shape tool is global.** "Shape: Linear" in the toolbar applies to the *selected
    point's next segment*, which is invisible until you notice the curve change. There are
    no line, curve or LFO-stamp tools, even though AUTOMATION.md specifies them.
13. **The toolbar hides lane state.** "Lane: Strings - LP sweep [on] [Delete lane]" acts on
    whichever lane was last clicked (marked by a tiny "> "). Deleting the wrong lane is
    one click away.
14. **Pencil writes one point per line, with no thinning.** A drawn sweep becomes dozens
    of points that are painful to edit.

### 1.4 Discovering and recording
15. **The "+ Lane" menu is a flat list of every instrument × parameter**
    ("02 Bass - Filter cutoff", "02 Bass - FX 2 Filter ▸", "Mixer ch 3 ▸"…), up to 320 px
    tall, with no search and no "automated only" filter.
16. **Knobs don't show that they're automated.** There's no LED or ring, and moving an
    automated knob doesn't say it's being overridden. The knob just jumps back.
17. **Knob moves can't be recorded.** There's no Read/Touch/Latch, and track lanes have no
    "record knobs" (deferred in TIMELINE.md M5).
18. **No "show lane for the knob I just touched"** (Ableton, Reaper and other DAWs all have it).

### 1.5 Filters specifically
19. **There are 8+ different filter UIs.**
    - Synth/Moog filter card, Analog ladder, Wave filter, the generators' "instrument
      filter" card, the Voice filter and Chip FX "Filters".
    - Filter / Phaser / Resonator FX devices and Grid filter modules.
    - Each has different knob names and ranges: Cutoff/Res/Env amt/F.Decay/Drive on Synth,
      Cutoff/Res on generators, cutoff+mode on the FX Filter.
20. **The commands don't appear on lanes.** 38xx/39xx (cutoff/resonance) and 3Axx/3Bxx
    (slides) win over a lane at their row, but the lane doesn't show them. Modulation from
    drag-to-modulate isn't drawn on the lane either, so the sound doesn't match the curve.

---

## 2. How popular DAWs do it

| DAW | Where lanes live | Choosing the parameter | Write modes | Clip-level | Notes |
|---|---|---|---|---|---|
| **Ableton Live 12** | One main lane per track under the header (A key), plus "+" extra lanes | Two choosers: **Device ▸ Control**. LEDs mark automated items; "Show automated parameters only" | Arrangement record (overdub), Re-enable Automation button | **Clip envelopes**: *modulation* (relative, blue) vs automation (absolute, red) | Click any device knob in Automation mode to show its lane. "Show Automation in New Lane" on right-click |
| **Another DAW (studied)** | Track automation lanes under the track (expandable) | Parameter chooser in the lane header; touching a knob shows its lane | Arranger automation write: Latch / Touch / Write | **Clip automation**, absolute or **relative (additive ±50 % / multiplicative)**; automation clips (in its latest version) | Modulators are separate from automation; mod ranges are drawn on knobs |
| **FL Studio** | **Automation clips** in the Playlist (on any track) | Right-click knob › *Create automation clip* | Record via the Automation button; **Event editor** per pattern | Pattern events (Edit events) | Clips can be looped, cloned and converted to LFO mode |
| **Logic Pro** | Lanes under each track (A), a disclosure triangle for more | Lane popup menu: **Plug-in ▸ Parameter**. Touch a control to show it | **Read / Touch / Latch / Write**, plus Trim and Relative | Region automation | Touch is the everyday mode; Latch for new levels |
| **Reaper 7** | Envelope lanes under the track, or overlaid on media | **Envelope Manager**: tree by FX, search, Visible/Armed/Active | Trim/Read / Read / Touch / Latch / Latch preview / Write | Take envelopes | "Show track envelope for **last touched** FX parameter" action; per-envelope arm |
| **Cubase 14** | Automation tracks under the track (show/hide arrow) | Parameter popup (More… tree) | R/W + Touch / Auto-Latch / Cross-over / Trim | Part data (MIDI CC lanes) | "Show used automation" |
| **Studio One 7** | Automation lanes under the track | Picker plus **Automation** panel | Auto: Off / Read / Touch / Latch / Write | **Part automation** in the Music Editor | Shape tools: line, parabola, sine, triangle, square, saw |
| **Renoise** | Pattern-based **Automation editor** in the lower panel | Device/parameter list on the left; the pattern grid holds curves | Record pattern effects | Patterns are the unit | Points, lines, curves; effect commands *and* envelopes |
| **Furnace / OctaMED** | Per-instrument **macros** (Furnace); effect commands (OctaMED) | Macro tabs per parameter | – | The instrument | Filter sweeps come from macros or per-row commands, not timeline lanes |

**Synths and FX expose filters the same way everywhere:** a Filter section with **type**
(LP/BP/HP/Notch), **slope** (12/24 dB), **Cutoff** (log, shown in Hz/kHz), **Resonance**
(0–100 %), **Env amount** (bipolar), **Key track** and **Drive**. Serum, Vital, other DAWs'
filter devices, Ableton Auto Filter and Wavetable, Logic Alchemy and Retro Synth all use
it, usually with a **draggable response curve** (X = cutoff, Y = resonance).
The cutoff lane in every DAW is **log-scaled with Hz ticks**.

### What's most standard (and why)
1. **Lanes under the track they belong to, which fold away.** This covers Ableton,
   Logic, Reaper, Cubase and Studio One. Automation lives where the sound is, and folding
   keeps the arrangement readable.
2. **A Device ▸ Parameter picker in the lane header**, with "automated only" LEDs and
   search (Ableton, Logic, Reaper). It scales to plugins with hundreds of parameters.
3. **Touch a knob and its lane shows** (Ableton, Logic, Reaper and others). It's the fastest
   path from "I want this to move" to drawing.
4. **Read / Touch / Latch (+ Write)** for recording knob and controller moves (Logic, Pro
   Tools, Reaper, Cubase, Studio One and others). Touch is the everyday default.
5. **Real-unit value scale** (Hz, dB, %), with log for cutoff.
6. **Clip/region envelopes** that move with the clip, often **relative** (Ableton
   modulation, additive/multiplicative clip modes in other DAWs, Logic region and Studio One part
   automation). This suits loop-based electronic music, where a filter wobble belongs to
   the bassline clip.
7. **Shape tools** (line, curve, sine/tri/saw/square stamps), range selection, and
   copy/duplicate.

FL-style automation clips are popular with FL users, but they're the odd one out. Renoise
and Furnace show that trackers want **both** per-row commands and an envelope view.

---

## 3. Recommended design for ChipWave

The goal is one lane system that works the same in timeline mode and in tracker mode.

### 3.1 Lanes under each track (the standard)
- **One automation system: track lanes.** Every timeline track gets an **▸ automation
  fold** in its header, replacing the bare **A** toggle. The fold shows a count badge
  ("▸ 3") when the track has lanes.
- **Lane header** (fixed-width column aligned with track headers, about 210 px):
  - **Colour swatch.**
  - **Device ▸ Parameter** picker, e.g. `Synth ▸ Filter cutoff` or `FX 2 Filter ▸ Cutoff`.
    It has a search box, LEDs on automated entries, a "Show automated only" toggle, and
    groups: Instrument, Generator, Filter, FX 1…n, Plugin, Mixer, Sends, Modulation depths.
  - **Live value in units** (`1.84 kHz`).
  - **Mode chip** (Read / Touch / Latch / Write / Off).
  - **On/off dot**, plus **×** in the right-click menu (not one click away).
- **Value scale ruler** on the lane's left edge, with ticks in real units:
  - Cutoff: log, 50 · 200 · 1k · 5k · 20k.
  - dB: −inf · −24 · −12 · −6 · 0.
  - Pan: L · C · R.
  - %: 0 · 50 · 100.
  - It's taken from the target's existing `to_real`/`format_in`.
- **Colour per parameter family**, not per lane index:
  - Cutoff: cyan. Resonance: magenta. Volume: green. Pan: amber.
  - FX mix: violet. Plugins: teal. Modulation depth: white.
  - The swatch can be overridden per lane.
- **Resizable lane height** (drag the bottom edge, 36–160 px; double-click toggles
  compact/large).
- **Bigger points:** 5 px, or 7 px on hover or selected, with a 10 px hit radius.
- **"Show only automated" / "Show all lanes"** per track, and **Alt+A** to open all
  automated lanes in the song.

### 3.2 Editing tools (in the timeline toolbar, while a lane has focus)
| Tool | Behaviour |
|---|---|
| **Pointer** (default) | Click a point to select it. Drag a point to move it. **Double-click** an empty spot to add a point (Ableton and others). Drag on empty space for a **rubber band** selection. Shift-drag to fine-tune the value. |
| **Pencil** | Freehand draw, thinned with Ramer–Douglas–Peucker. Snaps to the grid in step mode. |
| **Line** | Drag to draw a straight ramp, which replaces whatever is in that span. |
| **Curve** | Alt-drag a segment to bend it (`Point.curve` already exists). |
| **Shape stamp** | Sine, triangle, saw, square or random. Drag a range, and the rate snaps to the grid (1/16…4 bars); the vertical drag sets the depth. |
- Selection handles: **scale** (top edge), **offset** (drag the body), **tilt** (corner
  handles), **Ctrl+C / Ctrl+V / Ctrl+D**, and **Delete**.
- Right-click opens a context menu with Delete point, Segment shape (Linear / Step /
  Smooth / Curve), Clear range, Convert to commands and Remove lane.

### 3.3 Write modes (recording knob moves)
- **A global "● Automation write" toggle in the transport.** When it's off, nothing records.
- **Per-track mode chip**, Read by default:
  - **Read:** plays the lane.
  - **Touch:** records while you hold a knob, then glides back to the curve over about
    250 ms.
  - **Latch:** records from the first touch until playback stops.
  - **Write:** overwrites while playing.
  - **Off:** the lane is ignored.
- **Sources:** on-screen knobs, MIDI-learned CCs, and plugin GUIs (CLAP `param_value`,
  VST3 `performEdit`). One pass is one undo step, and it's thinned on stop.
- **When you move an automated knob in Read mode**, the knob shows an **override** state
  (hollow ring), and a **"↺ Back to automation"** button appears in the track header
  (Ableton's Re-enable Automation).

### 3.4 "Touch a knob, see its lane"
- **Any automatable knob** (instrument editor, generator, Wave, Grid macros, FX, plugin
  sliders, mixer) shows a small **automation LED** (the lane colour) when a lane exists.
- **Right-click a knob:**
  - **Show automation:** opens the track's fold and focuses the lane, creating it if
    needed (on the track that plays that instrument, not the song).
  - **Show in new lane.**
  - **Record automation (Touch).**
  - The existing Modulate items.
- **The "Show last-touched" button** (◎) in the track's fold header makes the main lane
  follow the last knob you moved, like Ableton's main lane and Reaper's last-touched
  action.
- **Don't switch the lower panel** when a lane is added. The instrument editor stays open.

### 3.5 Clip envelopes
- A clip lane is drawn **inside the clip's box** on the track lane (as now). It's also
  editable in the **piano roll's lower lane strip** (velocity / CC / *automation* tabs), the
  way Ableton's clip envelopes and Studio One's part automation work.
- **Each clip lane has a mode:**
  - **Absolute:** replaces the track lane while the clip plays (today's "innermost wins").
  - **Relative + (additive ±50 %):** adds to the track lane.
  - **Relative × (multiplicative 0–100 %):** scales it.
  - The relative modes come from another DAW. They let a "filter wobble" clip sit on top of a
    long track-level cutoff rise.
- Relative clip lanes are drawn **blue/dashed** on top of the track curve. The result
  (track + clip) is shown as a faint ghost.
- Clip lanes move, loop, trim, split and duplicate with the clip. **Glue should merge
  them** (it currently drops them).

### 3.6 What happens to the existing song lanes
- **On opening a song in timeline mode, each song lane migrates automatically:**
  - **Instrument, FX or plugin targets** move to the track whose clips play that
    instrument. If several tracks play it, they go to the first one, with a tooltip.
  - **Mixer channel targets** move to that channel's track.
  - **Return, master and tempo** move to a new pinned **Master / Song track** row at the
    bottom of the timeline. Tempo lives here, which also fixes "no tempo lane on the
    timeline".
  - This is a pure regrouping. Positions are already timeline rows, so **playback stays
    bit-exact**, and the existing tests cover it. Undo restores the old layout.
- **Tracker mode (Arranger view) shows the same lane widgets**, grouped under **channel
  headers** in a left gutter that matches the timeline track headers. "Song lane" stops
  being a separate concept in the UI. The file format can keep `song.automation`, with an
  optional `owner = channel N` for grouping.
- **Labels are generated, never stored as the display name.** They're built from the
  target and the current device names (`Strings ▸ FX 4 Chorus/Flanger ▸ Mix`). A custom
  name is optional.

### 3.7 Commands (38xx–3Bxx) and modulation on the lane
- **Commands as markers:** rows with `38xx`/`39xx`/`3Axx`/`3Bxx` (or `0C` volume, `Fcxx`)
  are drawn as small **diamonds** on the matching lane. A slide is drawn as a short ramp.
  Hovering shows "row 12: 3820 cutoff slide up".
  - The rule stays **command wins at its row until the next point**. Now you can *see* it.
- **Convert:**
  - **Lane → commands** writes `38xx` per row into the block (for MMD export or tracker
    purists).
  - **Commands → lane** reads them into a lane.
  - Both are in the lane's right-click menu.
- **Modulation band:** where a drag-to-modulate route targets the lane's parameter, the
  lane shows a **shaded band** (value ± depth) around the curve. While playing, a moving
  dot shows the live modulated value, using the same colours as the knob rings. Automating
  the *route depth* stays a "Modulation ▸ depth" lane.

### 3.8 A unified Filter card (instrument editor)
Every instrument with a filter shows the **same Filter card**: Synth, Moog, Analog, Wave,
Voice, the generators' instrument filter, Chip FX, and the FX Filter device.
- **Response graph** (about 220 × 90 px):
  - Drag the **cutoff/resonance handle** (X = cutoff log 20 Hz–20 kHz, Y = resonance).
  - The live curve includes the envelope and modulation.
  - A ghost shows the range the filter envelope sweeps.
- **Controls:**
  - **Type:** LP / BP / HP / Notch / Peak (only what the engine supports, with the rest
    greyed out).
  - **Slope:** 12 / 24 dB. **Model:** SVF / Ladder clean / Ladder vintage.
  - **Cutoff** (Hz/kHz), **Reso** (%), **Env amt** (± oct), **Env** A/D/S/R mini-strip,
    **Key track** (%) and **Drive**.
- **Header buttons:**
  - **A** opens the cutoff and resonance lanes on the track.
  - **~** adds an LFO → cutoff route.
  - **"Cmd 38xx"** shows the matching command.
- **Same names and units everywhere** (`Cutoff`, `Reso`, `Env amt`). Lane labels then read
  `Bass ▸ Filter ▸ Cutoff` no matter which engine is underneath.
- This is a presentation layer over the existing per-engine parameters: no DSP or file
  changes, just a mapping table per engine.

### 3.9 Alternative considered (mockup B)
- **What it is:** a Renoise/Ableton-clip-view-style **lower-panel automation editor**.
  - A device tree on the left (Instrument ▸ Filter ▸ Cutoff…, with LEDs) and a large
    editor with a full value scale on the right.
  - The timeline shows automation only as a **thin overlay** on each track's clips, with
    one "main" parameter per track (as in Reaper's "envelopes in media lane" and Logic's
    in-track mode).
- **Pros:** tracker-familiar. It doesn't make tracks taller, and it gives a big comfortable
  editing area.
- **Cons:** you can't see several tracks' automation against each other, and the
  lane→track relationship is still indirect. It's also less standard.
- **Verdict:** keep it as a *complement*. "Open in lower panel" from a lane header gives the
  big editor for detailed work. It shouldn't be the primary design.

---

## 4. Plan and rough effort

| Phase | Work | Days |
|---|---|---|
| A1 | Lane header widget (picker with search/LEDs, units, mode chip, colour by family), value scale ruler, resizable lanes, bigger points, double-click-to-add, generated labels, duplicate check | 3–4 |
| A2 | Knob LEDs, right-click "Show automation" to the *track* lane, last-touched follow, no lower-panel switch | 1–2 |
| A3 | Song-lane migration to track lanes and the Master/Song track (tempo lane); tracker-mode Arranger uses the same widgets with channel grouping; bit-exact tests | 2–3 |
| A4 | Write modes (Read/Touch/Latch/Write/Off), the global write toggle, recording from knobs/MIDI/plugin GUIs, thinning, override and back-to-automation | 3 |
| A5 | Rubber band, copy/paste/duplicate, scale/offset/tilt handles, line tool, shape stamps | 2–3 |
| A6 | Clip lanes: relative +/× modes, the piano-roll automation strip, merging lanes on Glue | 2–3 |
| A7 | Command diamonds and lane↔command conversion; modulation band on lanes | 1–2 |
| A8 | Unified Filter card (graph, mapping table for all engines and FX Filter) | 2–3 |
| | **Total** | **≈ 16–23 days** |

A1 and A2 fix most of what Del hit (it's usable after about 5 days). A8 can run in parallel.
Old songs must still play bit-exactly, which every phase is tested for. Any file
change (lane `owner`, clip lane `mode`) is optional and only written when used.

## Sources
- Ableton Live 12 manual: Automation and Editing Envelopes; Clip Envelopes; Arrangement View.
- Another DAW's user guide and release notes: automation, the automation editor,
  automation clips and relative clip automation.
- Image-Line FL Studio manual: Automation Clips; Event Editor.
- Apple Logic Pro user guide: Choose automation modes (Read/Touch/Latch/Write/Trim/Relative).
- REAPER 7 Envelope Manager; SOS "Automation Action"; REAPER last-touched-parameter actions.
- Steinberg Cubase forums ("Show used automation"); SOS "Automation Routes in Studio One".
- Renoise manual: Graphical Automation; Furnace manual (macros).
