# Instrument editor (lower panel, 0.2.3.19)

Since 0.2.3.19 instruments are edited in the **lower panel** under the Tracker, with
knobs, tabs and cards, instead of the long slider list in the sidebar. The sidebar keeps the
compact instrument list.

![The Octave Bass (Analog generator) from the Velvet Mirrorball demo in the lower-panel instrument editor, with the compact instrument list on the left](screenshots/dark-instrument-editor.png)

![The FX tab: the Disco Strings effect chain as knob cards](screenshots/dark-instrument-fx.png)

![Full-height view (F11): the lower panel hidden](screenshots/dark-pattern-full.png)

## 1. Sidebar

- **Instrument list:** one row per instrument. The coloured bar and the **type tag** on the right
  show what it is: Synth, Moog (a synth with the ladder filter), Sample, Voice, Drum, Kit, Chip FX,
  Analog, FM-4, Noise or CLAP. The list search also matches the tags ("fm", "clap", "kit").
- **Summary** under the list: number, name, type, volume, transpose, the number of FX devices and
  the number of blocks that use the instrument.
- **Edit in lower panel** (`Ctrl+I`) opens the editor. F3 was not free (F1–F8 set the octave),
  so the shortcut is `Ctrl+I`.
- **Classic editor:** **View › Classic sidebar instrument editor** brings back the pre-0.2.3.19
  slider editor under the list. It is saved in settings.

## 2. The lower panel

- **Order list | Instrument | Automation:** the switch bar at the top of the lower panel.
  - **Order list:** the docked Order list.
  - **Automation:** the Order list with its automation lanes. This is the same as the Order list's
    Automation checkbox.
  - **Instrument:** this editor.
  - **Default (Unreleased):** the panel opens on **Instrument** for new users and whenever no
    choice has been saved; once you pick a view (the switch, `Ctrl+I`, the sidebar button) it is
    remembered and used on the next start. Settings from 0.2.3.19 / 0.2.4.0 count as "no choice"
    (they always saved the Order list), so the first start after updating opens on Instrument.
    Adding an automation lane (right-click › Automate, **+ Lane**) switches the panel to the lanes.
- **Splitter:** the dotted bar between the pattern and the panel.
  - Drag it to resize.
  - Double-click it for the automatic height.
  - Drag it to the bottom to hide the panel (Full-height view).
  - The height is saved in settings.
- **Full-height view** (`F11`, **View › Full-height view**, the **Full height** toggle at
  the right of the Block bar, or the button on the switch bar) hides the whole lower panel.
  - Press it again to bring the panel back at the height it had.
  - `Ctrl+I` also brings it back, showing the instrument editor.
  - The state is saved in settings.
  - `F11` is not used for fullscreen by ChipWave. `Ctrl+Shift+F` was taken by Fill to length.
- **View › Dock lower panel under pattern** (formerly "Dock Order list under pattern") turns the
  docked panel off altogether.

## 2a. The simple card (Unreleased)

The **Instrument** tab opens on a simple card. **Advanced ›** shows the full editor described
below, and **‹ Simple** in its header comes back. Searching (`Ctrl+F`) and the Devices card's
**Advanced ›** open the full editor too.

- **Header:** `<` / `>` (previous / next instrument), the name, the type chip, **Preset: …**,
  **A** / **B** compare and **Advanced ›**. **Preset: …** names the preset the sound matches
  ("custom" once you change it). It lists that type's presets (the synth factory sounds, the Voice
  presets or the generator presets of the same engine), then **Save to library…** and **Library…**.
- **Macros:** a row of 8 big knobs, each with a caption saying what it drives. Macros store
  nothing: a macro shows its first target's value, and turning it moves all of its targets by the
  same amount (some scaled), so songs and old files are unchanged. Double-click resets the main
  target to its default; one Ctrl+Z undoes a turn.

  | Type | Macros |
  |---|---|
  | Synth, Moog | Bright (Cutoff + Res), Width (Detune + Stereo), Attack, Release, Motion (LFO > Cutoff), Chorus (Chorus mix), Drive (Filter drive), Space (Reverb mix) |
  | Voice | Bright, Formant, Breath, Attack, Release, Babble, Chorus, Space |
  | Analog | Bright, Width (Detune + Width), Attack, Release, Env (filter env amount), Chorus, Drive, Space |
  | Wave | Bright, Width (Detune + Spread), Attack, Release, Morph (table position A + B), Sub, Chorus, Space |
  | Drum, Kit, Chip FX, FM-4, Grid | the engine's own 8 macro knobs |
  | CLAP / VST3 | the plugin's 8 assignable macros |
  | Noise, Sample | none (every control fits in the sections) |

  **Chorus** and **Space** drive the first Chorus / Reverb in the instrument's effects. Without
  one they're greyed out; click to add that effect.
- **Sound / Filter / Envelope:** one graph each (waveform, filter response, envelope; FM-4 shows
  its algorithm, Sample its waveform) and the 2–6 knobs that change the sound right now. Choices
  (Wave, Mode, Legato…) sit in the section header. Hidden until they matter:
  - **Pulse width** only on the Pulse wave; **Detune** and **Stereo** only with more than one
    unison voice;
  - **F.Decay** only while **Env amt** isn't 0;
  - **Glide** only with **Legato** on;
  - an Analog oscillator or Wave Osc B at level 0 shows just its level;
  - a filter that is switched off (Drum, Kit, Chip FX, Wave) collapses to its header and "Filter
    off".

  Knobs work as everywhere (drag, Ctrl-drag fine, double-click default, right-click for
  automation, MIDI learn and modulation). Dice locks are only in the full editor.
- **Grid** instruments show their 8 macros and **Edit the patch ›** (the Grid tab).

## 3. Header (the full editor)

- `<` / `>`: the previous or next instrument.
- The name and the type tag.
- **Preset…** loads a preset of the same kind:
  - **Synth:** the factory sounds, by folder. Only the sound is replaced; the name and FX chain
    stay.
  - **Voice:** the Voice presets.
  - **Generators:** the presets of the same engine (an Analog only offers Analog presets).
- **Save:** saves the instrument to the instrument library.
- **Library…:** replaces the sound from the instrument library.
- **A / B compare:** **B** starts as a copy of the sound. Edit either one and switch to hear the
  other. A switch is an undo step. A/B is not offered for CLAP plugins (their state lives in the
  plugin) and it resets when you pick another instrument.
- **Search parameters… (`Ctrl+F`):**
  - Matching knobs get a ring, matching dropdowns and toggles turn the FX colour, and tabs with
    matches are tinted.
  - The list under the box shows each hit as *name · Tab › Card*. Click one, or press Enter for
    the first, to switch to its tab, expand and scroll to its card, and flash it.

## 4. Tabs and cards

There are four tabs: **Main**, **Modulation**, **FX** and **Advanced**. A tab only shows when it
has something, and its label gives the number of parameters.

Each tab holds side-by-side **cards**:

- Click a card's title to collapse it to a narrow strip. This is remembered per instrument type
  (collapse Oscillators on one Analog and every Analog shows it collapsed).
- Cards fit their knobs to the panel height. When they don't fit across, the row scrolls
  sideways.
- Cards with something to show get a small picture: envelope (ADSR), filter curve, waveform,
  sample peaks, or the FM algorithm.

| Type | Main | Modulation | FX | Advanced |
|---|---|---|---|---|
| Synth / Moog | Oscillator, Filter, Envelope, Voice (stereo, glide, legato, volume, transpose) | LFO, Velocity & accent | Chain + one card per device | Timing & character |
| Voice | Voice, Vowels, Words, Output, Filter, Envelope | LFO (incl. vowel / formant), Stutter & chop, Velocity & accent | Transform, Metallic resonance, Digital distortion, Sub bass, Chain + devices | Delay |
| Drum Synth | Dice, Drum (kind, model, follow note, 8 macro knobs, level), Voice, Filter | – | Chain + devices | – |
| Kit | Dice, Kit (piece buttons), Drum · *piece*, Voice, Filter | – | Chain + devices | – |
| Chip FX | Dice, Sound (categories, wave, follow note, level), Macros, Voice, Filter | – | Chain + devices | Pitch, Duty, Vibrato, Envelope, Filters, Phaser, Repeat & arp |
| Analog | Dice, Oscillators, Filter, Amp envelope, Voice | Filter envelope | Chain + devices | – |
| FM-4 | Dice, Algorithm, Macros, Voice | Operator 1–4 | Chain + devices | – |
| Noise | Dice, Noise, Filter, Shape, Voice | Gusts | Chain + devices | Thump & hiss |
| Sample | Sample (file, length, base note, loop, **Open in Sample editor**), Voice | – | Chain + devices | – |
| CLAP | Plugin (status, **Configure…**, Reload), Macros (8 knobs + assignments), Voice | – | Chain + devices | All parameters (searchable list) |

Generator knobs that are also `43xy` macros carry a small **M1–M8** badge.

## 5. Knobs

| Action | Effect |
|---|---|
| Drag (up or right) | change the value |
| `Ctrl`-drag (or `Shift`-drag) | fine: a tenth of the speed |
| Double-click | reset to the default |
| Hover or drag | show the value; the value is always written under the knob |
| Right-click | Reset to default, **Add automation lane**, **MIDI learn** / Forget, **Modulate with ›** / Remove routing (on automatable parameters) |
| Drop a modulator chip | route it to this knob (Unreleased, [MODULATION.md](MODULATION.md)); the ring's end dot sets the depth |
| `Alt`-drag | the selected modulator's depth on this knob (the value stays) |
| The padlock (generators) | lock the knob so Dice and Mutate leave it alone |

Stepped knobs (octave, unison voices, semitones) show a dot per step. Discrete settings are
dropdowns (with a **lock** on generators) or checkboxes.

Every drag is one undo step, the same as before.

**Modulators (Unreleased):** a strip of modulator chips sits between the tabs and the cards
(**+ Modulator** adds one). The **Modulation** tab starts with the **Modulators** card (settings
of the selected modulator) and the **Routings** card (every routing with its depth, × to remove,
Lane for a depth lane). Synth-patch knobs without a target (pulse width, detune, ADSR, glide,
LFO depths…) are now automatable and modulatable. See [MODULATION.md](MODULATION.md).

## 6. What is not in the new panel

- **Not changed:** audio and the song format. The editor writes the same instrument fields the
  sidebar editor wrote.
- **Not automatable:** the generators' own sound-design knobs (only Volume is, as before),
  because the engine has no automation target for generator macros. Plugin macros, synth level,
  cutoff, resonance, drive and every FX parameter are automatable.
- **Sample editing:** loop points, slicing and editing stay in the Sample view. The Sample card
  links there.
- **FX chain editing:** reordering devices and FX presets stay in the FX view. The FX tab edits
  every device parameter and offers **+ Add effect**.

## 7. Tests

- `params.rs` enumerates every parameter per instrument type and checks it against the list of
  properties of the 0.2.3.18 sidebar editor: Synth 49, Voice 92, Drum 38, Kit 38, Chip FX 53,
  Analog 54, FM-4 53, Noise 40, Sample 19 and CLAP 21 entries, with two FX devices.
- It checks that every parameter round-trips and changes the instrument, and that defaults are in
  range.
- `instpanel_tests.rs` draws every type and tab and finds each parameter's knob, dropdown or
  checkbox. It also tests:
  - knob drag + one undo, `Ctrl`-drag, and double-click reset
  - search jump
  - Full-height view (`F11`, the buttons, height restore, `Ctrl+I`)
  - the splitter (resize, collapse at the bottom)
  - the switch bar (Order list / Automation still dock the Order list)
  - card collapse, A/B, and the sidebar button
  - old settings files
