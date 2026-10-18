ChipWave Lite 0.1-m1 (milestone M1: command-line .mod player)
=============================================================

ChipWave Lite is a small native tracker for AmigaOS 3.1 and later, the
Amiga sibling of the PC tracker ChipWave. This first milestone is the
replay engine: a Shell command that plays ProTracker modules on Paula.

  ChipWaveLite      68020 or better (A1200, A3000, A4000)
  ChipWaveLite.000  68000 (A500, A500+, A600, A2000; also runs on 68020+)

Usage (from the Shell):

  ChipWaveLite <file.mod>            play until you press Ctrl-C
  ChipWaveLite <file.mod> LOOPS 1    play the song once, then exit
  ChipWaveLite <file.mod> SECONDS 30 play for 30 seconds

The module is loaded into chip RAM and played by ptplayer (CIA-B timed,
audio channels allocated through audio.device, so it plays nicely with
the OS). All ProTracker 2.3 effects are supported. Ctrl-C stops it,
silences Paula and gives back the audio channels and CIA-B timers.
4-channel M.K., M!K!, FLT4 and 4CHN modules are accepted.

The bootable disk starts ChipWaveLite.000 with Songs/cwlite-test.mod.

Test song: "Paula Steps" (Songs/cwlite-test.mod), made for ChipWave Lite
by a script (amiga/test/make_test_mod.py in the ChipWave repository); all
samples are synthesised. It is dedicated to the public domain (CC0 1.0).

Licence: GNU GPL version 3 or later (see LICENSE). Source:
https://github.com/postmanpat-303/ChipWaveStudio (folder amiga/)

Credits
  ptplayer 6.4 by Frank Wille (public domain), with small ChipWave Lite
    changes so 7xy/5xy/6xy behave exactly like ProTracker 2.3
  ProTracker behaviour checked against pt2-clone by Olav "8bitbubsy"
    Soerensen (BSD-3) and libxmp (MIT)
  m68k-amigaos-gcc by Stefan "bebbo" Franke and the AmigaPorts team
  vasm and vlink by Volker Barthelmann and Frank Wille
  NDK 3.2 headers by Hyperion Entertainment
  amitools (vamos, xdftool) by Christian Vogelgsang
  FS-UAE by Frode Solheim, the AROS ROM by the AROS Development Team,
    WinUAE by Toni Wilen
