# Timeline view

**Status:** read-only view (milestones M0 + M1 of [DAW_TIMELINE_PLAN.md](DAW_TIMELINE_PLAN.md)). You can't edit clips yet. That arrives in M2, when the timeline becomes something a song can be saved as.

![Timeline view: the Velvet Mirrorball demo playing, 14 tracks with headers on the left, coloured block clips with note previews, Intro/Verse/Chorus/Breakdown markers above the bar ruler, the playhead, the mini-map strip, and the instrument editor in the lower panel](screenshots/dark-timeline.png)

The **Timeline** view (the **Timeline** tab in the VIEW group, **View › Timeline**, or **Ctrl+T**) shows the song the way a DAW does: tracks run down the screen, time runs left to right, and each track holds the clips it plays. It is another view of the same song, so nothing about the song changes when you open it.

- **Tracks are channels.** Track 6 *is* channel 6. It has the same name, the same mixer strip, the same mute/solo and the same meter.
- **A clip is one column of one block.** If block 06 has notes in columns 1, 2 and 5, playing it from the order list puts a "06" clip on tracks 1, 2 and 5. Empty columns don't get a clip, and an empty column behaves exactly like a gap.
- When the order list plays the same block several times in a row, the clips merge into one **looped clip**, with small notches where each repeat starts.
- **Markers** come from the arranger's section roles (Intro, Verse, Chorus…).

The toolbar says **Tracker mode · from the order list**. Every song is in tracker mode for now: the order list and blocks are what you edit, and the timeline is worked out from them every time they change.

## Finding your way around

| Do | How |
|---|---|
| Open the view | **Ctrl+T**, the **Timeline** tab, or View › Timeline. **Ctrl+Tab** cycles Arranger → Timeline → Mixer |
| Zoom | **Ctrl+wheel** zooms around the mouse. **+** / **−** (keys or toolbar). **Fit** or **F** shows the whole song |
| Scroll | Wheel scrolls tracks. **Shift+wheel** scrolls time (the wheel scrolls time too when every track fits). Drag the **mini-map** strip under the clips, or the scroll bar on the right. **Home** / **End** jump to the start / end, and **←** / **→** scroll a bar |
| Track height | **Alt+wheel** |
| Move the play marker / seek | Click the **ruler** (or the marker strip). Like the arranger it snaps to the bar, and **Alt** gives the exact row. While playing, playback jumps there. Drag to scrub |
| Follow | **Follow** in the toolbar pages the view so the playhead stays about a third of the way in |
| Bars or time | **Bars** / **Time** switches the ruler between bar numbers and minutes:seconds. The readout shows `bar.beat · m:ss` at the playhead, or at the play marker while stopped |
| Open a clip | **Double-click** a clip. The pattern editor opens that block with the cursor in the clip's column, on the row you clicked. **Enter** opens the selected clip at its start. Double-clicking an empty part of a track opens the block playing there, in that column |
| Select | Click a clip (outlined). **Esc** clears the selection |
| Track header | Coloured strip, number, name and instrument. **M** / **S** mute and solo the channel. The mini-fader is the mixer volume (double-click for 0 dB), and the meter is the channel's. Click a header to put the pattern cursor on that channel |
| Full height | **F11** hides the lower panel and the timeline gets the whole height. **Ctrl+I** brings back the instrument editor under it |

Keys typed in the Timeline view never enter notes. Ctrl shortcuts, Space (play/stop) and the F-keys work as everywhere else.

## New songs

**File › New** opens on the Timeline view: 8 tracks, 16 bars across, and an outline on every track for the empty block 00 that says *empty · double-click to write*. Double-click a track to start writing in that channel. The instruments are kept, as before.

![A new song on the Timeline view: eight empty tracks with "00 Block 00 · empty · double-click to write" outlines](screenshots/dark-timeline-new-song.png)

To go straight to the pattern editor instead, turn off **Settings › New songs › Open on the Timeline view**. Songs you open from disk keep the view you are in.

**The decision (Del, 19 Oct 2026):** new songs start on the Timeline. Until M2 that means *the Timeline view*. The song itself is still a tracker-mode song with an order list. From M2 on, a new song will start in **timeline mode** (clips are the song). Existing songs always open in tracker mode, play exactly as before, and only switch if you ask (the first timeline edit offers it, and Undo puts it back).

![The Amiga theme: the same song with the arranger in the lower panel](screenshots/amiga-timeline.png)

## Performance

Clips, notes, grid lines and the mini-map are drawn as a few meshes. Only the clips in view are visited (a per-track binary search), and note previews are thinned to about one mark per pixel column. The derived timeline is rebuilt only when the song changes. A UI test draws a 32-track song with more than 9,000 clips and checks the frame time, both fitted to the window (everything visible) and zoomed in.

## Under the hood (for developers)

### Time base
- A **row is always a 16th note** (`model::ROWS_PER_BEAT = 4`). The engine's tick length is `sr·60/(bpm·4·speed)`, so speed changes how often effects step, never how long a row lasts. `lines_per_beat` only changes the display (row highlighting). M0 fixed the places that used it for timing (`arrange.rs` seconds, bars and target lengths, the chord histogram, the sample views). A test checks that changing it never changes any duration.
- The timeline counts in **ticks: 960 per quarter note**, so a row is 240 ticks and triplets and 1/64 swing come out exact.

### The model (`src/timeline.rs`)
Everything uses `serde(default)` and leaves out fields that are at their default, so a clip that just plays a block saves as five short lines.

- `Timeline { tracks, clips, notes, tempo, signature, markers, loop_range, view, next_id }`
- `Track { name, colour, kind: Note|Audio, height, folded, armed, auto }`. The track index is the mixer channel.
- `Clip { id, track, start, len, offset, loop_len, kind, name, colour, muted, gain, fade_in, fade_out, seed, auto }`
  - `ClipKind::Block { pattern, col }` is one column of a block. `ClipKind::Notes { data }` is polyphonic notes (the piano roll, M3b).
  - `offset` is a left trim. `loop_len` makes the source repeat. `source_at(t)` and `pass_at(t)` give the source position and which loop pass you're in.
- **Note data is shared**: `notes: Vec<Shared<NoteClipData>>`, where `Shared` is an `Arc` that serialises as plain data. Linked clips and undo snapshots copy a pointer, and an edit copies only the clip it touches (`Arc::make_mut`).
- **Notes, MIDI 2.0 style** (ideas from Microsoft's MIDI work, [research/microsoft-midi.md](research/microsoft-midi.md)): `Note { start, len, key, vel: u16, rel_vel: u16, instr, fx, tune, chance, expr }`.
  - Velocities are 16-bit. `vel_from_7` / `vel_to_7` use the MIDI 2.0 M2-115 min-centre-max scaling, which round-trips every 7-bit value exactly.
  - `tune` is a per-note detune in semitones. `chance` is a percentage (0 = always).
  - `expr` holds optional per-note expression lanes (`ExprKind` Volume, Pan, Tuning, Vibrato, Expression, Brightness, Pressure). They are numbered like CLAP's `CLAP_NOTE_EXPRESSION_*`, so they go to plugins unchanged.
- **Automation values are stored as 0–1 floats** and turned into parameter units only at the output. The **innermost lane wins**: clip lane, then track lane, then song lane (`innermost()`).
- **Randomness is reproducible.** Each clip has a `seed` (taken from its id when created). Anything random in a clip (note chance, later random modulators) must come from `ClipRng::new(seed, pass)`, a SplitMix64. So a random result depends only on the seed and the loop pass, never on playback history. Exports, seeking and looping stay bit-exact.
- **Tempo map** `TempoPoint { tick, bpm, ramp }`. A ramp goes linearly in BPM to the next point, and `seconds_at` integrates it exactly (a closed-form log). An empty map means `song.bpm`.
- **Time-signature map** `SigPoint { tick, num, den }` (ruler and snap only). `bars_in` and `bar_pos` give bar lines and bar.beat. **Markers** `Marker { name, tick, colour, track }` (`track` = a per-track marker).
- `TimelineView { zoom, scroll, scroll_y, track_h }` is UI state.

### Converting
- `from_order(song)` builds the derived timeline plus an `OrderMap` (order entry ↔ tick). A `0F00` pattern break ends a block's effective length after the break row.
- `consolidate(song, timeline)` goes the other way. It cuts the timeline at every clip start, end and loop wrap (at most 256 rows per block), builds one new block per distinct slice (identical slices share a block), and writes the order list. A later clip wins where clips overlap, muted clips are silent, and gaps are empty rows. A `0F00` survives only on the last row of a slice.
- **Tested bit-exact:** for the three demos, the 0.2.3.5 and 0.2.3.6 fixture songs, and a synthetic song with breaks, odd lengths, repeats and a 300-row block, order list → timeline → consolidated order list renders the **same samples**. The self-test (`--self-test`) checks the demo too.

### File format and older versions
Nothing is saved from the timeline yet: songs are written exactly as before, byte for byte. Since M0, ChipWave reads `min_reader_version` (in `manifest.toml` of a `.cwt`, or the top level / `[song]` table of a `.cwt.toml`) and refuses files that need a newer reader. You get *"This song was made with a newer ChipWave Studio … Please update"* and a **Check for updates** button, rather than a wrong-sounding song. This app reads version 1. Timeline-mode songs (M2) will save `timeline.toml` and write `min_reader_version = 2`, so this version and later refuse them cleanly, and **File › Export tracker-compatible copy** will save a consolidated version 1 song.

### Screenshots
`chipwave --screenshot out.png --view timeline --song demos/disco.cwt.toml` plays from order entry 8 (`CHIPWAVE_SHOT_POS=n` changes it). `--view timeline-new` shows a new song. See `scripts/screenshots.sh`.

## Limits in this version
- **Read-only:** no moving, resizing or splitting clips, no loop region, and no adding markers (M2).
- **Tempo:** the ruler's time and the readout use the song's BPM. `0Fxx` tempo commands inside blocks still play, but the timeline doesn't show their effect on time yet.
- **Song automation after a pattern break:** the engine places automation by *full* block rows, so a song that has both automation and `0F00` breaks would not consolidate bit-exactly. Nothing does that yet (the view never consolidates), and M2's tick transport removes the difference.
- Note clips, audio clips, per-track automation lanes and the arm button are in the model but not used yet (M3b, M4, M5).
