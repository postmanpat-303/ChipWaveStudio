# Generators (0.2.3.12)

Generators are instruments that synthesise a sound from a few **macro knobs**
instead of playing a sample or a full synth patch. Each one can be rolled with
**Dice**, nudged with **Mutate**, locked knob by knob, stepped back through
its last 16 rolls and **baked** to a sample. Research notes:
[research/sound-generators.md](research/sound-generators.md).

Add one with **New… > Generator (drums)** in the instrument list (kits, kicks,
snares, hats, claps, toms, percussion). Chip FX (sfxr-style game sounds)
follows in 0.2.3.13.

![Drum kit generator](screenshots/dark-generators-kit.png)

## Drum Synth

Six drum kinds, each with several models and eight macros. Recipes follow the
Sonic Pi **sc808** SynthDefs (after Yoshinosuke Horiuchi's SuperCollider
TR-808) and the **Plaits** / **DaisySP** analog drum models; all are
independent Rust implementations, with no samples.

| Kind | Models | Macros |
|---|---|---|
| Kick | v0 (punchy sine), 808, 909, Zap (psy) | Tune, Punch, P.Time, Decay, Click, Body, Drive, Noise |
| Snare | 808, 909, Shell (modal) | Tune, Tone, Snappy, Decay, Color, Click, Ring, Drive |
| Hat | 808 metal, Ring mod, Comb + FM | Tune, Decay, Metal, Tone, Color, Width, Click, Drive |
| Clap | 808, Tight | Tone, Decay, Spread, Claps, Q, Room, Width, Drive |
| Tom | 808 tom, Conga | Tune, Punch, P.Time, Decay, Click, Noise, Body, Drive |
| Perc | Cowbell, Claves, Rimshot, Cymbal, Zap | Tune, Decay, Tone, Color, Ring, Click, Width, Drive |

- **Pitch:** C-4 plays the drum as tuned. With **Follow note** on (the
  default), other notes transpose it, so a kick can be tuned to the song key.
  The Psy Zap Kick is tuned to E (41.2 Hz at C-4).
- **Knobs:** drag (Shift = fine), scroll, or double-click to reset to the
  middle of the dice range. The faint arc shows the range Dice rolls in.
- **Level, Release and Filter:** an optional filter after the drum (SVF or
  the Moog ladder, LP/BP/HP, cutoff, resonance).

### Kit mode

A kit holds eight pieces (Kick, Snare, Clap, Closed hat, Open hat, Tom, Perc,
Perc 2). The note picks the piece:

| Note | Piece |
|---|---|
| C | Kick |
| C# | Perc |
| D | Snare |
| D# | Clap |
| E, F, G, A | Tom at −7, −4, 0, +4 st |
| F#, G# | Closed hat |
| A# | Open hat |
| B | Perc 2 |

The octave transposes the piece around C-4. A channel is monophonic, so a
closed hat cuts an open hat on the same channel (choke). Put pieces that
should overlap on different channels. Presets: **Psy Kit** and **808 Kit**.

## Dice

- **Dice** rolls every unlocked knob of the selected drum within sensible
  ranges. **Dice kit** rolls every piece of a kit.
- **Mutate** moves each unlocked knob by up to ±N % (the number next to it)
  around its current value.
- **lock** under a knob keeps it out of Dice and Mutate.
- Every roll stores its **seed** (shown as hex). Typing a seed re-rolls that
  exact sound.
- **< / >** step through the last **16 rolls**. Ctrl+Z undoes knob changes
  and rolls like any other edit.
- **Bake to sample** renders the sound at C-4 (48 kHz, mono, tail trimmed, up
  to 8 s) into a new Sample instrument named "<name> (baked)", with the same
  volume and FX.

## Moog ladder filter

The Synth, Voice and Generator filters can be switched from the default **SVF
12 dB** to **Ladder (clean)** or **Ladder (vintage)**, with LP / BP / HP at
24 dB (or 12 dB with the "12 dB" box).

- **Design:** a zero-delay-feedback model after Vadim Zavalishin's *The Art
  of VA Filter Design* (TPT one-poles, solved feedback). Resonance
  self-oscillates near the top of the range and stays bounded.
- **Passband:** compensated as in DaisySP, so turning resonance up doesn't
  thin the bass as much.
- **Vintage:** saturates the input and every stage (after Huovilainen and
  Välimäki & Huovilainen 2006) and runs at 2x oversampling through a
  halfband filter (about 0.5 ms latency).
- **Tests:** the filter is checked against the analytic 4-pole response and
  cross-checked against ports of the DaisySP and fundsp ladders (and the
  MoogLadders collection).

Old songs keep the SVF and sound the same.

## Pattern effect 43xy

`43xy` sets generator macro **x** (1–8, in knob order) to **y** (0–F = 0–100 %)
on that channel. `4300` returns every macro to the instrument's value. The
values stay on the channel until another instrument plays on it. Example: on
a kick, `4320` removes the punch, `432F` gives full punch, and `4340` sets
the shortest decay (macro 2 = Punch, macro 4 = Decay).

## Text songs and .cwi

- **Text songs:** `patch = "gen:Psy Kit"` loads a generator preset. An
  `[instrument.gen]` table overrides fields of it, merged key by key.
- **Filter model:** any synth patch accepts `filter_model = "svf" | "ladder" |
  "vintage"` and `slope = 12 | 24`.
- **Instrument files:** generators save to `.cwi` and appear in the
  instrument library under "generator".

## Limitations

- Kit choke only works within one channel, and a channel plays one piece at
  a time.
- Bake renders C-4 only, mono, at 48 kHz.
- The vintage ladder adds about 0.5 ms of latency, which isn't compensated.
- Perc and hat width come from decorrelated noise, not true stereo models.
- Songs using generators or the ladder filter don't open in ChipWave
  versions before 0.2.3.12.

## Credits

- **Vadim Zavalishin**, *The Art of VA Filter Design*; **A. Huovilainen**,
  DAFx-04; **V. Välimäki & A. Huovilainen**, CMJ 30(2), 2006.
- **DaisySP** ladder by Richard van Hoesel and Infrasonic Audio (MIT).
- **fundsp** by Sami Perttu (MIT / Apache-2.0).
- **MoogLadders** by Dimitri Diakopoulos.
- **Sonic Pi** sc808 by Sam Aaron and contributors (MIT), after Yoshinosuke
  Horiuchi's SC-808.
- **Plaits** by Emilie Gillet (MIT), via DaisySP by Electrosmith (MIT).
- **Bitwig Studio**, for the macro and dice idea (no code or assets).

The licence notices are in `THIRD_PARTY_LICENSES.txt`.
