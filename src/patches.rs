//! The GTA VI v1.0.2.1 patches. Each group is one function returning (rva, expected bytes, new bytes) edits;
//! `patch_list` collects the groups the options ask for and checks every expected byte first (a different
//! game build, or an exe that is already patched, changes nothing). To add a fix: write a group function,
//! call it from `patch_list` under its option, add the key to options.rs.
#![allow(unsafe_code)]
use std::ffi::c_void;
use std::path::PathBuf;

use crate::options::Options;
use crate::sys::*;

pub type Patch = (usize, Vec<u8>, Vec<u8>);
type Read<'a> = &'a dyn Fn(usize, usize) -> Option<Vec<u8>>;

const PRESET_TABLE: usize = 0x615C0C0; // 5 entries of (w, h, fps, 1)
const ASSERT: usize = 0x247AF2B;
const HOOK: usize = 0x4FA3227;
const CAVE: usize = 0x5F40966;
const BACK_COPY: usize = 0x4FA3249;
const BACK_CMP: usize = 0x4FA322C;
const CAVE2: usize = CAVE + 0x6C;
const ASPECT_TABLE: usize = 0x654FC80;
const FOV_HOOK: usize = 0x416324C;
const FOV_BACK: usize = 0x4163254;
const FALLBACK: usize = 0x615A3EC;
const SCREEN_RECT: usize = 0x58048D8;
const SCREEN_RECT_END: usize = 0x58048DC;
const BASE_ASPECT_GETTER: usize = 0x73C0D0;
const CAMERA_ASPECT_CALLS: [usize; 5] = [0x417AB11, 0x4193A69, 0x418AAC9, 0x41798AA, 0x4163225];
const STUB: usize = 0x5F409EE;
const FOV_ASPECT_DISP: usize = 0x4163244;

fn f32b(v: f64) -> [u8; 4] {
    (v as f32).to_le_bytes()
}
fn rel(target: usize, next: usize) -> [u8; 4] {
    (target as i64 - next as i64).to_le_bytes()[..4].try_into().unwrap()
}
fn hex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn ui_cave(w: f64, h: f64) -> Vec<u8> {
    let (a, a2, b, b2) = (f32b(2.0 / 3840.0), f32b(2.0 / w), f32b(-2.0 / 2160.0), f32b(-2.0 / h));
    let mut c = hex("488d9424c000000031c9");
    let lp = c.len();
    c.extend(hex("8b048a3d"));
    c.extend(a);
    c.extend(hex("7509c7048a"));
    c.extend(a2);
    c.extend(hex("eb0e3d"));
    c.extend(b);
    c.extend(hex("7507c7048a"));
    c.extend(b2);
    c.extend(hex("ffc183f978"));
    c.extend([0x72, (lp as i64 - (c.len() as i64 + 2)) as u8]);
    c.extend(hex("4584ff0f85"));
    c.extend(rel(BACK_COPY, CAVE + c.len() + 4));
    c.push(0xe9);
    c.extend(rel(BACK_CMP, CAVE + c.len() + 4));
    assert!(c.len() <= CAVE2 - CAVE);
    c
}

fn fov_cave(scale: f64, orig: &[u8]) -> Vec<u8> {
    let c2 = CAVE2;
    let mut c = hex("c5c2593d");
    c.extend(rel(c2 + 21, c2 + 8));
    let target = (FOV_HOOK as i64 + 8 + i32::from_le_bytes(orig[4..8].try_into().unwrap()) as i64) as usize;
    c.extend(hex("c5f82e3d"));
    c.extend(rel(target, c2 + 16));
    c.push(0xe9);
    c.extend(rel(FOV_BACK, c2 + 21));
    c.extend(f32b(scale));
    c
}



/// Resolution and aspect: preset table, pool assert, UI composite projection, default viewport, in-game screen
/// rectangle, camera aspect (3D and cutscenes), FOV aspect factor.
fn resolution((rx, ry): (u32, u32), read: Read) -> Option<Vec<Patch>> {
    let (w, h) = (rx as f64, ry as f64);
    let mut p: Vec<Patch> = Vec::new();
    for i in 0..5usize {
        let at = PRESET_TABLE + 16 * i;
        let cur = read(at, 16)?;
        let d = |k: usize| u32::from_le_bytes(cur[k..k + 4].try_into().unwrap());
        if i == 4 || d(0) as u64 * d(4) as u64 >= rx as u64 * ry as u64 {
            let mut new = cur.clone();
            new[..4].copy_from_slice(&rx.to_le_bytes());
            new[4..8].copy_from_slice(&ry.to_le_bytes());
            p.push((at, cur, new));
        }
    }
    p.push((ASSERT, hex("7601"), hex("eb01")));
    let cave = ui_cave(w, h);
    p.push((CAVE, vec![0xcc; cave.len()], cave));
    let mut j = vec![0xe9];
    j.extend(rel(CAVE, HOOK + 5));
    p.push((HOOK, hex("4584ff751d"), j));
    p.push((FALLBACK, [3840f32.to_le_bytes(), 2160f32.to_le_bytes()].concat(), [(w as f32).to_le_bytes(), (h as f32).to_le_bytes()].concat()));
    p.push((SCREEN_RECT, rel(0xA9438EC, SCREEN_RECT_END).to_vec(), rel(0xA9438CC, SCREEN_RECT_END).to_vec()));
    let tbl: Vec<f32> = read(ASPECT_TABLE, 32)?.chunks(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    let aspect = (w / h) as f32;
    let mut tbl2 = tbl.clone();
    if !tbl.iter().any(|t| (t - aspect).abs() < 0.005) {
        p.push((ASPECT_TABLE + 0x14, 1.25f32.to_le_bytes().to_vec(), aspect.to_le_bytes().to_vec()));
        tbl2[5] = aspect;
    }
    let i = (0..8).min_by(|&a, &b| (tbl2[a] - aspect).abs().total_cmp(&(tbl2[b] - aspect).abs())).unwrap();
    let entry = ASPECT_TABLE + 4 * i;
    let mut stub = hex("c5fa1005");
    stub.extend(rel(entry, STUB + 8));
    stub.push(0xc3);
    p.push((STUB, vec![0xcc; 9], stub));
    for c in CAMERA_ASPECT_CALLS {
        p.push((c + 1, rel(BASE_ASPECT_GETTER, c + 5).to_vec(), rel(STUB, c + 5).to_vec()));
    }
    p.push((FOV_ASPECT_DISP, rel(0x615A1C0, FOV_ASPECT_DISP + 4).to_vec(), rel(entry, FOV_ASPECT_DISP + 4).to_vec()));
    Some(p)
}

/// Camera field of view multiplier.
fn fov(scale: f64, read: Read) -> Option<Vec<Patch>> {
    let orig = read(FOV_HOOK, 8)?;
    let mut j = vec![0xe9];
    j.extend(rel(CAVE2, FOV_HOOK + 5));
    j.extend([0x90; 3]);
    let cave = fov_cave(scale, &orig);
    Some(vec![(CAVE2, vec![0xcc; cave.len()], cave), (FOV_HOOK, orig, j)])
}

/// The edits the options ask for; None when an expected byte differs or `read` cannot read there yet.
pub fn patch_list(o: &Options, read: Read) -> Option<Vec<Patch>> {
    let mut p = Vec::new();
    if let Some(res) = o.res {
        p.extend(resolution(res, read)?);
    }
    if let Some(scale) = o.fov {
        p.extend(fov(scale, read)?);
    }
    for (at, old, _) in &p {
        if read(*at, old.len())? != *old {
            return None;
        }
    }
    Some(p)
}

/// True when the options ask for any patch at all.
pub fn wanted(o: &Options) -> bool {
    o.res.is_some() || o.fov.is_some()
}

/// Builds the patched copy of a game exe file.
fn patched(orig: &[u8], o: &Options) -> Option<Vec<u8>> {
    let read = |rva: usize, n: usize| -> Option<Vec<u8>> {
        let f = rva_to_off(orig, rva)?;
        orig.get(f..f + n).map(|s| s.to_vec())
    };
    let mut out = orig.to_vec();
    for (at, _, new) in patch_list(o, &read)? {
        let f = rva_to_off(orig, at)?;
        out[f..f + new.len()].copy_from_slice(&new);
    }
    Some(out)
}

/// RVA -> file offset through the PE section table.
fn rva_to_off(d: &[u8], rva: usize) -> Option<usize> {
    let pe = u32::from_le_bytes(d.get(0x3c..0x40)?.try_into().ok()?) as usize;
    let n = u16::from_le_bytes(d.get(pe + 6..pe + 8)?.try_into().ok()?) as usize;
    let opt = u16::from_le_bytes(d.get(pe + 20..pe + 22)?.try_into().ok()?) as usize;
    for i in 0..n {
        let s = pe + 24 + opt + 40 * i;
        let g = |o: usize| u32::from_le_bytes(d[s + o..s + o + 4].try_into().unwrap()) as usize;
        let (vs, va, raw, ptr) = (g(8), g(12), g(16), g(20));
        if rva >= va && rva < va + vs.max(raw) && rva - va < raw {
            return Some(ptr + rva - va);
        }
    }
    None
}

/// game_dir from victory.toml (VICTORY_CONFIG, the working directory, or next to this dll).
fn game_exe() -> Option<PathBuf> {
    let cands = [std::env::var_os("VICTORY_CONFIG").map(PathBuf::from), Some(PathBuf::from("victory.toml")), Some(dll_dir().join("victory.toml"))];
    for c in cands.into_iter().flatten() {
        if let Ok(t) = std::fs::read_to_string(&c) {
            for l in t.lines() {
                if let Some((k, v)) = l.split_once('=') && k.trim() == "game_dir" {
                    let v = v.trim().trim_matches(|c| c == '\'' || c == '"');
                    return Some(PathBuf::from(v).join("game_scarlett_master.exe"));
                }
            }
        }
    }
    None
}

/// true = the exe on disk is handled; false = it cannot be written (read-only game folder): patch the memory instead.
pub fn patch_exe(o: &Options) -> bool {
    let Some(exe) = game_exe() else { eprintln!("uw: no game_dir in victory.toml; patching memory"); return false };
    let bak = exe.with_extension("exe.bak-uw");
    let Ok(cur) = std::fs::read(&exe) else { eprintln!("uw: cannot read {}; patching memory", exe.display()); return false };
    let orig = if bak.exists() { std::fs::read(&bak).unwrap_or(cur.clone()) } else { cur.clone() };
    let Some(new) = patched(&orig, o) else { eprintln!("uw: exe bytes unexpected (not GTA VI 1.0.2.1, or already patched without a backup); trying memory"); return false };
    if !bak.exists() && std::fs::write(&bak, &orig).is_err() { eprintln!("uw: game folder is read-only; patching memory"); return false }
    if new != cur && std::fs::write(&exe, &new).is_err() { eprintln!("uw: cannot write {}; patching memory", exe.display()); return false }
    true
}

// ---- read-only game folder: patch the mapped image in memory as soon as VIctory has mapped it ----
#[repr(C)] struct Mbi { base: usize, alloc: usize, ap: u32, _p1: u32, size: usize, state: u32, protect: u32, ty: u32, _p2: u32 }
unsafe extern "system" { fn VirtualQuery(a: *const c_void, i: *mut Mbi, n: usize) -> usize; }
fn readable(addr: usize, n: usize) -> bool {
    let mut m = Mbi { base: 0, alloc: 0, ap: 0, _p1: 0, size: 0, state: 0, protect: 0, ty: 0, _p2: 0 };
    unsafe { VirtualQuery(addr as *const c_void, &mut m, size_of::<Mbi>()) != 0 && m.state == 0x1000 && m.protect & 0x1 == 0 && m.protect & 0x100 == 0 && addr + n <= m.base + m.size }
}
pub fn memory_patcher(o: Options) {
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(180) {
        let mut addr = 0x10000usize;
        let mut m = Mbi { base: 0, alloc: 0, ap: 0, _p1: 0, size: 0, state: 0, protect: 0, ty: 0, _p2: 0 };
        while addr < 0x7fff_ffff_0000 && unsafe { VirtualQuery(addr as *const c_void, &mut m, size_of::<Mbi>()) } != 0 {
            if m.state == 0x1000 && m.base == m.alloc && m.size >= 0x1000 && unsafe { *(m.base as *const u16) } == 0x5a4d {
                let base = m.base;
                let read = |rva: usize, n: usize| -> Option<Vec<u8>> { readable(base + rva, n).then(|| unsafe { std::slice::from_raw_parts((base + rva) as *const u8, n) }.to_vec()) };
                if let Some(list) = patch_list(&o, &read) {
                    for (at, _, new) in &list {
                        let mut old = 0u32;
                        unsafe {
                            VirtualProtect((base + at) as *mut c_void, new.len(), 0x40, &mut old);
                            std::ptr::copy_nonoverlapping(new.as_ptr(), (base + at) as *mut u8, new.len());
                            VirtualProtect((base + at) as *mut c_void, new.len(), old, &mut old);
                        }
                    }
                    eprintln!("uw: patched the game in memory at {base:#x} ({} patches)", list.len());
                    return;
                }
            }
            addr = m.base + m.size.max(0x1000);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    eprintln!("uw: game image not found in memory; no ultrawide patches");
}

