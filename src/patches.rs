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

/// Pause submenus at console (4K) proportions: hooks Scaleform MovieImpl::SetViewport (rcx movie, rdx viewport,
/// rsi the game's movie wrapper when called from SETVP_CALLER). Every pause screen is the same generic movie built
/// by one init (0x13cba40 returning to GENERIC_INIT_RET), and which screen it shows is only known later. So: the
/// pause sidebar is the first one built and stays alive under its submenus. A generic movie built while the
/// remembered root is alive and is the sidebar (wrapper +0x70/+0x74 = SIDEBAR_ID by then) is a submenu (Settings,
/// Help, Photo Studio, Load): it is marked and each full-screen viewport for it gets a canvas of w*s x 2160 drawn
/// back at Viewport.Scale s = 2160/h. Any other generic movie (the sidebar, layers built at boot) becomes the root.
/// MENU_STATE: root wrapper, root movie, marked wrapper, marked movie.
const SETVP: usize = 0x13CBCA0;
const SETVP_CALLER: usize = 0x13CBB37;
const GENERIC_INIT_RET: usize = 0x40421B0; // at [rsp+0xb0] on SetViewport entry
const MENU_CAVE: usize = 0x5F40B00;
// shortcut: the sidebar is recognised by wrapper fields +0x70/+0x74 (stable over three runs); if an update changes
// them the submenus simply stay unscaled. A dead root is read from freed heap, which the game's pools keep mapped.
const SIDEBAR_ID: (u32, u32) = (0x10, 0x7b);
static MENU_STATE: [std::sync::atomic::AtomicUsize; 4] = [const { std::sync::atomic::AtomicUsize::new(0) }; 4];

fn menu_cave(w: u32, h: u32, state: usize) -> Vec<u8> {
    let s = 2160.0 / h as f64;
    let (cw, ch) = ((w as f64 * s).round() as u32, 2160u32);
    let mut c = Vec::new();
    let mut fix: Vec<(usize, &str)> = vec![]; // rel32 slots to patch once the labels are known
    let mut jcc = |c: &mut Vec<u8>, op: &str, to: &'static str| { c.extend(hex(op)); c.extend([0; 4]); fix.push((c.len() - 4, to)); };
    c.extend(hex("4c8d15")); c.extend(rel(SETVP_CALLER, MENU_CAVE + 7)); // lea r10, [caller]
    c.extend(hex("4c391424")); jcc(&mut c, "0f85", "done"); // cmp [rsp], r10
    c.extend(hex("49ba")); c.extend((state as u64).to_le_bytes()); // mov r10, MENU_STATE
    c.extend(hex("4c8d1d")); c.extend(rel(GENERIC_INIT_RET, MENU_CAVE + c.len() + 4)); // lea r11, [init ret]
    c.extend(hex("4c399c24b0000000")); jcc(&mut c, "0f85", "check"); // cmp [rsp+0xb0], r11
    c.extend(hex("4d8b1a")); // mov r11, [r10]: root wrapper
    c.extend(hex("4d85db")); jcc(&mut c, "0f84", "root"); // test r11, r11
    c.extend(hex("41817b70")); c.extend(SIDEBAR_ID.0.to_le_bytes()); jcc(&mut c, "0f85", "root");
    c.extend(hex("41817b74")); c.extend(SIDEBAR_ID.1.to_le_bytes()); jcc(&mut c, "0f85", "root");
    c.extend(hex("4d8b9bc0000000")); // mov r11, [r11+0xc0]: its movie
    c.extend(hex("4d3b5a08")); jcc(&mut c, "0f85", "root"); // cmp r11, [r10+8]
    c.extend(hex("49897210")); c.extend(hex("49894a18")); jcc(&mut c, "e9", "size"); // mark (rsi, rcx)
    let l_root = c.len();
    c.extend(hex("498932")); c.extend(hex("49894a08")); // root = (rsi, rcx)
    c.extend(hex("49c7421000000000")); jcc(&mut c, "e9", "done"); // mov qword [r10+0x10], 0
    let l_check = c.len();
    c.extend(hex("49397210")); jcc(&mut c, "0f85", "done"); // cmp [r10+0x10], rsi
    c.extend(hex("49394a18")); jcc(&mut c, "0f85", "done"); // cmp [r10+0x18], rcx
    let l_size = c.len();
    c.extend(hex("817a10")); c.extend(w.to_le_bytes()); jcc(&mut c, "0f85", "done");
    c.extend(hex("817a14")); c.extend(h.to_le_bytes()); jcc(&mut c, "0f85", "done");
    for (off, v) in [(0x08u8, 0u32), (0x0c, 0), (0x10, cw), (0x14, ch), (0x2c, (s as f32).to_bits()), (0x00, cw), (0x04, ch)] {
        if off == 0 { c.extend(hex("c702")); } else { c.extend([0xc7, 0x42, off]); }
        c.extend(v.to_le_bytes());
    }
    let l_done = c.len();
    for (at, to) in fix {
        let t = match to { "done" => l_done, "check" => l_check, "root" => l_root, _ => l_size };
        c[at..at + 4].copy_from_slice(&rel(t, at + 4));
    }
    c.extend(hex("56574883ec68e9")); // the replaced entry: push rsi; push rdi; sub rsp, 0x68
    c.extend(rel(SETVP + 6, MENU_CAVE + c.len() + 4));
    assert!(c.len() <= 0x500, "past the end of the .text page tail");
    c
}

/// The settings' output size (+0x204/+0x208, from settings.xml, matched against the mode list) handed to the screen
/// size setter at +0x224352b. The saved size can be stale (e.g. 3440x1440 after switching to 7680x4320: no mode
/// matches and the title falls back to 2560x1440), so write the size asked for into the settings and set it, as
/// VIctory's VICTORY_OUTPUT does (its output_code, but that one only takes the title's own modes).
fn output(rx: u32, ry: u32) -> Patch {
    const AT: usize = 0x224352b;
    let old = hex("8b0d9b0370088b860402000039c1750d8b9608020000e8ea4b2300eb208bbe080200008b2d7c03700889c189fae8d34b230039fd7407b101e8388eb4fe");
    let mut c = vec![0xb8];
    c.extend(rx.to_le_bytes());
    c.extend(hex("898604020000bf"));
    c.extend(ry.to_le_bytes());
    c.extend(hex("89be080200008b2d"));
    c.extend(rel(0xA9438D0, AT + c.len() + 4));
    c.extend(hex("89c189fae8"));
    c.extend(rel(0x2478130, AT + c.len() + 4));
    c.extend([0x39, 0xfd, 0x74, (old.len() - (c.len() + 4)) as u8, 0xb1, 0x01, 0xe8]);
    c.extend(rel(0xd8c3a0, AT + c.len() + 4));
    c.extend([0xeb, (old.len() - (c.len() + 2)) as u8]);
    assert!(c.len() <= old.len());
    c.resize(old.len(), 0xcc);
    (AT, old, c)
}

/// Pixels to size the render pools by (as a 4K-equivalent output) when the stock ones are too small, else None. The
/// title renders at 2/3 (4/9 of the pixels), so a native render (disable_fsr) needs 9/4 as much; the UI target is the
/// UI size, which hud_scale below 1 grows (fine without growing up to 6450x2700; 12900x5400 failed in the heap).
fn pool_px(o: &Options) -> Option<u64> {
    let (rx, ry) = o.res?;
    let render = rx as u64 * ry as u64 * if o.disable_fsr { 9 } else { 4 } / 4;
    let ui = hud_k(o).map_or(0, |k| (rx as f64 * k * ry as f64 * k) as u64);
    (render > 3840 * 2160 || ui > 6450 * 2700).then_some(render.max(ui))
}

/// Resolution and aspect: preset table, pool assert, UI composite projection, default viewport, in-game screen
/// rectangle, camera aspect (3D and cutscenes), FOV aspect factor.
fn resolution((rx, ry): (u32, u32), pool_px: Option<u64>, read: Read) -> Option<Vec<Patch>> {
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
    p.push(output(rx, ry));
    // The back buffers' pool (+0x247ad45: 82 MB Series S, else 64 MB) fits two 4K buffers; bigger outputs got
    // E_INVALIDARG and an int3 in its allocator (+0x56a2afd, seen at 7680x4320). Two buffers of 4 bytes a pixel + 2 MB.
    let pool = ((rx as u64 * ry as u64 * 8 + 0x200000 + 0xffff) & !0xffff) as u32;
    // Render-target heap (index 2 of the per-console heap sizes at +0x6186250, Series X row +0x50: 2199 MB) is sized for
    // 4K output at 2/3; past that grow it with the pixel count (7680x4320: a 59 MB target failed in it, int3 at
    // +0x2219650; 3440x1440 rendered natively failed there too).
    if let Some(px) = pool_px {
        let at = 0x6186250 + 0x50 + 8 + 2 * 8;
        let cur = read(at, 8)?;
        let size = u64::from_le_bytes(cur[..].try_into().unwrap());
        // Its allocator keeps the size in 32 bits (+0x554f6d3), so stay under 4 GB (8796 MB wrapped to 604 MB).
        let grown = ((size * px / (3840 * 2160) + 0xfffff) & !0xfffff).min(0xfff0_0000);
        p.push((at, cur, grown.to_le_bytes().to_vec()));
        // Even 4 GB is not enough for native 8K (VICTORY_RENDER_SCALE=1). The target allocator (+0x193e1df) can take
        // title memory directly (+0x5f3fcc0) and only falls back to that heap when the call fails; the game picks
        // that path per resource (flag 0x40, [rsi+0x45] bit 4) and frees by the heap bit it sets (+0x193e274), so
        // send every target there first.
        p.push((0x193e1e3, hex("7514"), hex("eb14")));
        p.push((0x193e202, hex("742c"), hex("9090")));
        // The second pool (in 64 KB pages) grows in memory_only: its page array has to move first.
    }
    if pool > 0x4000000 {
        let pool = pool.max(0x5200000).to_le_bytes();
        p.push((0x247ad45, hex("b800002005b900000004"), [&[0xb8], &pool[..], &[0xb9], &pool[..]].concat()));
    }
    let cave = ui_cave(w, h);
    p.push((CAVE, vec![0xcc; cave.len()], cave));
    let mut j = vec![0xe9];
    j.extend(rel(CAVE, HOOK + 5));
    p.push((HOOK, hex("4584ff751d"), j));
    p.push((FALLBACK, [3840f32.to_le_bytes(), 2160f32.to_le_bytes()].concat(), [(w as f32).to_le_bytes(), (h as f32).to_le_bytes()].concat()));
    p.push((SCREEN_RECT, rel(0xA9438EC, SCREEN_RECT_END).to_vec(), rel(0xA9438CC, SCREEN_RECT_END).to_vec()));
    // The pixel size the UI scales by (1/w, 1/h at 0xA9E8890) is also taken from the 4K size when a flag is set.
    p.push((0x1F77FC8, rel(0xA9438CC, 0x1F77FCC).to_vec(), rel(0xA9438EC, 0x1F77FCC).to_vec()));
    // Past 16:9 this writes a narrowed centered window (0x7659A3C/44) for the UI; always skip it (jbe -> jmp).
    p.push((0x32BE289, hex("762c"), hex("eb2c")));
    p.push((0x1F77FD7, rel(0xA9438D0, 0x1F77FDB).to_vec(), rel(0xA9438F0, 0x1F77FDB).to_vec()));
    p.push((0x5EB2617, rel(0xA9438F0, 0x5EB261B).to_vec(), rel(0xA9438D0, 0x5EB261B).to_vec()));
    p.push((0x5EB261E, rel(0xA9438EC, 0x5EB2622).to_vec(), rel(0xA9438CC, 0x5EB2622).to_vec()));
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

/// Patches that only exist in memory: the menu cave lives in the zero-filled tail of the last .text page,
/// which is not in the exe file, and points at a static of this dll. `hud_at`: RVA of HUD_MEM fresh bytes for
/// the HUD stubs (None = no HUD scaling).
fn memory_only(o: &Options, hud_at: Option<usize>, pages_at: Option<usize>) -> Vec<Patch> {
    let Some((rx, ry)) = o.res else { return vec![] };
    let menu = ry < 2160;
    let mut p = vec![];
    if let Some(at) = pages_at {
        // Page pool, 64 KB pages (+0x1d488b1 `mov esi, 0x2c60`: 710 MB Series X), checked at +0x2473e83 (int3 when the
        // frame's targets need more: 2.2 GB at 7680x4320). Its page addresses go in a static array of 0x2c60 qwords
        // (+0x75eaee0) that a bigger count overran (access violation at +0x5524010), so the array's four users point at
        // PAGES_MEM fresh bytes instead.
        // Sized for a native render (VICTORY_RENDER_SCALE=1: 2.25x the targets of the title's 2/3; 4.65 GB needed at
        // 7680x4320). The block +0x1d488ac..c4 (Series S/X page counts, `shl ecx, 16`, call +0x5eb2780) is rewritten
        // with the shift in 64 bits; the reserve call takes a 64-bit size.
        let px = (rx as u64 * ry as u64 * 9 / 4).max(pool_px(o).unwrap_or(0));
        let pages = (0x2c60 * px / (3840 * 2160)).min(PAGES_MEM as u64 / 8) as u32;
        const AT: usize = 0x1d488ac;
        let mut c = vec![0x90; 7];
        c.push(0xbe);
        c.extend(pages.to_le_bytes()); // mov esi, pages
        c.extend(hex("4889f148c1e110e8")); // mov rcx, rsi ; shl rcx, 16 ; call
        c.extend(rel(0x5eb2780, AT + c.len() + 4));
        p.push((AT, hex("b890120000be602c0000480f45f089f1c1e110e8bc9e1604"), c));
        for d in [0x2473fa4, 0x247407c, 0x24740ef, 0x5eb2790] {
            p.push((d, rel(0x75eaee0, d + 4).to_vec(), rel(at, d + 4).to_vec()));
        }
    }
    let mut entry = None;
    if menu {
        let cave = menu_cave(rx, ry, MENU_STATE.as_ptr() as usize);
        p.push((MENU_CAVE, vec![0; cave.len()], cave));
        entry = Some(MENU_CAVE);
    }
    if let (Some(k), Some(at)) = (hud_k(o), hud_at) {
        let (code, mut sites) = hud_stubs(rx, ry, k, at);
        p.push((at, vec![0; code.len()], code));
        p.append(&mut sites);
    }
    if let Some(e) = entry {
        let mut j = vec![0xe9];
        j.extend(rel(e, SETVP + 5));
        j.push(0x90);
        p.push((SETVP, hex("56574883ec68"), j));
    }
    p
}

// ---- HUD scale ----
// The HUD lays itself out in the UI size (UI_SIZE, int w/h), read through 3 getters and 58 direct loads. In gameplay
// every one of them returns that size times K (a 4K-proportioned UI), so every movie is laid out, rendered and placed
// (D-pad-down markers included) for a K times larger screen and drawn back over the real one: 1/K the size. Scaling
// only the HUD lays it out wrong (the UI window is computed elsewhere), and switching sizes per UI mode (pause, D-pad
// view) left the HUD laid out for the other size, so it is the whole UI, all the time. A 16:9 UI size instead (HUD
// in a centered 16:9 box) stretches: the UI target is sized from it but D-pad markers stay in screen pixels.
// Notes: notes/HUD-SCALE.md (gitignored).
const UI_SIZE: usize = 0x92D7924;
const HUD_UPDATE: usize = 0x590C90; // per-wrapper update (rcx = wrapper): viewport re-apply, Advance, display
const HUD_HASHES: [u32; 2] = [0x944FA9D9, 0x3D88F3F5]; // joaat("hud"), joaat("minimap") at [[wrapper+0xb0]]
pub const HUD_MEM: usize = 0x2000;
/// Bytes for the moved page array of the page pool (its qword per 64 KB page; 0x20000 pages = 8 GB).
pub const PAGES_MEM: usize = 0x100000;
// Every instruction that loads UI_SIZE (+0 or +4) through a rip-relative operand, with its bytes.
const HUD_SITES: [(usize, &str); 66] = [(0x323910E, "4c8d350fe80906"), (0x323911C, "4c8d2505e80906"), (0x5B6D756, "488d15cba17603"), (0x5B6D7BF, "488d0d5ea17603"),(0x1204CA, "c5fa2a0552741b09"), (0x1204E1, "c5f22a0d3f741b09"), (0x20B464, "c5f96e05bcc40c09"), (0x20B46C, "c4e3792205aec40c0901"), (0x73C7C0, "c5fa2a055cb1b908"), (0x73C7C8, "c5f22a0d58b1b908"), (0x961776, "8b05ac619708"), (0x96177C, "448b0da1619708"), (0xCAB820, "8b05fec06208"), (0xCAB828, "8b05fac06208"), (0x156B7A7, "c5fa2a0575c1d607"), (0x156B7D7, "c5f22a0d49c1d607"), (0x17C0BFA, "3b05246db107"), (0x17C0C08, "3b051a6db107"), (0x1833FE3, "c5f22a0d3d39aa07"), (0x1833FED, "c5fa2a052f39aa07"), (0x1A4B883, "c5f96e0599c08807"), (0x1A4B88F, "c4e37922058fc0880701"), (0x1EDB02D, "8b0df5c83f07"), (0x1EDB046, "8b05d8c83f07"), (0x1EDC2B4, "448b0d6db63f07"), (0x1EDC2BB, "448b0562b63f07"), (0x1FB4E38, "8b35e62a3207"), (0x1FB4E3E, "8b2de42a3207"), (0x23402DD, "c5f96e053f76f906"), (0x23402F3, "c4e37922352b76f90601"), (0x25D3945, "8b05dd3fd006"), (0x25D394B, "448b0dd23fd006"), (0x25EFAD0, "8b054e7ece06"), (0x25EFAE0, "8b05427ece06"), (0x2627EEF, "8b052ffaca06"), (0x2627EF5, "8b0d2dfaca06"), (0x27F6BC7, "8b35570dae06"), (0x27F6BCD, "8b3d550dae06"), (0x28C0794, "c5f22a058871a106"), (0x28C07B1, "c5f22a0d6f71a106"), (0x28EDCBB, "c5f96e05619c9e06"), (0x28EDD00, "c4e37922051e9c9e0601"), (0x28FDE4A, "8b1dd89a9d06"), (0x28FDE50, "8b2dce9a9d06"), (0x32BCF46, "c5fa2a05d6a90106"), (0x32BCF56, "c5f22a0dcaa90106"), (0x32BE26D, "c5f22a05af960106"), (0x32BE275, "c5f22a0dab960106"), (0x3A00051, "c5f96e05cb788d05"), (0x3A00086, "c4e379220598788d0501"), (0x3C2DD60, "c5fa2a05bc9b6a05"), (0x3C2DD68, "c5f22a0db89b6a05"), (0x4F21F00, "8b05225a3b04"), (0x4F21F10, "8b050e5a3b04"), (0x5B67ADC, "448b0d45fe7603"), (0x5B67AE3, "448b053afe7603"), (0x5B67BBA, "448b0d67fd7603"), (0x5B67BC1, "448b055cfd7603"), (0x5B6D9CA, "c5fa2a35529f7603"), (0x5B6D9D2, "c5fa2a3d4e9f7603"), (0x5BC2A2C, "8b1df24e7103"), (0x5BC2A32, "8b2df04e7103"), (0x5BFA640, "8b05ded26d03"), (0x5BFA648, "448b05d9d26d03"), (0x5EFBD20, "8b05febb3d03"), (0x5EFBD28, "8b05fabb3d03")];

/// K = UI size multiplier (UI drawn 1/K the size); None = leave the UI alone.
fn hud_k(o: &Options) -> Option<f64> {
    let (rx, ry) = o.res?;
    // 4K proportions (the UI laid out for a 2160-tall screen), then hud_scale on top: 0.8 = 20% smaller. The UI target
    // can't be wider or taller than 16384 (the largest 2D texture), so very wide screens get a slightly larger UI.
    let k = ((2160.0 / ry as f64).max(1.0) / o.hud_scale.unwrap_or(1.0)).min(16384.0 / rx.max(ry) as f64);
    ((k - 1.0).abs() > 1e-3).then_some(k)
}

/// Tiny assembler: bytes at RVA `at`, rel32 to RVAs or to labels resolved by `done`.
struct Asm { at: usize, c: Vec<u8>, labels: Vec<(&'static str, usize)>, fix: Vec<(usize, &'static str)> }
impl Asm {
    fn b(&mut self, h: &str) -> &mut Self { self.c.extend(hex(h)); self }
    fn d(&mut self, v: u32) -> &mut Self { self.c.extend(v.to_le_bytes()); self }
    fn to(&mut self, rva: usize) -> &mut Self { let n = self.at + self.c.len() + 4; self.c.extend(rel(rva, n)); self }
    fn j(&mut self, op: &str, l: &'static str) -> &mut Self { self.b(op); self.fix.push((self.c.len(), l)); self.c.extend([0; 4]); self }
    fn l(&mut self, l: &'static str) -> &mut Self { self.labels.push((l, self.c.len())); self }
    /// disp32 to `rva` for an instruction that has one imm8 after the displacement
    fn to_imm(&mut self, rva: usize) -> &mut Self { let n = self.at + self.c.len() + 5; self.c.extend(rel(rva, n)); self }
    /// cmp byte [rip+rva], 0
    fn cmp0(&mut self, rva: usize) -> &mut Self { self.b("803d"); let n = self.at + self.c.len() + 5; self.c.extend(rel(rva, n)); self.b("00") }
    fn rva(&self, l: &str) -> usize { self.at + self.labels.iter().find(|x| x.0 == l).unwrap().1 }
    fn resolve(&mut self) {
        for (p, l) in std::mem::take(&mut self.fix) {
            let t = self.labels.iter().find(|x| x.0 == l).unwrap_or_else(|| panic!("label {l}")).1;
            self.c[p..p + 4].copy_from_slice(&rel(t, p + 4));
        }
    }
    /// r10d = [[reg+0xb0]] when that is a plausible pointer; jump to `yes` for the hud/minimap hashes, else `no`.
    fn hud_hash(&mut self, rcx: bool, yes: &'static str, no: &'static str) -> &mut Self {
        self.b(if rcx { "4c8b91b0000000" } else { "4c8b96b0000000" }) // mov r10, [rcx/rsi+0xb0]
            .b("4981fa00000100").j("0f82", no) // cmp r10, 0x10000; jb
            .b("49bbffffffffff7f0000").b("4d39da").j("0f87", no) // cmp r10, 0x7fffffffffff; ja
            .b("41f6c207").j("0f85", no) // test r10b, 7; jnz
            .b("458b12"); // mov r10d, [r10]
        for h in HUD_HASHES { self.b("4181fa").d(h).j("0f84", yes); }
        self.j("e9", no)
    }
}

/// The HUD stub page at RVA `at`: (its bytes, the jumps into it). Page layout: 8 thread ids (threads inside a
/// hud/minimap update), the scaled UI size (+0x20), the switch byte (+0x28, 0 unless changed live with
/// notes/tools/vp.py), then code. Switch 0: every read of the UI size returns the scaled size; the game then sizes
/// every movie's render target and viewport for it and draws it back over the screen. 1: everything native.
/// 2: like 0, but only threads inside a hud/minimap update.
fn hud_stubs(w: u32, h: u32, k: f64, at: usize) -> (Vec<u8>, Vec<Patch>) {
    let (tids, gs, off) = (at, at + 0x20, at + 0x28);
    let r = |v: f64| v.round() as u32;
    let mut a = Asm { at, c: vec![0; 0x20], labels: vec![], fix: vec![] };
    a.d(r(w as f64 * k)).d(r(h as f64 * k)).c.resize(0x80, 0);
    // chk: ZF = the current thread is in a hud/minimap update. Keeps every register but the flags.
    a.l("chk").b("50").b("658b042548000000"); // push rax; mov eax, gs:[0x48] (thread id)
    for i in 0..8 { a.b("3b05").to(tids + 4 * i).j("0f84", "chk_y"); }
    a.b("58c3").l("chk_y").b("58c3");
    // HUD_UPDATE entry: hud/minimap wrappers run the original with this thread's id in a free slot.
    let upd = "5657534883ec30";
    a.l("upd").hud_hash(true, "upd_hud", "upd_orig")
        .l("upd_orig").b(upd).b("e9").to(HUD_UPDATE + upd.len() / 2)
        .l("upd_hud").b("534883ec20").b("65448b0c2548000000") // push rbx; sub rsp, 0x20; mov r9d, gs:[0x48]
        .b("488d1d").to(tids).b("4531db") // lea rbx, [tids]; xor r11d, r11d
        .l("upd_s1").b("31c0").b("f0460fb10c9b").j("0f84", "upd_s2") // xor eax, eax; lock cmpxchg [rbx+r11*4], r9d
        .b("41ffc3").b("4183fb08").j("0f82", "upd_s1").b("31db").j("e9", "upd_s3") // no free slot: rbx = 0
        .l("upd_s2").b("4a8d1c9b") // lea rbx, [rbx+r11*4]
        .l("upd_s3").j("e8", "upd_orig").b("4885db").j("0f84", "upd_s4").b("c70300000000") // free the slot
        .l("upd_s4").b("4883c4205bc3");
    let mut p: Vec<Patch> = vec![];
    // Each load: (flags kept) the scaled copy while the switch is 0 (or 2 on a hud/minimap update thread), else the real one.
    for (site, bytes) in HUD_SITES {
        let ins = hex(bytes);
        let n = ins.len();
        let d = if n == 10 { 5 } else { n - 4 }; // vpinsrd carries an imm8 after the displacement
        let target = (site + n).wrapping_add_signed(i32::from_le_bytes(ins[d..d + 4].try_into().unwrap()) as isize);
        assert!(target == UI_SIZE || target == UI_SIZE + 4, "{site:#x}");
        let stub = at + a.c.len();
        // per-site labels: the three forward jumps are patched by hand below
        a.b("9c").cmp0(off).b("0f84").d(0); // pushfq; cmp byte [off], 0; je scaled
        let j0 = a.c.len();
        a.b("803d").to_imm(off).b("02").b("0f85").d(0); // cmp byte [off], 2; jne native
        let j1 = a.c.len();
        let chk = a.rva("chk");
        a.b("e8").to(chk).b("0f85").d(0); // call chk; jne native
        let j2 = a.c.len();
        for (i, src) in [gs + target - UI_SIZE, target].into_iter().enumerate() {
            let here = a.c.len();
            let set = |a: &mut Asm, j: usize| a.c[j - 4..j].copy_from_slice(&((here - j) as u32).to_le_bytes());
            if i == 0 { set(&mut a, j0); } else { set(&mut a, j1); set(&mut a, j2); }
            a.b("9d"); // popfq
            let s = a.c.len();
            a.c.extend(&ins);
            let next = at + a.c.len();
            a.c[s + d..s + d + 4].copy_from_slice(&rel(src, next));
            a.b("e9").to(site + n);
        }
        let mut j = vec![0xe9];
        j.extend(rel(stub, site + 5));
        j.resize(n, 0x90);
        p.push((site, ins, j));
    }
    a.resolve();
    assert!(a.c.len() <= HUD_MEM);
    let mut j = vec![0xe9];
    j.extend(rel(a.rva("upd"), HUD_UPDATE + 5));
    j.resize(upd.len() / 2, 0x90);
    p.push((HUD_UPDATE, hex(upd), j));
    (a.c.clone(), p)
}

/// Fresh executable memory within rel32 reach above the game image.
fn alloc_near(base: usize, read: Read, bytes: usize) -> Option<usize> {
    let pe = u32::from_le_bytes(read(0x3c, 4)?.try_into().ok()?) as usize;
    let size = u32::from_le_bytes(read(pe + 0x50, 4)?.try_into().ok()?) as usize;
    let start = (base + size + 0xffff) & !0xffff;
    (0..0x4000).map(|i| start + i * 0x10000).find_map(|a| {
        let p = unsafe { VirtualAlloc(a as *mut c_void, bytes, 0x3000, 0x40) }; // commit|reserve, rwx
        (!p.is_null()).then_some(p as usize)
    })
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

/// The title's own upscaler renders at 2/3 of the output size (+0x28f1e70 `mov dword [rcx+90h], 2/3`); 1.0 renders
/// natively. Already 1.0 (a fork's VICTORY_RENDER_SCALE=1): nothing to do. Other bytes: not this game, None.
fn full_render_scale(read: Read) -> Option<Vec<Patch>> {
    const AT: usize = 0x28f1e78;
    let (two_thirds, one) = (hex("c78190000000abaa2a3f"), hex("c781900000000000803f"));
    let cur = read(AT, 10)?;
    if cur == one { Some(vec![]) } else { (cur == two_thirds).then(|| vec![(AT, two_thirds, one)]) }
}

/// The edits the options ask for; None when an expected byte differs or `read` cannot read there yet.
pub fn patch_list(o: &Options, read: Read) -> Option<Vec<Patch>> {
    let mut p = Vec::new();
    if let Some(res) = o.res {
        p.extend(resolution(res, pool_px(o), read)?);
    }
    if let Some(scale) = o.fov {
        p.extend(fov(scale, read)?);
    }
    if o.disable_fsr {
        p.extend(full_render_scale(read)?);
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
    o.res.is_some() || o.fov.is_some() || o.disable_fsr
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

/// Older versions could patch the game exe on disk (patch_mode = exe, with a .bak-uw copy of the original). The game
/// is only patched in memory now, so an exe left patched is put back from that copy (which a patched exe would
/// otherwise make the memory patches refuse).
pub fn restore_exe() {
    let Some(exe) = game_exe() else { return };
    let bak = exe.with_extension("exe.bak-uw");
    let Ok(orig) = std::fs::read(&bak) else { return };
    // Nothing to do when the exe is already the original (or the copy doesn't look like an exe).
    if orig.len() < 0x100000 || !orig.starts_with(b"MZ") || std::fs::read(&exe).is_ok_and(|e| e == orig) { return }
    match std::fs::rename(&bak, &exe) {
        Ok(()) => eprintln!("uw: restored the original {} from its .bak-uw", exe.display()),
        Err(e) => eprintln!("uw: cannot restore {} from its .bak-uw ({e}); restore it by hand", exe.display()),
    }
}

// ---- patch the mapped image in memory as soon as VIctory has mapped it ----
#[repr(C)] struct Mbi { base: usize, alloc: usize, ap: u32, _p1: u32, size: usize, state: u32, protect: u32, ty: u32, _p2: u32 }
unsafe extern "system" { fn VirtualQuery(a: *const c_void, i: *mut Mbi, n: usize) -> usize; }
fn readable(addr: usize, n: usize) -> bool {
    let mut m = Mbi { base: 0, alloc: 0, ap: 0, _p1: 0, size: 0, state: 0, protect: 0, ty: 0, _p2: 0 };
    unsafe { VirtualQuery(addr as *const c_void, &mut m, size_of::<Mbi>()) != 0 && m.state == 0x1000 && m.protect & 0x1 == 0 && m.protect & 0x100 == 0 && addr + n <= m.base + m.size }
}
pub fn memory_patcher(o: Options) {
    let start = std::time::Instant::now();
    let (mut hud_mem, mut hud_tried): (Option<(usize, usize)>, bool) = (None, false); // (image base, stub page)
    let (mut pages_mem, mut pages_tried): (Option<(usize, usize)>, bool) = (None, false); // (image base, page array)
    while start.elapsed() < std::time::Duration::from_secs(180) {
        let mut addr = 0x10000usize;
        let mut m = Mbi { base: 0, alloc: 0, ap: 0, _p1: 0, size: 0, state: 0, protect: 0, ty: 0, _p2: 0 };
        while addr < 0x7fff_ffff_0000 && unsafe { VirtualQuery(addr as *const c_void, &mut m, size_of::<Mbi>()) } != 0 {
            if m.state == 0x1000 && m.base == m.alloc && m.size >= 0x1000 && unsafe { *(m.base as *const u16) } == 0x5a4d {
                let base = m.base;
                let read = |rva: usize, n: usize| -> Option<Vec<u8>> { readable(base + rva, n).then(|| unsafe { std::slice::from_raw_parts((base + rva) as *const u8, n) }.to_vec()) };
                if !hud_tried && hud_k(&o).is_some() && read(SETVP, 6) == Some(hex("56574883ec68")) {
                    hud_tried = true;
                    hud_mem = alloc_near(base, &read, HUD_MEM).map(|m| (base, m));
                    if hud_mem.is_none() { eprintln!("uw: no memory near the game for the HUD scale"); }
                }
                let hud_at = hud_mem.filter(|h| h.0 == base).map(|h| h.1 - base);
                if !pages_tried && pool_px(&o).is_some() && read(0x1d488b1, 5) == Some(hex("be602c0000")) {
                    pages_tried = true;
                    pages_mem = alloc_near(base, &read, PAGES_MEM).map(|m| (base, m));
                    if pages_mem.is_none() { eprintln!("uw: no memory near the game for the page pool"); }
                }
                let pages_at = pages_mem.filter(|h| h.0 == base).map(|h| h.1 - base);
                let extra = memory_only(&o, hud_at, pages_at);
                let extra_ok = !extra.is_empty() && extra.iter().all(|(at, old, _)| read(*at, old.len()).as_ref() == Some(old));
                let list = patch_list(&o, &read).map(|mut l| { if extra_ok { l.extend(extra) } l });
                if let Some(list) = list {
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


#[cfg(test)]
mod tests {
    #[test]
    fn pools_grow_only_past_what_was_verified() {
        let o = |res, hud_scale, disable_fsr| crate::options::Options { res: Some(res), hud_scale, disable_fsr, ..Default::default() };
        assert_eq!(super::pool_px(&o((3440, 1440), None, false)), None, "default 4K-proportion UI");
        assert_eq!(super::pool_px(&o((3440, 1440), Some(0.8), false)), None, "6450x2700 UI ran on the stock pools");
        assert!(super::pool_px(&o((3840, 1080), Some(0.8), false)).is_some(), "9600x2700 UI on 32:9");
        assert!(super::pool_px(&o((3440, 1440), None, true)).is_some(), "native render");
        assert!(super::pool_px(&o((7680, 4320), None, false)).is_some());
        assert_eq!(super::hud_k(&o((7680, 1080), Some(0.8), false)), Some(16384.0 / 7680.0), "UI target capped at 16384 wide");
    }

    #[test]
    fn full_render_scale_only_on_the_title_bytes() {
        let at = |b: &'static str| move |_: usize, _: usize| Some(super::hex(b));
        let p = super::full_render_scale(&at("c78190000000abaa2a3f")).unwrap();
        assert_eq!(p[0].2[6..], 1.0f32.to_le_bytes());
        assert!(super::full_render_scale(&at("c781900000000000803f")).unwrap().is_empty(), "a fork already did it");
        assert!(super::full_render_scale(&at("90909090909090909090")).is_none(), "another image");
    }

    #[test]
    fn menu_cave_dump() {
        let c = super::menu_cave(3440, 1440, 0x1122334455667788);
        std::fs::write(std::env::temp_dir().join("menu_cave.bin"), &c).unwrap();
        assert_eq!(&c[c.len() - 11..c.len() - 5], &super::hex("56574883ec68")[..]);
    }

    #[test]
    fn hud_stubs_dump() {
        let at = 0xB100000;
        let (c, p) = super::hud_stubs(3440, 1440, 1.5, at);
        std::fs::write(std::env::temp_dir().join("hud_stubs.bin"), &c).unwrap();
        assert_eq!(p.len(), 67);
        assert_eq!(&c[0x20..0x28], &[0x28, 0x14, 0, 0, 0x70, 0x08, 0, 0]); // 5160 x 2160
        assert!(p.iter().all(|(a, old, new)| old.len() == new.len() && new[0] == 0xe9 && (*a < at)));
    }
}

#[cfg(test)]
mod output_tests {
    #[test]
    fn output_dump() {
        let (at, old, new) = super::output(7680, 4320);
        assert_eq!((at, old.len(), new.len()), (0x224352b, 61, 61));
        std::fs::write("/tmp/out_patch.bin", &new).unwrap();
    }
}
