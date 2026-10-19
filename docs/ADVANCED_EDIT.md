# Advanced Edit (tracker)

A panel of block operations for the tracker (pattern editor), inspired by MorningDAW's **A<>E** panel. Open it with **Ctrl+Shift+A** (Cmd+Shift+A on a Mac), the **A<>E** toggle at the right of the Block bar, or **View › Advanced Edit**. It docks on the right of the pattern. **Float** turns it into a window you can move, and **Dock** puts it back. Whether it is open, and docked or floating, is saved in the settings.

The line under the title says what the next button will change, for example *block 08 · channels 1–2 · rows 4–19 · Note + Inst + Vol + FX#*, or why it can't (nothing selected, mask empty).

## Section to process

| Section | What it covers |
|---|---|
| Selection in pattern | the selection (green box) in this block |
| Column in pattern | the cursor's column (Note, Inst, Vol, FX# or FX val) of the cursor's channel, this block |
| Column in song | that column of that channel in every block |
| Track in pattern | the cursor's channel (every column the mask allows), this block |
| Track in song | the cursor's channel in every block |
| Whole pattern | every channel of this block |
| Whole song | every channel of every block |

"In song" sections skip derived blocks (variations rendered from another block, see [AUTO_ARRANGE.md](AUTO_ARRANGE.md)), because those follow their source block anyway. A *column* section is the *track* section limited to the column the cursor is in, so it also needs that column ticked in the mask.

## Content mask

**Note**, **Inst**, **Vol**, **FX#** (effect number) and **FX val** (effect value) choose which columns an operation touches. Untouched columns stay exactly where they are, so a Flip with only Note ticked reverses the melody and leaves the volumes and effects on their rows. **Right-click** a box for a *solo mask* (only that column), and use **All** to tick everything again.

## Operations

Every button is **one undo step** (Ctrl+Z), respects the section and the mask, and confirms what it did in the status bar.

- **Cut / Copy:** copy the section's masked columns (columns outside the mask are copied as empty). Cut also clears them. The copy is the same clipboard as Ctrl+C, so Ctrl+V pastes it too. Cut and Copy work on one block, so the "in song" sections are refused.
- **Paste:** pastes at the cursor (row and channel), only into the masked columns, clipped at the block's end and the last channel.
- **Mix-Paste:** like Paste, but only into cells that are empty (in the masked columns). It never overwrites anything.
- **Clear:** empties the masked columns of the section.
- **Flip:** reverses the rows.
- **Shrink:** keeps the even rows (0, 2, 4 …) packed to the top, so the section plays twice as fast in half the rows. The freed rows at the end are cleared.
- **Expand:** doubles the spacing: row n moves to row 2n, so the section plays at half speed. Rows that would land past the section's end are lost (Shrink first, or expand a half-filled section).
- **Notes -12 / -1 / +1 / +12:** transpose. With **Safe mode** on (the default), a note that would go below C-0 or above B-9 is left alone and the status bar counts them. With it off, notes stop at C-0 / B-9 (like the Block bar buttons). Note-offs never move.
- **Mirror:** mirrors notes around the pivot (C-4 unless you drag the pivot): a note n semitones above it becomes n below it. Safe mode works the same way.
- **Vol fade:** **From** / **To** (00–40, ChipWave's volume range) over the section's rows, in a straight line. Cells with a note or a volume value get the fade; empty cells and note-offs are left alone. In "in song" sections each block fades on its own. **Fade 0F->01** and **Fade 01->0F** are the classic tracker presets scaled to ChipWave's range (40 -> 04 and 04 -> 40).
- **Replace instrument:** every cell in the section that uses the first instrument uses the second instead (needs Inst in the mask).

## What the tracker already had (and still has)

- Ctrl+C / X / V (copy, cut and paste at the cursor) and Delete (clear the selection).
- Alt+= / Alt+- transpose (Shift = octave), the Block bar **-12 -1 +1 +12** buttons and **Edit › Transpose** (these clamp; they don't use Safe mode).
- Backspace / Insert (pull up / push down rows).
- Right-click an instrument › **Use this instead of…** (swaps one instrument's notes for another's, in the whole song or this block).

Advanced Edit uses the same clipboard and undo, and adds a section and a mask on top. It doesn't change what those keys do.

## Channel headers

Each tracker channel header now has:

- a **colour bar** in the channel's track colour: the timeline track's own colour (Inspector › Colour) when it has one, otherwise the same palette colour the timeline uses;
- the channel number and name, cut short with "…" when it doesn't fit;
- the channel's **instrument** ("01 909 Kick"): the instrument the track was given, otherwise the one its notes use most;
- **Play:** auditions that instrument on C-4 through the channel. **Shift+click** solo-plays: only this channel is soloed and the block plays from the top. Shift+click again un-solos it.
- **M / S:** mute and solo, the same as F9 / F10. A muted or not-soloed channel keeps its grey header and dimmed cells.

The headers keep the channel width, so 16–32 channels look the same as 4. Channels that don't fit scroll sideways as before.

## Limits

- No interpolate or humanise yet.
- No "Column" of a multi-column track: a ChipWave channel has one note column, so *Column* means one of its Note / Inst / Vol / FX columns.
- Expand loses what falls past the section's end, and Shrink drops the odd rows. Ctrl+Z brings them back.
- Paste always goes to the cursor, not to the start of the chosen section.
- The volume fade only writes cells that already have a note or a volume.
- Play auditions on C-4 only, not on the channel's current note.
- Shift+click solo-play changes the solo flags that are saved with the song, like the S button.

## For developers

`src/advedit.rs`: sections, masks and the operations on a `Song` (unit tests in `src/advedit/tests.rs`). `src/app/advedit.rs`: the panel and `App::adv_op` (one snapshot per button). `src/app/pattern.rs`: the channel headers (`channel_headers`, `header_op`). UI tests: `src/app/advedit_tests.rs`. Screenshots: `--view advedit` and `--view advedit-headers`.
