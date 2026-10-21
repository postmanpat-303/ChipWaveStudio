//! The native window title bar in the theme's colours (Windows only).
//! Native decorations stay (snap, caption buttons, resize); DWM colours the
//! bar: dark mode for the caption buttons (Windows 10 1809+), caption, text
//! and border colours on Windows 11 (build 22000+). Every call is best
//! effort: an older Windows ignores what it doesn't know. Elsewhere a no-op.
use eframe::egui::Color32;

/// what the title bar shows: (caption, text, border, dark caption buttons)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colours {
    pub caption: Color32,
    pub text: Color32,
    pub border: Color32,
    pub dark: bool,
}

/// the bar for a theme: the menu bar's colour (so the two read as one),
/// the theme's text colour, or white on a saturated / mid caption
pub fn for_theme(t: &crate::theme::Theme) -> Colours {
    let c = t.hdr_top;
    let lum = (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0;
    let dark = lum < 0.5;
    let text = if !dark {
        Color32::BLACK
    } else if t.light {
        // a light theme with a dark caption (Amiga's blue bar): white text
        Color32::WHITE
    } else {
        t.ink
    };
    Colours {
        caption: c,
        text,
        border: t.border,
        dark,
    }
}

/// Win32 COLORREF: 0x00BBGGRR
#[cfg_attr(not(windows), allow(dead_code))]
pub fn colorref(c: Color32) -> u32 {
    c.r() as u32 | (c.g() as u32) << 8 | (c.b() as u32) << 16
}

/// colour this process's main window; false while the window isn't there
/// yet (call again next frame)
#[cfg(windows)]
pub fn apply(c: Colours) -> bool {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::Graphics::Dwm::*;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    // the first top-level, unowned window of this (the UI) thread whose
    // title starts with "ChipWave" (plugin editors are owned / other titles)
    unsafe extern "system" fn find(h: HWND, lp: LPARAM) -> windows_sys::core::BOOL {
        unsafe {
            let mut buf = [0u16; 64];
            let n = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
            let title = String::from_utf16_lossy(&buf[..n]);
            if title.starts_with("ChipWave") && GetWindow(h, GW_OWNER).is_null() {
                *(lp as *mut HWND) = h;
                return 0;
            }
        }
        1
    }
    let mut hwnd: HWND = std::ptr::null_mut();
    unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(find),
            &mut hwnd as *mut HWND as LPARAM,
        );
    }
    if hwnd.is_null() {
        return false;
    }
    let set = |attr: DWMWINDOWATTRIBUTE, v: u32| unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attr as u32,
            &v as *const u32 as *const core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        )
    };
    let dark = c.dark as u32;
    // 20 since Windows 10 20H1; 19 before that (1809-1909)
    if set(DWMWA_USE_IMMERSIVE_DARK_MODE, dark) != 0 {
        let _ = set(19, dark);
    }
    // Windows 11 22000+; older builds return an error, which is fine
    let _ = set(DWMWA_CAPTION_COLOR, colorref(c.caption));
    let _ = set(DWMWA_TEXT_COLOR, colorref(c.text));
    let _ = set(DWMWA_BORDER_COLOR, colorref(c.border));
    true
}

#[cfg(not(windows))]
pub fn apply(_: Colours) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::THEMES;

    #[test]
    fn every_theme_gets_a_readable_bar() {
        for t in THEMES {
            let c = for_theme(&t);
            assert_eq!(c.caption, t.hdr_top, "{}: the menu bar's colour", t.name);
            let l = |c: Color32| {
                (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
            };
            assert!(
                (l(c.text) - l(c.caption)).abs() > 0.45,
                "{}: text {:?} on {:?}",
                t.name,
                c.text,
                c.caption
            );
            let h = |c: Color32| format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
            println!(
                "TITLEBAR {}: caption {} text {} border {} dark {}",
                t.name,
                h(c.caption),
                h(c.text),
                h(c.border),
                c.dark
            );
        }
        assert_eq!(colorref(Color32::from_rgb(0x12, 0x34, 0x56)), 0x0056_3412);
    }
}
