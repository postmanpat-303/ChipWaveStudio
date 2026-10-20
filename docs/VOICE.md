# Voice (glitch) instrument (0.2.3.6 – 0.2.3.11)

![Voice (glitch) panel](screenshots/dark-voice.png)

What this page covers:
- **Vowels, babble, chop and stutter** (0.2.3.6, effects `30`–`37`).
- **Spoken words** (`3E`) and **Throat** mode (0.2.3.10).
- The **Transform** stages: metallic resonance, digital distortion and sub (`3F`–`42`, 0.2.3.11). They are also available as the chrome FX devices **Metal resonator**, **Digital distortion** and **Sub bass** for any instrument.

All the pattern commands are listed in [TRACKER.md](TRACKER.md#5-effect-commands).

A synthetic voice for dark psytrance chops: **"ahh, eee, ohh, err"** vowel
glitches, babble and stutters. No recorded voice and no samples. Everything
is generated:

```
note pitch -> glottal pulse (Rosenberg, tension, jitter) --+
breath noise (follows the pulse) --------------------------+--> 5 parallel formant
                                                               band-passes (TPT SVF)
vowel sequence / babble / LFO / 30-32 -> formant table morph --^         |
                                                                         v
                         chop gate -> amp env / filter / drive (synth patch) -> stutter (33/34) -> FX chain
```

## Making one
- Use **New… > Voice (glitch) >** a preset (under the instrument list), or
  right-click an instrument and pick **New instrument > Voice (glitch)**.
- The **Voice** panel has these controls:
  - **Add** chips (A E I O U ER AE) add vowels to the sequence (up to 8).
    Click a chip in **Seq** to remove it.
  - **Register:** Bass, Tenor, Alto, Soprano or **Demon** (the bass formants scaled ×0.78).
  - **Source:** Glottal (the most voice-like), Saw, Pulse (robot/chip; uses Pulse width) or Whisper (noise).
  - **Step:** tempo-synced, how often to move to the next vowel. **Morph** is the glide time, and **Babble** is the chance of a random jump.
  - **Formant shift** (0.5–2×, independent of pitch), **Sharpness**, **Breath**, **Tension** and **Jitter**.
  - **LFO > vowel / LFO > shift:** the patch LFO (below the panel) sweeps the vowel or the throat.
  - **Stutter** (on every note: slice length, repeats, pitch step per repeat) and **Chop** (a tempo-synced gate).
- Below it are the usual envelope, filter, drive and LFO settings, plus the
  instrument FX chain (try Reverb + Delay + Chorus).

### Presets

| Preset | Sound |
|---|---|
| Demon Ahh | Demon register, a slow A > O > U morph via the LFO, breathy and driven |
| Choir Eee | soprano E > I > E > AE pad voice with vibrato (add reverb) |
| Glitch Babble | random vowel jumps every 1/16 with a 1/32 chop: alien chatter |
| Robot Err | pulse source, narrow formants, ER > A |
| Stutter Ohh | every note repeats its first 1/32 slice 3×, stepping down |
| Whisper Ooh | noise-only U > O with an LFO formant shift: a ghost voice |

## Effect commands 30–37
The 3x range is unused by OctaMED (see [OCTAMED_SPEC.md §5](OCTAMED_SPEC.md)). ChipWave uses `30`–`37` for the voice, `38`–`3D` for filter and acid commands, `3E` for words (below), `3F`–`42` for the Transform (below) and `43` for generator macros.

| Cmd | Effect | Argument |
|---|---|---|
| `30xy` | Set vowel | x = 0 A, 1 E, 2 I, 3 O, 4 U, 5 ER, 6 AE; y = glide in ×25 ms (0 = hard cut). Holds until the next note or `31` |
| `31xx` | Jump to sequence step xx | 00 = first vowel; stepping continues from there |
| `32xx` | Formant shift | 80 = ×1, 00 = ×0.5, FF ≈ ×2 |
| `33xy` | Stutter / beat-repeat (**any instrument**) | x = slice length: 1 = 1/64, 2 = 1/32, 3 = 1/16 (one row at 4 lines/beat), 4 = 1/8, 5 = 1/4, 6 = 1/2 note; y = repeats (0 = stop) |
| `34xy` | Stutter pitch step per repeat | +x semitones (up/faster), or −y (down/slower) |
| `35xx` | Breath | 00–40 |
| `36xy` | Babble | x = step length (as for 33), y = amount 0–F |
| `37xy` | Chop gate | x = cycle length (as for 33), y = open for y/16 of each cycle (0 = off) |

The effects in a row with a note apply to that new note. A new note resets
`30`, `32`, `35`, `36`, `37` and the stutter.

Example: `C-2 05 3000`, then `--- -- 3053` and later `--- -- 3022`, gives "ahh → err → eee".
`--- -- 3332` on a later row stutters the last 1/16 twice.

![Voice words](screenshots/dark-voice-words.png)

## Consonants and words (0.2.3.10)
Give the voice **Words** (the row of small text boxes under the vowel
sequence; **+ word** adds one, right-click or **clear** removes). Each new
note speaks the next word in the list, so a run of notes says
"ka ta ka da …". `3Exx` on a note speaks word `xx` instead; `3Exx` on a row
without a note says word `xx` again on the held note (a new syllable
without a new envelope). With no words the voice works as before (vowels only).

A word is a phoneme string. Letters:

| Kind | Letters | How it sounds |
|---|---|---|
| vowels | `a e i o u`, `er`, `ae` (also `ee` = i, `oo` = u, `y` = i, `w` = u, `r` = er) | the vowel table; a vowel that is not last lasts 150 ms, the last one holds until the note ends |
| plosives | `p b t d k g` (`c q` = k, `x` = k s, `ch` = t sh, `j` = d sh) | a closure (silent; a low *voice bar* for b d g), a 10 ms noise **burst** shaped by the place (lips ≈ 1.2 kHz, tongue tip ≈ 4.5 kHz, soft palate ≈ 2.3 kHz), then aspiration (p t k) and a **formant transition** from the consonant's locus into the vowel |
| fricatives | `s z sh f v h` (`th` = f) | band-limited noise (s ≈ 6.5 kHz, sh ≈ 3.2 kHz, f/v broad and soft), voiced for z and v; `h` is breath through the next vowel's formants |
| nasals | `m n ng` | a voiced, darker murmur (F1 ≈ 270 Hz, upper formants weak), then the transition into the vowel |
| pause | `.` `-` `,` | 80 ms of silence |

A word that ends on a consonant ("ak") goes silent after it until the next
word. **Speech rate** (0.25–3×) scales all phoneme lengths and the
transitions. The **Talk Ka-Ta** preset shows it.

## Throat mode (0.2.3.10)
**Tract: Throat** replaces the five formant filters with a simulated vocal
tract, inspired by **Pink Trombone** by Neil Thapen (MIT): a 44-section
waveguide tube (Kelly-Lochbaum) whose diameters follow a tongue position, the
lips and constrictions, with a 28-section nose behind a velum. The glottal
source, breath, tension and jitter are the same as in formant mode. Vowels
become tongue and lip shapes (A = tongue back and low, I = front and high, U =
back, high and rounded lips, …), consonants become closures and narrow
constrictions at the lips, tongue tip or soft palate with turbulence noise,
and nasals open the velum, so the transitions come out of the moving tract.
Formant shift and register scale the tract length; Morph and Speech rate set
how fast the tongue moves. Presets: **Throat Ahh**, **Throat Speech**, **Demon Words**.
Throat mode costs more CPU (8 throat voices render about 6× faster than realtime
on one core; formant mode is several times cheaper).

## Transform (0.2.3.11)

![Voice Transform](screenshots/dark-voice-transform.png)

The **TRANSFORM** section under the voice settings turns the voice into
metal, broken electronics or a sub-heavy monster. Three stages, each with its
own amount, and a master **Blend** between the dry voice (0) and the
transformed one (1). Blend 0 is off (the default, and what every song from
0.2.3.10 and older gets): the voice renders bit for bit as before.

Signal flow per note:

```
voice (formants / Throat) ─┬─ Digital ─ Resonance ─┐   (or Resonance ─ Digital: Order)
                           │                       ├─ Blend ─ mono below the crossover ─ filter, drive ─┬─ amp env
                           └───────────── dry ─────┘                                                    │
                           └─ Sub (sine / divider, follows the voice's envelope) ─ saturate ─ crossover ┘
```

**Metallic resonance**: six two-pole modal resonators (constant peak gain,
after Smith & Angell 1982). *Ratios*: harmonic (1 2 3 4 5 6, a robotic
comb buzz), metal (the TR-808 cymbal ratios), bell (church-bell partials:
hum, prime, tierce, quint, nominal, deciem), bar (free-free bar modes,
glassy) or odd (1 3 5 7 9 11, hollow). *Pitch*: Note (follows the note, plus
*Tune* in semitones) or Fixed (30–4000 Hz). *Decay* is the ring time of the
lowest mode (20 ms–4 s, the "feedback"); *Damping* makes the higher modes die
faster and darkens the ring; *Spread* detunes the modes apart, left against
right, for a stereo shimmer. The poles always stay inside the unit circle and
the output is soft-limited, so no setting can run away.

**Digital distortion**: *Pre HP* → *Drive* (0–36 dB) into a hard **Clip** or
a **Fold** → *Bits* (1–16) → *Rate* (sample-rate reduction, 300 Hz–48 kHz;
*Aliasing* on = raw sample-and-hold, off = filtered decimation) → *Ring mod*
at the note × *Ring ratio* → *Post LP*. Every step keeps the polarity of its
input (the fold is the in-phase fold of the Drive device), so blending never
hollows the voice out; the drive is level-compensated.

**Sub bass**: a layer under the voice. *Mode*: sine (follows the note pitch),
divider (an octave divider: a flip-flop on the zero crossings of the voice,
like an analog octave pedal) or both; *Octave* −1 or −2. The sub follows the
voice's own amplitude (2 ms attack / 60 ms release), *Drive* saturates it
(adds harmonics, keeps its peak level) and *Growl* sets how much of those
harmonics above the *Crossover* (60–250 Hz) you hear. Below the crossover the
sub is clean, and the whole voice is folded to mono there (only the side
signal is high-passed, 48 dB/oct, an octave above the crossover: the side is
at least ~50 dB down below the crossover, the mid is untouched). The sub is
added after the patch filter and drive, before the amp envelope, so the
filter never muddies it. *Formant drop* (0–12 semitones, scaled by the
Blend) lowers the formants for a monster voice.

**Order**: Dist > Res (default: the resonators ring on the grit) or
Res > Dist (the ringing gets crushed).

| Preset | Sound |
|---|---|
| **Metal Demon** | Demon A/O voice ringing through metal resonators, light crush, sub, formants −3 st |
| **Digital Ghost** | thin Soprano E/I/U whisper through a 5-bit 6 kHz aliasing crusher and a ring modulator, glassy bar resonators |
| **Sub Beast** | words (dum bom gha), formants −7 st, saturated sine + divider sub, clipped grit |
| **Cyber Choir** | Alto A-E-I-O choir through harmonic resonators an octave up, light filtered crush |
| **Ritual Growl** | Throat-mode Demon words (gha om kar osh), bell resonators an octave down, then folded distortion, divider sub |

Pattern effects (00 = the instrument's own value, 01–FF = 0–100 %; they
stay on the channel until a different instrument plays there):

| Effect | Sets |
|---|---|
| `3Fxx` | Transform blend |
| `40xx` | metallic resonance amount |
| `41xx` | digital distortion amount |
| `42xx` | sub level |

Values glide over 10 ms (no zipper noise). A voice whose Blend is 0 gets its
transform from `3Fxx` alone (`3FFF` = fully transformed).

![Chrome FX devices](screenshots/dark-fx-chrome.png)

**As FX devices.** The three stages are also devices for any instrument
(Add effect > *Metal resonator*, *Digital distortion*, *Sub bass*). The
resonator and the sub sine follow the pitch of the note playing on that
channel (without a note: the fixed pitch, and the sub falls back to the
octave divider); `40xx`–`42xx` set the device's mix on that channel. Device
presets: Robot ring, Cyber metal, Temple bell; 8-bit grit, Ring robot,
Broken DAC; Clean sine sub, Octaver growl.

In text songs: `[instrument.voice.transform]` (and `.res`, `.dig`, `.sub`
sub-tables) changes only the keys it names, e.g.
`[instrument.voice.transform]` `blend = 0.6`, `[instrument.voice.transform.sub]`
`octave = 2`.

## Text songs
In a `.cwt.toml` text song, `patch = "voice:Demon Ahh"` makes a Voice
instrument. An optional `[instrument.voice]` table overrides its fields
(e.g. `vowels = ["Er", "A"]`, `shift = 0.9`, `words = ["ka", "ta"]`,
`rate = 1.2`, `throat = true`), and the usual synth keys
(`cutoff`, `attack`, …) override its envelope and filter.

`scripts/make_voice_examples.py [out.mp3]` writes one example text song per
preset plus a 16-bar dark psy groove with voice glitches (effects 30–37 in
use), renders them with `chipwave --render` and joins them into one MP3.

## Limits (first version)
- The voice is mono, and unison does not apply (use Chorus or the delay for width).
- Consonants are simple (no l, no r colouring beyond the ER vowel, no
  diphthong rules); words are phoneme strings, not English text-to-speech.
- In Throat mode voiced closures (b d g) are almost silent: the tube has no
  wall radiation for the voice bar.
- Transform: the resonators and the sub start fresh with every note (the
  previous note fades out over 5 ms as before), so a long resonator ring is
  cut by the next note; use the Metal resonator FX device to ring across notes.
  The octave divider needs a clear fundamental; on noisy or whispered input it
  jitters (use the sine mode). Formant drop only applies to the Voice
  instrument, not the FX devices.
- Stutter slices are at most 1 second long and sit before the instrument FX chain.
  Live MIDI notes are not stuttered.

## Acknowledgements / Credits
- **Emilie Gillet / Mutable Instruments**: the design is inspired by the
  Plaits "naive speech synth" (MIT),
  https://github.com/pichenettes/eurorack/tree/master/plaits/dsp/speech.
  No code or table data is copied. The MIT notice is in THIRD_PARTY_LICENSES.txt.
- **Csound manual**, Appendix "Formant values" (the Csound community; Csound is
  LGPL-2.1): https://csound.com/docs/manual/MiscFormants.html
- **G. E. Peterson & H. L. Barney** (1952), "Control methods used in a study of
  the vowels", JASA 24(2), https://doi.org/10.1121/1.1906875
- **A. E. Rosenberg** (1971), glottal pulse shape, JASA 49(2),
  https://doi.org/10.1121/1.1912389. **G. Fant, J. Liljencrants & Q. Lin**
  (1985), the LF glottal flow model.
- **Dennis H. Klatt** (1980), "Software for a cascade/parallel formant
  synthesizer", JASA 67(3), https://doi.org/10.1121/1.383940
- **Vadim Zavalishin**, *The Art of VA Filter Design*: the TPT state-variable
  filter used for the formants.
- **Neil Thapen**, Pink Trombone (2017, MIT), https://dood.al/pinktrombone/ :
  the Throat mode's vocal-tract design. Independent implementation; the MIT
  notice is in THIRD_PARTY_LICENSES.txt.
- **J. O. Smith & J. B. Angell** (1982), "A constant-gain digital resonator
  tuned by a single coefficient", Computer Music Journal 6(4); J. O. Smith,
  *Introduction to Digital Filters*, https://ccrma.stanford.edu/~jos/filters/ :
  the Transform resonators.
- **N. H. Fletcher & T. D. Rossing**, *The Physics of Musical Instruments*
  (1998): bell partials and bar modes; the metal ratios are the Roland TR-808
  cymbal oscillators. **S. H. Linkwitz** (1976): the crossover. The octave
  divider follows analog octave pedals (Boss OC-2).
- **J. L. Kelly & C. C. Lochbaum** (1962), "Speech synthesis": the tube model.
- **P. Delattre, A. Liberman & F. Cooper** (1955), "Acoustic loci and
  transitional cues for consonants", JASA 27(4),
  https://doi.org/10.1121/1.1908024 : consonant loci.
- **K. N. Stevens**, *Acoustic Phonetics* (MIT Press, 1998): burst and
  frication noise bands.
- Research notes: [research/voice-glitch.md](research/voice-glitch.md).
