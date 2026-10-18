# Voice Glitch Generator — research notes

Status: research only (no code changed). Written 2026-10-18.
Goal: a synthetic "voice" instrument for ChipWave that makes *ahh / eee / ooh / err*
vowel chops and stutters for dark psytrance. No recorded voice and no samples:
everything is generated from a glottal-style source and formant filters.

All repos and licences below were checked against the GitHub API, crates.io, or the
source file headers on 2026-10-18. ChipWave is GPL-3, so MIT, BSD, Apache-2.0,
LGPL-2.1 and GPL-3(-or-later) are all fine to borrow from, as long as we keep the
notices. GPL-2-only and "no licence / abandonware" code is **not** fine to copy.

---

## 1. Candidates

| # | Project | URL | Licence (verified) | Lang | Vowel quality | CPU | Rust port | Rust crate? |
|---|---------|-----|--------------------|------|---------------|-----|-----------|-------------|
| 1 | **Mutable Instruments Plaits: speech engine** (`plaits/dsp/speech/`: `naive_speech_synth`, `sam_speech_synth`, `lpc_speech_synth`) | https://github.com/pichenettes/eurorack/tree/master/plaits/dsp/speech | **MIT** (STM32 code; file headers are MIT, Copyright Emilie Gillet). Repo has no SPDX id because AVR code is GPL-3 and the hardware is CC-BY-SA | C++ | Very good for *musical* vowels. The naive synth is a pulse plus 5 zero-delay SVF formants with a 5-phoneme by 5-register (bass to soprano) morph table, so it is exactly "ahh-eee-ooh". The SAM-style engine is formant sines with a pulse reset, very chippy/robotic. LPC10 gives "Speak & Spell" words | Very low (made for a Cortex-M4 at 48 kHz) | **Easy.** Small, self-contained files; we already have a TPT SVF | No (Surge XT's GPL-3 "Twist" oscillator is a Plaits port) |
| 2 | **Braids VOWL / VOSIM / VOWL_FOF** (`braids/digital_oscillator.cc`: `RenderVosim`, `RenderVowel`, `RenderVowelFof`) | https://github.com/pichenettes/eurorack/tree/master/braids | **MIT** (STM32) | C++ (fixed-point) | VOSIM is buzzy and "8-bit talky". VOWL gives classic chip vowels plus a few consonants. FOF is smoother | Tiny | Easy, but it's 16-bit fixed-point, so rewrite it in f32 | No |
| 3 | **Klatt cascade/parallel formant synth: klattsch-rs** | https://github.com/tgies/klattsch-rs (crate `klattsch-core` 0.1.2) | **MIT** | Rust | The most "speech-like": Klatt 1980 parallel formants, real-time-safe `FormantSynth::process`, phoneme strings ("HH AH L OW") | Low to medium (about 6 resonators plus noise) | **Already Rust.** Deps are only `rtrb`, `serde`, `serde_json` | **Yes** (`klattsch-core`, `klattsch-text`). Young (13★, 2026) |
| 3b | KlattSyn (reference Klatt in TypeScript) | https://github.com/chdh/klatt-syn | **MIT** | TypeScript | Excellent reference: Klatt 1980/1990, documented, cites all the classic sources | Low to medium | Easy to read and port | — |
| 3c | klatt (Rust) | https://github.com/TTWNO/klatt (crate `klatt` 0.0.0) | Apache-2.0 (repo), crate says MIT OR Apache-2.0 | Rust | Early stub (v0.0.0, 3★) | — | — | Not usable yet |
| 4 | **Pink Trombone** (Neil Thapen, 2017): 1D waveguide vocal tract + LF-style glottis | Original: https://dood.al/pinktrombone/ (MIT notice in page source). Rust: https://github.com/lostmsu/pink-trombone (crate `pink-trombone` 0.2.1) | Original **MIT**; Rust port **MIT**. (Fork zakaton/Pink-Trombone is GPL-3) | JS / Rust | The most *organic*: real "aaah → oooh" throat movements, gurgles and tongue glitches. Very creepy at low pitch, which suits dark psy. Hard to steer to exact vowels from pattern data | Medium (44-section waveguide at 2× rate; about 1 voice per core-percent, fine for 8 channels) | **Already Rust** | **Yes** (`pink-trombone`, MIT, last update 2021) |
| 5 | **Vital formant filter / "vocal tract"** (`src/synthesis/filters/formant_filter.*`, `formant_manager.*`, `modules/formant_module.*`) | https://github.com/mtytel/vital | **GPL-3.0** | C++ | Great sound-design filter: X/Y vowel morph, spread, transpose, resonance; AOIE-style pads | Low (SIMD) | Medium (tied to Vital's framework); copy the *idea* and tables | No |
| 6 | **Csound `fof` / `fof2` (FOF, Rodet's CHANT)** + Csound's published formant tables (bass/tenor/countertenor/alto/soprano for a e i o u) | https://github.com/csound/csound | **LGPL-2.1** (GPL-3 compatible) | C | Lush, choir-like vowels; great "aaah" pads | Medium (grain overlap at high pitch) | Medium | No |
| 7 | **Surge XT**: Twist osc (Plaits), sst-filters | https://github.com/surge-synthesizer/surge, https://github.com/surge-synthesizer/sst-filters | **GPL-3.0** | C++ | Same as Plaits; sst-filters has no dedicated vowel filter | Low | Use Plaits directly instead | No |
| 8 | **eSpeak NG** (formant tables + its own Klatt module `klatt.c`) | https://github.com/espeak-ng/espeak-ng | **GPL-3.0-or-later** | C | Speech-intelligible, but robotic. Useful as a **data source** for formant frequencies of many vowels and phonemes | Low | Data only | — |
| 9 | Talkie (TMS5220 LPC, "Speak & Spell") | https://github.com/ArminJo/Talkie (**GPL-3.0**); original https://github.com/going-digital/Talkie (header says "GPLv2", **avoid copying**) | GPL-3 / GPL-2 | C++ | Lo-fi word playback from LPC frames. Not a vowel *generator* without encoded data | Tiny | Easy | No (Plaits has an MIT LPC10 engine instead) |
| 10 | SAM (Software Automatic Mouth) | https://github.com/s-macke/SAM, https://github.com/discordier/sam | **No licence: "abandonware"** (both READMEs) | C / JS | Iconic C64 voice | Tiny | — | **Do not use.** Plaits' `sam_speech_synth` is a clean MIT re-creation of the *style* |
| 11 | Soundpipe (has `fof`, `vosim`, formant ops) | https://github.com/PaulBatchelor/Soundpipe | **MIT** | C | Good, small reference DSP | Low | Easy | No |
| 12 | Other Rust crates seen | `svara` (GPL-3.0-only, 0★ repo MacCracken/svara), `phoneme-synth` (MIT/Apache, GitLab), `dirtydata-dsp-vocal` (MIT), `fundsp` (MIT/Apache: has resonator/SVF building blocks, no vowel engine) | — | Rust | Not evaluated by ear; all young or low-usage | — | — | Not recommended as a dependency |

### Literature (algorithms, not code)
- **Klatt (1980)**, "Software for a cascade/parallel formant synthesizer", JASA 67(3). Parallel formants suit vowel morphing; cascade sounds more natural.
- **Glottal sources:** Rosenberg (1971) polynomial pulse (cheap, good enough); **LF model**, Fant–Liljencrants–Lin (1985) (better "breathy to pressed" control via one Rd parameter); Klatt KLGLOTT88. Pink Trombone uses an LF-style glottis plus aspiration noise.
- **VOSIM:** Kaegi & Tempelaars (1978). Sin² pulse trains, very cheap, buzzy "chip voice".
- **FOF:** Rodet (1984), CHANT. Each formant is a train of decaying-sine grains.
- **Vowel data:** Peterson & Barney (1952) F1/F2/F3 for American English; Csound manual "Formant values" appendix (F1–F5, amplitude and bandwidth for 5 voice types × a e i o u).
- **Glitch / stutter:** buffer-repeat (beat-repeat) with a shrinking window, retrigger with pitch step, granular "chop" (Hann-windowed grains, random position/pitch), tape-stop, sample-and-hold vowel jumps.

---

## 2. Recommendation for ChipWave

**Build our own small engine in Rust, modelled on Plaits' naive speech synth (MIT)**,
with the Klatt/Csound formant data and a Rosenberg/LF-lite glottal pulse. Don't add a
dependency. Keep `pink-trombone` (MIT, Rust) as an optional later "Throat" mode
for the creepy organic glitches.

Why: it's the cheapest way to get *musical* vowel chops that follow a note
pitch, morph smoothly and can be driven per row. It reuses ChipWave's existing TPT SVF,
envelopes, LFO and FX chain, and every borrowed piece is MIT, so the GPL-3 project stays clean.
Klatt-style intelligible speech isn't the goal. "Ahh/eee/ooh/err" chops are.

### 2.1 Signal flow (one voice)

```
 note pitch ─► Glottal source ──┐                   ┌─► F1 BP ─┐
 (+jitter,     (Rosenberg/LF-   │                   ├─► F2 BP ─┤
  vibrato)      lite pulse, Rd) ├─► mix ─► tilt LP ─┼─► F3 BP ─┼─► sum ─► glitch layer ─► existing
 aspiration ─► noise (breath) ──┘                   ├─► F4 BP ─┤        (stutter/chop/    filter/amp env
                                                    └─► F5 BP ─┘         S&H vowel)       ─► insert FX
           vowel position (0..N-1, fractional = morph) ─► formant table interp (F, BW, gain)
```

- **Source:** Rosenberg pulse with open-quotient (`OQ`) and spectral-tilt control;
  optional "Rd" macro (breathy ↔ pressed) that sets OQ, tilt and aspiration together.
  Adds jitter/shimmer for realism. Alternative sources: saw, pulse (chip), noise (whisper).
- **Formants:** 5 parallel band-pass SVFs (already in `synth.rs`; constant-peak-gain BP),
  per-formant freq/bandwidth/gain interpolated from a vowel table. Smooth coefficient
  changes (one-pole on F and BW) to avoid zipper noise during morphs.
- **Tables:** 5 voice registers (bass, tenor, alto, soprano, "demon" = bass with formants scaled ×0.8)
  × vowels **A, E, I, O, U, ER (ɜ), AE** (+ "OO/ʊ" optional). Values from the
  Csound appendix / Peterson & Barney. Example (bass, F1–F5 Hz):

  | Vowel | F1 | F2 | F3 | F4 | F5 | gains dB (F1..F5) |
  |---|---|---|---|---|---|---|
  | A  | 600 | 1040 | 2250 | 2450 | 2750 | 0, −7, −9, −9, −20 |
  | E  | 400 | 1620 | 2400 | 2800 | 3100 | 0, −12, −9, −12, −18 |
  | I  | 250 | 1750 | 2600 | 3050 | 3340 | 0, −30, −16, −22, −28 |
  | O  | 400 |  750 | 2400 | 2600 | 2900 | 0, −11, −21, −20, −40 |
  | U  | 350 |  600 | 2400 | 2675 | 2950 | 0, −20, −32, −28, −36 |
  | ER | 490 | 1350 | 1690 | 2900 | 3300 | (P&B male; low F3 gives the "err") |

  Bandwidths are roughly 40–130 Hz (F1 to F5). Store them as plain data in `src/voice_tables.rs`.
- **Formant shift / gender:** multiply all F by `shift` (0.5–2.0) independent of pitch.
  Low pitch plus a slightly low shift gives the dark psy "demon" voice.

### 2.2 Glitch layer (per voice, tempo-synced)

| Mode | What it does |
|---|---|
| **Stutter** | Captures the last *L* (1/64–1/4 beat) of output and repeats it *n* times, optionally shrinking the window each repeat (beat-repeat roll) |
| **Chop/gate** | Tempo-synced gate pattern with short fades (no clicks) |
| **Vowel S&H** | Jumps to a random or sequenced vowel every *step* (the "ah-ee-oh-er" babble) |
| **Pitch jump** | Per repeat, steps pitch by ±semitones (from a scale or random) |
| **Granular** | Short Hann grains from a 500 ms ring buffer, random position, pitch and reverse (scatter) |
| **Tape-stop** | Pitch and time ramp down at note end |

The buffers are preallocated (2 s mono per voice), so nothing allocates in the audio thread.

### 2.3 New instrument type

```rust
// model.rs
pub enum InstrumentKind {
    Synth(Patch),
    Sample { .. },
    Voice(VoicePatch),       // new
}

pub struct VoicePatch {
    pub register: Register,          // Bass/Tenor/Alto/Soprano/Demon
    pub source: VoiceSource,         // Glottal/Saw/Pulse/Noise
    pub breath: f32,                 // aspiration 0..1
    pub rd: f32,                     // breathy..pressed
    pub jitter: f32,
    pub vowel_seq: Vec<Vowel>,       // e.g. [A, E, O, ER] (up to 8)
    pub vowel_pos: f32,              // start position in seq (morph)
    pub morph_time: f32,             // glide between vowels, ms
    pub morph_rate: f32,             // auto-step speed (0 = off; sync to beat via lfo.sync)
    pub formant_shift: f32,          // 0.5..2.0
    pub resonance: f32,              // BW scale (narrow = whistly)
    pub glitch: GlitchParams,        // mode, len (beat frac), repeats, shrink, pitch_step, prob, mix
    // Reused from Patch: amp/filter/pitch envs, LFO (new target: vowel), filter, unison, volume.
    pub common: PatchCommon,
}
```

To keep the change small, `Voice` reuses the existing envelopes, LFO (add an LFO target
`Vowel` / `FormantShift`), post filter and insert FX. It's serialised like `Synth`, with
`#[serde(default)]`, so old songs load unchanged.

### 2.4 Pattern effect commands (proposal; check `docs/OCTAMED_SPEC.md` for clashes)

Currently used: 00–0D, 0F, 11, 12, 15, 16, 18, 19, 1A, 1B, 1D, 1E, 1F, 2E. The proposal uses the free **3x** block:

| Cmd | Meaning |
|---|---|
| `30xy` | Set vowel: x = vowel index (0=A 1=E 2=I 3=O 4=U 5=ER 6=AE), y = morph speed (0 = jump) |
| `31xx` | Vowel position in sequence (00–FF over the whole seq; fractional morph) |
| `32xy` | Vowel slide: x = up speed, y = down speed through the sequence |
| `33xx` | Formant shift (80 = ×1.0, 00 = ×0.5, FF = ×2.0) |
| `34xy` | Stutter: x = length (1=1/64 … 6=1/2 beat), y = repeats (0 = off) |
| `35xy` | Glitch pitch step per repeat: x = up, y = down semitones |
| `36xx` | Breath/aspiration amount |
| `37xy` | Vowel S&H babble: x = rate (ticks), y = randomness |

A row like `C-3 05 3000` then `--- -- 3053` gives "ahh → err" with a quick morph on one note.
`34 42` stutters it.

### 2.5 UI
In the instrument editor there would be a "Voice" panel. It would have a vowel strip (clickable A E I O U ER AE chips to build the
sequence), an X/Y pad (position × formant shift) and a register drop-down. The glitch
section would be a small mode selector plus knobs. It would ship with presets: "Ritual Choir" (Demon/A-O), "Glitch Babble"
(S&H), "Chip Talk" (pulse, narrow formants), "Whisper" (noise source) and "Err Stab".

### 2.6 CPU
Per voice there would be a pulse source, 5 SVFs, noise and the glitch ring buffer. That's about 40–60 flops per sample, roughly the cost of
one current synth voice with unison off. Eight channels are trivially fine. The optional Pink Trombone mode is
about 10× heavier, so limit it to 1–2 voices.

---

## 3. Effort estimate (rough)

| Step | Work |
|---|---|
| Voice core: glottal source, formant bank, tables, morph, `InstrumentKind::Voice`, serde | 1.5–2 days |
| Effect commands 30–37 + LFO targets + tests | 1 day |
| Glitch layer (stutter, chop, S&H, pitch jump; granular last) | 1.5–2 days |
| Instrument editor UI + presets + docs | 1–1.5 days |
| Tuning by ear with Del (demo usage in Hollow Ritual) | 0.5–1 day |
| **Total** | **about 5–7 working days** (roughly 3 days for a first "vowels + stutter" cut) |
| Optional later: "Throat" mode via `pink-trombone` crate (MIT) | +1–2 days |

## 4. Licence notes / attribution to add if code is borrowed
- Plaits/Braids (MIT, © Emilie Gillet): keep the MIT notice in the ported file and add it to THIRD_PARTY_LICENSES.
- Formant tables (Csound appendix, Peterson & Barney) are factual measurement data. Cite them in a comment.
- `pink-trombone` crate (MIT; original © 2017 Neil Thapen): `cargo deny` will pass (MIT).
- Avoid: SAM sources (no licence), going-digital/Talkie (GPL-2 wording), `svara` (GPL-3.0-only is fine
  licence-wise, but immature).
