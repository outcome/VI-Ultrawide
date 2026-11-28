//! Test-only stand-in pad inside this process: with UW_VPAD=<file> set, pad 0 also reports what that file says
//! ("buttons lt rt lx ly rx ry", XInput units). Nothing outside the game sees it. Hooks victory.exe's
//! XInputGetState import.
#![allow(unsafe_code)]
use std::ffi::c_void;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::sys::{GetModuleHandleW, GetProcAddress, LoadLibraryW, VirtualProtect, wide};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Pad { buttons: u16, lt: u8, rt: u8, lx: i16, ly: i16, rx: i16, ry: i16 }
#[repr(C)]
struct State { packet: u32, pad: Pad }

type GetState = unsafe extern "system" fn(u32, *mut State) -> u32;
static mut REAL: Option<GetState> = None;
static FILE: Mutex<Option<(String, Instant, Pad, u32)>> = Mutex::new(None);

fn virtual_pad() -> Pad {
    let mut g = FILE.lock().unwrap();
    let Some((path, at, pad, n)) = g.as_mut() else { return Pad::default() };
    if at.elapsed() > Duration::from_millis(10) {
        *at = Instant::now();
        let v: Vec<i32> = std::fs::read_to_string(&*path).unwrap_or_default().split_whitespace().filter_map(|s| s.parse().ok()).collect();
        let g = |i: usize| v.get(i).copied().unwrap_or(0);
        let new = Pad { buttons: g(0) as u16, lt: g(1) as u8, rt: g(2) as u8, lx: g(3) as i16, ly: g(4) as i16, rx: g(5) as i16, ry: g(6) as i16 };
        if new.buttons != pad.buttons || (new.lx, new.ly, new.rx, new.ry, new.lt, new.rt) != (pad.lx, pad.ly, pad.rx, pad.ry, pad.lt, pad.rt) {
            *n += 1;
        }
        *pad = new;
    }
    *pad
}

unsafe extern "system" fn hooked(slot: u32, out: *mut State) -> u32 {
    let r = unsafe { REAL.map_or(1167, |f| f(slot, out)) };
    if slot != 0 { return r; }
    let v = virtual_pad();
    let packet = FILE.lock().unwrap().as_ref().map_or(0, |f| f.3);
    let s = unsafe { &mut *out };
    if r != 0 { s.packet = packet; s.pad = v; return 0; } // no real pad: the stand-in is pad 0
    s.packet = s.packet.wrapping_add(packet);
    s.pad.buttons |= v.buttons;
    s.pad.lt = s.pad.lt.max(v.lt);
    s.pad.rt = s.pad.rt.max(v.rt);
    if v.lx != 0 || v.ly != 0 { (s.pad.lx, s.pad.ly) = (v.lx, v.ly); }
    if v.rx != 0 || v.ry != 0 { (s.pad.rx, s.pad.ry) = (v.rx, v.ry); }
    0
}

/// Point the main exe's XInputGetState import at `hooked` (call from a thread: imports are bound by then).
pub fn install() {
    let Ok(path) = std::env::var("UW_VPAD") else { return };
    *FILE.lock().unwrap() = Some((path, Instant::now() - Duration::from_secs(1), Pad::default(), 1));
    unsafe {
        let real = GetProcAddress(LoadLibraryW(wide("xinput1_4.dll").as_ptr()), c"XInputGetState".as_ptr().cast());
        if real.is_null() { return; }
        REAL = Some(std::mem::transmute::<*mut c_void, GetState>(real));
        let base = GetModuleHandleW(std::ptr::null()) as usize;
        let pe = base + *((base + 0x3c) as *const u32) as usize;
        let (dir, size) = (*((pe + 0x90) as *const u32) as usize, *((pe + 0x94) as *const u32) as usize);
        let mut d = base + dir;
        while d < base + dir + size && *((d + 16) as *const u32) != 0 {
            let mut slot = base + *((d + 16) as *const u32) as usize; // FirstThunk (bound IAT)
            while *(slot as *const usize) != 0 {
                if *(slot as *const usize) == real as usize {
                    let mut old = 0;
                    VirtualProtect(slot as *mut c_void, 8, 0x04, &mut old);
                    *(slot as *mut usize) = hooked as *const () as usize;
                    VirtualProtect(slot as *mut c_void, 8, old, &mut old);
                }
                slot += 8;
            }
            d += 20;
        }
    }
}
