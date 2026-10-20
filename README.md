<p align="center">
  <img src="assets/icon/chipwave_studio_256.png" alt="ChipWave Studio icon" width="128">
</p>

<h1 align="center">ChipWave Studio</h1>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-egui%20%2B%20cpal-orange?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-0a84ff" alt="Windows | macOS | Linux">
  <a href="LICENSE"><img src="https://img.shields.io/badge/licence-GPL--3.0-ff2bd6" alt="GPL-3.0"></a>
  <img src="https://img.shields.io/badge/version-0.2.4.1-00d0ff" alt="Version 0.2.4.1">
  <a href="https://www.youtube.com/@whatgoesaround1551"><img src="https://img.shields.io/badge/YouTube-demos%20%26%20promos-ff0000?logo=youtube&logoColor=white" alt="YouTube: demos and promos"></a>
  <a href="https://github.com/postmanpat-303/ChipWaveStudio/releases/latest"><img src="https://img.shields.io/github/v/release/postmanpat-303/ChipWaveStudio?label=release&color=7c4dff" alt="Latest release"></a>
</p>

<p align="center">
  <a href="https://youtu.be/6VWhFZxexIc"><img src="docs/screenshots/promo-thumb.jpg" alt="Watch the ChipWave Studio promo on YouTube" width="640"></a>
  <br><sub>▶ Watch the promo on YouTube · more demos on <a href="https://www.youtube.com/@whatgoesaround1551">the channel</a></sub>
</p>

```
 ┌──────────────────────────────────────────────────────────────────────────┐
 │  C H I P W A V E   S T U D I O            v0.2.4.1     BPM 140  SPD 06  │
 ├──────┬─────────────┬─────────────┬──────────────────┬───────────────────┤
 │  00  │ A-2 01 .. … │ ... .. .. … │ ... .. .. ....   │ OctaMED-inspired  │
 │  01  │ ... .. .. … │ A-2 02 38 … │ ... .. .. ....   │ tracker + synth   │
 │  02  │ ... .. .. … │ ... .. .. … │ C-5 03 38 0814   │ Windows · Linux   │
 └──────┴─────────────┴─────────────┴──────────────────┴───────────────────┘
```

<p align="center">
  <img src="docs/screenshots/pattern-playing.gif" alt="ChipWave 0.2.3.19 playing the Lumen Drift demo: the Tracker above, the Trance Kick open in the lower-panel instrument editor" width="800">
  <br><sub>Real capture of ChipWave 0.2.3.19 playing the built-in demo song “Lumen Drift”, with the instrument editor in the lower panel.</sub>
</p>

<p align="center">
  <a href="https://github.com/postmanpat-303/ChipWaveStudio/releases/latest"><b>⬇ Download latest for Windows</b></a> &nbsp;·&nbsp;
  <a href="https://github.com/postmanpat-303/ChipWaveStudio/releases/latest/download/ChipWave-macos-universal.dmg"><b>⬇ macOS</b></a> <sub>(<a href="docs/MACOS.md">first launch</a>)</sub> &nbsp;·&nbsp;
  <a href="#features"><b>Features</b></a> &nbsp;·&nbsp;
  <a href="#themes"><b>Themes</b></a> &nbsp;·&nbsp;
  <a href="#keys"><b>Keys</b></a> &nbsp;·&nbsp;
  <a href="#documentation"><b>Docs</b></a> &nbsp;·&nbsp;
  <a href="https://github.com/postmanpat-303/BigWaveRadio"><b>BigWave Radio</b></a> (sister project)
</p>

**ChipWave Studio** is a music tracker inspired by **OctaMED** and the Commodore **Amiga**: the fast, keyboard-driven pattern workflow of a classic tracker, plus a built-in synth, a synthetic Voice instrument, macro-knob Generators (drums, chip FX, analog, FM, noise), a visual Order list, a mixer with sidechain ducking and a true-peak limiter, automation and WAV/MP3 export, in up to 32 channels. Written in Rust with egui and cpal, for Windows, macOS (universal: Apple Silicon + Intel) and Linux. Early version, in active development.

> **ChipWave Studio was called ChipWave Tracker until 0.2.3.19.** It was renamed in 0.2.4.0 to tell it apart from an unrelated iOS app called "ChipWave". Same app, same settings, same `.cwt` songs; the old GitHub links redirect here. Website: **[postmanpat-303.github.io/ChipWaveStudio](https://postmanpat-303.github.io/ChipWaveStudio/)**. Details: [docs/RENAME.md](docs/RENAME.md).

All pictures on this page are real captures of the current build (no mockups). They were made with `scripts/screenshots.sh` and `scripts/record_gifs.sh`.

```
 ═══[ 01 ]═══ FEATURES ═════════════════════════════════════════════════════
```
<a id="features"></a>

<table>
  <tr>
    <td width="50%"><img src="docs/screenshots/dark-pattern.png" alt="Tracker playing the Velvet Mirrorball disco demo, with the Cyber Choir voice open in the lower-panel instrument editor"><br><b>Tracker</b>: OctaMED-style blocks (64 rows by default; set the length on the one-line Block bar), note / instrument / volume / effect columns, decimal row numbers (hex in Settings › Look), the classic Z–M keyboard layout, selection, copy/paste, transpose (keys, the Block bar’s -12 / -1 / +1 / +12 buttons or Edit › Transpose) and undo. The cursor follows the playing row. Shown: the “Velvet Mirrorball” disco demo with its Cyber Choir voice in the instrument editor below.</td>
    <td width="50%"><img src="docs/screenshots/dark-arranger.png" alt="Order list view of the Velvet Mirrorball demo with clips and automation lanes"><br><b>Order list</b>: the song order as coloured clips with role badges (Intro, Verse, Chorus, Bridge, Breakdown, Outro), drag to reorder, duplicate, zoom, plus <b>automation lanes</b> for filter sweeps, levels and delay feedback.</td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/screenshots/dark-mixer.png" alt="Mixer with channel strips, send returns and the master limiter"><br><b>Mixer</b>: per-channel fader, pan, mute/solo, peak meters, two send/return buses (delay and reverb) and <b>sidechain ducking</b>: pick a key channel and set Duck % per channel for the trance pump.</td>
    <td width="50%"><img src="docs/screenshots/dark-fx-chrome.png" alt="FX chain with the chrome devices"><br><b>FX chain</b>: per-instrument device strip with Filter, Chorus/Flanger, Delay, Reverb, Drive, Bitcrusher and Compressor, plus the chrome devices <b>Metal resonator</b>, <b>Digital distortion</b> and <b>Sub bass</b> (0.2.3.11, they follow the note; <code>40</code>–<code>42</code> set their mix); bypass, collapse, drag to reorder, presets.</td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/screenshots/dark-browser.png" alt="The Browser docked on the right of the Timeline: category tabs, search, tag chips, filter columns and rated results"><br><b>Browser</b> <sub>(Unreleased)</sub>: <code>Ctrl+B</code>. Instruments, CLAP / VST3 plugins, samples, songs, presets, Grid patches and wavetables in one searchable panel with tags, Category / Creator / Location filters, favourites and ratings. Double-click to load, drag onto a track header to give the track that sound, or below the last track for a new one. See <a href="docs/BROWSER.md">BROWSER.md</a>.</td>
    <td width="50%"><img src="docs/screenshots/dark-plugin-setup.png" alt="The Find plugins step: scan automatically, choose folders, or skip"><br><b>Find plugins</b> <sub>(Unreleased)</sub>: on the first start ChipWave offers to search the standard VST3 / CLAP folders (optionally the whole drive) in the background, let you pick folders, or skip. Always in Settings › Plugins.</td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/screenshots/dark-fill.png" alt="Fill to length"><br><b>Fill to length</b>: auto-arrange the song to a target length from a style, with an editable energy curve, variation, seed, preview and section locks.</td>
    <td width="50%"><img src="docs/screenshots/dark-settings.png" alt="Settings"><br><b>Settings</b>: summary cards for Audio, Look, Keys (remappable), Files &amp; Safety (autosave, backups, crash recovery, update check) and MIDI. <sub>(Captured on a machine without a sound card, hence “no audio output”.)</sub></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/screenshots/dark-pattern-full.png" alt="Full-height view: the lower panel hidden so the tracker fills the window"><br><b>Full-height view</b> <sub>(0.2.3.19)</sub>: <code>F11</code>, the <b>Full height</b> button, or dragging the splitter to the bottom hides the lower panel so the pattern fills the window; press <code>F11</code> again to bring it back.</td>
    <td width="50%"><img src="docs/screenshots/dark-instrument-fx.png" alt="The FX tab of the instrument editor showing the Disco Strings effect chain as knob cards"><br><b>FX tab</b> <sub>(0.2.3.19)</sub>: an instrument's effect chain as knob cards in the lower panel (here the disco demo’s strings: high-pass and low-pass filters and two chorus/flanger stages), next to the Main, Modulation and Advanced tabs.</td>
  </tr>
</table>

<table>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/instrument.gif" alt="The lower-panel instrument editor browsing the demo’s synth sounds"><br><b>Synth / sound editor</b>: oscillator, unison + detune, stereo width, LP/HP/BP filter (SVF or Moog ladder, clean / vintage, 12/24 dB) with envelope, ADSR, drive, an 808-style metallic source for hats, velocity → cutoff, start delay, 303 glide / legato and accent, and a tempo-syncable LFO to cutoff, amp and pitch. <a href="docs/screenshots/dark-instrument.png">Still</a>.</td>
    <td width="66%" valign="top"><img src="docs/screenshots/mixer-meters.gif" alt="Mixer meters moving while the demo plays"><br><b>Mixer while playing</b>: the pads and bass duck under the kick (sidechain), meters per channel and on the master.
    <br><br><img src="docs/screenshots/arranger.gif" alt="Order list playhead moving through the song"><br><b>Order list while playing</b>: the playhead runs through the clips and the automation curves.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-instrument-editor.png" alt="The Octave Bass (Analog generator) in the lower-panel instrument editor, with the compact instrument list on the left"></td>
    <td width="66%" valign="top"><b>Instrument editor</b> <sub>(0.2.3.19)</sub>: every instrument (synth, sample, Voice, generators, Chip FX, CLAP plugins) is edited under the pattern with <b>knobs</b> on side-by-side, collapsible cards in four tabs (Main, Modulation, FX, Advanced), small envelope / filter / wave pictures, presets, A/B compare and <b>parameter search</b> (<code>Ctrl+F</code>). Drag a knob, <code>Ctrl</code>-drag for fine steps, double-click for the default, right-click to automate or MIDI-learn. The lower panel switches between Order list, Instrument and Automation, resizes with a splitter, and <b>Full-height view</b> (<code>F11</code>) hides it. See <a href="docs/INSTRUMENT_EDITOR.md">INSTRUMENT_EDITOR.md</a>.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-voice.png" alt="Glitch Babble Voice instrument in the lower-panel editor: Voice, Vowels, Words, Output, Filter and Envelope cards"></td>
    <td width="66%" valign="top"><b>Voice (glitch) instrument</b> <sub>(0.2.3.6)</sub>: synthetic “ahh, eee, ohh, err” vowel chops for dark psytrance. No samples: a glottal pulse with breath and jitter through five formant filters, a vowel sequence you build from chips (A E I O U ER AE), smooth morphing, random <i>babble</i>, Bass/Tenor/Alto/Soprano/<b>Demon</b> registers, formant shift, a tempo-synced chop gate and <b>stutter / beat-repeat</b>. Pattern effects <code>30</code>–<code>37</code> change the vowel, shift and stutter per row (<code>33xy</code> stutters any instrument). Presets: Demon Ahh, Choir Eee, Glitch Babble, Robot Err, Stutter Ohh, Whisper Ooh. See <a href="docs/VOICE.md">VOICE.md</a>. <a href="docs/screenshots/amiga-voice.png">Amiga theme</a>.</td>
</tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-mixer-limiter.png" alt="Mixer with the Limiter and loudness panel"></td>
    <td width="66%" valign="top"><b>Master limiter &amp; loudness</b> <sub>(0.2.3.7)</sub>: the panel pinned to the right of the mixer, captured while the demo plays: a lookahead true-peak limiter (gain, ceiling −1 dBTP, release, gain-reduction bar, bypass = the old soft clipper) and momentary / short-term / integrated LUFS plus true peak (ITU-R BS.1770), with a loudness-target hint. Exports use the same limiter. Also new: pattern effects <code>38</code>–<code>3D</code> for per-row filter cutoff, resonance and cutoff slides, 303 glide (Legato + <code>3C</code>) and accent (<code>3D</code> or a velocity threshold); see <a href="docs/TRACKER.md">TRACKER.md</a> and <a href="docs/MIXER.md">MIXER.md</a>.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-32-channels.png" alt="Tracker with a 32-channel song"></td>
    <td width="66%" valign="top"><b>Up to 32 channels</b> <sub>(0.2.3.8)</sub>: a 32-channel test song in the Tracker (four sets of kick, hat, clap, bass, three arp voices and a lead). The bar under the grid reads “channels 1–11 of 32”; click or drag it, or Tab through the channels. The mixer strips scroll the same way beside the pinned master.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-chord-helper.png" alt="Chord helper window"></td>
    <td width="66%" valign="top"><b>Chord helper</b> <sub>(0.2.3.9)</sub>: pick or detect the song key (Krumhansl–Schmuckler), click chord pads (triad, 7th, sus, add9, power; octave and inversion) to hear them and build a progression, or pick a preset, then write it into the block as chords, an arpeggio or a bass line. Scale lock snaps typed and MIDI notes to the key. See <a href="docs/CHORDS.md">CHORDS.md</a>.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-instrument-library.png" alt="Instrument library window"></td>
    <td width="66%" valign="top"><b>Instrument library</b> <sub>(0.2.3.9)</sub>: the song's instruments, your own library folder of <code>.cwi</code> files and the factory sounds side by side, with search, audition, add, and “replace this slot's sound but keep its notes”. In the instrument list: rename (double-click), reorder (notes follow), search, and “use this instead of” to swap instruments across the song or one block. See <a href="docs/INSTRUMENT_LIBRARY.md">INSTRUMENT_LIBRARY.md</a>.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-voice-words.png" alt="Voice panel with Throat mode and words"></td>
    <td width="66%" valign="top"><b>Voice words and Throat mode</b> <sub>(0.2.3.10)</sub>: the Voice instrument speaks short words (<code>gha dum osh kar</code>), one per note or picked with <code>3Exx</code>: plosives, fricatives and nasals built from noise bursts and formant transitions. <b>Throat</b> swaps the formant filters for a simulated vocal tract inspired by <a href="https://dood.al/pinktrombone/">Pink Trombone</a> by Neil Thapen (MIT). See <a href="docs/VOICE.md">VOICE.md</a>.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-sample-slicing.png" alt="Sample editor slicing a breakbeat"></td>
    <td width="66%" valign="top"><b>Chop / slice</b> <sub>(0.2.3.10)</sub>: a synthesised breakbeat cut at its transients (or on a grid) into numbered slices. Select a slice to normalise, reverse or fade just that part; <b>To instruments</b> makes one instrument per slice and <b>To notes</b> writes them into the block at the song tempo, ready to rearrange. See <a href="docs/SAMPLES.md">SAMPLES.md</a>.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-voice-transform.png" alt="Voice Transform cards (metallic resonance, digital distortion, sub bass) on the FX tab of the instrument editor"></td>
    <td width="66%" valign="top"><b>Voice Transform</b> <sub>(0.2.3.11)</sub>: blend the voice into <b>metallic resonance</b> (modal resonators tuned to the note: metal, bell, bar, harmonic, odd), <b>digital distortion</b> (clip/fold, bits, sample-rate, ring mod) and a <b>heavy sub</b> (sine or octave divider, mono below the crossover), with formant drop for a monster voice. Presets Metal Demon, Digital Ghost, Sub Beast, Cyber Choir, Ritual Growl; <code>3F</code>–<code>42</code> per row; the stages are FX devices for any instrument too. See <a href="docs/VOICE.md">VOICE.md</a>.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-generators-kit.png" alt="Drum kit generator with macro knobs"></td>
    <td width="66%" valign="top"><b>Generators: Drum Synth</b> <sub>(0.2.3.12)</sub>: kick (v0, 808, 909, psy Zap), snare, hat, clap, tom and perc from eight macro knobs, or a whole <b>kit</b> on one instrument. <b>Dice</b> rolls a new sound within sensible ranges, <b>Mutate</b> nudges it, <b>lock</b> keeps a knob, 16 rolls of history, stored seed, <b>Bake to sample</b>. Plus a <b>Moog ladder filter</b> (clean or vintage, 12/24 dB LP/BP/HP) in every synth. <code>43xy</code> sets a macro per row. See <a href="docs/GENERATORS.md">GENERATORS.md</a>.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-generators-analog.png" alt="Analog generator"></td>
    <td width="66%" valign="top"><b>Generators: Analog</b> <sub>(0.2.3.14)</sub>: a Moog-style synth, written from scratch. Three drifting oscillators (saw, triangle, square, pulse with PWM; octave, semitone and detune; sync), noise, the vintage <b>ladder</b>, filter and amp envelopes and glide. Eight macros, Dice/Mutate/locks/Bake. Presets <b>Fat Moog Bass</b>, <b>Psy Rolling Bass</b>, <b>Warm Lead</b>, <b>Brass</b>, <b>Dark Pad</b>, <b>Acid Squelch</b>. Inspired by <a href="https://github.com/mtytel/vital">Vital</a> (Matt Tytel) and <a href="https://github.com/surge-synthesizer/surge">Surge XT</a>.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-generators-fm.png" alt="FM-4 generator"></td>
    <td width="66%" valign="top"><b>Generators: FM-4 and Noise</b> <sub>(0.2.3.14)</sub>: <b>FM-4</b> has four operators, the 8 DX21/TX81Z algorithms, ratios, feedback and an envelope per operator. Presets: <b>E-Piano</b>, <b>Metallic Bell</b>, <b>Psy Zap Bass</b>, <b>Growl</b>, <b>Dark Pluck</b>. Informed by <a href="https://github.com/asb2m10/dexed">Dexed</a>/MSFA. <b>Noise</b> offers white, pink, brown or crackle through a ladder or SVF with a sweep envelope. Presets: <b>White Riser</b>, <b>Downlifter</b>, <b>Impact</b>, <b>Wind</b>, <b>Vinyl Crackle</b>.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/dark-generators-chipfx.png" alt="Chip FX generator"></td>
    <td width="66%" valign="top"><b>Generators: Chip FX</b> <sub>(0.2.3.13)</sub>: retro game sounds from a port of <a href="https://www.drpetter.se/project_sfxr.html">sfxr</a> by DrPetter (via <a href="https://github.com/bzar/sfxr-rs">sfxr-rs</a>, both MIT). Click <b>Pickup</b>, <b>Laser</b>, <b>Explosion</b>, <b>Power-up</b>, <b>Hit</b>, <b>Jump</b> or <b>Blip</b> for a new sound, <b>Randomize</b> for anything, <b>Mutate</b> to nudge it; lock what you like, step back through 16 rolls, bake to a sample. Plays in tune with the pattern note. See <a href="docs/GENERATORS.md">GENERATORS.md</a>.</td>
  </tr>
  <tr>
    <td width="34%" valign="top"><img src="docs/screenshots/plugin-clap.png" alt="Surge XT CLAP plugin as an instrument, with its eight macro knobs in the lower panel"></td>
    <td width="66%" valign="top"><b>CLAP plugin instruments</b> <sub>(0.2.3.16)</sub>: Surge XT and other CLAP instruments play from the pattern like any other instrument. The lower panel shows the plugin card (Configure… opens its own editor on Windows), eight macro knobs for <code>43xy</code> and automation, and every plugin parameter, searchable, on the Advanced tab (<a href="docs/screenshots/plugin-clap-params.png">picture</a>). See <a href="docs/PLUGINS.md">PLUGINS.md</a>.</td>
  </tr>
</table>

**Also in the box**

| | |
|---|---|
| 🎹 **MIDI keyboard** | play the current instrument with velocity, sustain and pitch bend; record step or live into the pattern; chord spread; MIDI learn on faders and knobs |
| 🎚️ **Up to 32 channels** | each song sets its own channel count (1–32); the Tracker and mixer scroll with a bar showing which channels are in view |
| 🎼 **Chords & scale lock** | song key (pick or detect), chord pads, progressions written as chords, arps or bass, scale lock for typing and MIDI |
| 📚 **Instrument library** | `.cwi` instrument files, a library folder with categories, factory sounds, audition, rename / reorder / swap instruments |
| 🎛️ **Effects column** | `01`/`02` slide, `08` note decay, `09` speed, `0C` volume, `0D` volume slide, `0F` tempo; Voice `30`–`37` (`33`/`34` stutter any instrument); filter and acid `38`–`3D` (cutoff, resonance, cutoff slides, 303 glide, accent); `3E` speak word; `3F`–`42` Voice Transform and chrome devices; `43xy` generator macros. Full table: [TRACKER.md](docs/TRACKER.md#5-effect-commands) |
| 🔌 **Plugin instruments** | CLAP instruments such as Surge XT: crash-guarded scanner, browser, native editor (Windows), parameter list, 8 macros for `43xy` and automation, state saved in the song, offline render. VST3 is listed but not hosted yet. [PLUGINS.md](docs/PLUGINS.md) |
| 🎲 **Generators** | macro-knob instruments: Drum Synth and kits, Chip FX (sfxr), Analog (Moog-style), FM-4, Noise; Dice, Mutate, locks, 16-roll history, Bake to sample; 46 presets |
| 🗣️ **Voice** | synthetic vowels, babble, stutter, spoken words, Throat (vocal tract) mode and the Transform (metal, digital, sub) |
| 🧱 **Master limiter** | lookahead true-peak limiter on the master and in every export (ceiling −1 dBTP by default, gain, release, bypass), with a loudness meter: momentary, short-term and integrated LUFS plus true peak, and a loudness-target hint |
| 🔊 **Samples** | import WAV (button or drag and drop), sample view with loop points and preview, chop / slice into instruments or notes |
| 💾 **Files** | `.cwt` songs (a ZIP of TOML + WAV you can inspect), autosave, backups and crash recovery |
| 📼 **Export** | WAV and MP3 with ID3 tags; whole song, the current block or the Order list selection |
| 🎵 **Demo songs** | File > Demo songs: “Lumen Drift” (140 BPM uplifting trance), “Hollow Ritual” (147 BPM dark psytrance) and “Velvet Mirrorball” (104 BPM disco with Voice choir, phaser strings and filter sweeps), all original |
| ⬆️ **Update check** | Help > Check for updates…, and an optional quiet check at startup |
| 🖥️ **Scaling** | UI scale in 25 % steps (Ctrl+= / Ctrl+-, up to what fits the window), remembers window size and position |

<p align="center"><img src="docs/screenshots/dark-about.png" alt="Help > About" width="70%"><br><sub>Help > About</sub></p>

```
 ═══[ 02 ]═══ THEMES ═══════════════════════════════════════════════════════
```
<a id="themes"></a>

Two built-in themes, switchable in View > Theme or Settings > Look: **Dark Circuit** (default) and **Amiga (Workbench Gradient)**, a light Workbench look with the classic blue headers and orange cursor.

<p align="center">
  <img src="docs/screenshots/themes.gif" alt="Switching between Dark Circuit and Amiga Workbench" width="800">
</p>

<table>
  <tr>
    <th>Dark Circuit (default)</th>
    <th>Amiga (Workbench Gradient)</th>
  </tr>
  <tr>
    <td><img src="docs/screenshots/dark-pattern.png" alt="Tracker, Dark Circuit"></td>
    <td><img src="docs/screenshots/amiga-pattern.png" alt="Tracker, Amiga"></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/dark-arranger.png" alt="Order list, Dark Circuit"></td>
    <td><img src="docs/screenshots/amiga-arranger.png" alt="Order list, Amiga"></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/dark-mixer.png" alt="Mixer, Dark Circuit"></td>
    <td><img src="docs/screenshots/amiga-mixer.png" alt="Mixer, Amiga"></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/dark-fx.png" alt="FX chain, Dark Circuit"></td>
    <td><img src="docs/screenshots/amiga-fx.png" alt="FX chain, Amiga"></td>
  </tr>
</table>

The palettes, generated from the colour values in [`src/theme.rs`](src/theme.rs) (`scripts/theme_swatches.py`):

<p align="center"><img src="docs/screenshots/theme-palettes.png" alt="Dark Circuit and Amiga theme palettes with hex codes"></p>

```
 ═══[ 03 ]═══ DOWNLOAD ═════════════════════════════════════════════════════
```
<a id="download"></a>

Get the **Windows** installer [`ChipWave-setup-x64.exe`](https://github.com/postmanpat-303/ChipWaveStudio/releases/latest/download/ChipWave-setup-x64.exe) or the portable [`ChipWave-windows-x64.zip`](https://github.com/postmanpat-303/ChipWaveStudio/releases/latest/download/ChipWave-windows-x64.zip) from the [latest release](https://github.com/postmanpat-303/ChipWaveStudio/releases/latest) (these links always fetch the newest version; older versions are on the [releases page](https://github.com/postmanpat-303/ChipWaveStudio/releases)). Already on 0.2.3.5 or later? **Help > Check for updates…** offers it. The exe isn't code-signed yet, so SmartScreen may warn: click “More info”, then “Run anyway”. The app opens with a clean song (the same as File > New); to hear what it can do, open **File > Demo songs** and press **Space**.

**macOS**: the universal disk image [`ChipWave-macos-universal.dmg`](https://github.com/postmanpat-303/ChipWaveStudio/releases/latest/download/ChipWave-macos-universal.dmg) runs on both Apple Silicon and Intel Macs, macOS 11 or later (or get the zip, [`ChipWave-macos-universal.zip`](https://github.com/postmanpat-303/ChipWaveStudio/releases/latest/download/ChipWave-macos-universal.zip)). It isn't notarised, so open it the first time with Control-click → **Open** (or `xattr -dr com.apple.quarantine /Applications/ChipWave.app`). See [MACOS.md](docs/MACOS.md).

On Linux, build it from source (below).

```
 ═══[ 04 ]═══ KEYS ═════════════════════════════════════════════════════════
```
<a id="keys"></a>

| Keys | Action |
|---|---|
| `Z S X D C V G B H N J M` | notes C…B; `Q 2 W 3 E R 5 T 6 Y 7 U I 9 O 0 P` = octave up |
| `1` | note off |
| `F1`…`F8`, numpad `-` / `+` | base octave |
| `0`–`9` `A`–`F` | hex in the instrument / volume / effect columns |
| Arrows, `PgUp`/`PgDn`, `Home`/`End` | move (Ctrl+Home/End = first / last block) |
| `Tab` / `Shift+Tab` | next / previous channel |
| `Space` / `Shift+Space` | play / stop block (loop) / whole song (from the Order list play marker) |
| Click / drag the Order list ruler | move the play marker to that bar (jumps there while playing); drag = scrub, `Alt`+click = exact row |
| `Enter` / `Ctrl+Enter` | restart block / play from the cursor row |
| `Esc` | edit mode on / off |
| `Delete`, `Backspace`, `Insert` | clear, pull up, push down |
| `Shift`+arrows, mouse drag, `Ctrl+A` | select (again = whole block) |
| `Ctrl+C` / `Ctrl+X` / `Ctrl+V` | copy / cut / paste |
| `Ctrl+Z` / `Ctrl+Y` | undo / redo |
| `Alt +` / `Alt -` (Shift = octave) | transpose selection or channel (also the Block bar **-12 -1 +1 +12** buttons and **Edit › Transpose**, which has *Transpose by…* for -24…+24) |
| `F9` / `F10` | mute / solo channel |
| `Ctrl+T`, `Ctrl+3`…`5`, `Ctrl+Tab` | Timeline / Mixer / FX / Sample view (`Ctrl+1` / `Ctrl+2`: the Tracker / Order list, also View › Tracker) |
| `Ctrl+6` | the default layout: Timeline on top, piano roll below (also View › Reset to default layout) |
| `Ctrl+8` | Clip Arranger on / off: scenes and slots left of the Timeline (also View › Clip Arranger; off shows the Instruments list there) |
| `Ctrl+T` | Timeline view (tracks and clips; in timeline mode drag, loop, split, copy clips, [TIMELINE.md](docs/TIMELINE.md)) |
| `Ctrl+Shift+F` | Fill to length |
| `Ctrl+Shift+PgDn` / `Ctrl+Shift+PgUp` | double / halve the block length (or the Length field in the Block bar) |
| `Ctrl+N` / `Ctrl+O` / `Ctrl+S` | new / open / save |
| `Ctrl+E` / `Ctrl+Shift+E` | quick WAV export / export dialog |
| `Ctrl+,` | settings |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | UI scale up / down / 100 % |
| `Ctrl+I` | instrument editor in the lower panel (again: back to the Order list) |
| `R` (not in the Tracker) | Record: live into the Timeline, from the play marker (again: stop recording). Toolbar ● and **Click** (metronome, count-in) |
| `Ctrl+K` | musical typing on / off: `A S D F G H J K L ;` white keys, `W E T Y U O P` black keys, `Z` / `X` octave, `C` / `V` velocity |
| `I` (`Alt+I` in the Tracker) | show / hide the Inspector: the selected track or clip ([INSPECTOR.md](docs/INSPECTOR.md)) |
| `Ctrl+B` | show / hide the Browser (instruments, plugins, samples, songs…; [BROWSER.md](docs/BROWSER.md)) |
| `Ctrl+F` | search the instrument's parameters |
| `F11` | Full-height view: hide / show the lower panel |
| `F12` | the full key list (Help > Keys) |

```
 ═══[ 05 ]═══ BUILD ════════════════════════════════════════════════════════
```

```
cargo run --release                                    # Linux (needs libasound2-dev)
cargo build --release --target x86_64-pc-windows-gnu   # Windows exe from Linux (mingw-w64)
chipwave --self-test [out.wav]                         # headless check: renders the demo
chipwave --render demo out.mp3                         # render the demo (or a .cwt) to WAV/MP3
chipwave --check-updates                               # ask GitHub for a newer version
scripts/screenshots.sh && scripts/record_gifs.sh       # regenerate the pictures on this page
```

```
 ═══[ 06 ]═══ ROADMAP (PLANNED, NOT IN THE APP YET) ═════════════════════════
```

- MED (MMD0–MMD3) and MOD loading, more OctaMED effect commands, optional “Paula” mode
- More filter models (Sallen-Key, diode), wavetables, a full mod matrix, render-to-sample for synth and Voice instruments (generators already Bake), oversampled FM
- MIDI output and clock sync (CLAP instruments work since 0.2.3.16; plugin effects and VST3 hosting are built, unreleased)
- Stem export and LUFS-target normalisation; MOD/XM export
- Polyphony within one channel (today chords use several channels)
- See [OCTAMED_SPEC.md](docs/OCTAMED_SPEC.md), [SOUND_DESIGN.md](docs/SOUND_DESIGN.md) and [MIDI_VST.md](docs/MIDI_VST.md) for the plans.

<a id="documentation"></a>
## Documentation

- [ROADMAP.md](docs/ROADMAP.md) — things to add in the future, with good first picks; [CONTRIBUTING.md](CONTRIBUTING.md) — how to build, test and send a pull request
- [TRACKER.md](docs/TRACKER.md) — the core Tracker, keymap, 32 channels and all effect commands (00–43)
- [PACKAGING.md](docs/PACKAGING.md) — Windows/Linux/macOS packages and CI
- [MACOS.md](docs/MACOS.md): the universal Mac build, installing it, first launch, and building it
- [STARTER_SOUNDS.md](docs/STARTER_SOUNDS.md) — the psytrance and alien factory presets
- [SAFETY.md](docs/SAFETY.md) — autosave, crash recovery and recent files
- [MIXER.md](docs/MIXER.md) — the mixer, master limiter and LUFS meter (0.2.3.7)
- [AUTOMATION.md](docs/AUTOMATION.md) — automation lanes
- [FIRST_RUN.md](docs/FIRST_RUN.md) — the self-building first-run demo and the demo songs
- [SETTINGS.md](docs/SETTINGS.md) — settings
- [OCTAMED_SPEC.md](docs/OCTAMED_SPEC.md) — the full OctaMED feature blueprint
- [SOUND_DESIGN.md](docs/SOUND_DESIGN.md) — the synth and effects
- [FX.md](docs/FX.md) — the FX devices (Phaser and the rest)
- [ORDER_LIST.md](docs/ORDER_LIST.md) — the Order list (the order list as a lane of blocks)
- [TIMELINE.md](docs/TIMELINE.md) — the Timeline view (Ctrl+T) and timeline mode: arrange tracks and clips like a DAW
- [CLIP_ARRANGER.md](docs/CLIP_ARRANGER.md) — the Clip Arranger: scenes and slots left of the Timeline, launched on the next bar, and the Master panel
- [INSPECTOR.md](docs/INSPECTOR.md) — the Inspector (I) and the view tab bar along the bottom
- [BROWSER.md](docs/BROWSER.md) — the Browser (Ctrl+B): find and load instruments, plugins, samples, songs, presets, Grid patches and wavetables; drag them onto tracks; Find plugins
- [DAW_TIMELINE_PLAN.md](docs/DAW_TIMELINE_PLAN.md) — the DAW timeline plan and milestones
- [AUTO_ARRANGE.md](docs/AUTO_ARRANGE.md) — fill-to-length auto arrange and quick-start fun features
- [EXPORT.md](docs/EXPORT.md) — WAV/MP3 export
- [FILE_FORMAT.md](docs/FILE_FORMAT.md) — the `.cwt` song format
- [UI_SCALING.md](docs/UI_SCALING.md) — screen scaling and layouts
- [INSTRUMENT_EDITOR.md](docs/INSTRUMENT_EDITOR.md) — the lower-panel instrument editor: knobs, tabs, cards, search, A/B, Full-height view (0.2.3.19)
- [SAMPLES.md](docs/SAMPLES.md) — the instrument list, importer and chop / slice (0.2.3.10)
- [INSTRUMENT_LIBRARY.md](docs/INSTRUMENT_LIBRARY.md) — instrument library, `.cwi` files, rename / reorder / swap (0.2.3.9)
- [CHORDS.md](docs/CHORDS.md) — chord helper and scale lock (0.2.3.9)
- [VOICE.md](docs/VOICE.md) — the Voice (glitch) instrument: vowels, babble, stutter, words, Throat mode, Transform and chrome FX devices, effects 30–42 (0.2.3.6-11)
- [GENERATORS.md](docs/GENERATORS.md) — generators: Drum Synth, kits, Chip FX (sfxr), Analog, FM-4, Noise, Dice, Bake, Moog ladder filter, effect 43xy (0.2.3.12-14)
- [MIDI.md](docs/MIDI.md) — MIDI keyboard input: play, record, velocity, sustain, MIDI learn (0.2.3)
- [MIDI_VST.md](docs/MIDI_VST.md) — MIDI and plugins (plan)
- [PLUGIN_MANAGER.md](docs/PLUGIN_MANAGER.md) — the plugin manager (scan, verify, activate)
- [DESIGN_LANGUAGE.md](docs/DESIGN_LANGUAGE.md) — ChipWave's "Wave Cartridge" visual identity
- [UPDATES.md](docs/UPDATES.md) — the update check (0.2.3.5)
- [CHANGELOG.md](docs/CHANGELOG.md) — what changed in each version

## Credits

Created by **Del (postmanpat)**. To keep the software going I need Coffee :)

<p>
  <a href="https://buymeacoffee.com/postmanpat"><b>☕ Buy me a coffee</b></a> &nbsp;·&nbsp;
  <a href="https://paypal.me/derekh100"><b>PayPal</b></a> &nbsp;·&nbsp;
  <a href="https://www.youtube.com/@whatgoesaround1551"><b>▶ YouTube: demos &amp; promos</b></a>
</p>

### Acknowledgements / Credits

ChipWave stands on the shoulders of open source and published research. With thanks to:

- **Emilie Gillet / Mutable Instruments**: the Voice (glitch) instrument's design is inspired by the [Plaits speech engine](https://github.com/pichenettes/eurorack/tree/master/plaits/dsp/speech) (“naive speech synth”, MIT). No code is copied; the MIT notice is in `THIRD_PARTY_LICENSES.txt`.
- **The Csound community**: vowel formant values from the [Csound manual's formant table](https://csound.com/docs/manual/MiscFormants.html) ([Csound](https://github.com/csound/csound), LGPL-2.1).
- **G. E. Peterson & H. L. Barney** (1952), [“Control methods used in a study of the vowels”](https://doi.org/10.1121/1.1906875): the ER and AE vowels.
- **A. E. Rosenberg** (1971), [glottal pulse shape](https://doi.org/10.1121/1.1912389), and **G. Fant, J. Liljencrants & Q. Lin** (1985), the LF glottal flow model: the voice's pulse and “tension”.
- **Dennis H. Klatt** (1980), [“Software for a cascade/parallel formant synthesizer”](https://doi.org/10.1121/1.383940): the parallel formant structure.
- **Vadim Zavalishin**, *The Art of VA Filter Design*: the TPT state-variable filter used throughout the synth, and the zero-delay-feedback all-pass stages of the **Phaser** FX (whose swept-notch structure follows analog phaser pedals such as the MXR Phase 90 and Electro-Harmonix Small Stone; no circuit or code copied).
- **Harald Bode**'s single-sideband frequency shifter and **Olli Niemitalo**'s [Hilbert transformer all-pass coefficients](https://yehar.com/blog/?p=368): the **Frequency shifter** FX (independent implementation; published coefficients only, no code copied).
- **ITU-R BS.1770-4** and **EBU R 128 / Tech 3341**: the loudness meter (K-weighting, gating, momentary / short-term / integrated LUFS) and the 4x oversampled true-peak measurement used by the master limiter.
- **Jan Kokemüller** ([libebur128](https://github.com/jiixyj/libebur128), MIT) and **Christian Steinmetz** ([pyloudnorm](https://github.com/csteinmetz1/pyloudnorm), MIT): the sample-rate-independent K-weighting filter formulas (no code copied).
- **Geraint Luff / Signalsmith Audio**, [“Designing a straightforward limiter”](https://signalsmith-audio.co.uk/writing/2022/limiter/): the peak-hold + moving-average lookahead design of the master limiter.
- **Neil Thapen**, [Pink Trombone](https://dood.al/pinktrombone/) (MIT): the Voice instrument's **Throat** mode follows its vocal-tract design (a Kelly-Lochbaum tube with tongue, lips, nose and turbulence noise). Independent implementation; the MIT notice is in `THIRD_PARTY_LICENSES.txt`.
- **P. Delattre, A. Liberman & F. Cooper** (1955), [“Acoustic loci and transitional cues for consonants”](https://doi.org/10.1121/1.1908024), and **K. N. Stevens**, *Acoustic Phonetics* (1998): consonant formant transitions and noise bands. **J. L. Kelly & C. C. Lochbaum** (1962): the tube model.
- **Julius O. Smith & James B. Angell** (1982), “A constant-gain digital resonator tuned by a single coefficient” ([J. O. Smith, *Introduction to Digital Filters*](https://ccrma.stanford.edu/~jos/filters/)): the Transform's modal resonators. **N. H. Fletcher & T. D. Rossing**, *The Physics of Musical Instruments*: the bell and bar mode ratios; the “metal” ratios are the Roland TR-808 cymbal oscillator frequencies. **S. H. Linkwitz** (1976): the Linkwitz-Riley crossover that keeps the sub mono; the octave divider follows analog octave pedals (Boss OC-2).
- **Vadim Zavalishin**, *The Art of VA Filter Design*, **Antti Huovilainen** (DAFx-04) and **Vesa Välimäki & Antti Huovilainen** (CMJ 2006): the Moog ladder filter (clean and vintage). Cross-checked against [DaisySP](https://github.com/electro-smith/DaisySP)'s ladder (**Richard van Hoesel**, **Infrasonic Audio**, MIT), [fundsp](https://github.com/SamiPerttu/fundsp) (**Sami Perttu**, MIT/Apache-2.0) and [MoogLadders](https://github.com/ddiakopoulos/MoogLadders) (**Dimitri Diakopoulos**, Stefano D'Angelo).
- **Sonic Pi**'s [sc808](https://github.com/sonic-pi-net/sonic-pi) SynthDefs (**Sam Aaron** and contributors, MIT), after **Yoshinosuke Horiuchi**'s SuperCollider TR-808, and **Emilie Gillet**'s [Plaits](https://github.com/pichenettes/eurorack) drum models (MIT) as ported to [DaisySP](https://github.com/electro-smith/DaisySP) by **Electrosmith** (MIT): the Drum Synth recipes. **Bitwig Studio**: the macro + dice workflow idea (no code or assets). **Tomas Pettersson (DrPetter)**'s [sfxr](https://www.drpetter.se/project_sfxr.html) (MIT) via **Teemu Erkkola**'s [sfxr-rs](https://github.com/bzar/sfxr-rs) (MIT): the Chip FX generator. MIT notices are in `THIRD_PARTY_LICENSES.txt`.
- **Robert Moog**'s Minimoog and **Vesa Välimäki & Antti Huovilainen**'s polyBLEP oscillators (IEEE SPM 2007): the Analog generator. **Matt Tytel**'s [Vital](https://github.com/mtytel/vital) (GPL-3) and the **Surge Synth Team**'s [Surge XT](https://github.com/surge-synthesizer/surge) (GPL-3): oscillator drift, noise and macro ideas. **Pascal Gauthier**'s [Dexed](https://github.com/asb2m10/dexed) (GPL-3) and **Raph Levien**'s [MSFA](https://github.com/google/music-synthesizer-for-android) (Apache-2.0), plus the Yamaha DX21/TX81Z algorithms: the FM-4 generator. **Paul Kellet**'s [pink noise filter](https://www.musicdsp.org/en/latest/Filters/76-pink-noise-filter.html), and the Dust generators of [SuperCollider](https://github.com/supercollider/supercollider) and DaisySP: the Noise generator. All of it was written from scratch (no code copied).
- **The free-audio project** (**Alexandre Bique** and contributors): [CLAP](https://github.com/free-audio/clap) (MIT), the plugin standard ChipWave hosts, through **micahrj**'s [clap-sys](https://github.com/micahrj/clap-sys) bindings (MIT/Apache-2.0). Host design references, no code copied: [clack](https://github.com/prokopyl/clack) by **Adrien Prokopowicz** (MIT/Apache-2.0), [Carla](https://github.com/falkTX/Carla) by **Filipe Coelho (falkTX)** and [Ardour](https://ardour.org) (GPL-2.0+). Plugin loading uses [libloading](https://github.com/nagisa/rust_libloading) (**Simonas Kazlauskas**, ISC), editor windows [windows-sys](https://github.com/microsoft/windows-rs) (Microsoft, MIT/Apache-2.0). Tested with [Surge XT](https://surge-synthesizer.github.io) (Surge Synth Team, GPL-3; not bundled).
- **Carol L. Krumhansl & Edward J. Kessler** (1982), [key profiles](https://doi.org/10.1037/0033-295X.89.4.334), and the **Krumhansl–Schmuckler** key-finding algorithm (Krumhansl, *Cognitive Foundations of Musical Pitch*, 1990): the chord helper's key detection.
- **Scaler** by Plugin Boutique: the inspiration for the chord pads and progressions (no code, data or artwork used).
- The **Roland TB-303**, whose glide and accent the `3C` / `3D` effects imitate.
- **Teijo Kinnunen and Ray Burt-Frost** for [OctaMED](https://en.wikipedia.org/wiki/OctaMED), whose workflow ChipWave follows.
- The fonts of the Slate theme, embedded under the [SIL Open Font License 1.1](LICENSES/Inter-OFL.txt): [**Inter**](https://github.com/rsms/inter) by **Rasmus Andersson** (The Inter Project Authors), [**Barlow Semi Condensed**](https://github.com/jpt/barlow) by **Jeremy Tribby** (The Barlow Project Authors) and [**Geist Mono**](https://github.com/vercel/geist-font) by **Vercel** (The Geist Project Authors). Their licences are in `LICENSES/` and `THIRD_PARTY_LICENSES.txt`.
- The Rust crates ChipWave is built on (egui/eframe, cpal, midir, hound, toml, zip, mp3lame-encoder and more), listed with their licences in `THIRD_PARTY_LICENSES.txt`.

Research notes: [docs/research/voice-glitch.md](docs/research/voice-glitch.md).

## Licence

ChipWave Studio is free software, licensed under the [GNU General Public License v3.0](LICENSE) (GPL-3.0-or-later).

## 💛 Thank you

A big thank you to [storytold](https://github.com/storytold/), whose open-source [PhotoCraft](https://github.com/storytold/photocraft) inspired the look of the Wave family icons, and whose [ArtCraft Launcher](https://github.com/storytold/craft-launcher) inspired the upcoming Wave Hub. Open source at its best. Go give them a ⭐!
