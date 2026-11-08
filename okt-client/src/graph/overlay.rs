//! The flat overlay: a second canvas over `#graph`, pointer-transparent and sized to the pane at
//! the device pixel ratio, for marks that stay crisp and flat whatever the camera does. It draws
//! region names beside their glyphs, the pointer's hover ring, the keyboard walk's dashed ring,
//! the flash after a live change, and the compass in the bottom-right corner (the galaxy's axes
//! and horizon as the camera sees them, the focused region's direction, the scale), which names
//! keep clear of. Its text takes the `#goverlay` rule's font and colour in `app.css`, knocked out
//! of the background in `--graph-bg`. It is also where CSS colours are turned into numbers for
//! the GL.
//!
//! The labels it placed are written to `#graph[data-labels]` from the same list it drew.

use std::cell::RefCell;
use std::collections::HashMap;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, Element, HtmlCanvasElement};

use crate::dom;

/// A region's name, to place beside its glyph at (`x`, `y`), radius `r` px. `inward` points the
/// way the name prefers to sit (toward the pane centre, for a glyph on the rim).
pub struct Name {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub inward: [f64; 2],
    pub text: String,
}

/// A label as drawn: its rect in pane CSS px, its kind and its text.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub kind: &'static str,
    pub text: String,
}

/// The overlay's text style, read from the `#goverlay` rule.
struct Style {
    font: String,
    ink: String,
    knockout: String,
    /// the font's size in CSS px
    size: f64,
}

/// The compass's box: this many CSS px square, this far in from the pane's bottom-right corner.
pub const COMPASS_PX: f64 = 56.0;
pub const COMPASS_INSET: f64 = 10.0;

/// What the compass shows: the camera's frame (`r` screen right, `u` screen up, `dir` toward the
/// eye) in world axes, the focused region's core (a direction from the galaxy's centre), the
/// scale's name, and its colours as CSS.
pub struct Compass {
    pub r: [f64; 3],
    pub u: [f64; 3],
    pub dir: [f64; 3],
    pub region: Option<[f64; 3]>,
    pub scale: &'static str,
    /// how far above its corner the compass sits (clear of a legend reaching under it), CSS px
    pub lift: f64,
    pub ink: String,
    pub far: String,
    pub accent: String,
}

/// One ring, at pane CSS px.
pub struct Ring {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub width: f64,
    pub alpha: f64,
    pub dashed: bool,
}

pub struct Overlay {
    cv: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    style: RefCell<Style>,
    /// text widths at the current font, cleared when the style is read again
    widths: RefCell<HashMap<String, f64>>,
}

impl Overlay {
    /// The overlay canvas `#goverlay`, when the page has one.
    pub fn new() -> Option<Overlay> {
        let cv: HtmlCanvasElement = dom::try_id("goverlay")?.dyn_into().ok()?;
        let ctx: CanvasRenderingContext2d = cv.get_context("2d").ok().flatten()?.dyn_into().ok()?;
        let ov = Overlay {
            cv,
            ctx,
            style: RefCell::new(Style {
                font: "600 10.5px monospace".into(),
                ink: "#999".into(),
                knockout: "#000".into(),
                size: 10.5,
            }),
            widths: RefCell::new(HashMap::new()),
        };
        ov.restyle("#000");
        Some(ov)
    }

    /// Read the text style again (a theme or accent change): the `#goverlay` rule's font and
    /// colour, and `knockout`, the background its text is cut out of.
    pub fn restyle(&self, knockout: &str) {
        let cs = dom::win().get_computed_style(&self.cv).ok().flatten();
        let get = |k: &str| {
            cs.as_ref()
                .and_then(|s| s.get_property_value(k).ok())
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let mut st = self.style.borrow_mut();
        if let Some(f) = get("font") {
            st.size = f
                .split_whitespace()
                .find_map(|t| t.strip_suffix("px").and_then(|n| n.parse::<f64>().ok()))
                .unwrap_or(10.5);
            st.font = f;
        }
        if let Some(c) = get("color") {
            st.ink = c;
        }
        st.knockout = knockout.to_string();
        self.widths.borrow_mut().clear();
    }

    fn width(&self, text: &str) -> f64 {
        if let Some(w) = self.widths.borrow().get(text) {
            return *w;
        }
        let w = self
            .ctx
            .measure_text(text)
            .map(|m| m.width())
            .unwrap_or(text.chars().count() as f64 * 6.5);
        self.widths.borrow_mut().insert(text.to_string(), w);
        w
    }

    /// A CSS colour as straight RGBA in 0–1: the browser canonicalises it through `fillStyle`
    /// (to `#rrggbb` or `rgba(r, g, b, a)`), and that is parsed. Unparseable gives mid grey.
    pub fn canon(&self, css: &str) -> [f32; 4] {
        self.ctx.set_fill_style_str("#808080");
        self.ctx.set_fill_style_str(css);
        let s = self.ctx.fill_style().as_string().unwrap_or_default();
        parse_color(&s).unwrap_or([0.5, 0.5, 0.5, 1.0])
    }

    /// Clear to the pane's size at `dpr`, draw the compass, place and draw the region names (in
    /// the order given, clear of the compass; a name that fits nowhere free is dropped for this
    /// frame), and stroke the rings in `color`. Returns the labels drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        w: f64,
        h: f64,
        dpr: f64,
        color: &str,
        rings: &[Ring],
        names: &[Name],
        compass: Option<&Compass>,
    ) -> Vec<Placed> {
        let (bw, bh) = ((w * dpr).round() as u32, (h * dpr).round() as u32);
        if self.cv.width() != bw || self.cv.height() != bh {
            self.cv.set_width(bw);
            self.cv.set_height(bh);
        }
        let ctx = &self.ctx;
        ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
        ctx.clear_rect(0.0, 0.0, bw as f64, bh as f64);
        if rings.is_empty() && names.is_empty() && compass.is_none() {
            return Vec::new();
        }
        ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0).ok();
        let keep_out = compass.map(|c| self.compass(w, h, c));
        let placed = self.names(w, h, names, keep_out);
        if rings.is_empty() {
            return placed;
        }
        ctx.set_stroke_style_str(color);
        for r in rings {
            dash(ctx, if r.dashed { &[3.0, 2.5] } else { &[] });
            ctx.set_global_alpha(r.alpha);
            ctx.set_line_width(r.width);
            ctx.begin_path();
            ctx.arc(r.x, r.y, r.r, 0.0, std::f64::consts::TAU).ok();
            ctx.stroke();
        }
        dash(ctx, &[]);
        ctx.set_global_alpha(1.0);
        placed
    }

    /// Draw the compass in the bottom-right corner, and return its box (x, y, w, h). Each world
    /// axis is a line from the centre (dimmed when it points away from the eye); the horizon is
    /// the ground plane's unit circle; the focused region is a dot on the sphere (hollow when it
    /// lies behind); the scale's name sits under it.
    fn compass(&self, w: f64, h: f64, c: &Compass) -> (f64, f64, f64, f64) {
        let ctx = &self.ctx;
        let (x0, y0) = (
            w - COMPASS_INSET - COMPASS_PX,
            h - COMPASS_INSET - COMPASS_PX - c.lift,
        );
        let (cx, cy, rad) = (x0 + COMPASS_PX / 2.0, y0 + COMPASS_PX / 2.0 - 5.0, 19.0);
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let at = |p: [f64; 3]| (cx + rad * dot(p, c.r), cy - rad * dot(p, c.u));
        dash(ctx, &[]);
        ctx.set_line_width(1.0);
        // the horizon
        ctx.set_stroke_style_str(&c.far);
        ctx.set_global_alpha(0.8);
        ctx.begin_path();
        for k in 0..=48 {
            let a = k as f64 / 48.0 * std::f64::consts::TAU;
            let (x, y) = at([a.cos(), 0.0, a.sin()]);
            if k == 0 {
                ctx.move_to(x, y);
            } else {
                ctx.line_to(x, y);
            }
        }
        ctx.stroke();
        // the axes, far ones first
        let mut axes = [
            ([1.0, 0.0, 0.0], "x"),
            ([0.0, 1.0, 0.0], "y"),
            ([0.0, 0.0, 1.0], "z"),
        ];
        axes.sort_by(|a, b| dot(a.0, c.dir).total_cmp(&dot(b.0, c.dir)));
        let st = self.style.borrow();
        ctx.set_font(&st.font);
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        for (e, name) in axes {
            let far = dot(e, c.dir) < 0.0;
            let ink = if far { &c.far } else { &c.ink };
            let (x, y) = at(e);
            ctx.set_global_alpha(if far { 0.55 } else { 1.0 });
            ctx.set_stroke_style_str(ink);
            ctx.begin_path();
            ctx.move_to(cx, cy);
            ctx.line_to(x, y);
            ctx.stroke();
            ctx.set_fill_style_str(ink);
            let (lx, ly) = at([e[0] * 1.3, e[1] * 1.3, e[2] * 1.3]);
            ctx.fill_text(name, lx, ly).ok();
        }
        ctx.set_global_alpha(1.0);
        // the focused region's direction
        if let Some(p) = c.region {
            let l = dot(p, p).sqrt();
            if l > 1e-9 {
                let q = [p[0] / l, p[1] / l, p[2] / l];
                let (x, y) = at(q);
                ctx.begin_path();
                ctx.arc(x, y, 2.6, 0.0, std::f64::consts::TAU).ok();
                if dot(q, c.dir) >= 0.0 {
                    ctx.set_fill_style_str(&c.accent);
                    ctx.fill();
                } else {
                    ctx.set_stroke_style_str(&c.accent);
                    ctx.stroke();
                }
            }
        }
        // the scale
        ctx.set_fill_style_str(&c.far);
        ctx.fill_text(c.scale, cx, y0 + COMPASS_PX - st.size * 0.6)
            .ok();
        ctx.set_text_align("start");
        (x0, y0, COMPASS_PX, COMPASS_PX)
    }

    /// Place each name beside its glyph, trying the side it leans toward first, then the others;
    /// keep it inside the pane, off `keep_out` (the compass), and clear of every name placed
    /// before it; draw the ones placed.
    fn names(
        &self,
        w: f64,
        h: f64,
        names: &[Name],
        keep_out: Option<(f64, f64, f64, f64)>,
    ) -> Vec<Placed> {
        const PAD: f64 = 2.0;
        let st = self.style.borrow();
        let ctx = &self.ctx;
        ctx.set_font(&st.font);
        let th = (st.size * 1.25).ceil();
        let mut placed: Vec<Placed> = Vec::new();
        for n in names {
            let tw = self.width(&n.text).ceil();
            let gap = n.r + 4.0;
            let mut sides = [
                ([1.0, 0.0], n.x + gap, n.y - th / 2.0),
                ([-1.0, 0.0], n.x - gap - tw, n.y - th / 2.0),
                ([0.0, 1.0], n.x - tw / 2.0, n.y + gap),
                ([0.0, -1.0], n.x - tw / 2.0, n.y - gap - th),
            ];
            sides.sort_by(|a, b| {
                let da = a.0[0] * n.inward[0] + a.0[1] * n.inward[1];
                let db = b.0[0] * n.inward[0] + b.0[1] * n.inward[1];
                db.total_cmp(&da)
            });
            let clear = |x: f64, y: f64, (ox, oy, ow, oh): (f64, f64, f64, f64)| {
                x + tw + PAD <= ox || ox + ow + PAD <= x || y + th + PAD <= oy || oy + oh + PAD <= y
            };
            let fits = |x: f64, y: f64| {
                x >= 0.0
                    && y >= 0.0
                    && x + tw <= w
                    && y + th <= h
                    && keep_out.is_none_or(|k| clear(x, y, k))
                    && placed.iter().all(|p| {
                        x + tw + PAD <= p.x
                            || p.x + p.w + PAD <= x
                            || y + th + PAD <= p.y
                            || p.y + p.h + PAD <= y
                    })
            };
            let spot = sides.iter().find(|s| fits(s.1, s.2)).map(|s| (s.1, s.2));
            if let Some((x, y)) = spot {
                placed.push(Placed {
                    x,
                    y,
                    w: tw,
                    h: th,
                    kind: "region",
                    text: n.text.clone(),
                });
            }
        }
        ctx.set_text_baseline("middle");
        ctx.set_line_join("round");
        ctx.set_line_width(3.0);
        ctx.set_stroke_style_str(&st.knockout);
        ctx.set_fill_style_str(&st.ink);
        for p in &placed {
            let y = p.y + p.h / 2.0;
            ctx.stroke_text(&p.text, p.x, y).ok();
            ctx.fill_text(&p.text, p.x, y).ok();
        }
        placed
    }
}

thread_local! {
    /// The last `data-labels` written, so a frame writes it only when it changed.
    static LAST: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Write `#graph[data-labels]`, `[[x, y, w, h, kind, text], …]` in pane CSS px to 0.1, from the
/// labels the overlay drew this frame.
pub fn publish(cv: &Element, placed: &[Placed]) {
    let r = |v: f64| (v * 10.0).round() / 10.0;
    let rows: Vec<serde_json::Value> = placed
        .iter()
        .map(|p| serde_json::json!([r(p.x), r(p.y), r(p.w), r(p.h), p.kind, p.text]))
        .collect();
    let s = serde_json::Value::Array(rows).to_string();
    LAST.with(|l| {
        let mut l = l.borrow_mut();
        if *l != s {
            let _ = cv.set_attribute("data-labels", &s);
            *l = s;
        }
    });
}

fn dash(ctx: &CanvasRenderingContext2d, d: &[f64]) {
    let arr = js_sys::Array::new();
    for v in d {
        arr.push(&JsValue::from_f64(*v));
    }
    ctx.set_line_dash(&arr).ok();
}

/// `#rgb`, `#rrggbb`, `rgb(…)` or `rgba(…)` as straight RGBA in 0–1.
fn parse_color(s: &str) -> Option<[f32; 4]> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        let p = |i: usize, n: usize| u8::from_str_radix(h.get(i..i + n)?, 16).ok();
        return match h.len() {
            6 => Some([
                p(0, 2)? as f32 / 255.0,
                p(2, 2)? as f32 / 255.0,
                p(4, 2)? as f32 / 255.0,
                1.0,
            ]),
            3 => Some([
                p(0, 1)? as f32 / 15.0,
                p(1, 1)? as f32 / 15.0,
                p(2, 1)? as f32 / 15.0,
                1.0,
            ]),
            _ => None,
        };
    }
    let inner = s
        .strip_prefix("rgba(")
        .or_else(|| s.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let v: Vec<f32> = inner
        .split([',', '/', ' '])
        .filter(|x| !x.is_empty())
        .map(|x| x.parse::<f32>())
        .collect::<Result<_, _>>()
        .ok()?;
    match v.as_slice() {
        [r, g, b] => Some([r / 255.0, g / 255.0, b / 255.0, 1.0]),
        [r, g, b, a] => Some([r / 255.0, g / 255.0, b / 255.0, *a]),
        _ => None,
    }
}
