//! uw-options.ini: created once next to the dll, never overwritten. Empty or missing entries mean "off".
use crate::sys::dll_dir;

const INI_NAME: &str = "uw-options.ini";
const HEADER: &str = "// GTA VI ultrawide options for VIctory. Edit, save, then start the game again.
// Anything left empty (or missing) is ignored: its effect is simply not applied.
// This file is created once; updates only add options that are missing, they never change your values.
";

/// (first key, text): one block per option, so an older file can be topped up with the ones it lacks.
const BLOCKS: [(&str, &str); 6] = [
    ("auto_res", "
// Automatic resolution: true = while res_x and res_y below are empty, use the monitor's own resolution (the HUD
// and menus scale to it). false or empty = off (the game keeps its own resolution).
auto_res = true
"),
    ("res_x", "
// Output resolution in pixels (640-7680 x 360-4320). Set both, or leave both empty (auto_res above decides then).
res_x =
res_y =
"),
    ("borderless_fullscreen", "
// A borderless window on the monitor. true = fit the picture to the monitor keeping its shape (black bars when
// the shapes differ), stretch = fill the monitor even if that distorts, false or empty = off.
borderless_fullscreen =
"),
    ("fov_scale", "
// Field of view multiplier: 1.2 = 20% wider, 0.8 = 20% narrower (0.5 - 2). Empty = unchanged.
fov_scale =
"),
    ("hud_scale", "
// HUD and menu size. Empty or 1 = 4K proportions (the UI laid out as if the screen were 2160 tall, which is what
// it is designed for), 0.8 = 20% smaller, 2 = twice as large (0.8 - 2). Needs a resolution (auto_res or res_x/res_y).
hud_scale =
"),
    ("disable_fsr", "
// Render at full resolution: true = turn off the game's built-in upscaling (it renders at 2/3 of the resolution and
// upscales), sharper but heavier on the GPU. false or empty = off (the game's default).
disable_fsr = false
"),
];

fn has_key(text: &str, key: &str) -> bool {
    text.lines().any(|l| l.split(['/', ';', '#']).next().and_then(|l| l.split_once('=')).is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case(key)))
}

/// The blocks of `BLOCKS` that `existing` has no line for (all of them, with the header, for a new file).
fn missing(existing: Option<&str>) -> String {
    match existing {
        None => HEADER.to_string() + &BLOCKS.iter().map(|b| b.1).collect::<String>(),
        Some(t) => BLOCKS.iter().filter(|b| !has_key(t, b.0)).map(|b| b.1).collect(),
    }
}

/// `existing` with every missing block put where it belongs: above the comment of the next option it already has
/// (in `BLOCKS` order), or at the end.
fn top_up(existing: &str) -> String {
    let mut lines: Vec<String> = existing.lines().map(String::from).collect();
    for (i, (key, block)) in BLOCKS.iter().enumerate().rev() {
        let text = lines.join("\n");
        if has_key(&text, key) { continue; }
        let at = BLOCKS[i + 1..].iter().find_map(|(next, _)| {
            let mut at = lines.iter().position(|l| has_key(l, next))?;
            while at > 0 && lines[at - 1].trim_start().starts_with("//") { at -= 1; }
            Some(at)
        });
        let new: Vec<String> = block.trim_start_matches('\n').lines().map(String::from).collect();
        match at {
            Some(at) => { lines.splice(at..at, new.into_iter().chain([String::new()])); }
            None => { lines.push(String::new()); lines.extend(new); }
        }
    }
    lines.join("\n") + "\n"
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Borderless {
    #[default]
    Off,
    Fit,
    Stretch,
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub res: Option<(u32, u32)>,
    pub auto_res: bool,
    pub fov: Option<f64>,
    pub hud_scale: Option<f64>,
    pub disable_fsr: bool,
    pub borderless: Borderless,
}

/// `key = value` lines; `//`, `;` and `#` start a comment. Unknown keys are ignored.
fn value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let l = l.split("//").next()?.split([';', '#']).next()?;
        let (k, v) = l.split_once('=')?;
        let v = v.trim();
        (k.trim().eq_ignore_ascii_case(key) && !v.is_empty()).then(|| v.to_string())
    })
}

fn number(text: &str, key: &str, lo: f64, hi: f64) -> Option<f64> {
    let raw = value(text, key)?;
    match raw.parse::<f64>() {
        Ok(v) if (lo..=hi).contains(&v) => Some(v),
        _ => {
            eprintln!("uw: {key} = {raw} is not a number from {lo} to {hi}; ignored");
            None
        }
    }
}

pub fn options() -> Options {
    let path = dll_dir().join(INI_NAME);
    let mut text = std::fs::read_to_string(&path).ok();
    if !missing(text.as_deref()).is_empty() {
        let out = match &text { None => missing(None), Some(t) => top_up(t) };
        match std::fs::write(&path, &out) {
            Ok(()) => println!("uw: {} {}", if text.is_none() { "created" } else { "added missing options to" }, path.display()),
            Err(e) => eprintln!("uw: cannot write {}: {e}", path.display()),
        }
        text = Some(out);
    }
    let mut o = parse(&text.unwrap_or_default());
    if o.res.is_none() && o.auto_res {
        o.res = screen_size();
        match o.res {
            Some((w, h)) => println!("uw: auto_res: using the monitor's {w}x{h}"),
            None => eprintln!("uw: auto_res: monitor size unknown or out of range; resolution left alone"),
        }
    }
    o
}

/// The primary monitor's size, if it is inside the range res_x/res_y accept.
fn screen_size() -> Option<(u32, u32)> {
    let (w, h) = unsafe { (crate::sys::GetSystemMetrics(0), crate::sys::GetSystemMetrics(1)) }; // SM_CX/CYSCREEN
    ((640..=7680).contains(&w) && (360..=4320).contains(&h)).then_some((w as u32, h as u32))
}

fn parse(text: &str) -> Options {
    let res = match (number(text, "res_x", 640.0, 7680.0), number(text, "res_y", 360.0, 4320.0)) {
        (Some(x), Some(y)) => Some((x as u32, y as u32)),
        (None, None) => None,
        _ => {
            eprintln!("uw: res_x and res_y must both be set; resolution ignored");
            None
        }
    };
    let borderless = match value(text, "borderless_fullscreen").map(|v| v.to_ascii_lowercase()).as_deref() {
        Some("true" | "1" | "yes" | "on" | "fit") => Borderless::Fit,
        Some("stretch" | "fill") => Borderless::Stretch,
        _ => Borderless::Off,
    };
    let fov = number(text, "fov_scale", 0.5, 2.0).filter(|f| (f - 1.0).abs() > 1e-6);
    // below 0.8 menu panels shrink but their text doesn't (pause labels get cut off)
    let hud_scale = number(text, "hud_scale", 0.8, 2.0);
    let on = |k: &str| value(text, k).is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on"));
    let (auto_res, disable_fsr) = (on("auto_res"), on("disable_fsr"));
    Options { res, auto_res, fov, hud_scale, disable_fsr, borderless }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_text_only_turns_on_auto_res() {
        let o = parse(&missing(None));
        assert!(o.auto_res && o.res.is_none() && o.fov.is_none() && o.borderless == Borderless::Off && !o.disable_fsr);
        assert!(parse("disable_fsr = true\n").disable_fsr && !parse("disable_fsr =\n").disable_fsr);
        assert!(!parse("auto_res =\n").auto_res && !parse("auto_res = false\n").auto_res);
    }

    #[test]
    fn an_old_file_gets_only_the_missing_options() {
        let old = "res_x = 3440\nres_y = 1440\nborderless_fullscreen = true\nfov_scale =\n";
        let add = missing(Some(old));
        assert!(add.contains("hud_scale =") && !add.contains("res_x =") && !add.contains("fov_scale ="));
        assert!(missing(Some(&(old.to_string() + &add))).is_empty());
        assert!(has_key("  RES_X = 1 // c", "res_x") && !has_key("// res_x = 1", "res_x"));
    }

    #[test]
    fn missing_options_go_in_their_place() {
        // a v0.1.1 file: header, res, borderless, patch_mode (since dropped), fov (no auto_res, no hud_scale)
        let old = HEADER.to_string() + BLOCKS[1].1 + BLOCKS[2].1 + "\n// How the game is patched\npatch_mode =\n" + BLOCKS[3].1;
        let new = top_up(&old.replace("res_x =", "res_x = 3440").replace("res_y =", "res_y = 1440"));
        assert!(missing(Some(&new)).is_empty());
        let (a, r, h) = (new.find("auto_res = true").unwrap(), new.find("// Output resolution").unwrap(), new.find("hud_scale =").unwrap());
        assert!(new.find("This file is created once").unwrap() < a && a < r && r < h, "{new}");
        assert!(new.contains("auto_res = true\n\n// Output resolution") && new.contains("res_x = 3440\nres_y = 1440\n"));
        assert_eq!(top_up(&new), new, "nothing to add twice");
    }

    #[test]
    fn values_are_read_checked_and_unknown_keys_ignored() {
        let o = parse("res_x = 3440 // wide\nres_y=1440\nborderless_fullscreen = true\nfov_scale = 1.2\npatch_mode = EXE\nsmart_scaling = true\n");
        assert_eq!(o.res, Some((3440, 1440)));
        assert_eq!(o.borderless, Borderless::Fit);
        assert_eq!(parse("borderless_fullscreen = Stretch\n").borderless, Borderless::Stretch);
        assert_eq!(parse("borderless_fullscreen = false\n").borderless, Borderless::Off);
        assert_eq!(o.fov, Some(1.2));
        assert!(parse("res_x = 3440\n").res.is_none(), "both or neither");
        assert!(parse("res_x = 10\nres_y = 10\n").res.is_none(), "out of range");
        assert!(parse("fov_scale = 1\n").fov.is_none(), "1 is no change");
        assert!(parse("fov_scale = 9\n").fov.is_none(), "out of range");
    }
}
