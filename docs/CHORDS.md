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

## Chord lane in the piano roll (0.2.4.1)

![Chord lane](screenshots/pianoroll-chords-lane.png)

Note clips show a **chord lane** between the piano roll's toolbar and the notes (the toolbar's **Chords** button shows or hides it). Chords on the lane are markers: they don't play by themselves, they guide you and can write notes. They're saved with the clip and move, loop and copy with it. Songs without chords save exactly as before.

- **Double-click** empty lane: add the key's home chord (one bar). **Drag** on empty lane: draw a chord, its length follows the mouse. **Drag** a chord to move it, its right edge to change its length (snaps to the grid, at least a beat). **Click** a chord: play it on the track's instrument (and select it). **Del** deletes the selected chord.
- Names show as chord symbols with slash basses for inversions (*Gsus2/E*) and the roman numeral in the song key (*bVII*). Colours follow the degree.
- **Right-click** empty lane: the **chord pie** (a round menu: point at a wedge to open its ring). The inner wedges are styles (Pop, Rock, Blues, Jazz, Trance, Dark psy, House, Synthwave, Lo-fi), plus **Add chord** and **More…** (the Generate dialog). The outer ring lists the style's progressions (*I–V–vi–IV*, *i–bII* …); pointing at one plays it, clicking inserts it at the bar you right-clicked, in the song key, with the Generate dialog's speed, colour and "write the notes" choices. Esc, the centre or a click outside closes it.
- **Right-click a chord:** its own pie, with the chord in the centre (*Em / Minor*): **Play**, **Transpose** (±1, ±2, ±5 semitones or a scale step; pointing previews), **Chord type** (maj, min, sus2, sus4, 7, maj7, m7, dim, add9, m9, maj9, 5), **Suggestions** (relative and parallel swaps, colour, borrowed bVI / bVII, the V7 and the chord's own dominant; pointing previews), **Generate** (the dialog, starting at this chord), **Select** (the notes under it), **Delete**, **Write notes** (ring: Full, Triads, Shell, Bass only, Pad, Light, Open, Power) and **Edit** (root, type, bass, length).
- **Settings › Chord menus › List** uses ordinary list menus instead of the pies (the list also has **Detect chords from the notes**: one chord per bar from the clip's notes).
- The radial menu is a reusable widget (`src/app/radial.rs`).

### Generate chord progression

![Generate chord progression](screenshots/pianoroll-chord-generate.png)

**Generate** in the toolbar (or **More…** / **Generate** in a pie) opens the dialog:
- **Style:** Cinematic, Ambient, Indie Pop, Emo, Pop, Lo-fi / Neo Soul, Synthwave, EDM / Trance, Dark / Horror, Folk / Country, Blues, Heroic / Game, Dark psy, House, Jazz, Rock and Classical, each with a one-line description. Every style has a table of common progressions (for example Pop I–V–vi–IV and vi–IV–I–V, Rock I–bVII–IV, the 12-bar blues, Jazz ii–V–I and I–vi–ii–V, Trance vi–IV–I–V and i–VI–III–VII, Dark psy i–bII and i–bII–i–bVII, House i–iv and ii9–V9, Synthwave i–bVI–bIII–bVII, Lo-fi IVmaj7–iii7–vi7 and ii9–V13–Imaj9, the Andalusian i–bVII–bVI–V and the canon I–V–vi–iii–IV–I–IV–V). These are generic, centuries-old or genre-standard progressions.
- **Key:** C … B, and **Any / Major / Minor** (which of the style's progressions may be picked).
- **Length** 4 / 8 / 16 / 32 bars (the progression repeats), **Chord speed** Slower / Style / Faster, **Starts at bar** with − / +.
- **Colour** turns plain chords into 7ths, 9ths, add9 and sus chords; **Variation** swaps in related chords (relative minor / major, sus, 7ths), never the first one. **Walking bass (inversions)** picks slash basses so the bass moves to the nearest chord tone.
- The result shows the key and the numerals (*E minor  i - bIII - bVII - iv*) and coloured blocks with names. **Another one** rolls a new one (seeded, so the same settings give the same result), **Preview** plays it on the track's instrument.
- **Also write the notes on <instrument>** with a voicing (Full, Triads, Shell, Bass only, Pad, Light, Open, Power) and an octave writes voice-led notes too (each chord moves to the nearest inversion of the next). **Apply** replaces the chords it covers and is one undo step; a clip shorter than the progression grows to fit. With no song key yet, Apply sets it.
- **Target** names where Apply writes (*“Bass” on track 03, bars 1–4*) and what it does to the notes there. By default the chord notes **replace** the notes that start inside each chord's span, and only there: the rest of the clip is kept, and applying the same progression twice gives the same notes, never doubled ones. **Add on top (keep the notes already there)** keeps every note instead (still never doubling an identical note). If replacing would delete more than 8 notes, or any note outside the new chords' pitch range (a melody or a bass line), a **Replace notes?** box says how many and where, with **Replace (delete n)**, **Add on top** and **Cancel**. The chord pie's **Write notes** works the same way.
- Written notes carry the track's instrument (the one in its header), so they sound like that track.

## Scale lock

**Scale lock** (in the chord helper; shown as *lock A minor* in the block bar) snaps every note typed on the computer keyboard and every MIDI note (played and recorded) to the nearest note of the song key; ties go down. Note-offs are not changed. It is a setting, so it stays on until you turn it off; it does nothing while the song has no key.

## Credits

- C. L. Krumhansl & E. J. Kessler (1982), "Tracing the dynamic changes in perceived tonal organization in a spatial representation of musical keys", *Psychological Review* 89(4), 334–368, https://doi.org/10.1037/0033-295X.89.4.334: the key profiles.
- C. L. Krumhansl (1990), *Cognitive Foundations of Musical Pitch*, Oxford University Press: the Krumhansl–Schmuckler algorithm.
- The pad-and-progression workflow is inspired by **Scaler** by Plugin Boutique. No code, data or artwork from Scaler is used; scales, chord names and the presets are standard music theory.
