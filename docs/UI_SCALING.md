# ChipWave Tracker: UI Scaling and Screen Modes

**Status (0.2.3.14):** UI scale in 25 % steps (Ctrl+= / Ctrl+- / Ctrl+0, Settings > Look) and remembered window size and position are implemented; the per-layout ideas below are partly design.

Goal: ChipWave looks sharp and usable on a 1366×768 laptop, a 4K monitor, an ultrawide, or a portrait screen, and on any OS scale setting.

## 1. One scale factor: egui `pixels_per_point`
egui lays everything out in **points** and multiplies by `pixels_per_point` (ppp) to get physical pixels. ChipWave sets:

```
ppp = os_scale × user_scale
```

- **OS scale** comes from the windowing layer (winit, through eframe):
  - **Windows 10/11:** eframe/winit run the app as **Per-Monitor DPI Aware v2**, so the OS scale (100 %, 125 %, 150 %…) arrives per monitor and updates when the window moves. Rendering is never bitmap-stretched by Windows.
  - **Wayland (KDE/GNOME):** winit supports the `wp_fractional_scale_v1` protocol, so 125 %/150 % come through as fractional scale factors instead of being rendered at 200 % and downscaled (which is blurry).
  - **X11:** winit reads `Xft.dpi` from X resources (96 = 100 %); the `WINIT_X11_SCALE_FACTOR` environment variable overrides it. Many X11 setups report 96 regardless, which is why the user setting matters.
- **User scale:** 75–250 % in 5 % steps, in Settings → Display, and from the keyboard: **Ctrl +** / **Ctrl −** / **Ctrl 0** (reset to 100 %). Applied immediately. It's saved per monitor setup (§7).
- Real ppp = `os_scale × user_scale`, for example 150 % OS × 110 % user = 1.65.

## 2. Everything drawn as vectors
- Knobs, sliders, arcs, buttons, the filter response curve, envelopes, waveforms, the arranger clips and meters are all drawn with `egui::Painter` (paths, circles, rects, text), never pre-rendered bitmaps. They're crisp at any scale. (The mockups in `docs/mockups/` are SVG for the same reason.)
- Icons: SVG drawn as tessellated shapes (via `egui_extras`' SVG loader, rasterised at the current ppp and cached per scale) or simple painter glyphs.
- Waveform displays compute their min/max columns per **physical** pixel, so a 4K screen shows more detail rather than a stretched low-res picture.
- Mini pattern previews in the arranger are textures, but they're regenerated at the current ppp when the scale changes.
- Line widths that should look 1 px use `1.0 / ppp` points; others scale with the UI.

## 3. Tracker grid text (crisp monospace)
- The grid uses a monospace font (default **JetBrains Mono**, SIL OFL 1.1; alternative **IBM Plex Mono**, OFL). Font size defaults to 13 pt and can be set separately from the UI scale (Ctrl+Alt +/−).
- **Integer-pixel snapping:** row height and column width are rounded to whole physical pixels (`(pt × ppp).round() / ppp`), and each row's y position is snapped too, so glyphs never land on half pixels and blur while scrolling. Same for the arranger grid lines.
- Column widths come from the measured glyph advance, so any font fits.
- Row-number column, note/instrument/command columns and track separators have their own colours (see §8).

## 4. Responsive layout breakpoints
The layout reacts to the **available size in points** (after scaling), not to raw pixels, so the same rules work on every DPI.

| Class | Typical screen (at 100 %) | Width in points | Layout |
|---|---|---|---|
| **Compact** | 1366×768 laptop, or 1080p at 150 % | < 1400 | Tabs instead of side panels: Tracker / Arranger / Sound / Instruments. Sound-designer strip scrolls horizontally, and its FX strip opens in place of the main strip. Instrument list collapses to a dropdown. Transport bar squeezes into one row with icons only |
| **Standard** | 1920×1080 | 1400–2200 | Tracker centre; instrument list docked left; arranger docked above the tracker (collapsible to a 1-row mini-map); sound-designer strip docked at the bottom, FX strip as a tab next to it |
| **Large** | 2560×1440 | 2200–3000 | As Standard, plus the FX strip shown under the sound strip at the same time, and the sample/scope panel docked on the right |
| **4K** | 3840×2160 at 150 % (2560 pt) or 200 % (1920 pt) | (falls into Large/Standard by points) | Same as the matching class; the extra pixels make it sharper rather than smaller. At 100 % OS scale on 4K, ChipWave suggests a 150 % user scale on first run |
| **Ultrawide** | 3440×1440 (21:9), 5120×1440 (32:9) | aspect ≥ 2.2 | Everything side by side: instruments · tracker · sound strip + FX in one wide row; arranger full width across the top. On 32:9 an extra column holds the mixer/scopes. The tracker never grows wider than its columns (no stretched gaps); the extra space goes to the arranger and docked panels |
| **Portrait** | 1080×1920, tablets | aspect < 1 | Vertical stack: arranger (short) → tracker (tall, shows more rows) → sound strip, which wraps into two rows (osc + filter, then env + output) |
| **Minimum** | | 1024×640 pt | The window can't be made smaller than this (set via `ViewportBuilder::with_min_inner_size`). Below Compact width, panels hide rather than squash |

Breakpoints have hysteresis (±40 pt) so the layout doesn't flicker while you drag the window edge.

## 5. Resizable and dockable panels
Every panel (Tracker, Arranger, Instruments, Sound designer, FX, Sample editor, Scope/Mixer) is a dockable tab you can drag, split, stack or float. Splitters can be dragged; double-click a splitter to restore its default.

Crates evaluated (checked on crates.io, 16 Oct 2026):

| Crate | Version | Licence | Notes |
|---|---|---|---|
| `egui_dock` | 0.21.1 | MIT | Tabs + splits + floating windows out of the box; IDE-like docking with drop previews. Serialisable layout |
| `egui_tiles` | 0.17.1 | MIT OR Apache-2.0 | From the egui authors (rerun); a tree of tiles (tabs, horizontal/vertical, grid), drag-and-drop, serialisable. Less "IDE", more flexible |

**Version note:** both currently depend on **egui 0.36**, while egui 0.37 came out on 15 Oct 2026. We'll pin egui/eframe 0.36 until the docking crate updates (it usually takes a few weeks), or pick whichever updates first.

**Recommendation: `egui_tiles`.** Its grid layout handles the breakpoint presets well, it's maintained alongside egui itself, and it's dual-licensed. `egui_dock` stays the fallback if we need tabs that float in separate OS windows.

Layout presets: each breakpoint class (§4) has a default tile tree. The user can change it, and "Reset layout" restores the preset.

## 6. Window modes and multiple monitors
- **Windowed** (default), **Maximised**, **Borderless fullscreen** (F11: a window covering the current monitor, so switching is instant and alt-tab works), and **Exclusive fullscreen** (optional, only useful for the Amiga-mode "demo" look; not the default because it changes the display mode).
- **Moving between monitors with different scales:** winit reports `ScaleFactorChanged`, and eframe updates the native ppp. ChipWave reapplies `os_scale × user_scale`, rebuilds the font atlas at the new size, regenerates scaled textures, and recalculates the breakpoint. The window keeps the same *logical* size, so it looks the same on both screens.
- When the window moves to a new monitor setup, ChipWave restores that setup's saved layout (§7).
- Windows' "title bar double-click" and Linux tiling window managers (KWin, Hyprland) are left to work normally; we don't draw our own title bar in v1.

## 7. Layout persistence per screen setup
- Saved in `layout.toml` in the config folder (`dirs::config_dir()`: `%APPDATA%\ChipWave\` / `~/.config/chipwave/`).
- Key = a **screen-setup fingerprint**: the monitor list (sizes in pixels + scale factors), sorted. For each fingerprint we store window position/size/mode, the monitor it was on, user scale, grid font size and the tile tree.
- When opening, if the fingerprint matches, restore exactly; if not, take the closest one with the same breakpoint class, or the class preset. A saved position that would be off-screen (unplugged monitor) is moved onto the primary monitor.
- Per-song UI state (scroll positions, zoom, which instrument is open) is saved in the `.cwt` file's settings instead, not in `layout.toml`.

## 8. Themes and accessibility
- **Default dark theme:** the Wave/Neon palette in [DESIGN_LANGUAGE.md](DESIGN_LANGUAGE.md), used everywhere.
- **High-contrast theme:** pure black background, white text, thicker (2 px) outlines and focus rings, larger value arcs. It's turned on automatically when Windows' high-contrast mode is on (where we can detect it), and is always available in Settings.
- **Colour-blind-safe accents:** an alternative accent set based on the Okabe–Ito palette (blue `#0072B2`, orange `#E69F00`, sky `#56B4E9`, bluish green `#009E73`, yellow `#F0E442`, vermillion `#D55E00`, reddish purple `#CC79A7`). Modulation sources and clip colours never rely on colour alone: each modulation source also has a shape marker (envelope ▲, LFO ●, macro ■) on its arcs, and clips show the block number.
- Keyboard focus is always visible; every control can be reached from the keyboard; tooltips show values; text never goes below 9 pt at any scale; animations (follow-scroll, meter falloff) respect a "reduce motion" option.
- Note: egui's screen-reader support (AccessKit) is available through eframe; we'll label custom widgets so it can read them, but full screen-reader use of a tracker grid is a stretch goal.

## 9. Optional "Amiga mode" look
A retro skin that makes ChipWave look like an Amiga Workbench 1.3 tracker:
- **Palette:** the "Workbench Modern" skin from [DESIGN_LANGUAGE.md](DESIGN_LANGUAGE.md) (our own hex values, not taken from any assets).
- **Pixel font:** an 8×8 bitmap font drawn at **integer multiples only** (×1, ×2, ×3, ×4: the largest that fits the chosen size), **nearest-neighbour** filtering, positions snapped to whole pixels. Non-integer UI scales round down to the nearest integer for this font, and the extra space becomes padding.
- **Font licence:** the original Amiga **Topaz** font belongs to the Amiga ROM (copyright now held by Cloanto/Amiga Corporation) and **must not be bundled**. Fan "Topaz-like" fonts often have unclear licences, so we avoid them too. Options, in order of preference:
  1. **Our own 8×8 font**, designed in the Topaz *spirit* and released CC0 inside the repo (about 96 glyphs, a day's work; the font file can be written as a simple PNG grid or a text bitmap file).
  2. **unscii** (by Viznut, released into the **public domain**), which has 8×8 and 8×16 bitmap faces in a retro style.
  3. Let users pick their own installed font file (we don't ship it).
- Bevelled 3D buttons, a checker "busy" pattern, and an optional scanline/CRT shader on the tracker view (off by default).
- Exclusive fullscreen at an integer scale gives the full "real Amiga" effect.

## 10. Testing
- A screenshot test matrix (`egui_kittest` 0.37.0, headless rendering) at 100/125/150/200 % and the breakpoint sizes in §4, compared against stored images.
- Manual checks: Windows 11 at 125 % + 150 % dual monitors; KDE Plasma Wayland at 125 %; X11 with `Xft.dpi: 144`.

## 11. Roadmap placement
- **MVP:** OS scale (per-monitor on Windows, fractional on Wayland, `Xft.dpi` on X11), user scale 75–250 % with Ctrl +/−/0, vector widgets, crisp snapped grid text, minimum window size, windowed/maximised/borderless fullscreen, the Compact and Standard layouts, layout persistence (window position/size/scale). These are cheap if they're done from the start, and expensive to add later.
- **Phase 1.5:** dockable tiles (`egui_tiles`), the Large/Ultrawide/Portrait presets, per-screen-setup layouts, high-contrast and colour-blind themes.
- **Phase 2:** Amiga mode with its pixel font, CRT shader, exclusive fullscreen.
