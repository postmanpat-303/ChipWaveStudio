# Things to add in the future

This is ChipWave Studio's list of things that aren't built yet. It collects the "Not yet" and "Limits" notes from the other guides, plus the leftovers from recent work, so they live in one place. Nothing here is promised or scheduled. It's a list of good ideas waiting for someone to build them.

## Want to chip in?

You're very welcome to. ChipWave is free software (GPL-3.0-or-later), made by Del (postmanpat) and contributors, and help with anything on this list is appreciated, big or small.

1. **Pick an item.** The [Good first picks](#good-first-picks) are a gentle start.
2. **Claim it first:** open an issue (or comment on an existing one) saying what you'd like to take on, so two people don't build the same thing.
3. **Send a pull request** when it works. [CONTRIBUTING.md](../CONTRIBUTING.md) has the build steps and the checks CI runs.

Each item has a size, **[easy]** (an evening or two), **[medium]** (a few days) or **[big]** (a week or more, best talked through in the issue first), and a **start:** pointer to the file where the work would begin. *(Del asked)* marks things Del has asked for.

## Good first picks

- Instrument card: remove the duplicated name header. [easy] start: `src/app/instpanel.rs`
- Ask before replacing an instrument's sound (confirm dialog). [easy] start: `src/app/library.rs`
- Advanced Edit: paste to the start of the chosen section, not only at the cursor. [easy] start: `src/app/advedit.rs`
- Channel header Play: play the channel's current note instead of always C-4. [easy] start: `src/app/pattern.rs`
- Piano roll: Space+drag to pan, as an alternative to hold-to-pan. [easy] start: `src/app/pianoview.rs`
- Plugin deep scan: skip (or time out on) network drives. [easy] start: `src/plugin/scan.rs`
- Compressor: a gain-reduction meter on its device card. [easy] start: `src/app/devices.rs`
- Transpose: skip drum channels marked "no transpose". [easy] start: `src/app/edit.rs`
- Audio clips: reverse. [easy] start: `src/engine/audioclips.rs`
- Export: optional dither. [easy] start: `src/export.rs`
- macOS: open a `.cwt` song by double-clicking it in Finder. [easy] start: `packaging/macos/Info.plist.in`

## Piano roll

- Space+drag to pan, as an alternative to hold-to-pan. [easy] start: `src/app/pianoview.rs`
- Tracker block columns in the piano roll: resize a selected group of notes together. [medium] start: `src/timeline/roll.rs`
- An automation strip under the piano roll. [medium] start: `src/app/pianoview.rs`
- Play note expressions and release velocity. [medium] start: `src/engine/notes.rs`

## Tracker

- Advanced Edit: interpolate. [medium] start: `src/advedit.rs`
- Advanced Edit: humanise. [medium] start: `src/advedit.rs`
- Advanced Edit: paste to the start of the chosen section. [easy] start: `src/app/advedit.rs`
- Channel header Play: play the channel's current note instead of C-4. [easy] start: `src/app/pattern.rs`
- Channel headers: a mini level meter. [medium] start: `src/app/pattern.rs`
- Channel headers: collapse a channel to its note column. [medium] start: `src/app/pattern.rs`
- Transpose: skip drum channels marked "no transpose". [easy] start: `src/app/edit.rs`
- Polyphony within one channel. [big] start: `src/engine.rs`
- Load OctaMED (MMD0–MMD3) and ProTracker MOD songs. [big] start: `src/fileio.rs`
- More OctaMED effect commands and an optional "Paula" mode (see [OCTAMED_SPEC.md](OCTAMED_SPEC.md)). [medium] start: `src/engine.rs`

## Timeline and arrangement

- Range selection with insert / delete time. [medium] start: `src/app/timeline/edit.rs`
- Snap clips to other clips' edges and to markers. [medium] start: `src/app/timeline/edit.rs`
- Turn note clips back into blocks, so such songs can go back to tracker mode or export a tracker copy. [big] start: `src/timeline/migrate.rs`
- Import MIDI files into note clips. [medium] start: `src/timeline.rs`
- Show the effect of `0Fxx` tempo commands inside blocks on the ruler's time. [medium] start: `src/timeline.rs`
- Chase: replay slides that are running at a seek point, and re-strike plugin notes. [medium] start: `src/engine.rs`
- Send plugin notes sample-accurately (today they go out at engine-tick boundaries) and with CLAP note ids. [medium] start: `src/engine/notes.rs`
- Audio clips: time-stretch and repitch to tempo (Warp). [big] start: `src/engine/audioclips.rs`
- Audio clips: reverse. [easy] start: `src/engine/audioclips.rs`
- Audio clips: automatic crossfades between overlapping clips. [medium] start: `src/engine/audioclips.rs`
- Audio clips: Slice to sampler, and Collect and save. [medium] start: `src/app/timeline/audioclip.rs`
- Stream long audio files from disk instead of holding them in memory (M4b). [big] start: `src/audiofile.rs`
- Per-track and per-clip FX chains. [big] start: `src/fx.rs`
- Automation: record from MIDI controllers and plugin editors, with point thinning. [medium] start: `src/automation/mod.rs`
- Automation: a tempo-map lane and "record knobs" in timeline mode. [medium] start: `src/automation/mod.rs`
- Automation: convert between lanes and effect commands. [medium] start: `src/automation/mod.rs`
- Automation: rubber-band multi-select and copy of points. [medium] start: `src/app/timeline/autolanes.rs`
- Save CPU: freeze a track to audio, or deactivate it (no processing), from the track pie. [big] start: `src/engine.rs`
- FX (bus) tracks and track groups, so the track pie's Add › FX track and Organize › Group can work. [big] start: `src/timeline.rs`
- Use the track arm button (it is in the model but does nothing yet). [medium] start: `src/timeline.rs`
- New songs: don't keep the unused empty block 00. [easy] start: `src/model.rs`
- Clip launcher with scenes (M6, optional). [big] start: `src/timeline.rs`
- DAWproject import and export (M7, optional). [big] start: `src/fileio.rs`
- Order list variation lanes (optional, see [ORDER_LIST.md](ORDER_LIST.md)). [big] start: `src/app/arranger.rs`

## Browser and plugins

- Drum Patterns: a step editor with Euclid, Rotate and Random. *(Del asked)* [medium] start: `src/drumpat.rs`
- Drum Patterns: preview through the track's own instrument and FX. [medium] start: `src/engine/patprev.rs`
- Preview plugins in the Browser. [medium] start: `src/app/browserview.rs`
- List plugin presets (`.vstpreset`, CLAP preset files). [medium] start: `src/browser.rs`
- A waveform and a tempo-synced preview for samples. [medium] start: `src/app/browserpanel.rs`
- Keep file ratings and favourites when a file is moved (today they're keyed by path). [medium] start: `src/browser.rs`
- Drop onto the mixer view or onto a chosen FX slot position. [medium] start: `src/app/browserview.rs`
- Your own tags. [medium] start: `src/browser.rs`
- Plugin thumbnails on macOS and Linux, and less flashing while an editor is captured. [medium] start: `src/plugin/thumb.rs`
- Embedded plugin editors (VST3 included) on macOS and Linux. [big] start: `src/plugin/gui.rs`
- VST3 program lists and a preset browser. [medium] start: `src/plugin/vst3.rs`
- VST3 MIDI CC through `IMidiMapping`. [medium] start: `src/plugin/vst3.rs`
- Note expressions beyond tuning, and polyphonic modulation. [medium] start: `src/plugin/clap.rs`
- Plugin sidechain inputs, MIDI output and extra output buses. [big] start: `src/plugin/mod.rs`
- Undo steps for changes made in a plugin's own editor. [medium] start: `src/plugin/mod.rs`
- Latency compensation for plugin instruments and live playing. [medium] start: `src/plugin/fxrack.rs`
- LV2 plugin hosting (plan in [PLUGIN_MANAGER.md](PLUGIN_MANAGER.md)). [big] start: `src/plugin/mod.rs`

## Devices and effects

- Compressor: a gain-reduction meter. [easy] start: `src/app/devices.rs`
- Preview an effect before adding it. [medium] start: `src/app/devices.rs`
- FX chains on the return buses and the master, so plugin effects can go there. [big] start: `src/fx.rs`
- MIDI learn for plugin effect parameters. [medium] start: `src/plugin/fxrack.rs`
- Describe plugin devices in `.cwt.toml` text songs. [medium] start: `src/fileio.rs`
- Grid: container sub-patches, a sample player, step sequencer, envelope follower, Voice and chip modules. [big] start: `src/grid.rs`
- Grid: automatable mode menus, and Grid FX knobs inside the patch. [medium] start: `src/grid.rs`
- Grid: canvas zoom, module help pages and patch files of their own. [medium] start: `src/grid.rs`

## Instruments and synths

- Ask before replacing an instrument's sound. [easy] start: `src/app/library.rs`
- Instrument card: remove the duplicated name header. [easy] start: `src/app/instpanel.rs`
- More filter models: Sallen-Key, diode, comb and formant. [medium] start: `src/ladder.rs`
- Render synth and Voice instruments to samples. [medium] start: `src/engine.rs`
- Oversampled FM. [medium] start: `src/gen.rs`
- Per-voice modulation (today modulators are per instrument). [big] start: `src/modulation.rs`
- Modulator-to-modulator routing (an LFO on another LFO's rate). [medium] start: `src/modulation.rs`
- Modulate plugin instrument macros, tempo and discrete settings. [medium] start: `src/modulation.rs`
- Mixer faders and return buses as modulation drop targets. [medium] start: `src/app/mixer.rs`
- Keep modulators in `.cwi` instrument files and the instrument library. [medium] start: `src/instfile.rs`
- Wave: warp modes (bend, sync, FM from B) and spectral morphing. [big] start: `src/gen_wave.rs`
- Wave: draw a table, or turn a ChipWave sample into one. [medium] start: `src/wavetable.rs`
- Voice: richer consonants (l, r, diphthongs). [medium] start: `src/voice.rs`
- Voice: audible voiced closures (b, d, g) in Throat mode. [medium] start: `src/voice.rs`
- Voice: stutter live MIDI notes too. [medium] start: `src/voice.rs`
- Transform: let resonators ring across notes. [medium] start: `src/transform.rs`

## Audio engine and performance

- More audio hosts and modes: PipeWire, ALSA or JACK on Linux, WASAPI exclusive and ASIO on Windows, an input device. [big] start: `src/audio.rs`
- A live audio panel with round-trip latency and a test tone. [medium] start: `src/app/settingsview.rs`
- A multi-threaded engine (experimental). [big] start: `src/engine.rs`
- Stem export and LUFS-target normalisation. [medium] start: `src/export.rs`
- Optional export dither. [easy] start: `src/export.rs`
- MOD / XM export. [big] start: `src/export.rs`
- MIDI output and clock sync (send and receive). [medium] start: `src/midi.rs`
- More MIDI settings: thru, per-port channel filters, velocity curve, later MPE. [medium] start: `src/midi.rs`
- Windows MIDI Services (the MIDI 2.0 stack) once a Rust binding is ready. [big] start: `src/midi.rs`

## Files and drives

- Files tab: a folder tree, and rename, move and delete. [medium] start: `src/app/drumpat.rs`
- Plugin deep scan: skip (or time out on) network drives. [easy] start: `src/plugin/scan.rs`
- FLAC-compressed samples inside `.cwt` files (an option). [medium] start: `src/fileio.rs`

## Platforms

- Play-test the Mac build on real hardware (nobody has yet). [medium] start: `docs/MACOS.md`
- macOS: open a `.cwt` by double-clicking it in Finder. [easy] start: `packaging/macos/Info.plist.in`
- Test plugin hosting (CLAP and VST3) on macOS. [medium] start: `src/plugin/vst3.rs`

## Look and feel

- More radial menus beyond the chord and track pies, e.g. for clips (an idea, not picked yet). [medium] start: `src/app/radial.rs`
- A navy colour theme (an idea, not picked yet). [medium] start: `src/theme.rs`

## ChipWave Lite (Amiga)

- M2 Editor: screen, pattern view, keys, order and instrument lists, play / stop, mute / solo, undo (next, after Del's WinUAE test of M1). [big] start: `amiga/src/main.c`
- M3 Load / save: MOD save, 8SVX load and Lite's own file requester. [big] start: `amiga/src/core/mod.c`
- M3 Exchange: PC ChipWave's "Export Amiga 4-channel (.mod)" and "Import .mod". [big] start: `src/fileio.rs`
- M4 Packaging: a bootable ADF and an `.lha` with icons and demo songs. [medium] start: `amiga/pkg/make_dist.sh`
- Later (v0.2+): OctaMED MMD0, an IFF song format with ChipWave extras, 8-channel software mixing, a sample editor, MIDI in over serial, an AGA skin. [big] start: `docs/AMIGA_LITE_PLAN.md`

## Website and docs

- Grid module help pages. [easy] start: `docs/GRID.md`
- A guide to the Mac build written from a real test run, once there is one. [easy] start: `docs/MACOS.md`

---

Where these came from: the Limits and Not yet notes in [ADVANCED_EDIT.md](ADVANCED_EDIT.md), [BROWSER.md](BROWSER.md), [TIMELINE.md](TIMELINE.md), [DAW_TIMELINE_PLAN.md](DAW_TIMELINE_PLAN.md), [PLUGINS.md](PLUGINS.md), [FX.md](FX.md), [GRID.md](GRID.md), [VOICE.md](VOICE.md), [MODULATION.md](MODULATION.md), [GENERATORS.md](GENERATORS.md), [SOUND_DESIGN.md](SOUND_DESIGN.md), [SETTINGS.md](SETTINGS.md), [MIDI_VST.md](MIDI_VST.md), [TRACKER.md](TRACKER.md), [MACOS.md](MACOS.md), [AMIGA_LITE_PLAN.md](AMIGA_LITE_PLAN.md) and the README's Roadmap. When you finish an item, remove it here and from the guide's Limits list in the same pull request.
