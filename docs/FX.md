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
| **Frequency shifter** | Unreleased | Bode-style single-sideband shift, ±2000 Hz |
| **Plugin effect** | Unreleased | any installed CLAP or VST3 effect, see [Plugin effects](#plugin-effects-clap-unreleased) |
| **EQ** | Unreleased | 4 bands: low shelf, two peaks (with Q), high shelf, ±18 dB (RBJ Audio EQ Cookbook biquads); flat = bit-exact pass-through |

## Devices tab (Unreleased)

The lower panel's **Devices** tab (next to Piano roll; **DEVICES** in the view tab bar, **Ctrl+7**)
shows the selected track's chain left to right, the way Del (postmanpat) picked from a reference
screenshot: the **instrument card** (the lower-panel instrument editor; the chevron folds it to a
slim strip), an arrow, then a **card per device** and a **+** card at the end (opens Browser ›
Effects).

- **Cards:** power (bypass), the name (drag it to move the card; double-click collapses), collapse
  and **×** (remove, Ctrl+Z brings it back). Right-click: bypass, collapse, duplicate, move left /
  right, remove, presets.
- **Inline controls:** EQ (curve + a draggable dot per band: drag = frequency and gain,
  double-click = 0 dB; the band chips pick which band's knobs show), Compressor (transfer curve),
  Filter (response curve, mode chips), Delay (echo taps, sync and ping-pong chips), Reverb (decay
  shape), knobs for every other device. Knobs right-click for automation lanes and modulation like
  everywhere else. Plugin effects: thumbnail (Browser › Plugin thumbnails), **Editor…**, status
  and parameter sliders.
- **Adding:** Browser › **Effects** lists the built-ins, the Grid FX patches and the effect
  plugins; its header says **Adds to: …**. Double-click adds at the end; drag into the row to
  insert between two cards (an accent insertion marker shows where); drop on a timeline track to
  add to that track's chain. Dragging an instrument onto the row replaces the track's instrument
  and keeps its effects.
- **Follows the selection:** the Timeline's or tracker's current track picks the instrument shown
  (its most used instrument, or the one the track was given).
- **Real-time safety:** all edits change the song's `fx` list under the shared lock, like the FX
  view. The engine thread rebuilds only devices that are new (the ones that stay keep their state
  and plugin slot) and the audio callback only reads the ring buffer, so it never allocates or
  locks. Automation lanes (`instrument.N.fx.D.*`) and modulation routes follow their device when
  devices move.

## Plugin effects (CLAP, Unreleased)

Any CLAP or VST3 (Unreleased, [PLUGINS.md › VST3](PLUGINS.md#vst3-unreleased)) **audio effect** installed on the computer can sit in an instrument's FX chain, next to
the built-in devices.

- **Adding one:** every add-effect menu has a **Plugin effect ›** submenu: **+ Device** in the FX
  view, **+ Add effect** on the instrument editor's FX tab, and right-click an instrument ›
  **Add effect**. It lists the effects the plugin scan found (Settings › Plugins; the folders are
  the same as for plugin instruments, see [PLUGINS.md](PLUGINS.md#finding-plugins)). **Scan for
  plugins** at the bottom looks again.
- **The device** shows the plugin's name in the strip. Its panel has the vendor and format, the
  load status with the latency, **Editor…** (the plugin's own window), **Reload**, and every
  parameter as a slider (double-click = default). Power (bypass), reorder, collapse, duplicate and
  remove work as for built-in devices; presets live in the plugin's own editor.
- **Automation:** right-click a parameter slider › **Add automation lane**, or **+ Lane** ›
  *instrument* › **FX n *plugin*** › parameter. Lanes run 0–100 % over the parameter's range and are
  saved as `instrument.N.fx.D.p<plugin param id>` (CLAP or VST3 parameter id). Lanes play in exports too.
- **Saving:** the plugin's state is stored base64-encoded in the song (`.cwt`), in `.cwi`
  instrument files and in the library, with the plugin id, name, vendor, format and path.
  ChipWave asks the plugin for its state before save, autosave, export and copy. Duplicated or
  pasted devices start from the same state as their own instance.
- **Missing plugins:** when an effect isn't installed (or fails to load), the device stays in the
  chain, shows a red notice and is **bypassed** (the dry signal passes unchanged). Its saved state is kept untouched, so saving the song on that computer loses nothing.
  A warning lists missing effects when the song opens. **Reload** tries again (also after a scan).
- **Latency and delay compensation:** a plugin device works on 64-sample blocks, so it adds
  64 samples plus the latency the plugin reports (CLAP `latency` extension, VST3 `getLatencySamples`). ChipWave delays the
  other channels by the difference, so every channel reaches the mixer in time (live, in WAV/MP3
  export and `--render`). Bypass keeps the same delay, so toggling it doesn't shift the timing.
- **One instance per channel:** like plugin instruments, a device has an instance for each channel
  its instrument plays on (all channels in timeline mode) plus one for live playing; parameter
  edits go to all of them.
- The audio thread never allocates: instances are created and dropped on the UI thread, and the
  plugin's parameter events use pre-allocated queues (tested).

**Limits (Unreleased):** only instrument chains have devices; the return buses (fixed Delay /
Reverb) and the master (limiter) don't have FX chains yet, so plugins can't go there. Plugin
*instruments* aren't latency-compensated. Live playing (the jam chain) isn't compensated either.
Plugin effect parameters can't be MIDI-learned yet, and `.cwt.toml` text songs can't describe plugin
devices. A song with a plugin device saves `min_reader_version = 2`: builds with the reader check
refuse it with "please update", and 0.2.4.0 and older report a load error (the same goes for `.cwi`
files with plugin devices, which have no version check). Mono-only or
sidechain ports: only the first (main) stereo input and output are used.

**Tested with:** ChipWave's own fixture effect (`ChipWave Test FX` in `plugins/cw-test-synth`:
gain, polarity flip and a 32-sample reported latency) in the automatic tests, and **Surge XT
Effects 1.3.4** (GPL-3, built from source, not bundled) on Linux (self-test with
`CHIPWAVE_SELFTEST_PLUGIN_FX=<file.clap>`, screenshot). Nothing is bundled with ChipWave.

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

## Frequency shifter (Unreleased)

Unlike a pitch shifter, a frequency shifter moves **every partial by the same number of Hz**, so
harmonic sounds turn inharmonic and metallic: a 100 Hz bass with partials at 100, 200, 300 Hz
shifted +50 Hz has partials at 150, 250, 350 Hz. Small shifts (under ~5 Hz) give a slow phasing /
stereo detune; with feedback, every pass shifts again, giving the endless "barber pole" spiral.

| Knob | Range | Default | Notes |
|---|---|---|---|
| Shift | −2000 to +2000 Hz | +100 Hz | coarse |
| Fine | −50 to +50 Hz | 0 | added to Shift; for slow phasing |
| Output | up, down, both | up | upper sideband (input + shift), lower (input − shift), or both: left = up, right = down (Bode's two outputs) |
| Feedback | 0–95 % | 0 | the output fed back into the input: repeated shifts |
| Mix | 0–100 % | 50 % | |

A negative Shift with Output = up is the same as a positive one with Output = down. Presets:
**Barber pole**, **Metallic +170 Hz**, **Stereo detune**, **Alien down −600 Hz**. Factory patch files
take `type = "freqshift"` with `shift_hz`, `output` (`"up"`, `"down"`, `"both"`), `feedback`, `mix`.

**How it works** (`src/fxmod.rs`): Harald Bode's single-sideband modulator. A Hilbert transformer
splits each channel into two signals 90° apart, using two chains of four second-order all-pass
sections with the published coefficients of **Olli Niemitalo** ("Hilbert transform by IIR
all-pass filters"; 90° ± ~0.7° over nearly the whole band; one path delayed by a sample). They are
multiplied by a cosine / sine oscillator (double-precision phase, so long notes don't drift) and
added (up) or subtracted (down). No allocation, no FFT, no latency beyond a few samples.

Unit tests shift sines at 220 Hz–8 kHz by 7.5 Hz to ±2000 Hz and check the new tone is within
1 Hz of input + shift at full level, with the mirror sideband and the original at least 34 dB
down; that **down** and **both** pick the right sidebands; and that feedback adds the
re-shifted copies (600, 700, 800 Hz from 500 Hz at +100 Hz) and stays bounded. The engine test
sweeps the Mix with an automation lane, allocation-free.
