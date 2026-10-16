# ChipWave Tracker

A modern music tracker inspired by **OctaMED** and the Commodore **Amiga**, sister project to [BigWave Radio](https://github.com/postmanpat-303/BigWaveRadio).

**Status:** planning. No code yet.

## Goals
- OctaMED-style workflow: blocks, sections and sequences, the classic keyboard layout
- Sample instruments with loops, volume, transpose, finetune and hold/decay
- OctaMED effect commands, including the quirks that give it its sound
- Load MED (MMD0–MMD3) and MOD files; export WAV and MP3 (song, range, block, stems)
- OctaMED-style instrument list (255 slots) with a sample browser: audition, drag-and-drop, batch import of WAV/AIFF/FLAC/MP3/OGG/IFF 8SVX and samples from modules
- MIDI in/out with MIDI learn, and CLAP/VST3 instrument and effect hosting (sandboxed)
- Native `.cwt` song format: an inspectable ZIP of TOML and WAV files that holds everything
- Optional "Paula" mode for authentic Amiga sound
- **Built-in sound design:** analogue-style synth with Bitwig-inspired filters (SVF, ladder, Sallen-Key…), drag-to-modulate envelopes/LFOs, per-sound FX chain, and render-to-sample
- **Visual arranger:** blocks as coloured clips on a side-scrolling timeline, mapped to OctaMED sections and playing sequences
- Windows 11 and Linux, built in Rust with egui and cpal
- Scales cleanly on any screen: OS DPI and fractional scaling, a user UI-scale setting, vector-drawn widgets, responsive layouts from 1366×768 laptops to 4K, ultrawide and portrait, plus an optional Amiga look

## Roadmap
1. **MVP:** song, sequences and blocks; sample instruments; instrument list + sample browser/importer (preview, drag-and-drop, batch import, core formats, slot remap); core effects; track editor with undo; simple sequence list editor; MMD/MOD load; native `.cwt` save/load; UI scaling basics (OS + user scale, crisp vector widgets and grid text, compact/standard layouts, window modes); WAV and MP3 export (MP3 reuses the same offline renderer, so it's little extra work, and it's the format people actually share)
2. **Phase 1.5:** sound design v1 (synth, SVF + ladder filters, envelopes, LFOs, drag-to-modulate, 4 FX, render-to-sample, TOML presets) and the visual arranger; stem export, loudness normalisation; advanced import options (normalise, resample, Paula crunch, trim, auto loop), module sample extraction; MIDI input (keyboard entry, record, MIDI learn); dockable panels, ultrawide/portrait layouts, per-screen layouts, high-contrast and colour-blind themes. Both sit on top of the proven MVP core (the synth is just a new instrument type, the arranger a richer view of the same sequence data), so they don't risk the playback or file format work.
3. **Phase 2:** OctaMED synth and hybrid sounds, more filter models and FX, wavetables, full mod matrix, sample editor, MIDI output + clock sync, CLAP/VST3 plugin hosting, MOD/XM export, Amiga-mode retro look
4. **Phase 3:** Rhai scripting/plugins, XM/IT import, arranger variation lanes, ChipWave itself as a CLAP/VST3 plugin (nih-plug)

See [docs/OCTAMED_SPEC.md](docs/OCTAMED_SPEC.md) for the full OctaMED feature blueprint, [docs/SOUND_DESIGN.md](docs/SOUND_DESIGN.md) for the synth and effects, [docs/ARRANGER.md](docs/ARRANGER.md) for the timeline, [docs/AUTO_ARRANGE.md](docs/AUTO_ARRANGE.md) for fill-to-length auto arrange and quick-start fun features, [docs/EXPORT.md](docs/EXPORT.md) for WAV/MP3 export, [docs/FILE_FORMAT.md](docs/FILE_FORMAT.md) for the `.cwt` song format, and [docs/UI_SCALING.md](docs/UI_SCALING.md) for screen scaling and layouts, [docs/SAMPLES.md](docs/SAMPLES.md) for the instrument list and importer, [docs/MIDI_VST.md](docs/MIDI_VST.md) for MIDI and plugins, [docs/PLUGIN_MANAGER.md](docs/PLUGIN_MANAGER.md) for the plugin manager (scan, verify, activate), and [docs/DESIGN_LANGUAGE.md](docs/DESIGN_LANGUAGE.md) for ChipWave's "Wave Cartridge" visual identity.

![Sound designer mockup](docs/mockups/sound_designer.png)
![FX chain mockup](docs/mockups/fx_chain.png)
![Arranger mockup](docs/mockups/arranger.png)

## Licence

ChipWave Tracker is free software, licensed under the [GNU General Public License v3.0](LICENSE) (GPL-3.0-or-later).
