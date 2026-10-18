ChipWave Tracker 0.2.3.5 - Windows x64
==============================================================
GPL-3.0-or-later - https://github.com/postmanpat-303/ChipWaveTracker
Licence: LICENSE.txt (GPL-3). Third-party notices incl. LAME (LGPL): THIRD_PARTY_LICENSES.txt

Run: double-click ChipWave.exe (no install needed; single self-contained exe).
Windows SmartScreen may warn because the exe is not code-signed:
"More info" -> "Run anyway".

It opens the first-run demo "Lumen Drift" (uplifting trance, A minor, 140 BPM).
Press SPACE to play / stop the current block (Shift+Space = whole song).

Keys (press "? Keys" in the toolbar for the full list)
  Arrows, PgUp/PgDn, Home/End    move (Ctrl+Home/End = first/last block)
  Tab / Shift+Tab                next / previous channel
  Z S X D C V G B H N J M        notes C..B,  Q 2 W 3 E R 5 T 6 Y 7 U I 9 O 0 P = octave up
  1                              note off
  F1..F8, numpad - / +           base octave (also the - / + buttons by "Oct")
  0-9 A-F                        hex in instrument / volume / effect columns
  Delete / Backspace / Insert    clear (selection) / pull up / push down
  Shift+arrows/PgUp/PgDn, drag   select (highlighted green)
  Ctrl+A                         select channel; press again = whole block
  Ctrl+C / Ctrl+X / Ctrl+V       copy / cut / paste at cursor
  Space / Shift+Space            play/stop current block (loop) / play/stop song
  Enter / Ctrl+Enter             restart block / play from cursor row
  Esc                            edit mode on/off
  Ctrl+Z / Ctrl+Y                undo / redo (notes, arranger, sounds, FX, mixer, tempo, channels; each click or
                                 drag = 1 step; also works while a value field is being typed into)
  Alt +/-  (Shift = octave)      transpose selection (or whole channel)
  Ctrl+= / Ctrl+- / Ctrl+0       UI scale up / down / 100 %
  F9 / F10                       mute / solo channel
  Ctrl+Tab, Ctrl+1 .. 5          views: Pattern / Arranger / Mixer / FX / Sample
  FX / Sample view: Up / Down    previous / next instrument
  Sample view: Space             preview; drag = select, drag S/E = loop points
  F12                            key list (Help > Keys)
  Ctrl+Q                         quit
  FX device strip                power = bypass, folder = presets, click name =
                                 collapse, drag name = reorder, right-click = menu
  Ctrl+D / Ctrl+Shift+D          arranger: duplicate clip / as new block (also when docked)
  Mouse wheel                    pattern: 1 row per notch (Shift = 16); arranger: scroll, Ctrl = zoom
  Instrument list                Duplicate / Delete... buttons, right-click menu or Edit menu (asks
                                 first when notes use it; Ctrl+Z undoes)
  Ctrl+Shift+K                   debug: show the last 10 input events in the status bar
  Arranger (click to focus, light outline + ARRANGER tag): click / Shift+click / Ctrl+click select,
    Ctrl+X / C / V cut / copy / paste after selection (Ctrl+Shift+V = as new blocks),
    Del, Ctrl+A, Shift+Left/Right, right-click menu; click the pattern to focus it again
  Arranger: Left/Right, Del, Ins, Ctrl+D (Shift = unique copy), +/-  zoom
  Ctrl+N / Ctrl+O / Ctrl+S       new / open / save .cwt   (Ctrl+Shift+S = save as)
  Ctrl+E                         quick export WAV
  Ctrl+Shift+E                   export dialog (WAV / MP3, tags, range)
  Ctrl+,                         settings (arrows + Enter move/open cards)
  Ctrl+Shift+F                   Song > Fill to length (auto arrange)
  1-7 / 0 (arranger focus)       block role Intro/Verse/Chorus/Bridge/Breakdown/Outro/Fill / clear
  Fill dialog                    G generate, R reroll, L lock, P preview, H hide, S style,
                                 Up/Down variation, Left/Right section, Enter accept, Esc discard
  Unsaved prompt                 Enter = save, N = don't save, Esc = cancel
  (starts maximised; the last window size/position is remembered)

MIDI keyboard (0.2.3, details in docs/MIDI.md on GitHub)
  Settings > MIDI (or click the MIDI light, top right): pick your keyboard or "All devices";
    it reconnects when plugged back in. Channel filter, velocity on/off + curve, MIDI octave.
  Play: the keyboard plays the selected instrument (velocity, sustain pedal, pitch bend).
  Record: edit mode on (Esc) + Pattern view = note + instrument + velocity (volume column)
    at the cursor, cursor moves by Step. While the song plays: written at the playing row.
    Option: chords spread over the next channels. Ctrl+Z removes the last note.
  Toolbar (left of the MIDI light): "Chord" = chord spread on/off,
    "Rec live" / "Rec step" = record mode while playing (red when edit mode is on).
  MIDI learn: right-click a fader / pan / send / instrument Volume, Cutoff, Resonance,
    Drive / BPM > "MIDI learn", then move a knob or the mod wheel.
  Check a keyboard from cmd:  ChipWave.exe --midi-list   and   ChipWave.exe --midi-monitor 15

Effects: 01/02 slide up/down, 08 note decay (xx x 10 ms), 09 speed,
0C set volume, 0D volume slide,
0F tempo (0F00 = pattern break).

Instruments: the left panel lists the factory psytrance/alien synth sounds;
select one to edit oscillator, filter (LP/HP/BP, cutoff, resonance), envelope.
Import WAV samples with "Import WAV..." or drag .wav files onto the window.
Demo song "Lumen Drift" (trance, A minor, 140 BPM, 2:26): opens on first run;
load it any time with File > New from demo song or Help > Load demo song.
Synth: "Stereo width" spreads unison voices (and noise); LFO (shape, free Hz or
synced 1/16..2 bars) to cutoff, amp (trance gate) and pitch.
0.2.3.4: demo hi-hat is a classic offbeat hat (one hit between kicks).
0.2.3.3: reworked hi-hats in the demo; "Metal (808)" metallic hat source,
"Velocity > cutoff" (soft notes darker), "Start delay" (laid-back percussion),
new "shaker" sound, effect 08xx = note decay (short/long hats). A new note on
the same channel chokes the old one (open hat cut by the next closed hat).
Rows now play sample-accurately (the first row after Play was 1 tick early).
Right-click an instrument: Add effect, Copy effects, Paste effects (replace or
append), Open in FX view, Duplicate, Delete. Ctrl+Z undoes each.
Mixer: double-click a fader = 0 dB, double-click pan = centre.
  Sidechain: pick the "Sidechain key" channel (e.g. the kick) and a release;
  each channel's "Duck" % sets how far it pumps under the key notes.
Audio: "Buffer" in the toolbar (Auto/256/512/1024/2048). The status bar
shows latency and an underrun counter; latency grows automatically on dropouts.
BPM / Speed changes apply instantly, also while playing.
Arranger: song order as coloured clips; drag to reorder, click to open a block.
  "Dock arranger" shows it under the pattern editor.
Automation: lanes under the arranger (toggle "Automation", "+ Lane"): click = add point,
  drag = move, right-click = delete, Pencil = draw, Shape = Linear/Step/Smooth.
  Targets: synth cutoff/resonance/drive, amp level, mixer volume/pan, master, tempo.
Mixer: per-channel fader, pan, mute/solo, meters; master fader. Saved in .cwt.
Instrument panel: Transpose -12 / - / + / +12 per instrument.
Theme: "Dark Circuit (default)" or "Amiga (Workbench Gradient)" in the toolbar.
Voice (glitch) instrument (0.2.3.6): "New..." under the instrument list (or right-click
  an instrument > New instrument) > Voice (glitch) > a preset. Synthetic vowels
  (ahh eee ohh err), babble, chop and stutter. Pattern effects 30-37: 30xy vowel x
  (0 A 1 E 2 I 3 O 4 U 5 ER 6 AE) glide y*25 ms, 31xx sequence step, 32xx formant
  shift (80 = x1), 33xy stutter (x length 1=1/64 .. 3=1/16 .. 6=1/2 note, y repeats;
  any instrument), 34xy stutter pitch +x/-y, 35xx breath, 36xy babble, 37xy chop.
Demo songs: File > Demo songs (Lumen Drift, Hollow Ritual).
Credits: Help > About and THIRD_PARTY_LICENSES.txt (Voice design inspired by
  Mutable Instruments Plaits by Emilie Gillet, MIT; formant data from the Csound
  manual and Peterson & Barney 1952).
Updates (0.2.3.5): Help > Check for updates... asks GitHub for a newer version and
  shows the release notes; "Download installer" opens the setup .exe in your browser
  (nothing is downloaded or opened unless you click). Settings > Files & Safety >
  "Check for updates at startup" (on by default, at most once a day): a small
  "Update X available" badge appears in the menu bar. "Skip this version" stops
  reminders for that version.

Command line:
  ChipWave.exe --self-test [out.wav]   render the demo, check it is not silent
  ChipWave.exe --render demo out.wav   render the demo (or a .cwt) to WAV (or .mp3)
  ChipWave.exe --version
  ChipWave.exe --check-updates        ask GitHub whether a newer version is out

Created by Derek Heasley.
To keep the software going I need Coffee :)
Buy me a coffee: https://buymeacoffee.com/postmanpat
PayPal:          https://paypal.me/derekh100   (both also in Help > About)

Master limiter and loudness (0.2.3.7): Mixer > "Limiter & loudness" (right of
the master strip). True-peak limiter on the master and in every export:
gain, ceiling (default -1.0 dBTP), release; untick it for the old soft clip.
Readout: momentary / short-term / integrated LUFS and true peak (ITU-R BS.1770).

Filter and acid effects (0.2.3.7): 38xx cutoff (00 = instrument, 01-FF =
20 Hz-20 kHz), 39xx resonance, 3Axx / 3Bxx cutoff slide up / down per tick,
3Cxx glide to the note (00 off, 01 instrument time, 02-FF xx*5 ms), 3Dxx
accent. Sound editor: Glide time + Legato, Accent amount + "at vol >=".
Reverb has Low cut (default 200 Hz) and High cut; the fold drive no longer
flips polarity.


Up to 32 channels (0.2.3.8): each song sets its channel count (Channels box
in the block bar or the Song menu, 1-32). The bar under the pattern grid and
under the mixer strips shows which channels are in view; click or drag it to
scroll (Tab / Shift+Tab move through the channels). Songs with more than 8
channels open in older versions with only their first 8.

Instrument library (0.2.3.9): File > Instrument library... shows the song's
instruments, your library folder (.cwi files, sub-folders = categories) and
the factory sounds; > auditions, + adds, right-click replaces the selected
slot's sound but keeps its notes. In the instrument list: double-click to
rename, Up / Down to reorder (notes follow), right-click > Use this instead
of to swap instruments in the song or one block, Save to library, Export
.cwi. Drop a .cwi file on the window to import it.

Voice words and Throat mode (0.2.3.10): a Voice instrument's Words row
holds short words (ka, ta, sha, dum ...); each note speaks the next one and
3Exx speaks word xx. Tract: Throat swaps the formant filters for a simulated
vocal tract inspired by Pink Trombone by Neil Thapen (MIT). Presets: Talk
Ka-Ta, Throat Ahh, Throat Speech, Demon Words.

Chip FX (0.2.3.13): New... > Generator (chip FX) makes retro game sounds
(a port of sfxr by DrPetter via sfxr-rs, MIT). Click Pickup, Laser,
Explosion, Power-up, Hit, Jump or Blip for a new sound, Randomize for any
sound, Mutate to nudge it; "lock" keeps a parameter, < > walks 16 rolls,
Bake to sample renders it. 43xy sets knob x (1-8) at the note start.

Generators (0.2.3.12): New... > Generator (drums) adds a Drum Synth
(kick v0/808/909/Zap, snare, hat, clap, tom, perc) or a kit (one piece per
note: C kick, D snare, D# clap, F#/A# hats, E-A toms) played from eight macro
knobs. Dice rolls a sound, Mutate nudges it, "lock" keeps a knob, < > steps
through 16 rolls, Bake to sample renders it. The Synth filter can be the
Moog ladder (clean / vintage, 12/24 dB). Pattern: 43xy sets macro x (1-8)
to y (0-F); 4300 = instrument.

Voice Transform (0.2.3.11): the TRANSFORM section of a Voice instrument
blends three stages over the voice: metallic resonance (resonators tuned to
the note), digital distortion (clip / fold, bits, sample rate, ring mod) and
a sub-bass layer (sine or octave divider, mono below the crossover), with
formant drop for a monster voice. Blend 0 = off. Presets: Metal Demon,
Digital Ghost, Sub Beast, Cyber Choir, Ritual Growl. Pattern: 3Fxx blend,
40xx resonance, 41xx digital, 42xx sub (00 = instrument, 01-FF = 0-100 %).
The stages are also FX devices for any instrument (Metal resonator, Digital
distortion, Sub bass).

Chop / slice (0.2.3.10): in the Sample editor, Transients or Grid cuts the
sample into numbered slices; click one (or Prev / Next) to edit just that
slice; To instruments makes one instrument per slice, To notes also writes
them into the block on the cursor channel at the song tempo.

Chord helper (0.2.3.9): Song > Chord helper... or Chords... in the block bar.
Pick or detect the song key, click chord pads to hear them and build a
progression (or pick a preset), then Write into block as chords, arp or
bass. Scale lock snaps typed and MIDI notes to the key. Key detection:
Krumhansl-Schmuckler with the Krumhansl & Kessler (1982) profiles; the
workflow is inspired by Scaler (Plugin Boutique), no material used.
