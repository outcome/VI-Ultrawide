//! Borderless fullscreen: a watcher finds this process's top-level window and covers its monitor.
#![allow(unsafe_code)]
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
pub fn borderless_watcher() {
    let pid = unsafe { GetCurrentProcessId() };
    loop {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let mut f = Find { pid, hwnd: std::ptr::null_mut() };
        unsafe { EnumWindows(each, &mut f as *mut _ as isize) };
        if f.hwnd.is_null() { continue; }
        unsafe {
            let style = GetWindowLongPtrW(f.hwnd, -16);
            if style & 0x00C00000 != 0 { SetWindowLongPtrW(f.hwnd, -16, (style & !0x00CF0000 & !0x00040000) | 0x80000000u32 as isize); }
            let mon = MonitorFromWindow(f.hwnd, 2);
            let mut mi = MonitorInfo { size: size_of::<MonitorInfo>() as u32, monitor: Rect::default(), work: Rect::default(), flags: 0 };
            if GetMonitorInfoW(mon, &mut mi) == 0 { continue; }
            let m = mi.monitor;
            let mut now = Rect::default();
            GetWindowRect(f.hwnd, &mut now);
            if (now.l, now.t, now.r, now.b) != (m.l, m.t, m.r, m.b) || style & 0x00C00000 != 0 {
                // 0x4000 ASYNCWINDOWPOS | 0x20 FRAMECHANGED | 0x4 NOZORDER | 0x10 NOACTIVATE
                SetWindowPos(f.hwnd, std::ptr::null_mut(), m.l, m.t, m.r - m.l, m.b - m.t, 0x4000 | 0x20 | 0x4 | 0x10);
            }
        }
    }
}

