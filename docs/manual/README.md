# ChipWave Studio user manual

`ChipWave-Studio-User-Manual.pdf` is the user manual. It is built into the app
(`src/app/manual.rs`, `include_bytes!`) and opens from **Help › User Manual**
or the **User manual (PDF)** button on the About page, on every install type.

Source: `parts/*.html` (one file per chapter), `style.css` and the processed
screenshots in `img/`. The fonts are the app's own, in `../../assets/fonts`.

To rebuild after editing: `./build.sh` (needs Chrome / Chromium and PyMuPDF),
then rebuild the app so the new PDF is embedded.

The screenshots were taken with the app's hidden `--screenshot` mode at
1366×768, then cropped and numbered. The export dialog's save path was
repainted to a neutral `…/Music/ChipWave/` folder.
