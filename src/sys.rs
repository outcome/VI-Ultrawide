//! Win32 declarations and small helpers shared by the modules.
#![allow(unsafe_code, non_snake_case)]
use std::ffi::c_void;
use std::path::PathBuf;

pub type Hwnd = *mut c_void;
#[repr(C)] #[derive(Clone, Copy, Default)] pub struct Rect { pub l: i32, pub t: i32, pub r: i32, pub b: i32 }
#[repr(C)] pub struct MonitorInfo { pub size: u32, pub monitor: Rect, pub work: Rect, pub flags: u32 }
unsafe extern "system" {
    pub fn LoadLibraryW(name: *const u16) -> *mut c_void;
    pub fn GetProcAddress(m: *mut c_void, name: *const u8) -> *mut c_void;
    pub fn GetModuleHandleExW(flags: u32, addr: *const c_void, out: *mut *mut c_void) -> i32;
    pub fn GetSystemDirectoryW(buf: *mut u16, n: u32) -> u32;
    pub fn GetModuleFileNameW(m: *mut c_void, buf: *mut u16, n: u32) -> u32;
    pub fn SetEnvironmentVariableW(k: *const u16, v: *const u16) -> i32;
    pub fn VirtualProtect(addr: *mut c_void, size: usize, new: u32, old: *mut u32) -> i32;
    pub fn GetCurrentProcessId() -> u32;
    pub fn EnumWindows(f: unsafe extern "system" fn(Hwnd, isize) -> i32, l: isize) -> i32;
    pub fn GetWindowThreadProcessId(h: Hwnd, pid: *mut u32) -> u32;
    pub fn IsWindowVisible(h: Hwnd) -> i32;
    pub fn GetWindow(h: Hwnd, cmd: u32) -> Hwnd;
    pub fn GetWindowRect(h: Hwnd, r: *mut Rect) -> i32;
    pub fn GetWindowLongPtrW(h: Hwnd, i: i32) -> isize;
    pub fn SetWindowLongPtrW(h: Hwnd, i: i32, v: isize) -> isize;
    pub fn SetWindowPos(h: Hwnd, after: Hwnd, x: i32, y: i32, w: i32, hh: i32, flags: u32) -> i32;
    pub fn MonitorFromWindow(h: Hwnd, flags: u32) -> *mut c_void;
    pub fn GetMonitorInfoW(m: *mut c_void, info: *mut MonitorInfo) -> i32;
}


pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// The directory of this dll (where uw-options.ini lives).
pub fn dll_dir() -> PathBuf {
    let mut m = std::ptr::null_mut();
    let mut buf = [0u16; 1024];
    unsafe {
        GetModuleHandleExW(4 | 2, dll_dir as *const c_void, &mut m); // FROM_ADDRESS | UNCHANGED_REFCOUNT
        let n = GetModuleFileNameW(m, buf.as_mut_ptr(), 1024) as usize;
        PathBuf::from(String::from_utf16_lossy(&buf[..n])).parent().map(|p| p.to_path_buf()).unwrap_or_default()
    }
}
