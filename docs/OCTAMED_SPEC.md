# OctaMED Feature Specification — Blueprint for a Modern Clone
*Compiled 16 Oct 2026 for Del. Facts marked **[src]** come straight from primary sources checked in /workspace/octamed-spec. Facts marked **[recall – verify]** are well-known community knowledge I could not confirm against a primary document in this pass.*

## Sources
| # | Source | Used for |
|---|---|---|
| S1 | Teijo Kinnunen, *MED/OctaMED MMD0/MMD1/MMD2 file formats*, rev 4, 18.05.1995 — copy in [KONEY repo `MMD_FileFormat.txt`](https://github.com/KONEY/octamed_playroutines_amiga/blob/main/MMD_FileFormat.txt) | File format, flags, instrument types, synth structs |
| S2 | [OpenMPT `soundlib/Load_med.cpp`](https://github.com/OpenMPT/openmpt/blob/master/soundlib/Load_med.cpp) | Effects, tempo maths, MMD3, synth-language opcodes, quirks |
| S3 | [libxmp `src/loaders/mmd_common.c`, `mmd1_load.c`, `mmd3_load.c`, `med2/3/4_load.c`](https://github.com/libxmp/libxmp/tree/master/src/loaders) | Effect descriptions (quoted from OctaMED Pro manual text), MED2–4 support, tempo |
| S4 | [KONEY/octamed_playroutines_amiga](https://github.com/KONEY/octamed_playroutines_amiga) (based on Aminet [dev/src/OctaMED-R](http://aminet.net/package/dev/src/OctaMED-R)) | Playback, synth tests |
| S5 | Aminet [mus/edit/OctamedSS1.03c](http://aminet.net/package/mus/edit/OctamedSS1.03c) (last Amiga SoundStudio), [mods/instr/bc-medsounds](https://aminet.net/package/mods/instr/bc-medsounds) (SynthSounds doc) | Versions, synth docs (referenced, not fully read) |
| S6 | [KONEY/octamed-arexx-repo](https://github.com/KONEY/octamed-arexx-repo) | ARexx scripts examples |
| S7 | crates.io API (queried today) | Rust crate existence/versions |

**Gap:** I did not obtain the full OctaMED SoundStudio manual text in this pass (it exists on Aminet/archive.org as AmigaGuide, e.g. inside the OctamedSS archive). UI and editor details below are partly [recall – verify]; reading that manual is the #1 follow-up.

---
## 1. Version history (brief)
- **MED** (Teijo Kinnunen, ~1989–91) – 4-channel Amiga tracker. MED 2.0 had only "Sng+samples" format; **MMD0** was introduced in MED V2.1 [src S1]. libxmp still loads MED2/MED3/MED4 legacy formats [src S3].
- **OctaMED** (~1990/91, with Ray Burt-Frost involvement in distribution) – added **5–8 channel software mixing** (flag `FLAG_8CHANNEL 0x40`, "OctaMED 5-8 channel song") [src S1]; dates [recall – verify].
- **OctaMED Professional (V3–V6)** – **MMD1** format (larger blocks: up to 3200 lines, more tracks), **MMD2** (multiple player commands per note via *command pages*, "OctaMED V6"; play sequences/sections) [src S1]. V5+ added **ExtSample** (two extra-low octaves) [src S1]. MIDI, ARexx [recall – verify for exact version].
- **OctaMED SoundStudio** (V7, mid-90s, 1.03c last Amiga 68k release) – 1–64 tracks with mixing mode, 16-bit/stereo samples, saves **MMD3** only; can load MMD0–2 [src S3 comment: "can load MMD0-2 modules, but can only save MMD3"]; Amiga version later released free (CU Amiga cover disk) [recall – verify].
- **MED SoundStudio for Windows** (~1997–2000, by RBF Software) – PC port; OpenMPT notes fields "saved by MED Soundstudio for Windows" and tempo behaviour differences [src S2].

## 2. Core concepts
- **Blocks** – OctaMED's patterns. Each block has its own **number of tracks and lines** (not fixed 64 like MOD). MMD0: up to 256 lines/byte counts; MMD1+: up to 3200 lines/block in OctaMED Pro [src S1].
- **Sequences** – MMD0/1: one play sequence (list of block numbers, *songlen*). MMD2+: **Sections** → a section list references **Playing Sequences** (named, reusable lists of blocks) — lets you build song structure hierarchically (`psecnum`, `pseq` in header) [src S1].
- **Multi-modules** – one file holds several songs sharing samples (`extra_songs`, `nextmod`, `MCNT` ids) [src S1].
- **Tracks / channels**:
  - 4-channel mode → each track maps to an Amiga Paula hardware voice (hard-panned L R R L) [recall – verify panning order; OpenMPT notes Soundstudio "uses Amiga hard-panning" when track pan is 0 – src S2].
  - 5–8 channel mode (OctaMED) → software mixed on the 4 Paula voices; restricted tempo table and lower quality [src S1/S2].
  - SoundStudio mixing mode → **1–64 tracks**, 16-bit, per-track panning (`trackPanOffset` MMD2/3) [src S2].
- **Timing**: `deftempo` (main tempo slider), `tempo2` = **secondary tempo / ticks per line** ("timing pulses per line"), flags2: `FLAG2_BPM 0x20` BPM mode and bits 0–4 = **lines per beat** [src S1].
  - Tempo conversion quirks (from S2/S3): tempo 1–10 in non-BPM 4-ch mode = **SoundTracker compatible** (BPM = 6·1773447/14500/tempo); 8-channel mode uses a fixed table **{179,164,152,141,131,123,116,110,104,99} BPM** for tempos 1–10; BPM mode BPM = tempo·LPB/4; otherwise BPM = tempo/0.264. Bugs observed: BPM-mode tempo<7 → 111.5 BPM; SoundStudio 1–64ch mixing with tempo<8 → 157.86 BPM [src S2].
- **Global transpose** (`playtransp`) and per-instrument transpose (`strans`) [src S1].

## 3. Editor
[recall – verify against SoundStudio manual unless noted]
- **Track editor**: main screen showing block rows; columns per track = note, instrument number, command+argument (MMD0: 1 cmd per note; MMD2+: multiple *command pages* per note [src S1]).
- **Keyboard layout**: PC/Amiga "piano" layout — two rows (Z–M lower octave, Q–P upper), F1–F5 select octave base; Space toggles edit mode; Del clears; Return/arrow navigation. Amiga keypad used for instrument selection.
- **Edit modes**: edit on/off, chord/"spread" mode across tracks, step (edit-skip lines), "jumping" mouse pointer & jump every 8th line (old MED flags `FLAG_JUMPINGON`, `FLAG_JUMP8TH`, "not in OctaMED Pro") [src S1].
- **Range operations**: mark range in block → cut/copy/paste/clear, transpose up/down (semitone/octave), change instrument in range, expand/shrink (double/halve line spacing), swap tracks, split/join blocks, block insert/delete/duplicate.
- **Spread** – spreading notes/commands across tracks or interpolating command values over a range (e.g. volume ramps).
- **Undo** – SoundStudio had undo buffer [recall – verify]; earlier versions limited.
- **Track mute/solo**, track volumes (`trkvol` in song struct) [src S1 for trkvol].

## 4. Instruments
Instrument types from `MMD0sample/InstrHdr.type` [src S1]:
| Value | Type |
|---|---|
| -2 | HYBRID |
| -1 | SYNTHETIC |
| 0 | SAMPLE (ordinary 1-octave) |
| 1 / 2 | IFF 5-octave / 3-octave |
| 3–6 | IFF 2/4/6/7-octave (OctaMED Pro only) |
| 7 | EXTSAMPLE (two extra-low octaves, Pro V5+) |
V6+ adds flag bits in type (16-bit `S_16`, stereo) [src S1; exact bit meanings in doc].

Per-instrument parameters [src S1, S2]: loop start/length (`rep`, `replen`, words), **MIDI channel** & **preset** (`midich`, `midipreset`, `long_midi_preset`), **volume**, **transpose** (`strans`), **hold** and **decay** (InstrExt), **suppress MIDI off**, **finetune −8…+7**, `instr_flags` (loop on/off, ping-pong loop in SS, disabled), default pitch (`default_pitch`, used by 0FFD/"set pitch"), names in MMDInstrInfo.
- **Hold/Decay**: note plays full volume for *hold* ticks, then fades at *decay* rate — OctaMED's mini-envelope; also settable per note with cmd 08.
- **Multi-octave IFF samples**: one instrument with several octave-length copies, OctaMED picks the right one per note (Amiga trick to cover range at Paula's sample-rate limits).

### Synth sounds (SynthInstr) [src S1, S2]
- Up to **64 waveforms** (`wf[64]`, each short 8-bit wave, lengths in words), a **Volume table** (`voltbl[128]`) and **Waveform table** (`wftbl[128]`) — two parallel tiny programs, each with its own speed (`volspeed`, `wfspeed`).
- Hybrid sound = synth sound whose `wf[0]` is a full sample (sequence language applied to a sample) [src S1].
- **Sequence language opcodes** (byte ≥ 0xF0 = command; smaller values = data: volume 0–64 in vol table, waveform number in wf table) [src S2 lines ~714–777]:

| Byte | Vol table | WF table |
|---|---|---|
| FF | END | END |
| FE | JMP n | JMP n |
| FD | – | ARE (end arpeggio) |
| FC | – | ARP (begin arpeggio def.) |
| FB | HLT | HLT |
| FA | JWS (jump wf seq) | JWV (jump vol seq) |
| F7 | – | VWF (vibrato waveform) |
| F6 | EST (? – unclear) | RES (reset pitch) |
| F5 | EN2 (looping envelope) | VBS (vibrato speed) |
| F4 | EN1 (one-shot envelope) | VBD (vibrato depth) |
| F3 | CHU (vol change up speed) | CHU (pitch up speed) |
| F2 | CHD (vol down speed) | CHD (pitch down speed) |
| F1 | WAI n (wait) | WAI n |
| F0 | SPD n | SPD n |
Effect **0E** "synth jump" jumps the WF sequence to a line [src S3]. Full semantics: bc-medsounds doc (S5).

## 5. Effect commands
MMD0/1 commands are 4-bit (0–F); MMD1+ 8-bit (00–FF) [src S1]. Meanings from libxmp (quoting OctaMED Pro docs) and OpenMPT [src S2, S3]:

| Cmd | Name | Notes |
|---|---|---|
| 00 | Arpeggio | 3 pitches cycled |
| 01 | Slide up | period decrease per tick; 0 = no-op (no memory) |
| 02 | Slide down | as above |
| 03 | Portamento (tone porta) | |
| 04 | Vibrato | **twice PT depth**; 0 reuses previous |
| 05 | Slide + fade | PT 5 |
| 06 | Vibrato + fade | PT 6 |
| 07 | Tremolo | PT 7 |
| 08 | Hold & decay | on note row; hi nibble decay, lo nibble hold |
| 09 | Secondary tempo | ticks/line 01–20h |
| 0A | Volume slide (PT compat.) | |
| 0B | Position jump | |
| 0C | Set volume | decimal or hex mode (song flag); ≥80h sets default instr. volume (hex mode, unsupported by OpenMPT) |
| 0D | Volume slide | |
| 0E | Synth jump / MIDI pan | |
| 0F | Tempo / misc | 0F00 = pattern break; 0F01–F0 = set tempo; special: F1 play twice, F2 delay note (half line), F3 play three times, F8 filter off, F9 filter on (Amiga LED filter), FA/FB MIDI pedal on/off, FD set pitch, FE end song, FF note off |
| 10 | MIDI message (dump) | |
| 11 / 12 | Fine slide up/down (once) | PT E1x/E2x |
| 14 | Vibrato (PT depth) | |
| 15 | Set finetune | F8–FF = −8…−1 |
| 16 | Loop | 1600 start, 16xx count (PT E6x) |
| 18 | Stop note (cut at tick) | PT ECx |
| 19 | Sample offset | ×256 bytes |
| 1A / 1B | Fine volume up/down | PT EAx/EBx |
| 1C | MIDI program change | |
| 1D | Pattern break (hex line) | |
| 1E | Repeat row / pattern delay | PT EEx |
| 1F | Note delay & retrigger | hi=delay, lo=retrig |
| 20 | Reverse sample / skip | SoundStudio |
| 29 | Relative sample offset | SoundStudio |
| 2E | Set panning | SoundStudio |
Quirk: F1/F3 retrigger every 2 ticks, but OctaMED ≤4.00 bug retriggers on (tick & 7) ≥ 2 ("TODO verify" in libxmp) [src S3]. OpenMPT notes a tempo-command bug in Amiga OctaMED not emulated in SS Windows [src S2 l.498].

## 6. MIDI features
Per-instrument MIDI channel & preset, long preset (bank) numbers, suppress note-off, MIDI pedal (0FFA/FB), program change (1C), raw MIDI messages/dumps (10, MIDI dump structs in expdata) [src S1/S2]. MIDI recording from keyboard into tracks and MIDI sync [recall – verify]. KONEY's player removed MIDI support for demo use [src S4].

## 7. Sample editor [recall – verify]
Waveform display with zoom, range select, cut/copy/paste, loop start/end markers with play, reverse, volume change/fade in/out, resample/change rate, sampling from Amiga parallel-port samplers (e.g. Aura 16-bit card in SS — KONEY mentions dropped "Aura" code [src S4]), 16-bit/stereo in SoundStudio, IFF 8SVX/RAW load/save.

## 8. File formats & integration
- **MMD0** (MED 2.1+, 3 bytes/note, 4-bit cmds), **MMD1** (Pro; 4 bytes/note, 8-bit cmds, BlockInfo with hlmask, block names, command pages), **MMD2** (V6; sections/playing sequences, track volumes/pans, multiple cmd pages), **MMD3** (SoundStudio; same structure as MMD2 + mixing-mode semantics, 16-bit) [src S1, S2, S3]. Pointer-based structs relocated after load; all big-endian, even-aligned [src S1].
- Expdata: annotation text, song name, instrument ext info, MIDI dumps, channel split (`channelsplit`), "jump mask", RGB colours of the GUI (`rgbtable`) [src S1].
- Legacy: MED2/MED3/MED4 formats (libxmp loaders) [src S3].
- Load/save **ProTracker MOD**, with warning requester when OctaMED-only features are used [src S4 README]. Save song "Sng+samples", single instrument save.
- **ARexx** port for scripting the editor [src S4/S6].

## 9. Playback specifics
- Paula: 4 DMA voices, 8-bit samples, period-based pitch (PAL clock 3546895 Hz), max ~28 kHz normal DMA, hardware LED low-pass filter toggled by 0FF8/F9 [recall for clock/filter freq; effect src S3].
- Volume 0–64 (vol7bit flag variant in later versions: 0–127) [src S2 `vol7bit`].
- 8-channel mode: software-mixed pairs per Paula voice, fixed tempo table, reduced quality; synth sounds/some features restricted [tempo src S2; restrictions recall – verify].
- Period-based slides (01/02 operate on period, not linear pitch) [src S3].
- Note range: MMD0 notes 1–3 octaves ×12 base; hybrid/synth default note mapping differs in mix mode (libxmp `MMD3_DEFAULT_NOTE`, "MED mix mode note 0x01" comment) [src S3].
- Player variables written into header (pstate/pblock/pline) for visualisers [src S1]; KONEY adds per-channel level/instrument/note "magic values" [src S4].

## 10. Notable UI elements [recall – verify]
Main Intuition window (proper Workbench app, not custom GUI — KONEY [src S4]), tempo/secondary tempo sliders (src S1 references "main tempo slider" and "rightmost tempo slider"), instrument list, Sample Editor window, Synth Editor window (waveform drawing + vol/wf sequence lists), MIDI window, Sequence/Section editor, track on/off toggles, customisable palette (rgbtable).

---
## 11. Blueprint for a modern clone (Rust + egui)
### MVP (phase 1)
1. Data model: Song → Sections → Playing Sequences → Blocks (variable lines & tracks, 1–64), instruments table.
2. Sample instruments: 8/16-bit mono, loop (forward), volume, transpose, finetune, hold/decay.
3. Mixer: software mixing, up to 16–64 tracks, linear interpolation + optional "Paula" mode (no interpolation, period quantisation, hard L/R pan, LED filter).
4. Tempo: secondary tempo (ticks/line) + BPM mode with lines-per-beat.
5. Effects: 00–0F core + 11,12,14,15,16,18,19,1A,1B,1D,1E,1F.
6. Track editor: OctaMED keyboard layout, octave keys, edit toggle, edit step, ranges, cut/copy/paste, transpose, multi-level undo.
7. Load MMD0–MMD3 and ProTracker MOD; save own format (and MMD3 later); WAV export.
8. Cross-platform: Windows 11 + Linux (cpal: WASAPI / ALSA/PipeWire).
### Phase 2
Synth & hybrid instruments with Vol/WF sequence language + synth editor; multi-octave/ExtSample; command pages (multiple FX columns); sample editor (zoom, loop, reverse, fades, normalise, resample); MOD/XM export; MIDI out/in (midir) incl. per-instrument channel/preset; mute/solo/track volume/pan.
### Phase 3
Scripting in place of ARexx (Rhai or Lua — matching BigWave plugin design), MIDI recording/sync, multi-modules, XM/IT import, VST/CLAP hosting (optional), theming incl. classic Amiga palette, stem export.
### File formats priority
1. MMD0–3 load (S1 doc + port logic from libxmp/OpenMPT loaders — mind licences: libxmp is MIT [verify], OpenMPT BSD-3).  2. MOD load/save.  3. XM load/export.  4. Native format: `.cwt` (see [FILE_FORMAT.md](FILE_FORMAT.md)).
### Rust crates (verified on crates.io 16 Oct 2026)
| Crate | Version | Use |
|---|---|---|
| `cpal` | 0.18.2 | Audio output, cross-platform |
| `rodio` | 0.23.0 | Higher-level playback/previews (optional) |
| `xmrs` | 0.15.2 | Pure-Rust MOD/XM/S3M/IT model & loaders — no MED loader known [verify] |
| `modfile` | 1.4.6 | ProTracker MOD parser |
| `openmpt` | 0.3.1 | Bindings to libopenmpt — plays MED, useful as a **reference oracle** for tests |
Also evaluate: `egui`/`eframe` (GUI), `midir` (MIDI), `hound` (WAV), `symphonia` (sample import), `binrw`/`nom` (MMD parsing), `rhai` (scripting). I found **no existing pure-Rust MED/OctaMED player crate**; `libxmp` crate lookup returned none (bindings may exist under another name [verify]).
### Quirks worth preserving (as an "Authentic" toggle)
- Decimal volume mode & 0C semantics; vibrato 04 at double PT depth vs 14.
- 8-channel tempo table and SoundTracker-compatible tempo 1–10.
- Hold/decay envelopes; 0FF1/F2/F3 retrigger timing (incl. ≤4.00 bug as option).
- Amiga LED filter (0FF8/F9) and Paula period-step aliasing; hard-panning L R R L.
- Per-block variable length; synth sound 8-bit tiny waveforms & sequence timing.
- Classic font/palette layout reminiscent of SoundStudio.

## 12. Uncertainties / follow-ups
- Exact release years and authorship credits per version (verify via Aminet readmes / AmigaGuide history).
- Full SoundStudio manual for editor keymap, range ops, undo depth, sample editor list, MIDI sync.
- Precise synth opcode semantics (EST, EN1/EN2) — read bc-medsounds doc.
- MMD3-only struct differences beyond what S2/S3 implement.
- Licences of libxmp/OpenMPT before porting code.
