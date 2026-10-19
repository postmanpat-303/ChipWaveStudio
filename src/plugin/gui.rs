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

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
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
        let st = style(resizable);
        let mut r = RECT {
            left: 0,
            top: 0,
            right: w as i32,
            bottom: h as i32,
        };
        let t = wide(title);
        let hwnd = unsafe {
            AdjustWindowRectEx(&mut r, st, 0, 0);
            CreateWindowExW(
                0,
                class(),
                t.as_ptr(),
                st,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
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
        if !make(p, API.as_ptr(), false) {
            return Err("the plugin could not create its editor".into());
        }
        if let Some(s) = gui.set_scale {
            s(p, 1.0);
        }
        let (mut w, mut h) = (800u32, 500u32);
        if let Some(gs) = gui.get_size {
            gs(p, &mut w, &mut h);
        }
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
        if !gui.set_parent.map(|s| s(p, &win)).unwrap_or(false) {
            destroy(hwnd as isize);
            if let Some(d) = gui.destroy {
                d(p);
            }
            return Err("the plugin refused the editor window".into());
        }
        ShowWindow(hwnd, SW_SHOW);
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
        unsafe {
            ShowWindow(hwnd as HWND, SW_SHOW);
            SetForegroundWindow(hwnd as HWND);
        }
    }

    pub fn resize(hwnd: isize, w: u32, h: u32) {
        unsafe {
            let st = GetWindowLongW(hwnd as HWND, GWL_STYLE) as u32;
            let mut r = RECT {
                left: 0,
                top: 0,
                right: w as i32,
                bottom: h as i32,
            };
            AdjustWindowRectEx(&mut r, st, 0, 0);
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
