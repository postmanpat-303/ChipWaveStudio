# Linux desktop integration

Files for a Linux package (none is built by CI yet):

- `io.github.postmanpat_303.ChipWaveStudio.desktop` → `share/applications/`
- `io.github.postmanpat_303.ChipWaveStudio.metainfo.xml` → `share/metainfo/` (AppStream)
- `chipwave.xml` → `share/mime/packages/` (`.cwt` songs, `.cwpatch` patches; the MIME types are
  the same as before the rename, so existing associations keep working)
- icon: `assets/icon/chipwave_studio_256.png` → `share/icons/hicolor/256x256/apps/io.github.postmanpat_303.ChipWaveStudio.png`

The binary is `chipwave`; the window's app id / WM class is "ChipWave Studio". See docs/FILE_FORMAT.md §7 and docs/RENAME.md.
