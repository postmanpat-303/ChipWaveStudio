# Built-in Sound Generators: research notes

Status: research only (no code changed). Written 2026-10-18 against ChipWave 0.2.3.10 (`bf58402`).
Goal: let people **make sounds from scratch inside ChipWave**, with no WAVs and no MIDI, inspired by
the instruments of other DAWs and by classic subtractive (Moog-style) synths. It should feel like a tracker:
quick to get a sound with one click, deep if you want to tweak.

All GitHub repos and licences below were checked against the GitHub API (`/repos/...`,
`/license`), the LICENSE/README files, or source file headers on 2026-10-18. Star counts are from
that date. ChipWave is GPL-3, so the following can be **borrowed from (with notices kept)**: MIT,
BSD, ISC, Unlicense, BSL-1.0, Apache-2.0, LGPL-2.1+/3, GPL-2.0-or-later and GPL-3. The following
are **not fine to copy**: GPL-2.0-only, "custom/no licence" code, and AGPL-3 (compatible through
GPL-3 §13, but it would put AGPL network terms on the combined work, so learn from it only).
**Ideas and published maths are always fine to reimplement.** We credit the source anyway, as we
did for Plaits and Pink Trombone.

---

## 1. What ChipWave already has (inventory of `src/`)

| Area | What exists | File |
|---|---|---|
| Instrument kinds | `InstrumentKind::{Synth(Patch), Sample{..}, Voice(VoicePatch)}` | `model.rs` |
| Oscillators | osc1 (Saw / Square / Pulse / Triangle / Sine) with octave, level, PW; **unison up to 8 with detune and stereo width**; optional osc2 (wave, semitones, level); sub osc; noise (white xorshift, stereo-decorrelated); **"metal" source: six squares at 808 hi-hat ratios** | `synth.rs` |
| Anti-aliasing | **PolyBLEP** on saw/square/pulse (triangle and sine are naive, which is fine) | `synth.rs::poly_blep` |
| Filter | **One** Zavalishin TPT state-variable filter (12 dB LP/HP/BP), key-track, env amount, velocity-to-cutoff, drive (post) | `synth.rs::Svf` |
| **Ladder filter** | **None.** There is no Moog ladder anywhere in `src/` (the SVF is the only voice filter; the FX `Filter` device is also SVF-based) | — |
| Envelopes | amp ADSR, filter ADSR (exponential), pitch envelope (semitones, decay), per-note decay override (08xx) | `synth.rs::Env` |
| LFO | one per patch: shape, Hz or tempo-sync, to cutoff / amp (trance gate) / pitch | `synth.rs::Lfo` |
| Performance | 303 glide/legato, accent (3C/3D), row filter commands 38–3B, start delay | `synth.rs`, `engine.rs` |
| FM / PM | **None** for the Synth (the Voice has formant FM-ish morphing only) | — |
| Wavetable | **None** | — |
| Analog drift | **None** (oscillators are perfectly stable; there's no per-voice random detune or cutoff spread) | — |
| Drum synths | No dedicated drum engine. Drums are **Synth patches**: `presets.rs` has `hat(open)` (metal + noise + HP), Shaker, Clap, Trance Kick (pitch env 44 st / 16 ms), Noise Riser | `presets.rs` |
| Voice | Plaits-inspired formant voice, consonants/words (`speech.rs`), Throat mode (Pink Trombone-style waveguide); 0.2.3.11 is adding Voice FX (metal resonators, digital distortion, sub) | `voice.rs`, `speech.rs` |
| FX chain | Insert/return devices: Delay, Reverb (with low/high cut), Drive (incl. fold), Chorus, Filter, Bitcrusher, Compressor. Master: true-peak limiter + LUFS meter | `fx.rs`, `master.rs` |
| Sample tools | Sample editor with loop, chop/slice (transient/grid) into instruments or notes | `sample.rs`, `app/sampleview.rs` |
| Instrument mgmt | `.cwi` library (save/load/audition/swap), "New…" menu (Synth init, Voice presets) | `instfile.rs`, `app/library.rs`, `app/voiceview.rs` |
| Randomness | Used in Voice babble, fill/arranger, chord helper, **not** for patch generation | — |

**Summary:** ChipWave has a solid little subtractive voice (2 osc + sub + noise + metal, PolyBLEP,
TPT SVF, 3 envelopes, 1 LFO, unison). What it lacks for "generate any sound from scratch":

1. **No Moog-style ladder filter** (24 dB, self-oscillating, saturating) and no other filter models.
2. **No third oscillator, no osc sync, no FM/PM, no ring mod**, so there are no Minimoog stacks, no DX bells and no FM basses.
3. **No analogue drift or "imperfection" layer.** Everything is perfectly static and digital-clean.
4. **No wavetable** (and no spectral/warp tricks like Vital's).
5. **No dedicated drum generators.** Kicks/snares are hand-tuned Synth patches with one pitch env and no click/body/snap controls.
6. **No one-click generator / randomiser** (sfxr-style) and **no category preset browser for built-in sounds**, only 4 built-in drum presets.
7. Only **one LFO and no general modulation routing**. Other DAWs and Vital both live on "drag a modulator onto any knob".

---

## 2. Another DAW's generators (what to learn from)

Sources: that DAW's user guide (synth, drum and modular chapters), product pages and release notes.
That DAW is closed source; we take **ideas and layouts only**.

| Device | What it is | Idea for ChipWave |
|---|---|---|
| **Two-oscillator subtractive synth** | 2 identical oscillators. Each has a *Shape* knob that crossfades pulse(+1 oct) ← saw → saw(+1 oct), PW, a sub pulse, hard sync to a tunable master (0–60 st), and 1–16 unison voices with width. **Blend operators** between osc 1/2: MIX, NEG, WIPE, AM, SIGN, MAX. Noise, filter FM, pre-HP, drive, 7-mode filter with a waveshaper, FEG/AEG | One-knob "Shape" morph, sync, and blend operators are cheap and very characterful. Easy to add to our Synth |
| **4-operator FM synth** | 4 sine ops, ratio + Hz offset each, noise with resonant LP and drive, a **5×4 modulation matrix** (any op → any op incl. self-feedback, 0–999), a separate audio mixer, AEG | Matrix FM fits a tracker's grid-like UI perfectly: a 4×4 number grid, editable like pattern cells |
| **Phase-distortion synth** | 4 oscillators doing **phase distortion** (5 algorithms, Shape, Formant, phase) **plus phase modulation** from each other; a PD/PM order arrow; global Shape/Mod XY pad; filter FM from each osc | CZ-style PD is cheap and great for chippy/acid sounds; the XY pad is a nice "macro" idea |
| **One-oscillator synth** | Pick **one oscillator, one filter, one envelope**. Osc: Sine, Triangle, Pulse, Saw, Union (pulse+saw+tri blend, drifting), Phase-1 (PD + feedback), **Swarm** (8-voice unison saw/sine), **Bite** (dual osc: exp FM, sync, PWM, ring), **Wavetable** (custom unison modes), **Scrawl** (drawable). Filters: Low-pass LD (ladder), **Low-pass MG ("inspired by Mr Moog", mix-bus saturation via Drive)**, Sallen-Key (16 configs), SVF, XP (Oberheim), Comb, Vowels, Fizz, Rasp, Ripple. Env: ADSR, AR, AD (loop), Pluck, Segments. Plus sub osc, sync-to-sub, PM from sub (0–800 %), noise, HP, *Resonance Limit* | **The best template for ChipWave's "Analog" generator**: slot-based, simple, swappable modules. Note: the guide has **no "Pulsar" or separate "Sub" module** in it; *Sub* is a front-panel sub osc, not a module |
| **Modular environment** | 180+ module modular environment (instrument, FX and note patches); the one-oscillator synth converts to a modular patch | Too big for ChipWave now. A full modular is out of scope; slot-based generators give 80 % of the value |
| **Drum families** | **v1** (ex E-Kick/E-Snare/E-Hat/E-Clap/E-Tom/E-Cowbell); **v8** (TR-808-inspired: Kick, Snare, Clap, Hat, Tom, Cowbell, Cymbal, Claves, Maracas, Rimshot); **v9** (TR-909-inspired); **v0** (original hybrids: Kick, Snare, Hat, Cymbal, Tom and **Zap Kick, "a psy-inspired kick drum"**) | See details below; these map 1:1 onto a ChipWave Drum Synth |
| v1 Kick | Slightly rectified sine, AD env (fixed attack), Click, Tone (LP), pitch mod (amount st, decay, contour) | Minimal kick |
| v0 Kick | **Tune, Decay, Punch (pitch env), Click ("initial chaos"), Shape (osc sharpness), Body (shell emphasis), Noise (+stereo)**, velocity sens | **The ideal macro set for our kick: 7 knobs** |
| v0 Zap Kick | Pitch swoops down to a tuned target (or 0 Hz), Zap depth + Zap decay, Gated option, Pitch/Freq/Phase mod inputs | Psytrance kick, perfect for Del. Our Trance Kick preset approximates it |
| v1 Hat | Noise → **comb filter** (freq, ±feedback, mix) + **FM pair "hit"** (carrier freq/decay, mod freq/amount) → HP EQ; XY pad; Width | Comb + FM hat is a great alternative to our 808 squares |
| v0 Hat | Carrier **sine bank** + modulator sine bank (spread each), Tone, Range, built-in **phaser + freq shift** | Metallic, modern hats |
| v1 Snare | 2 sines (osc 2 offset in st, decay as % of osc 1), noise AD with contour + width, osc/noise mix, HP/LP with shared Q | Classic snare model |
| v0 Snare | Tune, Decay, Noise (+width), Click, **Shell level / Ringing / Size / Spread** | Shell resonators make snares sound real |
| **Organ** | Drawbar-style organ | Additive "drawbar" mode in our Analog/Wavetable is enough |
| **Sampler (6.1)** | Slicing, Repitch/Cycles (wavetable from samples, now with Pulse/Partials characters)/Textures/**Fragments** (granular)/**Spectral** | Cycles (sample → wavetable) is a nice follow-up for our wavetable engine |
| **Modulators** | The DAW's modulation system (its user guide, modulators chapter): add LFO/ADSR/Random/Steps/Segments/Keytrack/Expressions/Macro modulators to any device, click the modulator's routing button, then **drag on any knob** to set a per-target amount (shown as a ring). 6.0 added circular modulations | The UI pattern to copy (Vital does the same, see §4) |
| **Randomize (Note FX)** | Randomizes notes, not patches | That DAW has **no global patch "dice"**, so ChipWave can do better here (sfxr-style) |

---

## 3. Analogue / Moog modelling (the "warm" part)

### 3.1 Minimoog architecture (target for the "Analog" generator)
3 VCOs (Tri, Tri-saw, Rev-saw, Saw, Square, Wide/Narrow pulse; osc 3 can be an LFO / drop key tracking)
→ mixer (3 osc + noise white/pink + external, which overloads nicely when the levels are high)
→ **24 dB/oct transistor ladder** (cutoff, emphasis to self-oscillation, contour amount, key-track)
→ VCA. Two ADS(D) contours (Decay doubles as release), glide, and a modulation mix (osc 3 / noise / filter env).
That's the "fat bass/lead" recipe: detuned saws an octave apart + drive + ladder with a filter env.

### 3.2 Ladder filter models (verified refs)
| Model | Ref | Character / cost | Use |
|---|---|---|---|
| Stilson & Smith 1996 | T. Stilson, J. O. Smith, "Analyzing the Moog VCF with considerations for digital implementation", ICMC 1996 | Linear 4×one-pole + feedback; cheap; resonance/tuning compensation tables | Fallback / "clean" |
| **Huovilainen 2004** | A. Huovilainen, "Non-linear digital implementation of the Moog ladder filter", DAFx-04 | tanh in every stage, 2× oversampled, thermal voltage; the classic "real Moog" sound; heavier | "Vintage" mode |
| Välimäki & Huovilainen 2006 | "Oscillator and filter algorithms for virtual analog synthesis", Computer Music Journal 30(2) | Simplified non-linear ladder + bandlimited oscillators | Background reading |
| **D'Angelo & Välimäki 2013** | S. D'Angelo, V. Välimäki, "An improved virtual analog model of the Moog ladder filter", ICASSP 2013 | Physically derived, better tuning at high cutoff | "Improved" mode |
| **Zavalishin TPT ladder** | V. Zavalishin, *The Art of VA Filter Design* (Native Instruments, free PDF), ch. on ladder | Zero-delay feedback: 4 TPT one-poles solved together, tanh on the feedback/input (optionally per stage with one Newton step); stable under fast modulation; **matches our existing TPT SVF style** | **Recommended core** |

**MoogLadders repo** ([ddiakopoulos/MoogLadders](https://github.com/ddiakopoulos/MoogLadders), 402★,
repo default Unlicense) collects all of these in C++. The **per-file licences differ**: Stilson,
Microtracker and Krajeski are **Unlicense**, Improved (D'Angelo) is **ISC**, RKSimulation is
**BSD**, Huovilainen is **LGPLv3**, MusicDSP is "suggested CC-BY-SA", and Simplified (DAFX) and Oberheim (Pirkle) are **custom**.
→ We can port the Unlicense/ISC/BSD ones. **Don't copy** Simplified or Oberheim (Pirkle).

Other verified ladder sources: **DaisySP** `Source/Filters/ladder.h` (MIT, ported from Teensy Audio,
Richard van Hoesel 2021, and Infrasonic Audio 2024): a modern multimode ladder, very readable. **fundsp**
`moog`, `moog_hz` (MIT OR Apache-2.0, Rust). **Vital** `ladder_filter.cpp` (GPL-3, see §4).
Rust nih-plug examples: [stephanhorvath/moog-ladder-filter](https://github.com/stephanhorvath/moog-ladder-filter) (MIT),
[gpasquero/rusted-moog](https://github.com/gpasquero/rusted-moog) (MIT, a Minimoog-inspired VA in Rust, 6★, young).

**Multimode trick (from Vital, also the classic Oberheim Xpander maths):** mix the 5 ladder taps
(input, stage1…stage4) with binomial weights to get LP/BP/HP at 12 or 24 dB from one ladder, e.g.
24 dB HP = `[1,-4,6,-4,1]`, 24 dB BP = `[0,0,-1,2,-1]`, 12 dB LP = `[0,0,1,0,0]`, plus a
continuous **LP → BP → HP "blend"** knob. That's one filter with many shapes, and it's cheap.

### 3.3 Oscillators and "analog warmth"
- **PolyBLEP** (Välimäki, Pekonen, Nam 2012, "Perceptually informed synthesis of bandlimited classical
  waveforms using integrated polynomial interpolation", JASA 131(1)). We already have it. Add a
  **PolyBLAMP** for triangle and for **hard sync** discontinuities. **MinBLEP** (E. Brandt, "Hard sync
  without aliasing", ICMC 2001) is the high-quality alternative; Surge's Classic osc uses BLIT/BLEP-style
  windowed impulses (GPL-3).
- **Drift** (what makes it "analog"): per-voice, per-oscillator slow random pitch walk (filtered noise
  or a Perlin-style smooth random, ±0–10 cents, 0.1–2 Hz), random start phase per note (or "free-running"),
  tiny per-voice cutoff spread (±3–5 %), and envelope time spread (±2 %). Surge XT's oscillator "Drift"
  parameter and the one-oscillator synth's "Union … drifting" do exactly this. Odin2 and Vital use random phase too.
- **Saturation**: tanh/asymmetric soft clip at the mixer (Minimoog "overload") and in the ladder; DC blocker after it.
- **Oversampling** 2× for the non-linear ladder (cheap halfband, as Vital does with `iir_halfband_decimator`).

---

## 4. Vital deep dive (Del's favourite)

[mtytel/vital](https://github.com/mtytel/vital): **GPL-3.0** (verified; file headers say "Copyright
2013-2019 Matt Tytel … GPL v3 or later"), C++/JUCE, 2,171★, last push 2023-05. README rules we
**must** respect: *don't use the names "Vital", "Vital Audio", "Tytel" or "Matt Tytel" for marketing or
naming*; *don't connect to vital.audio web services*; *don't redistribute the factory presets*.
Credit is fine (and required by GPL notices): "Wavetable warp ideas adapted from Vital by Matt Tytel (GPL-3)".

### 4.1 Wavetable engine
- Frames of 2048 samples, stored with their **frequency-domain** version (`lookups/wave_frame.h`,
  `wavetable.h`). Playback is mip-mapped (band-limited per octave), with **frame position** morphing.
- The **wavetable creator** (`src/common/wavetable/`) builds tables from **components with keyframes**:
  sources (`wave_source`, `wave_line_source` (draw with points/curves), `file_source` (sample → table,
  with `pitch_detector`), `shepard_tone_source`) and modifiers (`phase_modifier`, `wave_window_modifier`,
  `frequency_filter_modifier`, `slew_limit_modifier`, `wave_fold_modifier`, `wave_warp_modifier`).
  Each keyframe is interpolated across the 256-frame table. **This "keyframe + modifier stack" is
  the best idea to borrow**: a user makes a whole evolving table from 2 or 3 keyframes.
- `wave_warp_modifier`: horizontal and vertical **power-curve warps** (symmetric or asymmetric)
  via `(e^(p·x) − 1)/(e^p − 1)`. Trivial to reimplement.

### 4.2 Oscillator warp modes (from `synth_oscillator.h`)
- **Phase-distortion "Distortion" modes** (per oscillator): None, **Sync, Formant, Quantize, Bend,
  Squeeze, Pulse Width, FM ← Osc A/B/Sample, RM ← Osc A/B/Sample**.
- **Spectral Morph** (the "spectral warping" in Vital's tagline; done on the FFT frame, then re-synthesised):
  None, **Vocode, Form Scale, Harmonic Stretch (Scale), Inharmonic Scale, Smear, Random Amplitudes,
  Low Pass, High Pass, Phase Disperse, Shepard Tone, Skew**.
- **Unison stack types**: Normal, Center Drop Octave (×2), Octave (×2), Power Chord (×2), Major, Minor,
  Harmonic Series, Odd Harmonic Series, plus per-voice spread, stereo and random phase.
  The **chord/octave unison stacks** are easy and very "big" sounding. Add them to our unison.

### 4.3 Filters (`src/synthesis/filters/`, factory in `synth_filter.cpp`)
UI model → class: **Analog → `SallenKeyFilter`**, **Dirty → `DirtyFilter`**, **Ladder → `LadderFilter`**
(4 one-pole stages + tanh-saturated input/feedback, resonance 0.001–4.1, drive raises resonance,
stage-tap mixing for 12/24 dB LP/BP/HP plus Notch-Pass-Swap, Dual-Notch-Band and Band-Peak-Notch "styles" with a
continuous pass-blend), **Digital → `DigitalSvf`**, **Diode → `DiodeFilter`** (303-style),
**Formant → `FormantFilter`** (AOIE / AIUO vowel styles + a vocal-tract mode), **Comb → `CombFilter`**
(comb / +flange / −flange × low-high blend / band-spread), **Phaser → `PhaserFilter`**.
Every filter has the same front panel: cutoff, resonance, drive, **blend (LP↔BP↔HP)**, style, plus
XY "interpolate" for formant. **Port target**: the *one front panel, many models* idea and the
ladder tap-mixing. The DSP itself we write ourselves in the TPT style (the maths is textbook).
A comb filter mode is easy to add, and it's great for metallic psy plucks and hats.

### 4.4 Modulation (`interface/editor_sections/modulation_manager.cpp`, `synth_parameters.cpp`)
- **Drag-and-drop**: each source (ENV, LFO, Random, Macro, velocity, note, aftertouch…) has a
  drag handle. Drop it on any knob and a **ring** appears around the knob. Drag the ring to set
  the amount, and you see the live modulated value as a moving arc.
- Each connection has **amount (−1..1), power (curve, −10..10), bipolar, stereo (inverts R), bypass**.
  The modulation matrix tab lists every connection (64 max in the current source).
- LFOs are **drawable** shapes with sync modes Trigger, Sync, Envelope, Sustain Envelope, Loop Point,
  Loop Hold and Time/Tempo/Dotted/Triplet/Keytrack rates. **Random LFO** types: **Perlin, Sample & Hold,
  Sine Interpolate, Lorenz Attractor** (`random_lfo.h`). Perlin is perfect for "analog drift".
- **For ChipWave**: an egui knob already gives us drag; add a "mod source chip" that you drag onto a
  knob, storing `(source, target param id, amount, curve, bipolar)` in the patch. This is a medium-sized
  job, but it would make every generator far deeper. It's phase 2.

### 4.5 Random and "text to wavetable"
- **Text to wavetable** in Vital uses a speech service on vital.audio (only `authentication.*` is in
  the public source; no TTS code). **We can't and shouldn't use it.** But **ChipWave already has a
  local speech engine** (`speech.rs`, Voice words/consonants): we can render a word to audio and turn it
  into a wavetable (cut into frames, pitch-sync like Vital's `file_source`). That's "Speak to
  wavetable", fully offline and our own. Great for dark psy "talking" leads.
- Random LFO (above), random unison phase, "Random Amplitudes" spectral morph. Vital has **no patch
  randomiser button**, so a ChipWave Dice would beat it here as well.

### 4.6 UI ideas worth taking
Live **waveform/wavetable 3D view** that reacts to warp knobs; every knob shows its modulation ring;
filter response curve drawn live; "drag source onto knob"; macro knobs (4) on the main page;
envelope/LFO **graphs you drag directly** (points + curve handles); per-section on/off LEDs.
For ChipWave's tracker look: draw these in the Dark Circuit / Amiga Workbench themes (pixel-ish lines).

---

## 5. One-shot drum generators and "sound generator" UIs

### 5.1 How the classics make drums from scratch
- **Kick** = sine (or a slightly clipped/rectified sine) + **exponential pitch envelope** (start
  +24…+48 st, 5–80 ms; the "Punch"), amp decay 0.1–1.5 s, **click** (1–5 ms noise or a short HP'd
  impulse/square burst), optional **body** resonance (bandpass at 2–3× fundamental), drive/tanh, and
  a 909-style triangle-shaped attack. The **psy "zap"** kick is a long, deep pitch sweep to the tuned note,
  with the tail tuned to the song key (E for Hollow Ritual).
  Refs: Sonic Pi `sc808_bassdrum` (MIT; based on Yoshinosuke Horiuchi's free SC-808, the SuperCollider code is
  in `etc/synthdefs/designs/supercollider/sc808.scd`): pitch env `note*7 → note*1.35 → note`, decay curve −7, click level.
  Plaits/DaisySP `AnalogBassDrum` (MIT, Emilie Gillet): a resonant bridged-T style SVF pinged by a pulse with FM attack + tone LP.
  Geonkick (GPL-3): drawn amplitude and frequency envelopes per oscillator, which is the "kick designer" approach.
- **Snare** = 2 tuned sines (e.g. 180 Hz + 330 Hz, short decays) or a **pinged resonator pair (shell)**
  + **noise** through BP/HP with its own decay ("snappy") + click. Plaits `AnalogSnareDrum` uses 5 resonant modes + noise.
- **Hi-hat** = 6 detuned squares at 808 ratios (we have these) → BP 7–10 kHz → HP, short or long decay,
  closed/open choke group. Alternatives: **ring-modulated square pairs** (Plaits `HiHat` has "square noise"
  or "ring-mod" metallic sources, MIT), **noise → comb** + **FM hit** (another DAW's hi-hat device), **sine-bank FM** (v0 Hat).
- **Clap** = noise → BP (~1–1.5 kHz) with **3–4 retriggered 8–12 ms bursts** then a longer tail
  (808 style: sawtooth-shaped multi-envelope) + a little reverb.
- **Tom** = like a kick with a higher pitch, less pitch-env depth, and a bit of noise; **conga** = shorter, with a resonant pitch blip.
- **Cowbell** = 2 squares at **540 + 800 Hz** → BP → short and long decays (808). **Claves/rim** = very short pinged resonator (~2.5 kHz)
  plus a click. **Cymbal** = the 6-square metal source with a longer, filtered decay in 3 bands.
- **Zap/laser/FX** = sfxr-style: square/saw + fast pitch slide + vibrato/arp + HP/LP sweep.

### 5.2 Generator UIs worth copying
- **sfxr** (DrPetter 2007, **MIT**; [grimfang4/sfxr](https://github.com/grimfang4/sfxr), 313★) and
  its descendants: **category buttons** (Pickup/Coin, Laser/Shoot, Explosion, Powerup, Hit/Hurt, Jump,
  Blip/Select) each roll **random values inside sensible ranges for that category**, plus
  **Randomize** (fully random) and **Mutate** (small nudge around the current patch). Parameters:
  wave (square/saw/sine/noise), attack/sustain/punch/decay, start freq, min freq cutoff, slide,
  delta slide, vibrato depth/speed, arpeggio change amount/speed, square duty + duty sweep, repeat speed,
  phaser offset/sweep, LP cutoff/sweep/resonance, HP cutoff/sweep. That's **only about 22 params and pure chip**.
  [bfxr](https://github.com/increpare/bfxr) (Apache-2.0, per its readme, 1,025★) adds more waves (triangle, breaker,
  tan, whistle, pink noise, bitnoise) and **lock** toggles per parameter so Mutate leaves locked
  ones alone; [bfxr2](https://github.com/increpare/bfxr2) (MIT, 331★) is the JS remake.
  **[bzar/sfxr-rs](https://github.com/bzar/sfxr-rs) (MIT, Rust, 53★)** is already a Rust library, so it could be
  vendored almost as-is.
- **Geonkick** ([quamplex/geonkick](https://github.com/quamplex/geonkick), **GPL-3.0**, 193★; primary
  repos on Codeberg/GitLab): 16-instrument kit, **3 layers × 3 oscillators** (sine/square/tri/saw/noise/sample),
  FM osc1 → osc2, **drawn envelopes** for amplitude, frequency, filter cutoff and distortion drive, per-layer and
  global filters, and distortion types (hard clip, tan, atan, log, exp, polynomial). Exports SFZ and kits as JSON. The
  "**draw the pitch curve of the kick**" UI is the thing to copy.
- **Another DAW's macro drums**: 5–8 **macro knobs per drum** (Tune, Decay, Punch, Click, Shape, Body, Noise),
  which hides the DSP. **This is the UI level a tracker wants.**
- **Dice with sensible ranges** = define per-generator param **ranges for each category**
  (e.g. Kick: tune 40–65 Hz, punch 18–48 st, punch decay 8–60 ms, decay 0.15–0.9 s, click 0–0.6),
  roll with a seeded RNG (store the seed so a sound can be recalled), keep **locks**, offer **Mutate ±10 %**,
  and keep an **undo history of the last 16 rolls** ("go back to that one").

---

## 6. GitHub sweep: relevant open-source software

Searched with the GitHub search API: `topic:synthesizer language:rust`, `topic:drum-synth`,
`sfxr`, `kick synthesizer`, `808 supercollider`, `topic:wavetable`, `topic:fm-synthesis`,
`topic:procedural-audio`, `moog ladder filter`, `topic:modular-synth`, plus named projects.
Licence = verified via API/LICENSE/README/headers. "Port" = effort to bring the relevant idea or code to Rust.
GPL-3 OK = may we copy code into ChipWave (with notices)?

| Name | URL | Licence (verified) | Lang | ★ | Relevant to ChipWave | Port to Rust | GPL-3 OK |
|---|---|---|---|---|---|---|---|
| **Vital** | github.com/mtytel/vital | GPL-3.0 | C++ | 2171 | Wavetable creator (keyframes + modifiers), spectral morph, PD warps, ladder/diode/comb/formant filters, drag modulation | Medium–hard (JUCE + SIMD framework); port algorithms, not structure | **Yes** (no "Vital" naming, no presets) |
| **Surge XT** | github.com/surge-synthesizer/surge | GPL-3.0 | C++ | 4063 | Osc types (Classic BLIT, Modern, Wavetable, Window, Sine, FM2/FM3, String, Twist = Plaits, Alias, S&H noise), oscillator drift, huge filter set | Medium | **Yes** |
| sst-filters | github.com/surge-synthesizer/sst-filters | GPL-3.0 | C++ | 34 | Surge's filters as a standalone lib: vintage ladder, diode ladder, K35, OB-Xd, cutoff/resonance warp, tri-pole | Medium (header-only, SIMD) | **Yes** |
| sst-waveshapers | github.com/surge-synthesizer/sst-waveshapers | GPL-3.0 | C++ | 29 | Many waveshapers (soft/hard/asym/fold/fuzz/rectify) | Easy | **Yes** |
| **Odin 2** | github.com/TheWaveWarden/odin2 | GPL-3 (LICENSE text; font is OFL) | C++ | 821 | 3 osc with Analog, Wavetable, Multi, Vector, **Chiptune**, FM, PM, Noise, WaveDraw/ChipDraw/SpecDraw; LP24 ladder, SEM12, Korg35, Diode, Formant, Comb filters | Medium | **Yes** |
| **Dexed** | github.com/asb2m10/dexed | GPL-3.0 (msfa engine Apache-2.0) | C++ | 3578 | DX7 6-op FM engine (Raph Levien's msfa), 32 algorithms, DX7 sysex | Medium (msfa is compact) | **Yes** |
| Helm | github.com/mtytel/helm | GPL-3.0 (archived) | C++ | 2504 | Vital's predecessor: simple poly synth, step sequencer mod, clear code | Easy–medium | **Yes** |
| ZynAddSubFX | github.com/zynaddsubfx/zynaddsubfx | GPL-2.0-or-later | C++ | 1178 | ADDsynth / SUBsynth (bandpass-bank) / PADsynth (wavetable from harmonic profile, gorgeous pads) | Medium (PADsynth alone is easy) | **Yes** (2-or-later) |
| Yoshimi | github.com/Yoshimi/yoshimi | GPL-2.0-or-later (COPYING foreword) | C++ | 292 | Zyn fork; same engines | Medium | **Yes** |
| **Mutable Instruments eurorack** | github.com/pichenettes/eurorack | MIT (STM32 code) / GPL-3 (AVR) / CC-BY-SA (hw) | C++ | 3311 | **Plaits**: VA, waveshaping, FM, grain/formant, harmonic, wavetable, chords, speech, swarm, noise, particle, string, modal, **analog/synthetic bass drum, snare, hi-hat**; Braids; Rings (modal); Elements; Clouds | **Easy** (small, float, self-contained) | **Yes** |
| DaisySP | github.com/daisyaudio/DaisySP (was electro-smith) | MIT (some modules in `DaisySP-LGPL`, LGPL) | C++ | 1261 | Readable DSP blocks: `Drums/` (analog/synth bass drum, snare, hihat from Plaits), ladder (MIT), fm2, formantosc, vosim, oscillatorbank, PolyBLEP osc | **Easy** | **Yes** |
| VCV Rack | github.com/VCVRack/Rack | GPL-3.0-or-later (LICENSE.md) | C++ | 4429 | Modular framework (reference for a future "Grid") | Hard (framework) | Yes |
| VCV Fundamental | github.com/VCVRack/Fundamental | GPL-3.0-or-later (LICENSE.md) | C | 259 | VCO (minBLEP + sync), VCF (ladder-ish), Noise, Random, ADSR modules | Easy | **Yes** |
| Cardinal | github.com/DISTRHO/Cardinal | GPL-3.0 | C++ | 3225 | VCV Rack + many open modules in one plugin; module catalogue to learn from | Hard | Yes |
| **Bespoke Synth** | github.com/BespokeSynth/BespokeSynth | GPL-3.0 | C++ | 4742 | Live modular in the style of other DAWs' modular environments; Karplus-Strong, FM synth, drum synth, "Drum Player" | Hard (as a whole) | Yes |
| Faust libraries | github.com/grame-cncm/faustlibraries | Per-function (mostly LGPL / MIT / STK; declared before each function) | Faust | 258 | `vaeffects.lib` (moog_vcf, Korg35, diode ladder), `oscillators.lib` (PolyBLEP, BLIT), `physmodels.lib`, `pinktrombone.lib`, `dx7/` | Easy (algorithms read like maths) | **Yes, per function** (check each declare) |
| **fundsp** | github.com/SamiPerttu/fundsp | MIT OR Apache-2.0 | **Rust** | 1226 | `moog`/`moog_hz` ladder, PolyBLEP `poly_saw/square/pulse`, DSF saw/square, organ/pluck, noise colours | **Already Rust** (could depend, or read) | **Yes** |
| glicol | github.com/chaosprint/glicol | MIT | Rust | 3012 | Graph DSP nodes (osc, filters, envelopes, sequencers) | Already Rust | Yes |
| twang | github.com/FuchsiaBytes/twang | Apache-2.0 | Rust | 159 | Pure-Rust synthesis primitives | Already Rust | Yes |
| Actuate | github.com/ardura/Actuate | GPL-3.0 | **Rust (nih-plug + egui!)** | 152 | Synth/sampler/granulizer **with egui UI**: closest tech stack to ChipWave | Already Rust | **Yes** |
| niner | github.com/hyperfocusdsp/niner | GPL-3.0 | **Rust** | 39 | Monophonic analogue **kick synth**: 3 voices, comp, distortion, EQ | Already Rust | **Yes** |
| OctaSine | github.com/greatest-ape/OctaSine | AGPL-3.0 | Rust | 818 | 4-op FM synth in Rust (great FM-4 reference) | Already Rust | **Learn only** (AGPL) |
| Yazz | github.com/icsga/Yazz | MIT | Rust | 43 | Small wavetable synth in Rust | Already Rust | Yes |
| rusted-moog | github.com/gpasquero/rusted-moog | MIT | Rust | 6 | Minimoog-inspired VA + ladder in Rust (young) | Already Rust | Yes |
| moog-ladder-filter | github.com/stephanhorvath/moog-ladder-filter | MIT | Rust | 8 | nih-plug ladder example | Already Rust | Yes |
| **MoogLadders** | github.com/ddiakopoulos/MoogLadders | Unlicense repo; **per-model** (see §3.2) | C++ | 402 | 8 ladder models side-by-side + white-noise test | **Easy** | Yes for Stilson/Microtracker/Krajeski/Improved/RK; **no** for Simplified/Oberheim |
| houvilainenfilter | github.com/esluyter/houvilainenfilter | GPL-3.0 | C++ | 14 | Huovilainen non-linear ladder port | Easy | Yes |
| **sfxr** | github.com/grimfang4/sfxr | MIT (sfxr/LICENSE.txt) | C++ | 313 | The original one-click chip SFX generator (7 categories + randomize/mutate) | **Very easy** | **Yes** |
| **sfxr-rs** | github.com/bzar/sfxr-rs | MIT | **Rust** | 53 | sfxr as a Rust library | Already Rust (vendor) | **Yes** |
| bfxr | github.com/increpare/bfxr | Apache-2.0 (readme) | ActionScript | 1025 | sfxr + more waves, **param locks**, mixer | Easy | Yes |
| bfxr2 | github.com/increpare/bfxr2 | MIT | JS/HTML | 331 | Modern bfxr (also has drum/"Bfxr" synth variants) | Easy | Yes |
| jsfxr | github.com/chr15m/jsfxr | Unlicense | JS | 457 | Clean, readable sfxr port | Easy | Yes |
| sfxr-qt | github.com/agateau/sfxr-qt | MIT | C++ | 95 | Desktop sfxr UI reference | Easy | Yes |
| **Geonkick** | github.com/quamplex/geonkick | GPL-3.0 | C++ | 193 | Percussion designer: layers, drawn envelopes, FM, kit of 16, SFZ export | Medium | **Yes** |
| drumkv1 | github.com/rncbc/drumkv1 | GPL-2.0-or-later | C++ | 44 | Sample-based drum synth (less relevant: needs samples) | Medium | Yes |
| ChowKick | github.com/Chowdhury-DSP/ChowKick | BSD-3-Clause | C++ | 203 | Kick modelled on drum-machine circuits (pulse shaper + resonant filter, WDF) | Medium | **Yes** |
| **Sonic Pi** (sc808 synthdefs) | github.com/sonic-pi-net/sonic-pi | MIT (app/ code; LICENSE.md) | SuperCollider/Ruby | 12208 | `sc808.scd`: full 808 kit as readable SynthDefs (bassdrum, snare, clap, hats, toms, congas, cowbell, cymbal, claves, maracas, rimshot) | **Easy** (UGen graphs → Rust) | **Yes** (keep credit to Yoshinosuke Horiuchi's SC-808) |
| percupuff | github.com/lilyvanoekel/percupuff | GPL-3.0 | Cmajor/TS | 24 | Semi-realistic drum synth | Easy–medium | Yes |
| Vaporizer2 | github.com/VASTDynamics/Vaporizer2 | GPL-3.0 | C++ | 592 | Hybrid wavetable/additive synth with a wavetable editor | Medium | Yes |
| AKWF-FREE | github.com/KristofferKarlAxelEkstrand/AKWF-FREE | CC0-1.0 | data | 611 | 4,000+ single-cycle waveforms, public domain | Data (just include) | **Yes** (CC0) |
| OxiSynth | github.com/PolyMeilex/OxiSynth | LGPL-2.1 (Cargo.toml) | Rust | 104 | SoundFont synth (FluidSynth port); not a generator | — | Yes |
| rustysynth | github.com/sinshu/rustysynth | MIT (LICENSE.txt) | Rust | 234 | SoundFont synth; not a generator | — | Yes |
| shortcircuit-xt | github.com/surge-synthesizer/shortcircuit-xt | MIT | C++ | 567 | Sampler (zone/processor ideas) | — | Yes |
| PotatoChips | github.com/Arhythmetic-Units/PotatoChips | GPL-3.0 | C++ | 56 | VCV modules emulating classic sound chips (NES/SNES/YM/POKEY/SCC/…) | Medium | **Yes** (chip-sound ideas for an "8-bit" generator) |

Not found / not verifiable under the names given: `yotammann/sc-808`, "LB-Kick", `KottV/SfxrDroid`,
`x42/setBfree` on GitHub (setBfree lives elsewhere), HexoSynth/HexoDSP (no longer at
`WeirdConstructor/*` on GitHub). Vitalium is distributed inside DISTRHO-Ports (no licence metadata on
the repo; it inherits Vital's GPL-3); treat it like Vital. **Beware**: the topic searches also surfaced repos
pretending to be "Zebra2 / Cherry Voltage … installer for Windows". Those are **probable malware bait, ignore them**.

### 6.1 Top 10 to learn from / port (in priority order)
1. **Mutable Instruments Plaits** (MIT): drum models (analog/synth kick, snare, hi-hat) + VA/FM/wavetable/swarm/chord engines, all tiny and float. **Highest value per line of code.**
2. **sfxr / sfxr-rs** (MIT, Rust): the one-click "Chip FX" generator with categories, Randomize and Mutate. **Perfect for a tracker.**
3. **Vital** (GPL-3): wavetable keyframes + modifiers, spectral morph, PD warps, unison stacks, ladder tap-mixing, drag modulation UX.
4. **Sonic Pi sc808** (MIT): a complete, readable 808 kit recipe for our Drum Synth.
5. **DaisySP** (MIT): readable ladder + drum + PolyBLEP blocks to check our ports against.
6. **Surge XT + sst-filters** (GPL-3): drift, filter zoo, Classic osc with sync; the reference for "pro VA quality".
7. **MoogLadders** (Unlicense/ISC/BSD parts): ladder model shoot-out to pick and test our ladder.
8. **Geonkick** (GPL-3): the "draw the kick's pitch/amp curve" kick designer + kit workflow.
9. **Odin 2** (GPL-3): Chiptune/ChipDraw/WaveDraw oscillators and a filter set that matches ChipWave's spirit.
10. **Dexed/msfa** (GPL-3/Apache-2.0) + **OctaSine** (AGPL, read only): FM operator maths, envelopes and algorithms for FM-4.
Honourable mentions: **fundsp** (Rust ladder/PolyBLEP to cross-check), **Actuate** (Rust + egui synth UI), **niner** (Rust kick), **AKWF** (CC0 waveforms to seed wavetables), **ZynAddSubFX PADsynth** (pads).

### 6.2 Ideas that especially suit a tracker
- **sfxr-style "Chip FX" generator**: category buttons → instant chippy instrument; ideal for ChipWave's OctaMED roots.
- **Geonkick-style kick designer**: draw the pitch curve; tune the tail to the song key (we have key detection in the chord helper!).
- **Macro-knob drums** (as in other DAWs): 6–8 knobs per drum, quick to tweak while the pattern loops.
- **"Bake to sample"**: render any generator note to a Sample instrument (the OctaMED synth→sample habit), then chop it in the existing sample editor.
- **Pattern commands drive generator macros** (like 38–3B for the filter): e.g. a `3Fxy` "macro x = y" command per row, so drum punch/decay can change every step.
- **Seeded dice**: store the seed in the instrument, so a pattern can even re-roll per row in future (glitch hats).

---

## 7. Recommended feature set: the "Generators" family

New instrument kind: `InstrumentKind::Gen(GenPatch)` with `GenPatch { engine: GenEngine, macros: [f32; 8], seed: u64, base: Patch /* amp/filter env, LFO, FX-ish shared bits */ }`
and `enum GenEngine { Analog(AnalogP), Fm4(Fm4P), Wavetable(WtP), Drum(DrumP), ChipFx(SfxrP), Noise(NoiseP) }`.
All serde-defaulted so old songs and `.cwi` files still load. Every engine renders through the existing
`SynthVoice` envelope/filter/LFO/glide/accent path, so pattern commands 03/08/38–3D keep working.

| Engine | Contents | Macros (front page) |
|---|---|---|
| **Analog** (Moog-style) | 3 osc (Saw/Tri/Square/Pulse/Tri-saw/Sine, octave 32'–2', fine, level), osc 3 → LFO option, **hard sync 2→1**, **FM 3→1**, ring mod, noise (white/pink), **mixer overload** (tanh), **ladder filter** (TPT, 12/24 dB, LP↔BP↔HP blend, drive, self-oscillation, key-track) with Vintage (Huovilainen-style per-stage tanh, 2× OS) / Clean modes, **drift** (pitch/phase/cutoff spread, Perlin), unison stacks (Vital-style octave/chord), glide, legato | Cutoff, Resonance, Contour, Drive, Detune, Drift, Glide, Brightness |
| **FM-4** | 4 sine ops (ratio + Hz offset, level, fixed-freq option), **4×4 matrix incl. feedback** + noise column (after other DAWs' 4-operator FM devices), per-op AD(S)R, 8 algorithm presets to fill the matrix, optional PD "shape" per op (phase-distortion idea) | Index, Ratio, Feedback, Brightness, Attack, Decay, Noise, Shape |
| **Wavetable** | 64–256 frame tables, 2048 samples/frame, mip-mapped; **table builder** from keyframes: basic shapes, harmonic sliders (drawbars), draw, **AKWF CC0 waves**, **"speak to wavetable" from our Voice engine**, sample → table; modifiers: warp (power), fold, window, spectral LP/HP, smear, phase disperse, harmonic stretch; **frame position** + PD warps (sync, bend, squeeze, PW, quantize) | Position, Warp, Warp amt, Spectral, Fold, Unison, Detune, Width |
| **Drum Synth** | Kit of generators: **Kick** (v0/808/909/Zap), **Snare** (sines/shell modes + noise snappy), **Hat** (808 squares / ring-mod / comb+FM; closed/open choke), **Clap** (multi-burst noise), **Tom/Conga**, **Perc** (cowbell, claves/rim, cymbal, zap). Each is one instrument; a "Kit" helper creates 8 at once | Kick: Tune, Punch, Punch time, Decay, Click, Body, Drive, Noise; Snare: Tune, Tone, Snappy, Decay, Shell, Click…; Hat: Tune, Decay, Metal↔Noise, Tone, Width, Choke |
| **Chip FX** (sfxr) | sfxr's 22 params (wave, envelope, slide, delta slide, vibrato, arp, duty sweep, repeat, phaser, LP/HP sweeps) + bfxr extra waves; plays pitched from the pattern or as one-shots | Categories as buttons: Coin, Laser, Explosion, Powerup, Hit, Jump, Blip, **+ Zap, Riser, Downlifter** |
| **Noise / Texture** | White/pink/brown/crackle/bitnoise (LFSR, NES-style), S&H, **resonator bank** (tuned comb/modal, key-tracked), granular "dust", tape hiss; noise → ladder/comb | Colour, Density, Resonance, Tune, Spread, Movement |
| **Dice / Randomise** (all engines) | Per-engine, per-category ranges; **Dice**, **Mutate (±amount)**, per-parameter **locks**, seed stored, **16-step roll history** with ←/→, "Dice → Save to library" | Dice button + Mutate amount on every generator panel |
| **Preset browser** | Built-in factory presets per engine and category (Bass, Lead, Pad, Pluck, Acid, Kick, Snare, Hat, FX, Psy…), searchable, audition on hover/click with the current note, favourites, merges with the `.cwi` library | — |
| **Bake to sample** | Render a note (length, pitch) → new Sample instrument, which opens in the sample editor | — |

Shared upgrades (benefit the existing Synth too): **ladder filter model** as a choice in `FilterMode`,
**drift**, **unison stacks**, **comb filter mode**, **PolyBLAMP + hard sync**, and later
**drag-to-modulate** (sources: ENV1–3, LFO1–2, Random/Perlin, Velocity, Note, Macro 1–8).

### 7.1 UI sketch (egui, tracker style)

```
┌ Instruments ─────────┐┌ GENERATOR: 0A "Zap Kick"  [Drum ▾ Kick ▾]   [🎲 Dice] [Mutate ▸10%] [◀ ▶ history] [💾] [Bake ▸ sample] ┐
│ 01 Trance Kick   GEN ││ ┌ Pitch curve ───────────────────────┐  ┌ Macros ────────────────────────────────────────┐ │
│ 02 Rolling Bass  GEN ││ │\                                   │  │ (TUNE)  (PUNCH)  (P.TIME)  (DECAY)              │ │
│ 03 Acid 303      SYN ││ │ \__                                │  │  E-1     +36st    22ms      0.42s              │ │
│ 04 Metal Hat     GEN ││ │    \______________ ← drag points   │  │ (CLICK) (BODY)   (DRIVE)   (NOISE)             │ │
│ 05 Demon Voice   VOX ││ └────────────────────────────────────┘  │  0.30    0.15     0.40      0.05   [🔒 lock]    │ │
│ ...                  ││ ┌ Wave/scope ┐ ┌ Amp env ┐ ┌ Filter: LADDER 24dB ▾  cut 2.1k res 0.3 drive 0.2 ┐   │ │
│ [New… ▾]             ││ │ ∿∿∿~~~      │ │ |\___   │ │  ⌒⌒\___  (live response curve)                   │   │ │
│  Synth / Voice /     ││ └─────────────┘ └─────────┘ └──────────────────────────────────────────────────┘   │ │
│  Generator ▸ Analog  ││ Presets: [Kick ▾] Psy Zap · 909 Long · 808 Boom · Rolling Short · ★ Hollow Ritual   │ │
│   FM-4 Wavetable     ││ Advanced ▸ (expands full engine page: osc/mix/env/matrix)                          │ │
│   Drum Kit… ChipFX   │└──────────────────────────────────────────────────────────────────────────────────────┘ │
│   Noise              │  Pattern: …  C-3 0A 3F28   ← "3Fxy" set macro x to y on this row (proposal)
└──────────────────────┘
```
Analog page: 3 osc strips (wave buttons, octave 32'–2', fine, level) → Mixer (overload LED) → Ladder (big cutoff knob,
response curve) → Contours (2 ADSR graphs) → Drift/Glide. FM-4 page: 4 op strips + a **4×5 hex grid** edited like
pattern cells (arrow keys + hex typing). Wavetable page: 3D table view + keyframe strip + modifier stack list.
Chip FX page: sfxr's category buttons down the left, sliders right.

---

## 8. Effort estimate (one developer, ChipWave codebase as it is now)

| # | Piece | Work | Effort |
|---|---|---|---|
| 0 | **Foundations**: `InstrumentKind::Gen`, serde/`.cwi`/text-song I/O, "New… ▸ Generator" menu, shared generator panel shell (macros, Dice/Mutate/locks/history, Bake to sample), tests + self-test hooks | Data model, UI frame, RNG ranges infra | **3–4 days** |
| 1 | **Ladder filter** (TPT, 12/24 dB, tap-mix blend, Vintage 2× OS mode, Clean mode) as a `FilterMode` for Synth + Gen; tuning/resonance compensation; tests vs MoogLadders "Improved"/Stilson | DSP + tests | **2–3 days** |
| 2 | **Drum Synth** kit: Kick (v0/808/909/Zap), Snare, Hat (3 sources, choke), Clap, Tom/Conga, Perc (cowbell/claves/cymbal/zap) + macros + ~40 factory presets + "New kit" helper | DSP (Plaits/sc808/DaisySP recipes) + presets | **5–7 days** |
| 3 | **Chip FX (sfxr)** engine: port sfxr-rs (MIT) params into a pitched voice, categories, bfxr waves, presets | Mostly porting | **2–3 days** |
| 4 | **Analog** (3-osc Moog-style): 3 osc, sync (PolyBLAMP/BLEP), osc-3 FM/LFO, ring, pink noise, overload, drift (Perlin), unison stacks, ~30 presets | DSP + UI page | **5–6 days** |
| 5 | **FM-4**: 4 ops, matrix + feedback, per-op envs, algorithm presets, PD shape, hex-grid UI, ~30 presets | DSP + UI | **4–5 days** |
| 6 | **Wavetable**: engine (mip-mapped frames, position, PD warps), table builder (shapes, harmonics, draw, AKWF import, sample→table, **speak→table**), modifier stack (warp, fold, window, spectral LP/HP, smear, disperse, stretch), 3D view | Biggest piece | **8–12 days** |
| 7 | **Noise/Texture** engine (colours, LFSR bitnoise, resonator bank, dust) | DSP | **2–3 days** |
| 8 | **Preset browser** (categories, search, audition, favourites, merge with library) | UI | **2–3 days** |
| 9 | **Drag-to-modulate** (sources → any generator knob, rings, amount/curve/bipolar) | Engine + UI, touches everything | **5–7 days** |
| | **Total** | | **≈ 38–53 days**; first useful release (0+1+2+3) ≈ **12–17 days** |

## 9. Suggested build order (one build per batch, as before)
1. **Build A: "Generators I: Drums & Chip"** = Foundations + Ladder filter + **Drum Synth** + **Chip FX** + Dice/Mutate/locks + Bake to sample.
   Biggest, quickest win: any user can make a kit and chip SFX from nothing, and the ladder upgrades the existing Synth right away.
   Ship with a demo kit in Hollow Ritual style (Zap kick tuned to E, metal hats).
2. **Build B: "Analog"** = the 3-osc Moog-style engine with drift, sync, overload, unison stacks + presets (Moog bass, acid, leads, pads).
3. **Build C: "FM-4 + Noise"** = FM matrix engine and noise/texture engine + preset browser.
4. **Build D: "Wavetable"** = engine + builder + Vital-inspired warps + speak-to-wavetable.
5. **Build E: "Modulation"** = drag-to-modulate across all generators (and the old Synth).

Credits to add as each lands (README credits + About + THIRD_PARTY_LICENSES + source headers):
Emilie Gillet / Mutable Instruments (MIT) for drum models; DrPetter (sfxr, MIT) and bzar (sfxr-rs, MIT); Yoshinosuke Horiuchi (SC-808) via Sonic Pi (MIT); Matt Tytel (Vital, GPL-3) for wavetable/warp/filter ideas
(no "Vital" naming); Surge XT team (GPL-3); Stilson & Smith, Huovilainen, D'Angelo & Välimäki, Zavalishin papers;
ddiakopoulos/MoogLadders authors per model; Adventure Kid (AKWF, CC0) if waves are bundled; other DAWs for UI inspiration only (no code, closed source).
