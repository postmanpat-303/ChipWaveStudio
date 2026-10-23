//! Plugin editor windows. Windows: a top-level Win32 window that the plugin
//! embeds its editor into (CLAP gui ext, api "win32"; VST3 `IPlugView`
//! attached to the HWND, see vst3.rs); a CLAP plugin that only offers a
//! floating window gets that. Other platforms: floating editors only
//! (x11 / cocoa), otherwise the parameter list is the editor. CLAP gui
//! extension: free-audio CLAP (MIT) via clap-sys (MIT OR Apache-2.0); Win32
//! through windows-sys (Microsoft, MIT OR Apache-2.0).
use clap_sys::ext::gui::*;
use clap_sys::plugin::clap_plugin;
use std::ffi::CString;

#[cfg(windows)]
const API: &std::ffi::CStr = CLAP_WINDOW_API_WIN32;
#[cfg(all(unix, not(target_os = "macos")))]
const API: &std::ffi::CStr = CLAP_WINDOW_API_X11;
#[cfg(target_os = "macos")]
const API: &std::ffi::CStr = CLAP_WINDOW_API_COCOA;

/// CLAP `gui.set_scale` factor for a monitor DPI (96 = 100 %); unknown
/// (0) means 100 %, odd values are kept within 50-400 %
#[cfg_attr(not(windows), allow(dead_code))]
pub fn scale_from_dpi(dpi: u32) -> f64 {
    if dpi == 0 {
        return 1.0;
    }
    (dpi as f64 / 96.0).clamp(0.5, 4.0)
}

/// can this plugin show an editor here (embedded on Windows, else floating)?
pub fn supported(p: *const clap_plugin, g: *const clap_plugin_gui) -> bool {
    unsafe {
        let Some(sup) = (*g).is_api_supported else {
            return false;
        };
        (cfg!(windows) && sup(p, API.as_ptr(), false)) || sup(p, API.as_ptr(), true)
    }
}

pub struct Window {
    #[cfg_attr(not(windows), allow(dead_code))]
    floating: bool,
    #[cfg(windows)]
    hwnd: isize,
}

impl Window {
    /// a ChipWave window the editor was embedded into (VST3)
    #[cfg(windows)]
    pub fn embedded(hwnd: isize) -> Window {
        Window {
            floating: false,
            hwnd,
        }
    }
    pub fn closed(&self) -> bool {
        #[cfg(windows)]
        if !self.floating {
            return win::closed(self.hwnd);
        }
        false
    }
    pub fn raise(&self) {
        #[cfg(windows)]
        if !self.floating {
            win::raise(self.hwnd);
        }
    }
    pub fn resize(&self, w: u32, h: u32) {
        #[cfg(windows)]
        if !self.floating {
            win::resize(self.hwnd, w, h);
        }
        let _ = (w, h);
    }
}

pub fn open(
    p: *const clap_plugin,
    g: *const clap_plugin_gui,
    title: &str,
) -> Result<Window, String> {
    unsafe {
        let gui = &*g;
        let sup = gui.is_api_supported.ok_or("no editor")?;
        #[cfg(windows)]
        if sup(p, API.as_ptr(), false) {
            return win::open_embedded(p, g, title);
        }
        #[cfg(windows)]
        if win::silent() {
            // thumbnail capture: a plugin-made floating window can't be kept
            // off-screen and quiet, so it gets a card instead
            return Err("floating editor (not captured)".into());
        }
        if sup(p, API.as_ptr(), true) {
            let create = gui.create.ok_or("no editor")?;
            if !create(p, API.as_ptr(), true) {
                return Err("the plugin could not create its editor".into());
            }
            if let (Some(t), Ok(c)) = (gui.suggest_title, CString::new(title)) {
                t(p, c.as_ptr());
            }
            if let Some(s) = gui.show {
                s(p);
            }
            return Ok(Window {
                floating: true,
                #[cfg(windows)]
                hwnd: 0,
            });
        }
        Err(if cfg!(windows) {
            "this plugin has no Windows editor; use the parameter list".into()
        } else {
            "this plugin's editor can't open in a separate window on this system yet; use the parameter list (Windows: full editor)".into()
        })
    }
}

/// destroy the window an editor was embedded into (VST3: after `removed`)
#[cfg(windows)]
pub fn destroy_window(w: Window) {
    if !w.floating {
        win::destroy(w.hwnd);
    }
}

pub fn close(p: *const clap_plugin, g: *const clap_plugin_gui, w: Window) {
    unsafe {
        let gui = &*g;
        if let Some(h) = gui.hide {
            h(p);
        }
        if let Some(d) = gui.destroy {
            d(p);
        }
    }
    #[cfg(windows)]
    if !w.floating {
        win::destroy(w.hwnd);
    }
    let _ = w;
}

#[cfg(windows)]
pub mod win {
    use super::*;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    /// whose editor a window holds (raw pointers kept as integers)
    #[derive(Clone, Copy)]
    pub enum Owner {
        /// CLAP plugin + gui extension
        Clap { p: usize, g: usize },
        /// VST3 `IPlugView`
        Vst3 { view: usize },
    }

    /// (hwnd, owner, closed by the user, resized by the plugin itself)
    struct Entry {
        hwnd: isize,
        owner: Owner,
        closed: bool,
        quiet: bool,
    }
    static WINDOWS: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

    /// thumbnail-capture child: editor windows are created off-screen, never
    /// activated, kept off the taskbar and out of Alt+Tab
    static SILENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    pub fn set_silent() {
        SILENT.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn silent() -> bool {
        SILENT.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// silent window style: a borderless tool window that never takes focus
    const SILENT_EX: u32 = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;

    /// a spot just right of every monitor (the window's whole rect is off-screen)
    fn off_screen() -> (i32, i32) {
        unsafe {
            let x = GetSystemMetrics(SM_XVIRTUALSCREEN) + GetSystemMetrics(SM_CXVIRTUALSCREEN);
            (x + 64, GetSystemMetrics(SM_YVIRTUALSCREEN))
        }
    }

    /// show without activating (silent) or normally
    fn show(hwnd: HWND) {
        unsafe {
            ShowWindow(hwnd, if silent() { SW_SHOWNOACTIVATE } else { SW_SHOW });
        }
    }

    /// second try for a capture that came back blank off-screen: make the
    /// window fully transparent and click-through, and put it on the screen
    /// (some editors only paint where a monitor is). Still never activated,
    /// still invisible.
    pub fn transparent_on_screen(hwnd: isize) {
        unsafe {
            let h = hwnd as HWND;
            let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
            SetWindowLongW(
                h,
                GWL_EXSTYLE,
                (ex | WS_EX_LAYERED | WS_EX_TRANSPARENT | SILENT_EX) as i32,
            );
            SetLayeredWindowAttributes(h, 0, 0, LWA_ALPHA);
            SetWindowPos(
                h,
                HWND_BOTTOM,
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// DPI of the monitor `hwnd` is on: GetDpiForWindow (Windows 10 1607+,
    /// looked up at run time; the app is per-monitor DPI aware), else the
    /// system DPI
    pub fn dpi_of(hwnd: HWND) -> u32 {
        type GetDpi = unsafe extern "system" fn(HWND) -> u32;
        static GET: std::sync::OnceLock<Option<GetDpi>> = std::sync::OnceLock::new();
        let get = GET.get_or_init(|| unsafe {
            use windows_sys::Win32::System::LibraryLoader::GetProcAddress;
            let user32 = GetModuleHandleW(wide("user32.dll").as_ptr());
            if user32.is_null() {
                return None;
            }
            GetProcAddress(user32, c"GetDpiForWindow".as_ptr() as *const u8)
                .map(|f| std::mem::transmute::<unsafe extern "system" fn() -> isize, GetDpi>(f))
        });
        if let Some(f) = get {
            if !hwnd.is_null() {
                let d = unsafe { f(hwnd) };
                if d > 0 {
                    return d;
                }
            }
        }
        unsafe {
            use windows_sys::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, ReleaseDC, LOGPIXELSX};
            let dc = GetDC(std::ptr::null_mut());
            if dc.is_null() {
                return 96;
            }
            let d = GetDeviceCaps(dc, LOGPIXELSX as i32);
            ReleaseDC(std::ptr::null_mut(), dc);
            if d > 0 {
                d as u32
            } else {
                96
            }
        }
    }

    /// tell the editor its DPI scale; false when the plugin has no
    /// set_scale or refuses it (it then follows the DPI itself, and asks
    /// for a new size through request_resize)
    unsafe fn apply_scale(p: *const clap_plugin, g: *const clap_plugin_gui, dpi: u32) -> bool {
        let sc = super::scale_from_dpi(dpi);
        let ok = (*g).set_scale.map(|s| s(p, sc)).unwrap_or(false);
        if !ok {
            crate::plugin::debug::log(|| {
                format!("editor refused set_scale({sc:.2}) at {dpi} dpi; it scales itself")
            });
        }
        ok
    }

    /// the editor's size now (after a scale change)
    unsafe fn editor_size(p: *const clap_plugin, g: *const clap_plugin_gui) -> Option<(u32, u32)> {
        let (mut w, mut h) = (0u32, 0u32);
        let gs = (*g).get_size?;
        (gs(p, &mut w, &mut h) && w > 0 && h > 0).then_some((w, h))
    }

    unsafe extern "system" fn proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_CLOSE => {
                // hide now; the UI destroys the editor and the window next frame
                ShowWindow(h, SW_HIDE);
                if let Ok(mut v) = WINDOWS.lock() {
                    for e in v.iter_mut().filter(|e| e.hwnd == h as isize) {
                        e.closed = true;
                    }
                }
                0
            }
            WM_SIZE => {
                let (w, hh) = ((lp & 0xFFFF) as u32, ((lp >> 16) & 0xFFFF) as u32);
                let found = WINDOWS.lock().ok().and_then(|v| {
                    v.iter()
                        .find(|e| e.hwnd == h as isize && !e.quiet)
                        .map(|e| e.owner)
                });
                if let Some(Owner::Vst3 { view }) = found {
                    vst3_size(view, w, hh);
                }
                if let Some(Owner::Clap { p, g }) = found {
                    let (p, g) = (p as *const clap_plugin, g as *const clap_plugin_gui);
                    if (*g).can_resize.map(|c| c(p)).unwrap_or(false) && w > 0 && hh > 0 {
                        let (mut aw, mut ah) = (w, hh);
                        if let Some(a) = (*g).adjust_size {
                            a(p, &mut aw, &mut ah);
                        }
                        if let Some(s) = (*g).set_size {
                            s(p, aw, ah);
                        }
                    }
                }
                DefWindowProcW(h, msg, wp, lp)
            }
            WM_DPICHANGED => {
                // moved to a monitor with another scale: rescale the editor
                // and fit the window to its new size
                let dpi = ((wp >> 16) & 0xFFFF) as u32;
                let found = WINDOWS
                    .lock()
                    .ok()
                    .and_then(|v| v.iter().find(|e| e.hwnd == h as isize).map(|e| e.owner));
                if let Some(Owner::Clap { p, g }) = found {
                    let (p, g) = (p as *const clap_plugin, g as *const clap_plugin_gui);
                    if apply_scale(p, g, dpi) {
                        if let Some((w, hh)) = editor_size(p, g) {
                            resize_quiet(h as isize, w, hh);
                        }
                    }
                }
                0
            }
            _ => DefWindowProcW(h, msg, wp, lp),
        }
    }

    /// the user resized a VST3 editor window: let the view follow if it can
    fn vst3_size(view: usize, w: u32, h: u32) {
        use vst3::Steinberg::{IPlugView, IPlugViewTrait, ViewRect};
        let Some(v) = (unsafe { vst3::ComRef::from_raw(view as *mut IPlugView) }) else {
            return;
        };
        if w == 0 || h == 0 || unsafe { v.canResize() } != vst3::Steinberg::kResultTrue {
            return;
        }
        let mut r = ViewRect {
            left: 0,
            top: 0,
            right: w as i32,
            bottom: h as i32,
        };
        unsafe {
            v.checkSizeConstraint(&mut r);
            v.onSize(&mut r);
        }
    }

    fn class() -> *const u16 {
        static CLASS: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
        CLASS
            .get_or_init(|| {
                let name = wide("ChipWavePluginEditor");
                unsafe {
                    let wc = WNDCLASSW {
                        style: 0,
                        lpfnWndProc: Some(proc),
                        cbClsExtra: 0,
                        cbWndExtra: 0,
                        hInstance: GetModuleHandleW(std::ptr::null()),
                        hIcon: std::ptr::null_mut(),
                        hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                        hbrBackground: std::ptr::null_mut(),
                        lpszMenuName: std::ptr::null(),
                        lpszClassName: name.as_ptr(),
                    };
                    RegisterClassW(&wc);
                }
                name
            })
            .as_ptr()
    }

    fn style(resizable: bool) -> u32 {
        if resizable {
            WS_OVERLAPPEDWINDOW & !WS_MAXIMIZEBOX
        } else {
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX
        }
    }

    /// a top-level window with a client area of `w` x `h` for an editor
    pub fn create(
        title: &str,
        w: u32,
        h: u32,
        resizable: bool,
        owner: Owner,
    ) -> Result<isize, String> {
        let quiet = silent();
        let st = if quiet { WS_POPUP } else { style(resizable) };
        let ex = if quiet { SILENT_EX } else { 0 };
        let (x, y) = if quiet {
            off_screen()
        } else {
            (CW_USEDEFAULT, CW_USEDEFAULT)
        };
        let mut r = RECT {
            left: 0,
            top: 0,
            right: w as i32,
            bottom: h as i32,
        };
        let t = wide(title);
        let hwnd = unsafe {
            AdjustWindowRectEx(&mut r, st, 0, ex);
            CreateWindowExW(
                ex,
                class(),
                t.as_ptr(),
                st,
                x,
                y,
                r.right - r.left,
                r.bottom - r.top,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            return Err("could not create the editor window".into());
        }
        if let Ok(mut v) = WINDOWS.lock() {
            v.push(Entry {
                hwnd: hwnd as isize,
                owner,
                closed: false,
                quiet: false,
            });
        }
        Ok(hwnd as isize)
    }

    /// resize for the plugin (no size callback back into it)
    pub fn resize_quiet(hwnd: isize, w: u32, h: u32) {
        let set = |q: bool| {
            if let Ok(mut v) = WINDOWS.lock() {
                for e in v.iter_mut().filter(|e| e.hwnd == hwnd) {
                    e.quiet = q;
                }
            }
        };
        set(true);
        resize(hwnd, w, h);
        set(false);
    }

    pub unsafe fn open_embedded(
        p: *const clap_plugin,
        g: *const clap_plugin_gui,
        title: &str,
    ) -> Result<Window, String> {
        let gui = &*g;
        let make = gui.create.ok_or("no editor")?;
        let t = crate::plugin::debug::Timed::new(|| format!("{title}: gui.create (embedded)"));
        if !make(p, API.as_ptr(), false) {
            return Err("the plugin could not create its editor".into());
        }
        drop(t);
        // scale for the monitor ChipWave is on (thumbnails: always 100 %);
        // checked again once the window exists
        let mut dpi = if silent() {
            96
        } else {
            dpi_of(GetForegroundWindow())
        };
        apply_scale(p, g, dpi);
        let (w, h) = editor_size(p, g).unwrap_or((800, 500));
        let resizable = gui.can_resize.map(|c| c(p)).unwrap_or(false);
        let hwnd = match create(
            title,
            w,
            h,
            resizable,
            Owner::Clap {
                p: p as usize,
                g: g as usize,
            },
        ) {
            Ok(h) => h as HWND,
            Err(e) => {
                if let Some(d) = gui.destroy {
                    d(p);
                }
                return Err(e);
            }
        };
        let win = clap_window {
            api: API.as_ptr(),
            specific: clap_window_handle { win32: hwnd },
        };
        let t = crate::plugin::debug::Timed::new(|| format!("{title}: gui.set_parent"));
        let parented = gui.set_parent.map(|s| s(p, &win)).unwrap_or(false);
        drop(t);
        if !parented {
            destroy(hwnd as isize);
            if let Some(d) = gui.destroy {
                d(p);
            }
            return Err("the plugin refused the editor window".into());
        }
        if !silent() {
            let real = dpi_of(hwnd);
            if real != dpi {
                dpi = real;
                if apply_scale(p, g, dpi) {
                    if let Some((w, h)) = editor_size(p, g) {
                        resize_quiet(hwnd as isize, w, h);
                    }
                }
            }
        }
        crate::plugin::debug::log(|| format!("{title}: editor at {dpi} dpi"));
        show(hwnd);
        let _t = crate::plugin::debug::Timed::new(|| format!("{title}: gui.show"));
        if let Some(s) = gui.show {
            s(p);
        }
        Ok(Window {
            floating: false,
            hwnd: hwnd as isize,
        })
    }

    pub fn closed(hwnd: isize) -> bool {
        WINDOWS
            .lock()
            .map(|v| v.iter().any(|e| e.hwnd == hwnd && e.closed))
            .unwrap_or(false)
    }

    pub fn raise(hwnd: isize) {
        show(hwnd as HWND);
        if !silent() {
            unsafe {
                SetForegroundWindow(hwnd as HWND);
            }
        }
    }

    pub fn resize(hwnd: isize, w: u32, h: u32) {
        unsafe {
            let st = GetWindowLongW(hwnd as HWND, GWL_STYLE) as u32;
            let ex = if silent() { SILENT_EX } else { 0 };
            let mut r = RECT {
                left: 0,
                top: 0,
                right: w as i32,
                bottom: h as i32,
            };
            AdjustWindowRectEx(&mut r, st, 0, ex);
            SetWindowPos(
                hwnd as HWND,
                std::ptr::null_mut(),
                0,
                0,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    pub fn destroy(hwnd: isize) {
        if let Ok(mut v) = WINDOWS.lock() {
            v.retain(|e| e.hwnd != hwnd);
        }
        unsafe {
            DestroyWindow(hwnd as HWND);
        }
    }

    /// most recently created editor HWND (for thumbnail capture)
    pub fn last_hwnd() -> Option<isize> {
        WINDOWS.lock().ok()?.last().map(|e| e.hwnd)
    }
}

#[cfg(test)]
mod scale_tests {
    use super::scale_from_dpi;

    #[test]
    fn dpi_maps_to_the_clap_scale() {
        assert_eq!(scale_from_dpi(96), 1.0);
        assert_eq!(scale_from_dpi(120), 1.25);
        assert_eq!(scale_from_dpi(144), 1.5);
        assert_eq!(scale_from_dpi(192), 2.0);
        assert_eq!(scale_from_dpi(0), 1.0, "unknown DPI: 100 %");
        assert_eq!(scale_from_dpi(10), 0.5);
        assert_eq!(scale_from_dpi(2000), 4.0);
    }
}
