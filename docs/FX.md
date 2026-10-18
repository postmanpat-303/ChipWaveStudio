# FX devices

Every instrument has an **FX chain** of devices (FX view, `F7`, or the instrument editor's **FX**
tab: **+ Add effect** / **+ Device**). Devices run in order on the instrument's channel, each with a
power button (bypass, 5 ms crossfade), presets (folder icon) and its own **Mix**. Every knob is an
automation target (right-click › **Automate**, or **+ Lane › FX param**) and MIDI-learnable, and
the DSP never allocates in the audio thread: buffers are built once when a chain changes.

| Device | Since | What it does |
|---|---|---|
| Delay | 0.1.8 | tempo-synced or ms delay, ping-pong, HP/LP in the loop |
| Reverb | 0.1.8 | Freeverb-style rooms, pre-delay, width, wet-only low/high cut |
| Drive | 0.1.8 | soft / hard / fold, tone, output |
| Chorus / Flanger | 0.1.8 | modulated short delay with feedback |
| Filter | 0.1.8 | LP / HP / BP SVF with an LFO |
| Bitcrusher | 0.1.8 | bit depth and sample-rate reduction |
| Compressor | 0.1.8 | feed-forward peak compressor |
| Metal resonator, Digital distortion, Sub bass | 0.2.3.11 | the Voice Transform stages as devices ([VOICE.md](VOICE.md)) |
| **Phaser** | Unreleased | 4–12 all-pass stages swept by an LFO |

## Phaser (Unreleased)

A real phaser: a cascade of first-order **all-pass** filters whose corner frequency is swept by an
LFO, mixed with the dry signal. Where the cascade's phase reaches 180°, 540°, … the wet and dry
cancel, so an *n*-stage phaser has *n*/2 notches that sweep up and down together (the swirl of an
MXR Phase 90 or Small Stone). Feedback from the cascade's output to its input makes the peaks
between the notches resonant (positive) or hollows them out (negative).

| Knob | Range | Default | Notes |
|---|---|---|---|
| Stages | 4, 6, 8, 12 | 6 | 2, 3, 4 or 6 notches |
| Sync | Hz, 1/4, 1/2, 1 bar, 2, 4, 8 bars | Hz | one LFO cycle per note length at the song tempo; "Hz" uses Rate |
| Rate | 0.02–10 Hz | 0.35 Hz | the LFO speed when Sync = Hz |
| Depth | 0–100 % | 70 % | sweep range: the centre ± 2.5 octaves at 100 % |
| Centre | 80 Hz–5 kHz | 800 Hz | the stages' corner frequency at the middle of the sweep |
| Feedback | −95 to +95 % | +40 % | resonance |
| Stereo phase | 0–180° | 90° | how far the right channel's LFO runs ahead of the left |
| Mix | 0–100 % | 50 % | 50 % gives the deepest notches; 100 % is a pure all-pass (no notches, only phase) |

Presets: **Slow swirl**, **Disco strings (1 bar)**, **Jet 12-stage**, **Vibe (4-stage)**. Factory
patch files take `type = "phaser"` with `stages`, `rate` (`"1/4"` etc. or Hz), `depth`,
`feedback`, `mix`.

**How it works** (`src/fxmod.rs`): each stage is a zero-delay-feedback (TPT) one-pole all-pass after
Vadim Zavalishin, *The Art of VA Filter Design*, with `g = tan(π·fc/fs)` computed per sample, so the
notches land exactly where theory puts them up to Nyquist. With every stage at `fc` the notches
are at `fs/π · atan(g · tan((2k+1)·π/2n))`, k = 0 … n/2−1 (`fc · tan((2k+1)·90°/n)` in analog terms).
Unit tests check those notch positions for 4/6/8/12 stages (gain under −30 dB at each), that
100 % wet is flat, that feedback sharpens the peaks, that the LFO sweeps the notch and that the
stereo phase offsets the right channel. An engine test sweeps the Mix with an automation lane
and checks that rendering doesn't allocate.
