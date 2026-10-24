# ChipWave Studio: UI Scaling, Fonts and Screen Modes

**Status (fonts rewrite):** implemented: OS display scale, Settings › Look › **UI scale** 90–150 % and **Text size** 90–150 % (5 % steps), one central type scale with a 10 pt floor, rows that grow with the text, and text colours that follow the surface they sit on. Sections 6–8 are design notes, not all implemented.

Goal: every main view fits a 1366×768 laptop at 100 % without truncation, overlap or clipping, and stays sharp and readable at 150 % on 1080p, 4K, and on fractional OS scales on Windows, Wayland and X11.

## 1. Fonts

All UI fonts are bundled (embedded with `include_bytes!` from `assets/fonts/`), so the app looks the same on every machine. They are under the SIL Open Font License 1.1; the licences are in `LICENSES/`, and they are credited in Help › About › Credits and the README.

| Role | Font | Used for |
|---|---|---|
| UI text (proportional, `label`) | **Atkinson Hyperlegible Next** Regular / SemiBold (Braille Institute of America) | menus, buttons, labels, headings, knob names, browser rows |
| Numbers (monospace, `num`) | **Atkinson Hyperlegible Mono** Medium / SemiBold | the tracker grid, the LCD, BPM / position / dB readouts, knob values, step numbers |
| Fallback | **Inter** Regular (Rasmus Andersson) | symbols, Greek and Cyrillic that Atkinson lacks |

Atkinson Hyperlegible was designed for low-vision readers: distinct `I l 1`, `O 0`, `B 8`, open counters. The same families are used in every theme (Slate, Dark, Amiga); the earlier Ubuntu-Light and Hack defaults are gone.

## 2. The type scale

Every size comes from `theme::ts` (no `FontId` with a literal size outside `src/theme.rs`; a test enforces it):

| Role | pt | Role | pt |
|---|---|---|---|
| `TINY` | 10 | `BODY` | 13 |
| `SMALL` | 11 | `LCD` | 13 |
| `LABEL` | 11.5 | `LARGE` | 15 |
| `COMPACT` | 12.5 | `HEADING` | 16 |
| `MONO` | 12.5 | `TITLE` | 19 |
| | | `DISPLAY` | 26 |

- `ts::px(role)` = role × Text size, and **never below 10 pt** (`ts::MIN`), even at a 90 % Text size.
- Helpers: `theme::prop`, `mono`, `label_font`, `num_font` build a `FontId` from a role; `row_h(p, font)` is the height a row needs for that font; `text_w` measures; `text_fit` paints one line that never runs past a width (eliding with "…"); `shrink_to_fit` steps a font down by 0.5 pt (to 10 pt at most) before anything is elided.
- **Layout follows the text:** list rows (Browser, drum-kit lanes, pads, Rack section heads, top-bar tools) take their height from `row_h`, so a larger Text size makes rows taller instead of overlapping.
- Long hint and status lines elide with "…" and show the full text as a tooltip.

## 3. Two user scales on top of the OS scale

egui lays out in **points**; ChipWave renders at

```
pixels per point = OS scale × UI scale
```

and text additionally at × Text size.

- **OS scale** comes from eframe / winit and is respected as-is:
  - **Windows 10/11:** the app is Per-Monitor DPI Aware v2, so 100 / 125 / 150 / 175 % arrive per monitor and update when the window moves to another screen. Windows never bitmap-stretches it.
  - **Wayland (KDE, GNOME):** fractional scales (125 %, 150 %) come through `wp_fractional_scale_v1`, so they are rendered natively rather than at 200 % and downscaled.
  - **X11:** winit reads `Xft.dpi` (96 = 100 %, 144 = 150 %); `WINIT_X11_SCALE_FACTOR=1.5` overrides it. Many X11 desktops report 96 regardless; use the UI scale then.
- **UI scale** (Settings › Look): 90–150 % in 5 % steps, scales everything (text, controls, spacing). The value applies when you let go of the slider; while dragging it shows the workspace size at that scale. **Ctrl+=** / **Ctrl+-** step by 10 %, **Ctrl+0** resets to 100 %.
  - The UI needs about 1000×560 points. If a scale would not fit the window, it is limited to the largest that fits, with a status message. A saved scale outside 90–150 % is snapped into the range.
- **Text size** (Settings › Look): 90–150 % in 5 % steps, scales only the text (rows grow with it), on top of the UI scale. Reset returns to 100 %.
- Both are saved in the settings file.

Examples: a 1366×768 laptop at 100 % OS × 100 % UI is 1366×768 points; 1920×1080 at 150 % OS is 1280×720 points; 1920×1080 at 100 % OS and 150 % UI is also 1280×720 points.

## 4. Contrast

- `theme::contrast(a, b)` is the WCAG ratio; `on(bg)` / `on2(bg)` give the primary / secondary text colour for a fill, picking light or dark ink so it reads at **≥ 4.5 : 1**.
- Buttons, LEDs, pads, selected tabs and top-bar tools take their text colour from the fill they sit on, so a pale-blue Amiga selection gets dark text and a dark Slate face light text.
- Each theme's `ink` and `ink2` reach 4.5 : 1 on its panels (Dark's `ink2` is `#B4BAC2`); tests check it.
- Disabled buttons keep a solid face with faded text, rather than fading the whole button.

## 5. Vector drawing and crisp text

- Knobs, meters, waveforms, clips and icons are painted as vectors with `egui::Painter`, so they are sharp at any scale.
- The font atlas is rebuilt at the current pixels-per-point, so text is rasterised at the real size, not scaled.
- Tracker rows and columns are measured from the monospace glyph advance and snapped to whole physical pixels.

## 6. Small screens (design notes)

- Minimum window: 800×500 points (`with_min_inner_size`).
- At 1366×768 the Timeline toolbar wraps, the device chain and tracker channels scroll horizontally, Rack sections shrink proportionally (a section's combo drops its name label when narrow), and compact drum-kit pads hide the key and choke (shown in the tooltip).
- Ideas not implemented yet: breakpoint layouts for ultrawide and portrait screens, dockable panels.

## 7. Window state

- Window size, position and maximised state are remembered. Moving between monitors with different OS scales keeps the same logical size.

## 8. Testing

- Unit tests: every type-scale role ≥ 10 pt (also at the smallest Text size), theme ink contrast ≥ 4.5 : 1, `on` / `on2` contrast on dark and light fills, no raw `FontId` outside the theme.
- Screenshot checks: every main view (Timeline, Rack, Generators, piano roll, tracker, Browser, Settings, LCD / top bar, drum kit) in Slate, Dark and Amiga at 1366×768 100 %, and Slate at 1920×1080 150 %.
- Manual checks worth doing on real hardware: Windows 11 at 125 % and 150 % across two monitors; KDE Plasma Wayland at 125 %; X11 with `Xft.dpi: 144`.
