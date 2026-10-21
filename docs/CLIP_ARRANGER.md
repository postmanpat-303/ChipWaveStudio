# Clip Arranger

A grid of **scenes** and **slots** on the left of the Timeline, one row per track, lined up with the
timeline's track rows. Inspired by other DAWs' session views. It takes the place of the left
Instruments list on the Timeline; **Ctrl+8** or **View › Clip Arranger** switches between the two.
The Tracker view keeps its Instruments list.

![Clip Arranger](screenshots/dark-clip-arranger.png)

## Scenes and slots

| | What it is | Click |
|---|---|---|
| **Scene** (header button) | block N (`03 Drop B`) | every track plays block N, looping, from the next bar |
| **Slot** (one per track and scene) | the track's column of block N, with a mini preview of its notes | ▶ plays just that track: in the playing scene it joins at the next bar; otherwise scene N starts with the other tracks held |
| **■** at the end of a row | | that track stops at the next bar |
| **■** in the header | | everything stops at the next bar |

- A dot is an empty slot (no notes in that column).
- The playing scene is lit; the slots that play have an accent outline and a progress bar. While the
  arrangement plays, the slot of the block under the playhead on each track is lit instead.
- A queued scene, slot or stop has a pulsing outline until its bar line (steady with Settings › Look ›
  Reduce motion). Clicking a queued scene again takes it back.
- **Launch** (top right): **1 bar**, **½ bar**, **Beat** or **Off** (the next row). Bars follow the
  timeline's time signature.
- **Drag a slot onto the timeline:** it lands as an ordinary **Tracker clip** (block + column) where
  the preview shows, as long as the block. One undo step. See *Dropping a slot* below.
- **Double-click a slot** to open that block and column in the Tracker.
- **‹ ›** page through the scenes when there are more than fit.
- The Song / Master row isn't launchable.

### Songs with note clips

A song with note or audio clips (every new song, recorded takes, Drum Kit patterns) has no Tracker
blocks to show, so its scenes are the **clip start times** instead: one scene per start (named after a
marker there, else *Bar 5*), and each track's slot holds its clip that starts there, with a preview of
its notes. A scene ▶ loops that stretch of the timeline on every track; a slot ▶ loops its clip with
the other tracks held (or joins the playing scene). Double-click a slot to open the clip. Songs whose
clips are all Tracker clips keep the blocks as scenes.

Instruments live in **Browser › Instruments** and in the **Inspector** (I): its Instrument section has
a picker for the selected track (every note on the track switches, one undo step).

### Dropping a slot

Drag any slot (Tracker blocks or note clips) onto the Timeline. While you hold it there:
- the target track is tinted and a **ghost clip** shows where it lands and how long it is;
- a dashed **snap line** runs through the ruler with the bar it lands on and the snap in use (the
  toolbar's **Snap** setting; hold **Shift** for single rows);
- a card at the pointer says what will happen: the clip and its length, *→ Track 02 Bass at bar 9*,
  the instrument that will play it, and whether it is **linked** (the default: a copy that shares its
  notes or block, so editing one changes both) or, with **Alt** held, an **independent copy**;
- clips in the way are outlined red and the card says *Overlaps …*.

**Esc** cancels. Letting go over a free place puts it there. Letting go over other clips asks first:
**Replace** (the clips in the way lose only the part under the new clip), **Place after it** (the
first free place on that track, with its bar on the button), **Keep both** (note and audio clips play
together; Tracker clips can't, because one hides the other) or **Cancel**. Dropping on the Song /
Master row, below the tracks, or a note clip on an audio track makes a **new track** with the source
track's instrument. Every drop is one undo step (Ctrl+Z also removes a track it added).

Browser **Drum Patterns** dragged onto the Timeline use the same preview and question.

### Launcher clips

The scenes above come **from the arrangement** (the status line says so). **Capture** (at the right of
the status line) turns them into the launcher's **own clips**: one scene row per scene, each slot a clip
of its own that is **linked** to the arrangement's (it shares the notes, so editing them changes both;
position, length and name are each clip's own). From then on the grid shows the launcher:
- **▶ on a slot** plays that clip on its track, looping, from the next boundary (the Launch setting);
  the other tracks play on, so clips from different scenes mix. From a stop only that track plays.
- **▶ on a scene** plays its clips, each looping on its track; tracks with no clip in that row stop.
  Click it again while it waits to take it back. **■** stops a track, **Stop all** everything.
- **Double-click an empty slot** for a new one-bar clip there; it opens in the piano roll and plays the
  track's instrument. Double-click a clip to edit it; a click selects it for the roll.
- **Drag a clip onto the Timeline**: the same preview and question as above, a linked copy (Alt:
  independent).
- **Right-click a clip**: *Duplicate (variation)* (an independent copy in the next free row of the
  track) or *Delete*. Right-click a scene: *Duplicate scene* (independent copies) or *Delete scene*.
  **+ Scene** adds an empty row.
- A **×N** badge on a slot (and after a clip's name on the Timeline) counts the clips sharing its notes;
  the piano roll shows **Linked ×N**, and clicking it makes the clip unique (one undo step).

Launcher clips are saved with the song (`[[launcher]]` and `[[scenes]]` in `timeline.toml`, reader
version 6); songs without them save exactly as before. They never play in the arrangement, the render
or an export. Limits for now: launcher clips are note clips (Drum Kit patterns included); Capture is
offered for songs whose scenes are note clips; recording straight into a slot and the generators
writing into a launcher clip come later.

## Master

Under the grid: **Volume** (the mixer's Master fader), the limiter's **Drive** and **Ceiling**, a
**Limiter on / off** switch, L / R level and gain-reduction meters, short-term loudness and true peak,
and a level graph of the last few seconds. The same settings as Mixer › Master.

## How it plays (phase 1)

- A scene launch is the Tracker's block loop ("loop block N"), switched on a boundary. From the
  arrangement it stops the timeline there (note and audio clips fade out) and loops the block.
- Stopped tracks are held silent through their mixer gain (a short ramp, no clicks); launching a whole
  scene plays every track again. Play, Stop and Panic clear everything.
- Real-time safe: a click queues a small `Cue` (plain data) for the engine under the same lock as every
  other engine call; the render thread applies it on the boundary row inside the row step, with no extra
  locks or allocation (`src/engine/launch.rs`, tested with an allocation counter).
- Launched launcher clips play per track from their own loop clock (counted from the launch), so they
  never jump when the arrangement loops; other tracks keep the arrangement. Songs without launches take
  exactly the old engine path (render tests compare sample for sample).
- Until you Capture, nothing is saved in the song: the panel reads the song's blocks and the timeline's tracks. Songs open,
  save and play exactly as before (a test renders with and without the launcher and compares). The
  on / off switch and the Launch setting are app settings.

## Not yet (phase 2, see [ROADMAP.md](ROADMAP.md))

For Tracker-block scenes: independent per-track slots (launcher clips do this for note clips). Recording
into a launcher slot, and recording a session into the timeline.
