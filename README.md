# ChipWave Tracker

A modern music tracker inspired by **OctaMED** and the Commodore **Amiga**, sister project to [BigWave Radio](https://github.com/postmanpat-303/BigWaveRadio).

**Status:** planning. No code yet.

## Goals
- OctaMED-style workflow: blocks, sections and sequences, the classic keyboard layout
- Sample instruments with loops, volume, transpose, finetune and hold/decay
- OctaMED effect commands, including the quirks that give it its sound
- Load MED (MMD0–MMD3) and MOD files, export WAV
- Optional "Paula" mode for authentic Amiga sound
- Windows 11 and Linux, built in Rust with egui and cpal

## Roadmap
1. **MVP:** song, sequences and blocks; sample instruments; core effects; track editor with undo; MMD/MOD load; WAV export
2. **Phase 2:** synth and hybrid sounds, sample editor, MIDI, MOD/XM export
3. **Phase 3:** Rhai scripting/plugins, XM/IT import

See [docs/OCTAMED_SPEC.md](docs/OCTAMED_SPEC.md) for the full OctaMED feature blueprint.
