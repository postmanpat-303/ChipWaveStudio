//! Plugin editor thumbnails for the Browser (docs/BROWSER.md).
//!
//! On demand a crash-guarded child (`chipwave --capture-thumb …`) opens the
//! plugin's editor off-screen, captures its HWND (Windows / Wine), downscales
//! and writes a PNG under `<config>/plugin-thumbs/`. Failures and plugins
//! with no editor get a tidy generated card. The main process loads those
//! PNGs lazily and shows them in the Plugins list.
use super::{clap, vst3, Hosted};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// capture child timeout (CHIPWAVE_THUMB_TIMEOUT seconds overrides)
pub const TIMEOUT: Duration = Duration::from_secs(25);
/// longest side of a cached thumbnail
pub const MAX_SIDE: u32 = 256;
/// row thumbnail size in the Browser
pub const ROW: f32 = 28.0;
/// hover / details preview size
pub const PREVIEW: f32 = 160.0;

pub fn timeout() -> Duration {
    std::env::var("CHIPWAVE_THUMB_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .map(Duration::from_secs_f64)
        .unwrap_or(TIMEOUT)
}

/// `<config>/plugin-thumbs`
pub fn dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CHIPWAVE_THUMB_DIR") {
        return Some(PathBuf::from(p));
    }
    crate::settings::config_dir().map(|d| d.join("plugin-thumbs"))
}

/// stable cache key: format + id + version
pub fn key(format: &str, id: &str, version: &str) -> String {
    let scrub = |s: &str| {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
    };
    format!(
        "{}_{}_{}",
        scrub(format),
        scrub(id),
        scrub(if version.is_empty() { "0" } else { version })
    )
}

pub fn png_path(k: &str) -> Option<PathBuf> {
    dir().map(|d| d.join(format!("{k}.png")))
}

pub fn meta_path(k: &str) -> Option<PathBuf> {
    dir().map(|d| d.join(format!("{k}.json")))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// real editor screenshot
    Capture,
    /// drawn card (no editor / capture failed)
    Card,
    /// capture failed; a card was written
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meta {
    pub kind: Kind,
    pub format: String,
    pub id: String,
    pub version: String,
    pub name: String,
    #[serde(default)]
    pub message: String,
}

/// how the Browser should treat a plugin's thumbnail
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ready,
    Missing,
}

pub fn status(k: &str) -> Status {
    match png_path(k) {
        Some(p) if p.is_file() => Status::Ready,
        _ => Status::Missing,
    }
}

pub fn read_meta(k: &str) -> Option<Meta> {
    let p = meta_path(k)?;
    let t = std::fs::read_to_string(p).ok()?;
    serde_json::from_str(&t).ok()
}

pub fn write_meta(k: &str, m: &Meta) {
    if let Some(p) = meta_path(k) {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(t) = serde_json::to_string_pretty(m) {
            let _ = std::fs::write(p, t);
        }
    }
}

pub fn save_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(f, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wtr = enc.write_header().map_err(|e| e.to_string())?;
    wtr.write_image_data(rgba).map_err(|e| e.to_string())
}

pub fn load_png(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let f = std::io::BufReader::new(f);
    let dec = png::Decoder::new(f);
    // ignore colour profile warnings from plugin UIs
    let mut reader = dec.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    // expand to RGBA8
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            let mut o = Vec::with_capacity(buf.len() / 3 * 4);
            for &[r, g, b] in buf.as_chunks::<3>().0 {
                o.extend_from_slice(&[r, g, b, 255]);
            }
            o
        }
        png::ColorType::Grayscale => {
            let mut o = Vec::with_capacity(buf.len() * 4);
            for &g in &buf {
                o.extend_from_slice(&[g, g, g, 255]);
            }
            o
        }
        png::ColorType::GrayscaleAlpha => {
            let mut o = Vec::with_capacity(buf.len() / 2 * 4);
            for &[g, a] in buf.as_chunks::<2>().0 {
                o.extend_from_slice(&[g, g, g, a]);
            }
            o
        }
        other => return Err(format!("unsupported PNG colour {other:?}")),
    };
    Ok((info.width, info.height, rgba))
}

/// downscale so the longest side is at most `max`
pub fn downscale(w: u32, h: u32, rgba: &[u8], max: u32) -> (u32, u32, Vec<u8>) {
    if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
        return (w, h, rgba.to_vec());
    }
    let long = w.max(h);
    if long <= max {
        return (w, h, rgba.to_vec());
    }
    let nw = ((w as u64 * max as u64) / long as u64).max(1) as u32;
    let nh = ((h as u64 * max as u64) / long as u64).max(1) as u32;
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        for x in 0..nw {
            let sx = (x as u64 * w as u64) / nw as u64;
            let sy = (y as u64 * h as u64) / nh as u64;
            let si = ((sy * w as u64 + sx) * 4) as usize;
            let di = ((y * nw + x) * 4) as usize;
            out[di..di + 4].copy_from_slice(&rgba[si..si + 4]);
        }
    }
    (nw, nh, out)
}

/// a tidy fallback card: category colour strip, name, creator, format tag
pub fn generate_card(name: &str, vendor: &str, format: &str, rgb: [u8; 3]) -> (u32, u32, Vec<u8>) {
    let (w, h) = (240u32, 140u32);
    let mut px = vec![0u8; (w * h * 4) as usize];
    let put = |px: &mut [u8], x: u32, y: u32, c: [u8; 4]| {
        if x < w && y < h {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&c);
        }
    };
    let fill = |px: &mut [u8], x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 4]| {
        for y in y0..y1.min(h) {
            for x in x0..x1.min(w) {
                put(px, x, y, c);
            }
        }
    };
    // deep navy card
    fill(&mut px, 0, 0, w, h, [0x1a, 0x22, 0x30, 255]);
    // colour strip
    fill(&mut px, 0, 0, 8, h, [rgb[0], rgb[1], rgb[2], 255]);
    // top accent bar
    fill(&mut px, 8, 0, w, 4, [rgb[0], rgb[1], rgb[2], 180]);
    // format pill background
    let tag = format.to_uppercase();
    let tw = (tag.len() as u32 * 7 + 16).min(w - 20);
    fill(
        &mut px,
        w - tw - 10,
        12,
        w - 10,
        30,
        [0x2a, 0x36, 0x48, 255],
    );
    // crude 5x7 glyphs for a short label (ASCII)
    blit_text(&mut px, w, h, 18, 48, name, [0xee, 0xf2, 0xf7, 255], 2);
    if !vendor.is_empty() {
        blit_text(&mut px, w, h, 18, 78, vendor, [0x9a, 0xa8, 0xba, 255], 1);
    }
    blit_text(
        &mut px,
        w,
        h,
        w - tw - 4,
        16,
        &tag,
        [rgb[0], rgb[1], rgb[2], 255],
        1,
    );
    (w, h, px)
}

#[allow(clippy::too_many_arguments)]
fn blit_text(px: &mut [u8], w: u32, h: u32, x0: u32, y0: u32, s: &str, col: [u8; 4], scale: u32) {
    let mut x = x0;
    for ch in s.chars().take(28) {
        let g = glyph(ch);
        for row in 0..7u32 {
            for col_i in 0..5u32 {
                if (g[row as usize] >> (4 - col_i)) & 1 != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let xx = x + col_i * scale + dx;
                            let yy = y0 + row * scale + dy;
                            if xx < w && yy < h {
                                let i = ((yy * w + xx) * 4) as usize;
                                px[i..i + 4].copy_from_slice(&col);
                            }
                        }
                    }
                }
            }
        }
        x += 6 * scale;
        if x + 6 * scale >= w {
            break;
        }
    }
}

fn glyph(c: char) -> [u8; 7] {
    // very small 5x7 font for A-Z 0-9 . - space and lowercase→upper
    let c = if c.is_ascii_lowercase() {
        (c as u8 - b'a' + b'A') as char
    } else {
        c
    };
    match c {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        '-' => [0, 0, 0, 0x1F, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0x0C, 0x0C],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1F],
        '3' => [0x0E, 0x11, 0x01, 0x06, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x01, 0x01, 0x01, 0x01, 0x11, 0x11, 0x0E],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '+' => [0, 0x04, 0x04, 0x1F, 0x04, 0x04, 0],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        _ => [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F],
    }
}

fn format_colour(format: &str) -> [u8; 3] {
    match format {
        "clap" => [0x6E, 0xC6, 0xA8],
        "vst3" => [0x7A, 0xA2, 0xE8],
        _ => [0xB0, 0xB8, 0xC4],
    }
}

/// write a generated card for `k` and its meta
pub fn write_card(k: &str, name: &str, vendor: &str, format: &str, kind: Kind, msg: &str) {
    let (w, h, rgba) = generate_card(name, vendor, format, format_colour(format));
    if let Some(p) = png_path(k) {
        let _ = save_png(&p, w, h, &rgba);
    }
    write_meta(
        k,
        &Meta {
            kind,
            format: format.into(),
            id: String::new(),
            version: String::new(),
            name: name.into(),
            message: msg.into(),
        },
    );
}

/// run in the child: load, open editor, capture, write PNG. Exit code 0 = ok.
pub fn cli_capture(args: &[String]) -> i32 {
    // --capture-thumb <format> <path> <id> <out.png> [--name N] [--vendor V] [--version V]
    let Some(i) = args.iter().position(|a| a == "--capture-thumb") else {
        return 2;
    };
    let (Some(format), Some(path), Some(id), Some(out)) = (
        args.get(i + 1),
        args.get(i + 2),
        args.get(i + 3),
        args.get(i + 4),
    ) else {
        eprintln!("usage: --capture-thumb <format> <path> <id> <out.png>");
        return 2;
    };
    let flag = |n: &str| {
        args.iter()
            .position(|a| a == n)
            .and_then(|j| args.get(j + 1))
            .cloned()
            .unwrap_or_default()
    };
    let name = flag("--name");
    let vendor = flag("--vendor");
    let version = flag("--version");
    let name = if name.is_empty() { id.clone() } else { name };
    let k = key(format, id, &version);

    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Diagnostics::Debug::*;
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
    }

    let deadline = Instant::now() + timeout();
    let result = (|| -> Result<(), String> {
        let path_buf = PathBuf::from(path);
        if !path_buf.exists() {
            return Err(format!("plugin file missing: {path}"));
        }
        let mut inst: Box<dyn Hosted> = match format.as_str() {
            "clap" => Box::new(clap::ClapInst::new(&path_buf, id, 48_000.0)?),
            "vst3" => Box::new(vst3::Vst3Inst::new(&path_buf, id, 48_000.0)?),
            f => return Err(format!("unknown format {f}")),
        };
        if !inst.has_editor() {
            return Err("no editor".into());
        }
        inst.open_editor(&format!("{name} – ChipWave thumb"))?;
        // let the plugin paint
        let mut pumps = 0;
        while Instant::now() < deadline && pumps < 40 {
            inst.poll_main();
            pump_messages();
            std::thread::sleep(Duration::from_millis(50));
            pumps += 1;
        }
        let hwnd = editor_hwnd(&*inst).ok_or("no editor window to capture")?;
        // park off-screen so the user barely sees it
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::*;
            SetWindowPos(
                hwnd as _,
                std::ptr::null_mut(),
                -32000,
                -32000,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        inst.poll_main();
        pump_messages();
        std::thread::sleep(Duration::from_millis(100));
        let (w, h, rgba) = capture_hwnd(hwnd)?;
        let (w, h, rgba) = downscale(w, h, &rgba, MAX_SIDE);
        save_png(Path::new(out), w, h, &rgba)?;
        write_meta(
            &k,
            &Meta {
                kind: Kind::Capture,
                format: format.clone(),
                id: id.clone(),
                version: version.clone(),
                name: name.clone(),
                message: String::new(),
            },
        );
        inst.close_editor();
        Ok(())
    })();

    match result {
        Ok(()) => 0,
        Err(e) => {
            write_card(
                &k,
                &name,
                &vendor,
                format,
                if e == "no editor" {
                    Kind::Card
                } else {
                    Kind::Failed
                },
                &e,
            );
            // still write the card path the parent expects
            if let Some(p) = png_path(&k) {
                if p != Path::new(out) {
                    let _ = std::fs::copy(&p, out);
                }
            }
            eprintln!("capture-thumb: {e}");
            1
        }
    }
}

fn pump_messages() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let mut msg = std::mem::zeroed();
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn editor_hwnd(_inst: &dyn Hosted) -> Option<isize> {
    #[cfg(windows)]
    {
        // the most recently created ChipWave editor window
        super::gui::win::last_hwnd()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
pub fn capture_hwnd(hwnd: isize) -> Result<(u32, u32, Vec<u8>), String> {
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetClientRect(hwnd as HWND, &mut rc) == 0 {
            return Err("GetClientRect failed".into());
        }
        let w = (rc.right - rc.left).max(1) as u32;
        let h = (rc.bottom - rc.top).max(1) as u32;
        let hdc = GetDC(hwnd as HWND);
        if hdc.is_null() {
            return Err("GetDC failed".into());
        }
        let mem = CreateCompatibleDC(hdc);
        let bmp = CreateCompatibleBitmap(hdc, w as i32, h as i32);
        let old = SelectObject(mem, bmp as _);
        // PrintWindow (user32) — better for off-screen / layered UIs
        let ok_pw = print_window(hwnd, mem);
        if ok_pw == 0 {
            BitBlt(mem, 0, 0, w as i32, h as i32, hdc, 0, 0, SRCCOPY);
        }
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w as i32;
        bmi.bmiHeader.biHeight = -(h as i32); // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        let got = GetDIBits(
            mem,
            bmp,
            0,
            h,
            bgra.as_mut_ptr() as *mut _,
            &mut bmi,
            DIB_RGB_COLORS,
        );
        SelectObject(mem, old);
        DeleteObject(bmp as _);
        DeleteDC(mem);
        ReleaseDC(hwnd as HWND, hdc);
        if got == 0 {
            return Err("GetDIBits failed".into());
        }
        // BGRA → RGBA
        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        Ok((w, h, bgra))
    }
}

#[cfg(windows)]
fn print_window(hwnd: isize, hdc: windows_sys::Win32::Graphics::Gdi::HDC) -> i32 {
    // linked via user32; flag 2 = PW_RENDERFULLCONTENT when available
    #[link(name = "user32")]
    unsafe extern "system" {
        fn PrintWindow(
            hwnd: windows_sys::Win32::Foundation::HWND,
            hdc_blt: windows_sys::Win32::Graphics::Gdi::HDC,
            nflags: u32,
        ) -> windows_sys::core::BOOL;
    }
    unsafe { PrintWindow(hwnd as _, hdc, 2) }
}

#[cfg(not(windows))]
pub fn capture_hwnd(_hwnd: isize) -> Result<(u32, u32, Vec<u8>), String> {
    Err("editor capture is Windows-only for now".into())
}

/// one-at-a-time queue used by the Browser
#[derive(Default)]
pub struct Queue {
    pending: Vec<Pending>,
    busy: bool,
    child: Option<std::process::Child>,
    child_key: String,
    started: Option<Instant>,
}

#[derive(Clone)]
struct Pending {
    format: String,
    path: String,
    id: String,
    version: String,
    name: String,
    vendor: String,
}

impl Queue {
    pub fn enqueue(
        &mut self,
        format: &str,
        path: &str,
        id: &str,
        version: &str,
        name: &str,
        vendor: &str,
    ) {
        let k = key(format, id, version);
        if status(&k) == Status::Ready {
            // refresh only when forced via clear
            if read_meta(&k).is_some_and(|m| m.message != "refresh") {
                return;
            }
        }
        if self
            .pending
            .iter()
            .any(|p| key(&p.format, &p.id, &p.version) == k)
            || self.child_key == k
        {
            return;
        }
        self.pending.push(Pending {
            format: format.into(),
            path: path.into(),
            id: id.into(),
            version: version.into(),
            name: name.into(),
            vendor: vendor.into(),
        });
    }

    pub fn force_refresh(
        &mut self,
        format: &str,
        path: &str,
        id: &str,
        version: &str,
        name: &str,
        vendor: &str,
    ) {
        let k = key(format, id, version);
        if let Some(p) = png_path(&k) {
            let _ = std::fs::remove_file(p);
        }
        write_meta(
            &k,
            &Meta {
                kind: Kind::Card,
                format: format.into(),
                id: id.into(),
                version: version.into(),
                name: name.into(),
                message: "refresh".into(),
            },
        );
        self.enqueue(format, path, id, version, name, vendor);
    }

    /// pump: start / reap capture children. Returns true when a thumb finished.
    pub fn poll(&mut self, exe: &Path) -> bool {
        let mut finished = false;
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.child = None;
                    self.child_key.clear();
                    self.busy = false;
                    self.started = None;
                    finished = true;
                }
                Ok(None) => {
                    if self.started.is_some_and(|t| t.elapsed() > timeout()) {
                        let _ = child.kill();
                        let _ = child.wait();
                        self.child = None;
                        self.child_key.clear();
                        self.busy = false;
                        self.started = None;
                        finished = true;
                    }
                }
                Err(_) => {
                    self.child = None;
                    self.child_key.clear();
                    self.busy = false;
                    self.started = None;
                }
            }
        }
        if !self.busy {
            if let Some(p) = self.pending.pop() {
                let k = key(&p.format, &p.id, &p.version);
                if let Some(out) = png_path(&k) {
                    if let Some(d) = out.parent() {
                        let _ = std::fs::create_dir_all(d);
                    }
                    let mut cmd = Command::new(exe);
                    cmd.args([
                        "--capture-thumb",
                        &p.format,
                        &p.path,
                        &p.id,
                        out.to_str().unwrap_or(""),
                        "--name",
                        &p.name,
                        "--vendor",
                        &p.vendor,
                        "--version",
                        &p.version,
                    ])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                    #[cfg(windows)]
                    {
                        use std::os::windows::process::CommandExt;
                        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                        cmd.creation_flags(CREATE_NO_WINDOW);
                    }
                    match cmd.spawn() {
                        Ok(c) => {
                            self.child = Some(c);
                            self.child_key = k;
                            self.busy = true;
                            self.started = Some(Instant::now());
                        }
                        Err(_) => {
                            write_card(
                                &k,
                                &p.name,
                                &p.vendor,
                                &p.format,
                                Kind::Failed,
                                "could not start capture",
                            );
                            finished = true;
                        }
                    }
                }
            }
        }
        finished
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len() + if self.busy { 1 } else { 0 }
    }
}

static QUEUE: Mutex<Option<Queue>> = Mutex::new(None);

pub fn queue() -> std::sync::MutexGuard<'static, Option<Queue>> {
    QUEUE.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_scrubs_and_card_is_valid_png() {
        let k = key("vst3", "id/with spaces", "1.0.0");
        assert!(k
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)));
        let (w, h, rgba) = generate_card("Dexed", "Digital Suburban", "vst3", [0x7A, 0xA2, 0xE8]);
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        let dir = tempfile_dir();
        let p = dir.join("t.png");
        save_png(&p, w, h, &rgba).unwrap();
        let (w2, h2, rgba2) = load_png(&p).unwrap();
        assert_eq!((w2, h2), (w, h));
        assert_eq!(rgba2.len(), rgba.len());
        let (dw, dh, _) = downscale(800, 600, &vec![0u8; 800 * 600 * 4], 256);
        assert!(dw <= 256 && dh <= 256);
    }

    fn tempfile_dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!("cw-thumb-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&p);
        p
    }
}
