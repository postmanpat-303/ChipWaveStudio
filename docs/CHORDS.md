# Chord helper and scale lock (0.2.3.9)

![Chord helper](screenshots/dark-chord-helper.png)

Open it with **Song > Chord helper…** or **Chords…** in the block bar. The window can stay open while you edit.

## Song key

Pick a root and a scale, or press **Detect from song**. The key is saved with the song (`.cwt`, and `key = "A minor"` in a text song's `[song]` table); old songs simply have no key.

Scales: major, minor, harmonic minor, melodic minor, dorian, phrygian, lydian, mixolydian, locrian, phrygian dominant, major pentatonic, minor pentatonic, blues.

**Detection** uses the Krumhansl–Schmuckler key-finding algorithm (Krumhansl 1990) with the Krumhansl & Kessler (1982) major and minor key profiles. It builds a duration-weighted pitch-class histogram of every melodic note in the song order (drum channels and one-shot percussion are skipped), correlates it with the profile rotated to each root, and blends that with how many of the notes fit each scale (so modes and pentatonic scales can be found too). The top six keys are shown as chips with their score; click one to use it. The Lumen Drift demo detects as A minor.

## Chord pads

One pad per scale degree, labelled with the roman numeral and the chord name (for A minor: i Am, ii° Bdim, III C, iv Dm, v Em, VI F, VII G). The chord type buttons build **triad**, **7th**, **sus2**, **sus4**, **add9** or **power** chords from the scale; **Octave** and **Inversion** (root, 1st, 2nd) set the voicing. Clicking a pad plays the chord with the current instrument (about 0.7 s) and, with *Click adds to progression* on, appends it to the progression.

## Progression

Chips show the chords in order with their length; click a chip to cycle 1 bar → 2 bars → ½ bar, right-click to remove it. **Preset progression…** fills in one of:

| Preset | Degrees |
|---|---|
| Uplifting i–VI–III–VII | Am F C G in A minor |
| Pop I–V–vi–IV | C G Am F in C major |
| Sad vi–IV–I–V | major key |
| Epic i–VII–VI–VII | minor |
| Andalusian i–VII–VI–V | harmonic minor gives the major V |
| Dark i–iv–i–v | minor |
| Psy i–II–i–VII | phrygian (the flat II) |
| Jazz ii–V–I–I | use 7th chords |
| 50s I–vi–IV–V | doo-wop |
| Royal road IV–V–iii–vi | J-pop |

## Write into block

Writes the progression into the current block with the current instrument, starting at row 0 (or the cursor row) and, with *Repeat to fill the block*, looping until the block ends. A note-off ends the last chord at the end of the range. It replaces the notes on the target channels only, as one undo step (Ctrl+Z).

- **Chords**: one chord tone per channel, from the cursor channel to the right (a triad uses 3 channels, a 7th 4). Tones that do not fit in the song's channels are dropped and the status bar says so.
- **Arp**: one channel; pattern up, down, up-down or random, one note every N rows, over 1–3 octaves.
- **Bass**: one channel, the chord roots two octaves down; rhythm whole, 8ths, offbeat, rolling 16ths or octaves.

## Scale lock

**Scale lock** (in the chord helper; shown as *lock A minor* in the block bar) snaps every note typed on the computer keyboard and every MIDI note (played and recorded) to the nearest note of the song key; ties go down. Note-offs are not changed. It is a setting, so it stays on until you turn it off; it does nothing while the song has no key.

## Credits

- C. L. Krumhansl & E. J. Kessler (1982), "Tracing the dynamic changes in perceived tonal organization in a spatial representation of musical keys", *Psychological Review* 89(4), 334–368, https://doi.org/10.1037/0033-295X.89.4.334: the key profiles.
- C. L. Krumhansl (1990), *Cognitive Foundations of Musical Pitch*, Oxford University Press: the Krumhansl–Schmuckler algorithm.
- The pad-and-progression workflow is inspired by **Scaler** by Plugin Boutique. No code, data or artwork from Scaler is used; scales, chord names and the presets are standard music theory.
