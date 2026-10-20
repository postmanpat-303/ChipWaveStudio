# Inspector and view tab bar

*Unreleased (after 0.2.4.0).* Two pieces of the "inspired" layout Del picked
from his reference screenshots (mockups E and F in `/workspace/shots`):
a collapsible **Inspector** on the left and a **view tab bar** along the
bottom. Neither adds new sound features: every control is bound to a setting
ChipWave already has.

## Inspector

| | |
|---|---|
| Show / hide | **I**, View › Inspector, the **Inspector** button on the right of the view tab bar, or **×** in its header |
| In the Tracker | **Alt+I** (plain **I** types a note there, as before). Alt+I works in every view |
| Text fields | while you type in a text field (search, a name), I is just a letter |

It shows whatever you clicked last:

- **A track** (click a track header, or move the pattern cursor to another
  channel) or **a clip on the Timeline** (click it). Clicking a clip shows
  the clip's section first and its track below.

### Clip section (Timeline view)

| Control | What it changes |
|---|---|
| Name, file, where it sits | read-only: block / note clip / audio file name, bar.beat start and end, length in beats, loop length, file length and rate |
| **Muted** | the clip's mute (same as **M**) |
| **Gain** (audio clips) | the clip gain, −48 … +12 dB (same as right-click › gain) |
| **Fade in / Fade out** (audio clips) | in beats, same as dragging the clip's top corners |
| **Shape** (audio clips) | equal power / linear / S-curve fades |
| **Piano roll** | opens the clip's notes in the lower panel (same as double-click) |
| **Tracker** (Tracker clips) | the clip's block in the Tracker (same as Enter) |

In tracker mode the clips come from the order list, so the clip section is
read-only and says so; switch the song to timeline mode to edit clips.

### Track sections

| Section | Controls |
|---|---|
| **Track NN** | **Name** (commits when you click away or press Enter; **Esc** cancels), **Colour** (Pick… → 12 track colours or Automatic; timeline mode, saved with the song), **Icon** (the header icon: Pick… → Auto or one of 16 icons; timeline mode, saved with the song; also in the track pie), **Volume** (dB), **Pan** (C button centres), **M** / **S** |
| **Sends** | the two returns (Delay, Reverb by default; their names come from the Mixer) and the sidechain **Duck** depth |
| **Instrument** | the slot the track plays with its type and icon, **Edit** (instrument editor in the lower panel), **Browse…** (the Browser's Instruments tab). Drop an instrument from the Browser on the track header to swap it |

There is no record-arm, input or monitor matrix: ChipWave has no audio
recording yet, so those controls would do nothing and are left out.

Every edit is a normal undo step (**Ctrl+Z**); slider drags are one step
when you let go.

## View tab bar

A slim bar above the status line:

- **Transport:** to start (play marker to bar 1), **Play song** (from the
  play marker), **Stop**, and the position: `bar 12.3 · 0:41.2` in
  timeline mode, `pos 03 · row 12` in tracker mode, plus the tempo.
- **TIMELINE + PIANO:** the default layout (Timeline on top, piano roll
  below; Ctrl+6, View › Reset to default layout). Lit while it is showing.
- **Tabs:** **TIMELINE** · **TRACKER** (Tracker) · **ORDER** ·
  **PIANO** (piano roll in the lower panel) · **INST** (instrument editor in
  the lower panel) · **AUTO** (automation lanes in the lower panel) ·
  **MIX** · **FX** · **SAMPLE**. The lit tab is what is showing; when a
  lower-panel tab is lit, the view it sits under (TIMELINE or TRACKER) is
  tinted.
- **Right:** **Inspector** and **Browser** toggles.

The tabs call the same switches as the toolbar's VIEW group, the View menu
and the shortcuts (Ctrl+1 … 6, Ctrl+T, Ctrl+I, F11), so those keep working
and the bar follows them. **View › View tab bar** hides it (saved).

## Settings

`inspector_open` (default off) and `view_bar` (default on) in
`settings.toml`.

## For developers

`src/app/inspector.rs`, `src/app/viewbar.rs`, the header icon glyphs in
`src/app/glyphs.rs`; tests in `src/app/inspector_tests.rs`. Screenshots:
`CHIPWAVE_SHOT_INSPECTOR=track|clip` (with `CHIPWAVE_SHOT_SEL=<track>`) on
any `--screenshot` view.
