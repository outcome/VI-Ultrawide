//! Borderless fullscreen. A watcher finds this process's top-level window and places it on its monitor:
//! `Fit` keeps the picture's shape (centred, black bars from a backdrop window), `Stretch` fills the monitor.
#![allow(unsafe_code)]
use crate::options::Borderless;
use crate::sys::*;

struct Find { pid: u32, hwnd: Hwnd }
unsafe extern "system" fn each(h: Hwnd, l: isize) -> i32 {
    let f = unsafe { &mut *(l as *mut Find) };
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(h, &mut pid) };
    if pid == f.pid && unsafe { IsWindowVisible(h) } != 0 && unsafe { GetWindow(h, 4) }.is_null() {
        let mut r = Rect::default();
        unsafe { GetWindowRect(h, &mut r) };
        if r.r - r.l >= 640 { f.hwnd = h; return 0; }
    }
    1
}

/// The largest (w, h) of the aspect `a` inside the monitor `m`, centred.
fn fit(m: Rect, a: f64) -> Rect {
    let (mw, mh) = ((m.r - m.l) as f64, (m.b - m.t) as f64);
    let (w, h) = if mw / mh > a { (mh * a, mh) } else { (mw, mw / a) };
    let (w, h) = (w.round() as i32, h.round() as i32);
    let (l, t) = (m.l + (m.r - m.l - w) / 2, m.t + (m.b - m.t - h) / 2);
    Rect { l, t, r: l + w, b: t + h }
}

fn pump() {
    let mut msg = Msg { hwnd: std::ptr::null_mut(), message: 0, wparam: 0, lparam: 0, time: 0, pt: [0; 2] };
    while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, 1) } != 0 {
        unsafe { TranslateMessage(&msg); DispatchMessageW(&msg); }
    }
}

/// `want`: the picture's size from the options; without it, the window's own shape is kept.
pub fn borderless_watcher(mode: Borderless, want: Option<(u32, u32)>) {
    let pid = unsafe { GetCurrentProcessId() };
    let mut aspect = want.map(|(w, h)| w as f64 / h as f64);
    let mut backdrop: Hwnd = std::ptr::null_mut();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(250));
        pump();
        let mut f = Find { pid, hwnd: std::ptr::null_mut() };
        unsafe { EnumWindows(each, &mut f as *mut _ as isize) };
        if f.hwnd.is_null() { continue; }
        unsafe {
            let mut now = Rect::default();
            GetWindowRect(f.hwnd, &mut now);
            if aspect.is_none() && now.b > now.t { aspect = Some((now.r - now.l) as f64 / (now.b - now.t) as f64); }
            let style = GetWindowLongPtrW(f.hwnd, -16);
            if style & 0x00C00000 != 0 { SetWindowLongPtrW(f.hwnd, -16, (style & !0x00CF0000 & !0x00040000) | 0x80000000u32 as isize); }
            let mon = MonitorFromWindow(f.hwnd, 2);
            let mut mi = MonitorInfo { size: size_of::<MonitorInfo>() as u32, monitor: Rect::default(), work: Rect::default(), flags: 0 };
            if GetMonitorInfoW(mon, &mut mi) == 0 { continue; }
            let m = mi.monitor;
            let target = match (mode, aspect) { (Borderless::Fit, Some(a)) => fit(m, a), _ => m };
            let bars = (target.l, target.t, target.r, target.b) != (m.l, m.t, m.r, m.b);
            let fg = GetForegroundWindow() == f.hwnd;
            if bars && backdrop.is_null() {
                // WS_POPUP | WS_VISIBLE | SS_BLACKRECT on a STATIC control; WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
                backdrop = CreateWindowExW(0x80 | 0x0800_0000, wide("STATIC").as_ptr(), wide("").as_ptr(), 0x8000_0000 | 0x1000_0000 | 0x4, m.l, m.t, m.r - m.l, m.b - m.t, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            }
            // 0x10 NOACTIVATE | 0x40 SHOWWINDOW; topmost only while the game has focus, so alt-tab still works
            let band = if bars && fg { -1isize } else { -2isize } as Hwnd;
            if !backdrop.is_null() {
                if bars { SetWindowPos(backdrop, band, m.l, m.t, m.r - m.l, m.b - m.t, 0x10 | 0x40); }
                ShowWindow(backdrop, if bars && fg { 5 } else { 0 });
            }
            if (now.l, now.t, now.r, now.b) != (target.l, target.t, target.r, target.b) || style & 0x00C00000 != 0 {
                // 0x4000 ASYNCWINDOWPOS | 0x20 FRAMECHANGED | 0x4 NOZORDER | 0x10 NOACTIVATE
                SetWindowPos(f.hwnd, std::ptr::null_mut(), target.l, target.t, target.r - target.l, target.b - target.t, 0x4000 | 0x20 | 0x4 | 0x10);
            }
            if bars && fg { SetWindowPos(f.hwnd, -1isize as Hwnd, 0, 0, 0, 0, 0x2 | 0x1 | 0x10); } // topmost, above the backdrop
        }
    }
}
