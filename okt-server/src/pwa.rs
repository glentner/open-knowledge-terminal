//! The installable app: icon, manifest, and service worker. The icon is a 5x7 pixel-font "OK"
//! and a block cursor, kept as rectangles on a 1024 grid, so the same geometry renders as SVG (the
//! favicon, and the icon Safari puts in the Dock) and as PNG (Chrome) without shipping image files.
//! It is a full, opaque square with everything inside the maskable safe zone.

use std::io::Write;

use serde_json::{Value, json};

#[rustfmt::skip]
const ICON_GLYPHS: [[&str; 7]; 2] = [
    [".XXX.", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX."], // O
    ["X...X", "X..X.", "X.X..", "XX...", "X.X..", "X..X.", "X...X"], // K
];
const ICON_BG: &str = "#0a0a0a";
const ICON_CURSOR: f64 = 0.5; // the block cursor is the accent at half strength
pub const ICON_PNGS: &[(&str, u32)] = &[
    ("/icon-192.png", 192),
    ("/icon-512.png", 512),
    ("/apple-touch-icon.png", 180),
];

pub fn theme_bg(theme: &str) -> &'static str {
    match theme {
        "dark" => "#16171a",
        "light" => "#fbfaf7",
        _ => "#000000",
    }
}

/// (x0, y0, x1, y1, is_cursor) on a 1024 canvas: two glyphs and a 3-cell cursor on a 15x7 grid.
fn icon_rects(fill: f64) -> Vec<(f64, f64, f64, f64, bool)> {
    let cell = 1024.0 * fill / 15.0;
    let (ox, oy) = ((1024.0 - 15.0 * cell) / 2.0, (1024.0 - 7.0 * cell) / 2.0);
    let mut out = Vec::new();
    for (g, glyph) in ICON_GLYPHS.iter().enumerate() {
        for (r, line) in glyph.iter().enumerate() {
            let chars: Vec<char> = line.chars().collect();
            let mut c = 0;
            while c < chars.len() {
                if chars[c] != 'X' {
                    c += 1;
                    continue;
                }
                let start = c;
                while c < chars.len() && chars[c] == 'X' {
                    c += 1;
                }
                let x = ox + (g as f64 * 6.0 + start as f64) * cell;
                out.push((
                    x,
                    oy + r as f64 * cell,
                    x + (c - start) as f64 * cell,
                    oy + (r as f64 + 1.0) * cell,
                    false,
                ));
            }
        }
    }
    out.push((
        ox + 12.0 * cell,
        oy,
        ox + 15.0 * cell,
        oy + 7.0 * cell,
        true,
    ));
    out
}

/// The app icon as SVG; the favicon variant has rounded corners and larger glyphs.
pub fn icon_svg(accent: &str, favicon: bool) -> String {
    let cursor = format!(" fill-opacity=\"{ICON_CURSOR}\"");
    let cells: String = icon_rects(if favicon { 0.84 } else { 0.70 })
        .iter()
        .map(|(x0, y0, x1, y1, cur)| {
            format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\"{}/>",
                x0,
                y0,
                x1 - x0,
                y1 - y0,
                if *cur { cursor.as_str() } else { "" }
            )
        })
        .collect();
    let radius = if favicon { " rx=\"190\"" } else { "" };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1024 1024\"><rect width=\"1024\" height=\"1024\"{radius} fill=\"{ICON_BG}\"/><g fill=\"{accent}\" shape-rendering=\"crispEdges\">{cells}</g></svg>"
    )
}

fn rgb(h: &str) -> [f64; 3] {
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0) as f64;
    [p(1), p(3), p(5)]
}

/// The app icon as an RGB PNG, rasterized with exact pixel coverage (no image library).
pub fn icon_png(size: u32, accent: &str) -> Vec<u8> {
    let (bg, fg, k) = (rgb(ICON_BG), rgb(accent), size as f64 / 1024.0);
    let rects: Vec<(f64, f64, f64, f64, [f64; 3])> = icon_rects(0.70)
        .into_iter()
        .map(|(x0, y0, x1, y1, cur)| {
            let s = if cur { ICON_CURSOR } else { 1.0 };
            (
                x0 * k,
                y0 * k,
                x1 * k,
                y1 * k,
                [
                    (fg[0] - bg[0]) * s,
                    (fg[1] - bg[1]) * s,
                    (fg[2] - bg[2]) * s,
                ],
            )
        })
        .collect();
    let mut raw = Vec::with_capacity((size * (size * 3 + 1)) as usize);
    for y in 0..size {
        let yf = y as f64;
        let mut px: Vec<[f64; 3]> = vec![bg; size as usize];
        for (x0, y0, x1, y1, delta) in &rects {
            let cy = (yf + 1.0).min(*y1) - yf.max(*y0);
            if cy <= 0.0 {
                continue;
            }
            let xs = x0.floor().max(0.0) as u32;
            let xe = (x1.ceil() as u32).min(size);
            for x in xs..xe {
                let xf = x as f64;
                let a = ((xf + 1.0).min(*x1) - xf.max(*x0)) * cy;
                if a > 0.0 {
                    let p = &mut px[x as usize];
                    for c in 0..3 {
                        p[c] += delta[c] * a;
                    }
                }
            }
        }
        raw.push(0);
        for p in px {
            for v in p {
                raw.push(v.round_ties_even().clamp(0.0, 255.0) as u8);
            }
        }
    }
    let chunk = |tag: &[u8], data: &[u8]| -> Vec<u8> {
        let mut c = Vec::new();
        c.extend((data.len() as u32).to_be_bytes());
        c.extend(tag);
        c.extend(data);
        let mut crc = flate2::Crc::new();
        crc.update(tag);
        crc.update(data);
        c.extend(crc.sum().to_be_bytes());
        c
    };
    let mut ihdr = Vec::new();
    ihdr.extend(size.to_be_bytes());
    ihdr.extend(size.to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]);
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    z.write_all(&raw).unwrap();
    let idat = z.finish().unwrap();
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(chunk(b"IHDR", &ihdr));
    png.extend(chunk(b"IDAT", &idat));
    png.extend(chunk(b"IEND", &[]));
    png
}

pub fn manifest(app_name: &str, theme: &str, root_display: &str) -> Value {
    let bg = theme_bg(theme);
    let png = |n: u32, purpose: &str| json!({"src": format!("/icon-{n}.png"), "sizes": format!("{n}x{n}"), "type": "image/png", "purpose": purpose});
    json!({
        "id": "/", "name": app_name, "short_name": short_name(app_name),
        "description": format!("Live, read-only terminal for the knowledge base at {root_display}"),
        "start_url": "/", "scope": "/", "display": "standalone",
        "launch_handler": {"client_mode": ["focus-existing", "auto"]},
        "background_color": bg, "theme_color": bg,
        // Safari prefers the SVG; Chrome on macOS masks the maskable PNG into the Dock's rounded square
        "icons": [{"src": "/icon.svg", "sizes": "any", "type": "image/svg+xml", "purpose": "any"}, png(192, "any"), png(512, "any"), png(512, "maskable")],
        "shortcuts": [{"name": "Graph", "url": "/#v=graph"}, {"name": "Files", "url": "/#v=files"}, {"name": "Log", "url": "/#v=log"}],
    })
}

pub fn short_name(app: &str) -> String {
    if app == "Open Knowledge Terminal" {
        "OK Terminal".into()
    } else {
        app.into()
    }
}

/// The service worker keeps the app shell, so the installed app opens even while the server is
/// down (the page then waits for it and reconnects by itself). The shell is network-first, so a
/// running server always serves the current version; live data (/api, /raw) never touches the
/// cache. A new build installs a new worker and the old cache is dropped.
pub fn service_worker(build: &str, shell: &[String]) -> String {
    let list = serde_json::to_string(shell).unwrap();
    format!(
        r#"/* Open Knowledge Terminal service worker. */
const CACHE = 'okt-{build}';
const SHELL = {list};

self.addEventListener('install', e => {{
  e.waitUntil(caches.open(CACHE).then(c => c.addAll(SHELL)).then(() => self.skipWaiting()));
}});

self.addEventListener('activate', e => {{
  e.waitUntil(caches.keys()
    .then(keys => Promise.all(keys.filter(k => k.startsWith('okt-') && k !== CACHE).map(k => caches.delete(k))))
    .then(() => self.clients.claim()));
}});

self.addEventListener('fetch', e => {{
  const req = e.request;
  const url = new URL(req.url);
  if (req.method !== 'GET' || url.origin !== self.location.origin) return;
  if (url.pathname.startsWith('/api/') || url.pathname.startsWith('/raw/')) return;
  const key = req.mode === 'navigate' ? '/' : url.pathname;
  if (!SHELL.includes(key)) return;
  e.respondWith(fetch(req)
    .then(res => {{
      if (res.ok) {{ const copy = res.clone(); caches.open(CACHE).then(c => c.put(key, copy)); }}
      return res;
    }})
    .catch(() => caches.match(key).then(hit => hit || Response.error())));
}});
"#
    )
}
