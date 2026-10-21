# Racks and Nested Device Chains

ChipWave Studio 0.2.6 (test builds) adds **Nested Device Chains**, a panel
under the Timeline that shows an instrument, its layers and their effects as
a tree. It also adds **racks**: one instrument made of up to four
instruments that play together. You can save racks to **My Racks** and use
them in any song.

## The panel

- Click a track or an instrument and Nested Device Chains opens under the
  Timeline (**Ctrl+7**). Settings › *Follow the Timeline selection* can open
  the Instrument tab instead.
- The **tree** is on the left. **Fold tree** narrows it to names, and
  **Cards** brings back the old row of device cards. The **detail** for the
  selected row is on the right: the instrument card, a device's controls, a
  Split's paths or a layer's settings.
- **+ Add Device** (in the header and on every chain) opens a searchable list
  of ChipWave effects, a Split and your plugin effects. The new device goes
  exactly where the title of the list says.
- Drag rows to reorder them or to move a device into another chain. Hold
  Ctrl while dropping to copy it instead. Right-click a row for Bypass,
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
