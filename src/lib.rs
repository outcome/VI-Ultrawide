//! GTA VI ultrawide mod for any VIctory build: drop this dxgi.dll next to victory.exe.
//! Options (uw-options.ini, created on first start): res_x/res_y, borderless_fullscreen, fov_scale. Anything left
//! empty has no effect. Linux/Proton needs WINEDLLOVERRIDES=dxgi=n,b ; Windows needs nothing.
#![allow(unsafe_code)]
mod options;
mod patches;
mod proxy;
mod sys;
mod window;

use std::ffi::c_void;

use sys::{SetEnvironmentVariableW, wide};

fn set_env(k: &str, v: &str) {
    unsafe { SetEnvironmentVariableW(wide(k).as_ptr(), wide(v).as_ptr()) };
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(_h: *mut c_void, reason: u32, _r: *mut c_void) -> i32 {
    if reason == 1 {
        let o = options::options();
        if let Some((w, h)) = o.res {
            set_env("VICTORY_DISPLAY_MODE", &format!("{w}x{h}")); // read by VIctory forks that have a display mode
        }
        if patches::wanted(&o) {
            set_env("VICTORY_NO_UW", "1"); // a build with its own ultrawide module stands aside
            if !(o.patch_exe && patches::patch_exe(&o)) {
                let o2 = o.clone();
                std::thread::spawn(move || patches::memory_patcher(o2));
            }
        }
        if o.borderless {
            std::thread::spawn(window::borderless_watcher);
        }
    }
    1
}
