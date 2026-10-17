ChipWave Tracker 0.2.3.4 - Windows x64
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

Command line:
  ChipWave.exe --self-test [out.wav]   render the demo, check it is not silent
  ChipWave.exe --render demo out.wav   render the demo (or a .cwt) to WAV (or .mp3)
  ChipWave.exe --version

Created by Derek Heasley.
To keep the software going I need Coffee :)
Buy me a coffee: https://buymeacoffee.com/postmanpat
PayPal:          https://paypal.me/derekh100   (both also in Help > About)
