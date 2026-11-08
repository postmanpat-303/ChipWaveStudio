# Sable Current: notes

Original dark psytrance by Del (postmanpat). A Phrygian, 145 BPM, 176 bars, 4:51.
No samples. Generator: `scripts/make_sable_current.py`.

## Timing

Each block is 4 bars (6.6 s).

| Bars | Clock | What plays |
|---|---|---|
| 1–32 | 0:00 | Kick and one short bass note per beat. Resonant low-pass opens from about 70 Hz to 620 Hz |
| 33–40 | 0:53 | Theme: short filtered bleeps and quiet hats. Not the full arrangement yet |
| 41–48 | 1:06 | The answering bleep phrase |
| 49–64 | 1:19 | StringWave pad enters under the bleeps |
| 65–96 | 1:46 | The theme fills out: rim, shaker, a second bass note only on the last beat of a phrase |
| 97–128 | 2:39 | Breakdown, then one eerie sweep |
| 129–144 | 3:32 | Kick and one bass note return |
| 145–160 | 3:58 | Bleeps return, then the fullest section |
| 161–176 | 4:25 | Parts leave |

## Bass

Sine plus a quiet triangle. Amp decay 45 ms, sustain 0, release 28 ms. One note on the off-beat of each beat. A second note happens only on the last beat of some later 4-bar phrases, never in the intro or the return of the roll. Fader −9 dB. Send 0.07 into the echo return, which is high-passed at 350 Hz, so that echo is a quiet repeat of the harmonic, not another bass note and not sub. Low-pass automation is the native lane from 70 Hz to 620 Hz over bars 1–32, and it stays under 750 Hz after that.

## Lead

Phrygian Bleep: triangle plus a little pulse, not a sustained square. Amp decay 50 ms, sustain 0, release 25 ms. Notes are one row long with gaps. Synth low-pass moves from about 360 Hz when the theme enters up to about 980 Hz at the fullest part, resonance 0.2. Echo send 0.06.
