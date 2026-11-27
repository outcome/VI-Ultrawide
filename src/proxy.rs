//! The proxy: this dll is named dxgi.dll, so VIctory loads it; it forwards everything to the real one.
#![allow(unsafe_code, non_snake_case)]
use std::ffi::c_void;

use crate::sys::*;

fn real(name: &[u8]) -> *mut c_void {
    static LIB: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let lib = *LIB.get_or_init(|| {
        let mut b = [0u16; 512];
        let n = unsafe { GetSystemDirectoryW(b.as_mut_ptr(), 512) } as usize;
        let mut p = String::from_utf16_lossy(&b[..n]);
        p.push_str("\\dxgi.dll");
        unsafe { LoadLibraryW(wide(&p).as_ptr()) as usize }
    });
    unsafe { GetProcAddress(lib as *mut c_void, name.as_ptr()) }
}

macro_rules! forward {
    ($name:ident, $sym:expr, ($($a:ident: $t:ty),*)) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "system" fn $name($($a: $t),*) -> i32 {
            let f: unsafe extern "system" fn($($t),*) -> i32 = unsafe { std::mem::transmute(real($sym)) };
            unsafe { f($($a),*) }
        }
    };
}
forward!(CreateDXGIFactory, b"CreateDXGIFactory\0", (riid: *const c_void, out: *mut *mut c_void));
forward!(CreateDXGIFactory1, b"CreateDXGIFactory1\0", (riid: *const c_void, out: *mut *mut c_void));
forward!(CreateDXGIFactory2, b"CreateDXGIFactory2\0", (flags: u32, riid: *const c_void, out: *mut *mut c_void));
forward!(DXGIGetDebugInterface1, b"DXGIGetDebugInterface1\0", (flags: u32, riid: *const c_void, out: *mut *mut c_void));
