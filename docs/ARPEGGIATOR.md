# The Arp

ChipWave's Arp is a note effect. It sits first in a track's Rack, before
the instrument, and plays the notes you hold one at a time, in time with
the song. A Drum Kit gets the same card under the name **Note repeat**.
When the Arp is off, the instrument's `arp` is `None`, nothing is saved,
and the engine plays exactly as it did before.

## A short history

- **Organs and the first synths (late 1960s to 1970s).** Step sequencers
  such as Moog's 960 could already cycle a few pitches. Home organs then
  added factory "arpeggio" and auto-accompaniment effects that broke a held
  chord into a moving figure. Synths with a built-in arpeggiator followed in
  the mid-1970s: the RMI Harmonic Synthesizer (1974) is often named as one
  of the first, and ARP's Quadra (1978) had one too.
- **Roland and Korg (1978–1984).** The Jupiter-4 (1978) had Up, Down,
  Up/Down and Random modes and could follow an external clock. The
  Jupiter-8 (1981) added a 1–4 octave range and Hold. The Juno-60 and
  Korg's Polysix (1981–82) made the arpeggiator standard on affordable
  polyphonic synths. The Polysix added **Latch**, which keeps the pattern going
  after you let go until you play a new chord. Clock-in jacks, and later
  MIDI clock, let the arp lock to a drum machine. That is where today's
  tempo-synced rates come from.
- **Chip music (1980s).** The C64's SID and the NES APU had only three to
  five voices, so composers faked chords by cycling the notes of a chord on
  a single voice once per 50/60 Hz frame. Rob Hubbard's driver did this
  with an octave flicker. Trackers later wrote it as effect `0xy` (play the
  base note, then +x and +y semitones). ProTracker on the Amiga and
  FamiTracker on the NES both use `0xy`, and ChipWave's own tracker does too.
- **Phrase arps and workstations (1990s–2000s).** Workstations went from
  playing note orders to playing whole phrases. Yamaha's Motif line grew
  from 256 arpeggio types to several thousand, could run four at once and
  could switch between them by velocity. Korg's KARMA (2001) generated
  patterns that respond to what and how you play.
- **Modern arps (2000s onward).** Software and current hardware treat the
  arp as a small step sequencer driven by the held chord. Common features:
  - note-order variations (outside-in, inside-out, as played);
  - per-step velocity, gate, ties, rests and ratchets;
  - per-step probability and seeded randomness;
  - scale or key quantise;
  - octave walks;
  - Euclidean rhythms (Toussaint, 2005);
  - retrigger versus free-running;
  - arp parameters as modulation targets.

## What ChipWave's Arp does

The simple controls are on the card's left. The advanced ones are in
**More ▸**.

| Feature | Where |
|---|---|
| Modes: Up, Down, Up-down, Down-up, Converge, Diverge, Random (seeded), As played, Chord | **Mode** knob |
| Octave range 1–4 | **Octaves** knob |
| How the octaves are walked: Up, Down, Bounce | More › **Octaves go** |
| Tempo-synced rate from 1/4 to 1/128, with triplet (T) and dotted (.) values | **Rate** knob |
| Chip fast-chord rates 1/64, 1/96 (one 50 Hz frame at 125 BPM) and 1/128 | Rate knob, or More › **Fast chord** |
| Gate, Swing, step velocity depth | knobs |
| Latch (hold) | **Latch** button |
| Sustain pedal (CC64): released keys keep playing until the pedal comes up | MIDI keyboard |
| Random seed | **Seed** button |
| Retrigger (a new note restarts the pattern) vs Free-run (the pattern stays on the song grid) | More › **Pattern** |
| Scale lock to the song key (step transposes count scale steps) | More › **Scale lock** (it shows the key, or "set a song key") |
| Seeded velocity humanise | More › **Humanise** |
| Euclidean fill: hits / steps, rotate | More › **Euclid** and **Fill** |
| Per-step velocity, gate, accent, rest/tie, ratchet ×1–4, transpose | the step grid (**Steps**) |
| Per-step chance, seeded so every play is the same | grid › **Chance** row |
| Pattern length 8 / 16 / 32 | header **Length** (at 32 the card widens to show all 32 steps) |
| Named presets: Trance Gate 16ths, Chip Fast Chord, Synthwave Octaves, Euclid 5/8 Pluck, Converge Glass, In-key Runs, Probability Sparkle, and more | header **Preset ▾** |
| Rate, Gate, Swing, Octaves and Humanise as automation and modulation targets (`instrument.N.arp.rate` and so on) | right-click the Rate, Gate, Swing or Octaves knob › **Add automation lane**, or drop a modulator chip on it; all five (with Humanise) are in Automation › **+ Lane** |
| Bake the Arp's steps into the note clip (the Arp switches off; one undo step) | timeline clip menu › **Bake Arp to notes** |

### Tracker cells

On an instrument with an Arp, a tracker cell's note is held until the next
note or a note-off (`===`), and the Arp steps through it. A `0xy` cell is
treated as a chord: the note plus x and y semitones. The same code runs in
tracker mode and for a pattern clip on the timeline, so both play the same
steps at the same times. On instruments without an Arp, `0xy` is still
the classic per-tick arpeggio.

### Determinism

Every random choice is seeded by the Arp's seed and the step's position in
the song, so the song sounds the same on every play. This covers Random
mode, step chance and humanise, and holds through Bake and export.

## Sources

The full list with links is in the arp2 research notes. The main ones:

- Roland Jupiter-4 and Jupiter-8 specifications (support.roland.com).
- The Wikipedia articles on the Roland Jupiter-4 and the ARP Quadra.
- The Korg Polysix and Roland Juno-60 owner's manuals.
- Sweetwater, "The History of Sequencers".
- Rob Hubbard's music driver notes (1xn.org), and the chord-arpeggio page
  of the m64 Chiptune Wiki.
- Battle of the Bits, "FamiTracker Effects Commands", and the ProTracker
  2.3D documentation.
- Yamaha, "The Complete Musical Instruments of the MOTIF Series".
- Sound On Sound's review of the Korg KARMA.
- Apple's Logic Pro Arpeggiator guide.
- The Arturia MiniFreak manual.
- G. Toussaint, "The Euclidean Algorithm Generates Traditional Musical
  Rhythms", Bridges 2005.
