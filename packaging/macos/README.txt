ChipWave Studio @VERSION@ - macOS (universal: Apple Silicon + Intel)
=====================================================================
GPL-3.0-or-later - https://github.com/postmanpat-303/ChipWaveStudio
Licence: LICENSE.txt (GPL-3). Third-party notices incl. LAME (LGPL): THIRD_PARTY_LICENSES.txt

Install: drag ChipWave.app onto the Applications folder.
Needs macOS 11 (Big Sur) or later. One app runs natively on both
Apple Silicon (M1/M2/M3/M4...) and Intel Macs.

First launch: the app is not signed with an Apple Developer ID or notarised,
so macOS will refuse to open it with a double-click the first time.
  1. In Finder, Control-click (or right-click) ChipWave.app > Open > Open.
  2. On macOS 15 and later, if there is no Open button: try to open it once,
     then System Settings > Privacy & Security > "Open Anyway".
  3. Or in Terminal:  xattr -dr com.apple.quarantine /Applications/ChipWave.app
You only need to do this once.

It opens the first-run demo song. Press SPACE to play / stop the current
block (Shift+Space = whole song). On a Mac, shortcuts written as Ctrl in
the app also work with the Command key (Cmd+S saves). Settings are kept in
~/Library/Application Support/ChipWave/.

More: https://github.com/postmanpat-303/ChipWaveStudio/blob/main/docs/MACOS.md
