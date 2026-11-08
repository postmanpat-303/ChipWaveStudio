# Sable Current: notes

Original dark psytrance by Del (postmanpat). A Phrygian, 145 BPM, 256 bars, 7:04.
No samples. Generator: `scripts/make_sable_current.py`.

## Timing

Each block is 4 bars (6.6 s).

| Bars | Clock | What plays |
|---|---|---|
| 1–16 | 0:00 | Kick, and two short bass pulses in the gap after each kick. Filter nearly closed |
| 17–32 | 0:26 | Same pulse, with A-Phrygian turns. Filter still opening, staying under 750 Hz |
| 33–48 | 0:53 | Offbeat closed hats enter. Filter is open |
| 49–64 | 1:19 | Shaker under the roll |
| 65–80 | 1:46 | StringWave pad (A and E) |
| 81–96 | 2:12 | Pad leans on the flat second (A# and F) |
| 97–128 | 2:39 | Two riff phrases, rim on 2 and 4 |
| 129–160 | 3:32 | Fuller hats and a fill at the end of each 4 bars |
| 161–168 | 4:24 | Breakdown: pad and a quiet riff, no kick or bass |
| 169–184 | 4:38 | One held note, resonant filter sweeps up |
| 185–192 | 5:04 | Sweep falls, a downlifter |
| 193–200 | 5:17 | Kick and bass return, filter opens again |
| 201–224 | 5:31 | Hats, then the riff and pad |
| 225–240 | 6:11 | Fullest section |
| 241–256 | 6:37 | Parts leave. Last bars are kick, bass, and the pad |

## Sounds

- Deep Kick: built-in trance kick, low drive
- Sable Bass: sine plus a quiet triangle (not a saw). Each hit is two 16ths after the kick, amp decay 45 ms, sustain 0, release 28 ms. Low-pass sweeps about 70 Hz to 620 Hz over the first 32 bars and never goes to a bright buzz. High-pass at 42 Hz, fader −9 dB, a small send to the high-passed echo. Ducked by the kick.
- Closed Hat, Open Hat, Dust (shaker), Rim
- Phrygian Riff: square wave, short glide, its own low-pass lane
- Night Pad: StringWave Slow Swell Orchestra, cutoff and reverb lanes, a slow LFO once it enters
- Eerie Sweep: saw plus noise, resonant low-pass, one automation arc
- Fall: Downlifter

Automation lanes are song-length. The bass low-pass and the sweep low-pass are checked by rendering: the end of the opening is brighter than the start and brighter than the same bars with that lane removed, and the sweep's bright point is brighter than its closed point and brighter than the same moment with that lane removed.
