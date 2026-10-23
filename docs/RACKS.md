# The Rack, the chain editor and racks

Since 0.2.9.0 a track's sound is shown in the **Rack**, one row under the
Timeline: the instrument first, then its effects in order. The tree from
0.2.7.0 (Nested Device Chains) is still there as the **chain editor**, for
Splits, layer settings and moving a device into another chain. **Racks**
are one instrument made of up to four instruments that play together. You
can save racks to **My Racks** and use them in any song.

## The Rack

The lower panel has one switch, **Notes | Rack** (Ctrl+7 shows the Rack).
With the Tracker on, **Order list** and **Automation** sit next to it, on the
Notes side.

- Click a track or an instrument and the **Rack** opens under the Timeline:
  one row, the instrument first, then its effects in order, then a **+** drop
  zone. A row wider than the panel scrolls sideways.
- Every card has a **side tab**: ⏻ bypass, ✕ remove and ‹ collapse (a
  collapsed card is a slim name strip). The instrument's tab only collapses;
  it is swapped, not removed (right-click › **Replace instrument…**).
  Right-click a card for the rest: Presets ›, Open advanced editor, Make
  rack and Save rack… on the instrument; Duplicate, Move left / right,
  Show all parameters and Open in the chain editor on an effect.
- The instrument card shows a few chosen knobs and **Volume**; everything
  else is behind **Advanced ›**, which opens the full Instrument editor. A
  Drum Kit shows the selected pad's knobs and a pad picker. A rack shows its
  layers in place, each with **Open ›** to edit that layer.
- Click a card to select it (built-in instruments outline in violet, the rest
  in the accent colour). **Delete** removes it, **Ctrl+D** duplicates it and
  **0** bypasses it, but only while the Rack was the last thing you clicked:
  click the Timeline and Delete deletes clips again. **Ctrl+Z** undoes.
- Drag a title to reorder the cards. Drop an effect from the Browser on the
  row (or the **+** zone) to put it there, or on a timeline track.
- Presets are on the card (its preset field and right-click › **Presets ›**),
  so **Browser › Instruments** lists the instruments themselves: the
  ChipWave instruments and your plugin instruments.
- **Save rack…** (right of the switch) keeps the whole row as a custom rack.
- **Open in the chain editor** shows the instrument as a **tree** on the left
  with the selected row's **detail** on the right (Splits, layer settings,
  moving a device into another chain). **‹ Back to Rack** returns.
- **+ Add Device** (in the header and on every chain) opens a searchable list
  of ChipWave effects, a Split and your plugin effects. The new device goes
  exactly where the title of the list says.
- In the chain editor, drag rows to reorder them or to move a device into
  another chain. Hold Ctrl while dropping to copy it instead. Right-click a
  row for Bypass,
  Duplicate, Remove and "Add device before / after".
- Every change is one undo step (**Ctrl+Z** undoes it, **Ctrl+Y** redoes it).

## Split: parallel paths

A **Split** sends the sound down 2 to 4 paths side by side and mixes them
back together. Each path has its own devices, plus Level, Pan and On. An
empty path passes the sound through unchanged, so adding a Split doesn't
change the sound until you add a device to a path. You can put a Split
inside a path of another Split (two levels at most).

Two limits apply inside a Split in this version. Plugin effects can't go
there; they are shown greyed with the reason. Devices inside a Split have no
automation lanes. Their knobs work by hand, and the automate handles are
hidden so nothing pretends.

## Racks and layers

- **Make a rack from this instrument** (in the panel or the Instruments list
  menu) turns the instrument into a rack in the same slot. Its sound becomes
  **Layer A**, and it sounds exactly as before. The rack keeps the name,
  volume, transpose and effects.
- **+ Add layer** adds a new synth layer. **Add an instrument as a layer**
  uses an instrument already in the song. A rack holds at most 4 layers.
- Each layer has **On**, **Level**, **Pan**, **Transpose** and a **key range**
  (Lowest / Highest), plus its own effect chain ("this layer only"). The
  **Rack effects** chain processes the mixed layers.
- Layers play wherever notes play: timeline clips, the piano roll and
  Tracker blocks. Tracker slides, vibrato and arpeggio don't reach the
  layers yet.
- Layers are listed under their rack in the Instruments list as
  "(layer of 07)".

## Saving and My Racks

- **Save Custom Rack…** (in the panel header or the Instruments list menu)
  asks for a name, description, sound type and tags, and shows what will be
  saved. A plain instrument is saved as a rack with one layer. If a rack with
  that name already exists, choose **Update existing** or **Save as new**.
  ChipWave never overwrites a rack without asking.
- **Browser › My Racks** lists your racks, with search by name, tag, sound
  type or description. Double-click a rack, or use **Load onto track NN**, to
  load it onto the selected track. Its instruments are added as new slots and
  only that track switches to the rack; this is one undo step. **Add as new
  instrument** adds it without giving it to a track. **Preview** plays a
  short phrase.
- Right-click a rack for **Rename**, **Duplicate**, **Update from track's
  sound** (the old version goes to `Racks/.backup`, and the newest 3 are
  kept), **Edit description and tags…**, **Show in folder** and **Delete…**.
  Delete asks first, then moves the file to `Racks/Deleted`.
- If a rack uses a plugin effect that isn't installed here, My Racks says so.
  Loading it offers **Find plugins…** or **Load anyway**. Load anyway
  bypasses that effect but keeps its settings.

Racks live in the `Racks` folder inside ChipWave's settings folder. The file
format is described in [racks/FORMAT.md](racks/FORMAT.md).

## Not in this version

These are not supported yet:

- plugin instruments as layers;
- plugin effects or automation inside a Split;
- tracker pitch effects on layers;
- live MIDI playing and note preview of layers;
- per-layer mixer sends;
- racks inside racks;
- velocity ranges.

Songs without racks or Splits load, play and save exactly as before.
