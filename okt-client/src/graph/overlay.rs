//! The flat overlay: a second canvas over `#graph`, pointer-transparent and sized to the pane at
//! the device pixel ratio, for marks that stay crisp and flat whatever the camera does. It draws
//! the pointer's hover ring, the keyboard walk's dashed ring and the flash after a live change.
//! It is also where CSS colours are turned into numbers for the GL.

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::dom;

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
}

impl Overlay {
    /// The overlay canvas `#goverlay`, when the page has one.
    pub fn new() -> Option<Overlay> {
        let cv: HtmlCanvasElement = dom::try_id("goverlay")?.dyn_into().ok()?;
        let ctx: CanvasRenderingContext2d = cv.get_context("2d").ok().flatten()?.dyn_into().ok()?;
        Some(Overlay { cv, ctx })
    }

    /// A CSS colour as straight RGBA in 0–1: the browser canonicalises it through `fillStyle`
    /// (to `#rrggbb` or `rgba(r, g, b, a)`), and that is parsed. Unparseable gives mid grey.
    pub fn canon(&self, css: &str) -> [f32; 4] {
        self.ctx.set_fill_style_str("#808080");
        self.ctx.set_fill_style_str(css);
        let s = self.ctx.fill_style().as_string().unwrap_or_default();
        parse_color(&s).unwrap_or([0.5, 0.5, 0.5, 1.0])
    }

    /// Clear to the pane's size at `dpr` and stroke the rings in `color`.
    pub fn draw(&self, w: f64, h: f64, dpr: f64, color: &str, rings: &[Ring]) {
        let (bw, bh) = ((w * dpr).round() as u32, (h * dpr).round() as u32);
        if self.cv.width() != bw || self.cv.height() != bh {
            self.cv.set_width(bw);
            self.cv.set_height(bh);
        }
        let ctx = &self.ctx;
        ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
        ctx.clear_rect(0.0, 0.0, bw as f64, bh as f64);
        if rings.is_empty() {
            return;
        }
        ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0).ok();
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
    }
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
