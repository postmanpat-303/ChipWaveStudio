# Round Robin: patterns that change each time they play

Round Robin is a set of small, deterministic tools that make a loop sound a
little different on each pass or each hit, without giving up exact, repeatable
renders. Everything is **off by default**, nothing is written to a song file
until you use it, and songs that don't use it play and render exactly as
before (checked against the reference fingerprints).

All randomness comes from the clip's (or pad's) **seed** plus the **pass or
hit number**. There is no hidden state: seeking, looping, exporting from any
point or rendering twice always gives the same result. Each feature draws from
its own random stream, so turning one on never changes another.

## 1. Step conditions (piano roll)

Each note can carry a condition that decides on which loop passes it plays,
on top of its **Chance** (percent):

| Condition | Chip | Plays… |
|---|---|---|
| Always | (none) | every pass (the default) |
| Every X of Y | `2:4` | on pass X of every Y (1:4 = passes 1, 5, 9…) |
| First | `1st` | only the first time through the clip |
| Not first | `not 1st` | every pass except the first |
| Fill | `Fill` | only in a fill |
| Not fill | `no Fill` | except in a fill (makes room for fill notes) |

A pass is a **fill** when it's the clip's last pass (of at least two), when a
song marker starts the next section as it ends, or while the **Fill** toggle
in the lane header is on during playback (live only; never in renders).

How to use it: in the piano roll, click the lane name under the notes and
choose **Conditions** or **Chance**. Click a note's chip (or right-click a note
› **Play this note…**) to pick its condition. Notes with a condition are drawn
striped so you can see them at a glance.

## 2. Clip Variations

A note clip can hold up to four takes (**A–D**); one plays on each loop pass.
The piano roll shows tabs A, B, C, D: the tab you pick is the take you edit,
and copy, paste and linked clips work as usual. **+** copies the current take
into a new one.

The **order** decides which take plays each pass:

- **In order**: A, B, C, D, then A again
- **Random**: a random pick each pass (repeats allowed)
- **Random no repeat** (default): shuffled, never the same twice in a row
- **Ping-pong**: A, B, C, D, C, B, A…
- **Every N**: take A, with another take every Nth pass

Open it from the clip's pie (**Variations**) or the piano roll tabs. The
popover previews the next passes ("A B A C…").

## 3. Drum pad Round Robin

Each drum pad can rotate through up to four sounds (its own drum plus three
more) and give every hit a little **drift**, as real drummers never hit twice
alike. In the drum kit, select a pad and open the **Round Robin** tab:

- **Sounds**: add sounds B–D (any drum kind and settings).
- **Order**: the same five orders as Clip Variations.
- **Variation** (0–100 %) scales the drift ranges: **Pitch** (semitones),
  **Level** (dB), **Tone**, **Decay** (±%), **Timing** (ms, always late, never
  early) and **Pan**. Each range has a beginner tooltip.
- **Seed**: a new seed gives a different, still repeatable, set of hits.
- **Next 8 hits** shows which sound the coming hits will play.

On the timeline, hits are numbered per pad from the clip start, so a loop
plays the same hits every time you play it from the same spot.

## 4. Follow actions (Clip Arranger)

A launcher clip can say what happens after it has looped a few times:

> after **N** loops: **X %** *action A*, else *action B*

Actions: **Next** (the clip in the next scene down, wrapping), **Again**,
**Other** (any other clip on the track, never itself), **First**, **Last**,
**Any** (any clip including itself) and **Stop**.

Open it from a launcher clip's pie or right-click menu (**Follow action…**).
Clips with a follow action show a **»** badge, and the tooltip describes it.
The popover previews the run "From here" (e.g. `A › B › B › A …`).

**Capture to timeline** renders the run for a number of bars as ordinary,
linked timeline clips placed after the end of the arrangement (rounded up to
the bar). They keep the source clip's seed, so the capture sounds
bit-for-bit like the launcher. One Ctrl+Z undoes it.

## Saving

New fields are written only when used (`cond` on notes, `var` on note clips, `rr` on drum pads, `follow` on launcher clips), so a
song that doesn't use Round Robin saves byte-for-byte as before, and older
versions of ChipWave simply ignore what they don't know.

## Known limits

- Fill is a toggle, not a key you hold.
- Launcher clips with Variations unroll 16 passes before repeating.
- Tracker rows, live keys, rack layers and previews count drum hits from the
  start of play, so seeking in the tracker doesn't recall the hit number.
- Tone drift moves one macro per drum kind.
- Follow actions work on note clips; capture always goes after the
  arrangement's end.

## Credits

Ideas only, no code copied: the round-robin and random opcodes of the SFZ
format and sfizz, the layered drum instruments of Hydrogen and DrumGizmo,
and the per-cycle alternation of Strudel / TidalCycles. See Help › About ›
Credits.
