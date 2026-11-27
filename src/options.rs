//! uw-options.ini: created once next to the dll, never overwritten. Empty or missing entries mean "off".
use crate::sys::dll_dir;

const INI_NAME: &str = "uw-options.ini";
const INI_TEXT: &str = "// GTA VI ultrawide options for VIctory. Edit, save, then start the game again.
// Anything left empty (or missing) is ignored: its effect is simply not applied.
// This file is created once and never overwritten by updates.

// Output resolution in pixels (640-7680 x 360-4320). Set both, or leave both empty to keep the game's own.
res_x =
res_y =

// A borderless window covering the monitor: true or false. Empty = false.
borderless_fullscreen =

// Field of view multiplier: 1.2 = 20% wider, 0.8 = 20% narrower (0.5 - 2). Empty = unchanged.
fov_scale =
";

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub res: Option<(u32, u32)>,
    pub fov: Option<f64>,
    pub borderless: bool,
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
    if !path.exists() {
        match std::fs::write(&path, INI_TEXT) {
            Ok(()) => println!("uw: created {}", path.display()),
            Err(e) => eprintln!("uw: cannot create {}: {e}", path.display()),
        }
    }
    parse(&std::fs::read_to_string(&path).unwrap_or_default())
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
    let borderless = matches!(value(text, "borderless_fullscreen").as_deref().map(str::to_ascii_lowercase).as_deref(), Some("true" | "1" | "yes" | "on"));
    let fov = number(text, "fov_scale", 0.5, 2.0).filter(|f| (f - 1.0).abs() > 1e-6);
    Options { res, fov, borderless }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_text_changes_nothing() {
        let o = parse(INI_TEXT);
        assert!(o.res.is_none() && o.fov.is_none() && !o.borderless);
    }

    #[test]
    fn values_are_read_checked_and_unknown_keys_ignored() {
        let o = parse("res_x = 3440 // wide\nres_y=1440\nborderless_fullscreen = true\nfov_scale = 1.2\nsmart_scaling = true\n");
        assert_eq!(o.res, Some((3440, 1440)));
        assert!(o.borderless);
        assert_eq!(o.fov, Some(1.2));
        assert!(parse("res_x = 3440\n").res.is_none(), "both or neither");
        assert!(parse("res_x = 10\nres_y = 10\n").res.is_none(), "out of range");
        assert!(parse("fov_scale = 1\n").fov.is_none(), "1 is no change");
        assert!(parse("fov_scale = 9\n").fov.is_none(), "out of range");
    }
}
