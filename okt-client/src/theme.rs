//! Color: OKLCH helpers, accent derivations, monochrome families (`applyTheme()`).

use crate::dom;
use crate::js;
use crate::state::{boot, cfg};

fn hex2rgb(h: &str) -> [f64; 3] {
    let h = h.trim_start_matches('#');
    let p = |i: usize| {
        u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16)
            .map(|v| v as f64 / 255.0)
            .unwrap_or(f64::NAN)
    };
    [p(0), p(2), p(4)]
}

fn rgb2hex(rgb: [f64; 3]) -> String {
    let c = |v: f64| {
        let x = (v.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as i64; // Math.round on non-negatives
        format!("{x:02x}")
    };
    format!("#{}{}{}", c(rgb[0]), c(rgb[1]), c(rgb[2]))
}

fn lin(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn delin(c: f64) -> f64 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn rgb2oklab(rgb: [f64; 3]) -> [f64; 3] {
    let (r, g, b) = (lin(rgb[0]), lin(rgb[1]), lin(rgb[2]));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

fn oklab2rgb(lab: [f64; 3]) -> [f64; 3] {
    let [ll, a, b] = lab;
    let l = (ll + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (ll - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (ll - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        delin(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
        delin(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
        delin(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s),
    ]
}

fn to_lch(hex: &str) -> (f64, f64, f64) {
    let [l, a, b] = rgb2oklab(hex2rgb(hex));
    (l, a.hypot(b), b.atan2(a))
}

fn from_lch(l: f64, c0: f64, h: f64) -> String {
    let mut c = c0;
    while c >= 0.0 {
        let rgb = oklab2rgb([l, c * h.cos(), c * h.sin()]);
        if rgb.iter().all(|v| *v >= -0.001 && *v <= 1.001) {
            return rgb2hex(rgb);
        }
        c -= 0.005;
    }
    rgb2hex(oklab2rgb([l, 0.0, 0.0]))
}

fn rel_lum(hex: &str) -> f64 {
    let [r, g, b] = hex2rgb(hex).map(lin);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn contrast(a: &str, b: &str) -> f64 {
    let (x, y) = (rel_lum(a), rel_lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

pub fn theme_bg(theme: &str) -> &'static str {
    match theme {
        "dark" => "#16171a",
        "light" => "#fbfaf7",
        _ => "#000000",
    }
}

pub const ACCENTS: &[(&str, &str)] = &[
    ("amber", "#ffa028"),
    ("orange", "#ff7a1a"),
    ("green", "#33ff77"),
    ("cyan", "#2fd6ff"),
    ("blue", "#6aa8ff"),
    ("magenta", "#ff4fd8"),
    ("red", "#ff5555"),
    ("white", "#e8e6e1"),
];

const MONO_L: &[(&str, f64)] = &[
    ("core", 0.95),
    ("cards", 0.86),
    ("rules", 0.79),
    ("memory", 0.72),
    ("records", 0.65),
    ("workspace", 0.58),
    ("other", 0.5),
    ("world", 0.82),
];

fn is_hex6(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// `applyTheme()`
pub fn apply_theme() {
    let (theme, accent, reader, ticker) = {
        let c = cfg();
        (c.theme.clone(), c.accent.clone(), c.reader_font(), c.ticker)
    };
    let root = dom::doc().document_element().unwrap();
    let _ = root.set_attribute("data-theme", &theme);
    let acc = if is_hex6(&accent) {
        accent.to_lowercase()
    } else {
        "#ffa028".into()
    };
    let (l, c, h) = to_lch(&acc);
    let bg = theme_bg(&theme);
    let mut text = acc.clone();
    if contrast(&text, bg) < 4.5 {
        let dark = rel_lum(bg) > 0.5;
        let mut ll = l;
        while ll > 0.05 && ll < 0.99 {
            text = from_lch(ll, c, h);
            if contrast(&text, bg) >= 4.5 {
                break;
            }
            ll += if dark { -0.02 } else { 0.02 };
        }
    }
    let rgb = hex2rgb(&acc).map(|v| (v * 255.0 + 0.5).floor() as i64);
    let st = root.unchecked_ref::<web_sys::HtmlElement>().style();
    let set = |k: &str, v: &str| {
        let _ = st.set_property(k, v);
    };
    set("--accent", &acc);
    set("--accent-2", &from_lch((l + 0.1).min(0.97), c * 0.8, h));
    set(
        "--accent-dim",
        &format!("rgba({}, {}, {}, .3)", rgb[0], rgb[1], rgb[2]),
    );
    set(
        "--accent-ink",
        if rel_lum(&acc) > 0.33 { "#000" } else { "#fff" },
    );
    set("--accent-text", &text);
    set(
        "--sel",
        &format!(
            "rgba({}, {}, {}, {})",
            rgb[0],
            rgb[1],
            rgb[2],
            if theme == "light" { "0.14" } else { "0.13" }
        ),
    );
    for (f, ml) in MONO_L {
        let name = format!("--f-{f}");
        if theme == "terminal" {
            set(
                &name,
                &from_lch(*ml, c.min(0.19) * if *f == "world" { 1.0 } else { 0.92 }, h),
            );
        } else {
            let _ = st.remove_property(&name);
        }
    }
    set("--reader", &format!("var(--{reader})"));
    if let Some(font) = &boot().font {
        set(
            "--mono",
            &format!(
                "\"{font}\", \"Berkeley Mono\", \"JetBrains Mono\", \"SF Mono\", ui-monospace, Menlo, monospace"
            ),
        );
    }
    dom::toggle(&dom::id("app"), "noticker", !ticker);
    if let Some(m) = dom::q("meta[name=theme-color]") {
        let _ = m.set_attribute("content", bg);
    }
    let _ = js::num; // (keeps the import used in all cfgs)
}

use wasm_bindgen::JsCast;

/// A CSS custom property's current value on :root.
pub fn css_var(name: &str) -> String {
    let root = dom::doc().document_element().unwrap();
    dom::win()
        .get_computed_style(&root)
        .ok()
        .flatten()
        .and_then(|s| s.get_property_value(name).ok())
        .map(|v| v.trim().to_string())
        .unwrap_or_default()
}
