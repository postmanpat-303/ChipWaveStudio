# Starter Sounds: factory preset pack (psytrance + alien)

A factory pack for the built-in synth ([SOUND_DESIGN.md](SOUND_DESIGN.md)), tuned to Del's taste: **psytrance** and **alien** sci-fi sounds. Every preset is a plain TOML patch in the `.cwpatch` format ([SOUND_DESIGN.md §8](SOUND_DESIGN.md), [FILE_FORMAT.md](FILE_FORMAT.md)). They're stored as `.toml` here so GitHub previews them; the loader accepts both extensions.

Files: [`presets/factory/psytrance/`](../presets/factory/psytrance) and [`presets/factory/alien/`](../presets/factory/alien).

These are **starting values on paper**. They'll be tuned by ear once the engine runs, and golden renders will lock them down so later DSP changes don't silently alter them.

A few keys these presets use go slightly beyond the §8 example; they're all parameters that already exist in SOUND_DESIGN.md §2–§6: `env_amount`, `hold_ms`, `curve`, `loop`, `phase`/`start_phase`, `fm_amount`, `ring_mix`, `voice.mode`/`glide_ms`, and synced times like `"8 bars"`. `note = …` fields are comments for humans, kept by the loader as unknown keys.

## Psytrance (145 BPM)

| Preset | Osc | Filter | Envelopes | LFO / mod | FX |
|---|---|---|---|---|---|
| **Rolling Offbeat Bass** | Saw −1 oct, phase reset; sine sub −1 | Ladder LP24, 180 Hz, res 0.28, drive 0.35, key track 80 %, env +0.55 | Amp 0/95 ms/0 sus/25 ms (exp); Filter 0/70 ms/0 | Velocity → cutoff 0.2; Macro 1 "Brightness" → cutoff | Soft dist 0.15 → Comp 4:1, −14 dB, 1/60 ms |
| **Punchy Psy Kick** | Sine −1 oct, start phase 90°; white noise 0.25 click | SVF LP 6 kHz | Amp hold 40 + decay 210 ms; Mod env 45 ms → pitch (+~36 st falling); Filter env 6 ms → noise level | Mono retrigger | Tube dist 0.3 → EQ (+2 dB 60 Hz, −4 dB 300 Hz) → Comp 6:1 |
| **Acid Squelch** | Saw | Diode LP24, 520 Hz, res 0.82, drive 0.45, dirty | Amp 1/300/0.7/30; Filter 0/180 ms/0 (exp), env +0.6 | **Accent:** velocity → env amount 0.4 and res 0.1. **Slide:** mono legato glide 60 ms (or 03 porta); Macro 1 "Squelch" → cutoff | Foldback 0.35 → ping-pong delay 3/16, fb 0.4, HP 300 Hz |
| **Trance Gate Pad** | Saw ×6 unison, detune 22 ct, spread 0.8; square +1 oct PW 35 % | SVF LP 2.8 kHz, res 0.2 | Amp 350/500/0.85/900 | **Gate:** square LFO 1/16 → amp level −0.9 (Macro 1 = depth); sine 2 bars → cutoff | Chorus → Reverb 0.8, 4 s |
| **Supersaw Lead** | Saw ×7, detune 28 ct, spread 0.9; saw +1 oct 0.25 | SVF LP 7.5 kHz, env +0.2 | Amp 5/400/0.8/250 | Delayed vibrato sine 5.5 Hz (fade 300 ms) → fine; mod wheel → depth | Ping-pong delay 1/8 → Reverb 2.5 s |
| **Psy Pluck** | Square PW 30 %; saw +12 st +7 ct | Sallen-Key LP2, 900 Hz, res 0.55, env +0.7 | Amp 0/160/0; Filter 0/110/0 | Macro 1 "Length" → both decays | Delay 3/16 fb 0.45 → Reverb 1.8 s |
| **Noise Riser 8 bars** | White noise 0.8 + saw 0.3 | SVF HP 150 Hz, res 0.45 | Amp attack 8 bars; Mod env attack 8 bars (exp) | Mod env → cutoff 0.9 and saw pitch (+24 st) | Phaser 8 stages, 1/4 → Reverb 5 s |
| **Downlifter** | Saw ×3 + pink noise | Ladder LP12, 12 kHz | Amp and mod decay 2 bars | Mod env → cutoff and pitch (falling) | Reverb 6 s |
| **Psy Zap** | Square +2 oct | SVF BP 3 kHz, res 0.6, key 100 % | Amp 0/120; Mod env 80 ms → pitch 0.8 | Random-per-note → cutoff | Ping-pong delay 1/16 |
| **Noise Sweep 1 bar** | White noise | SVF BP 1.2 kHz, res 0.7 | Amp 40/–/1.0/400 | Triangle LFO 1 bar → cutoff 0.85 | Chorus → Reverb 3 s |

**Rolling bass tip:** in the tracker, put the kick on every beat (rows 00, 04, 08…) and the bass on rows 01, 02, 03 of each beat with the same note. The short amp decay and key-tracked filter give the classic "rolling" feel. The template below already has this pattern.

## Alien

| Preset | Osc | Filter | Envelopes | LFO / mod | FX |
|---|---|---|---|---|---|
| **Flutter Warble** | Triangle + Amiga sine32 +7 st | SVF LP 2.2 kHz, res 0.5 | Amp 20/300/0.8/400 | **S&H** 14 Hz → pitch (~±1 st); smoothed random 3 Hz → cutoff; Macro 1 → S&H rate | Chorus 1.2 Hz → Delay 1/8 |
| **Resonant HP/LP Sweep** | Saw ×4 + square −1 oct PW 20 % | F1 Sallen-Key HP2 300 Hz res 0.75 dirty → F2 Ladder LP24 3 kHz res 0.6 (serial) | Amp 600/–/1.0/1500 | Sine 2 bars → F1 +0.6 and F2 −0.6 (opposite sweep) | Phaser → Reverb 5 s |
| **Comb Talker** | Saw + pink noise | F1 **Comb** + 440 Hz (key-tracked), fb 0.85, damping 0.4 → F2 SVF BP 1.1 kHz | Amp 10/200/0.7/200 | **Tracker-table LFO** 1/16 (0,40,10,70,25,90,5,55) → BP cutoff = "talking" steps; triangle 0.7 Hz → comb | Soft dist → Delay 3/16. Swap F2 for the **Formant** model when phase 2 adds it |
| **Ring-Mod Chatter** | Sine with FM 0.35 + ring mix 0.6; Osc 2 square +11 st +37 ct (non-harmonic) | SVF BP 1.8 kHz | **Looping** amp env 2/150/0.5 (rhythmic chatter) | S&H 1/32 → FM amount and Osc 2 pitch | Bitcrush 8-bit → Ping-pong delay 1/16 |
| **Shimmer Cloud** | Wavetable "glass" ×8 unison + sine +1 oct | SVF HP 250 Hz | Amp 1.2 s/–/1.0/4 s | Per-voice smoothed random 9 Hz → fine (micro jitter); random-per-note and 4-bar triangle → WT position | Chorus 4 voices → Ping-pong delay 1/4 fb 0.55 → Reverb 9 s, mix 0.55 |
| **Sci-Fi Zap** | Sine +1 oct, FM 0.6 from Osc 2 (+19 st) | SVF LP 9 kHz | Amp 0/220; Mod env 15/150 (exp) | Mod env → pitch −0.7 and FM +0.4; random-per-note → pan | Ping-pong delay 1/16 fb 0.6 → Reverb |

**Honest limits:** the synth has no granular engine and no ring-mod FX device. *Shimmer Cloud* imitates granular texture with unison, jitter and wavetable motion, and the ring sound uses the oscillator's ring-mod mix ([SOUND_DESIGN.md §2](SOUND_DESIGN.md)). A real granular oscillator could be a later addition. The formant filter is phase 2, so *Comb Talker* uses comb + band-pass until then.

## Psytrance quick-start song template
Available in File → New from template and in [AUTO_ARRANGE.md](AUTO_ARRANGE.md) as style **"Psytrance (145)"**.
- **145 BPM**, 4/4, 16 rows per bar (lines per beat 4), speed 6.
- **Channels:** 1 Kick (Punchy Psy Kick), 2 Bass (Rolling Offbeat Bass), 3 Hats (factory sample), 4 Acid (Acid Squelch), 5 Lead (Supersaw Lead), 6 Pad (Trance Gate Pad), 7 FX (Riser / Zap / Noise Sweep), 8 Alien (Flutter Warble).
- **Starter blocks, already tagged with roles:**
  - Intro: kick + hats, filtered bass via Macro 1.
  - Verse: kick + rolling bass + acid.
  - Breakdown: pad + alien, no kick.
  - Build: riser + snare roll.
  - Drop (tagged Chorus): everything, plus the supersaw.
  - Outro.
- **Bass pattern:** E-1 on rows 01, 02, 03 of each beat (E is a common psy key; easy to transpose).
- **Default target length** 7:00, snapped to 16 bars. Psytrance tracks are long and DJ-friendly, with 32-bar intro and outro for mixing.
- **Auto Arrange style rules:**
  ```toml
  # arrange/psytrance.toml
  [grammar]
  start = ["Intro"]
  next.Intro     = [["Verse", 1.0]]
  next.Verse     = [["Breakdown", 0.5], ["Verse", 0.5]]
  next.Breakdown = [["Fill", 1.0]]       # Fill = build/riser block
  next.Fill      = [["Chorus", 1.0]]     # Chorus = drop
  next.Chorus    = [["Verse", 0.6], ["Outro", 0.4]]
  end = ["Outro"]
  min_bars = { Intro = 32, Outro = 32, Fill = 8 }
  max_repeat = 4
  favour = ["filter_sweep", "density", "mute_channels", "echo", "drum_fill"]
  energy = [[0.0,0.3],[0.15,0.6],[0.35,0.2],[0.45,1.0],[0.7,0.4],[0.8,1.0],[1.0,0.3]]
  ```
