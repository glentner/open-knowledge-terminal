//! The flat overlay: a second canvas over `#graph`, pointer-transparent and sized to the pane at
//! the device pixel ratio, for marks that stay crisp and flat whatever the camera does. It draws
//! the labels `labels::place` placed (region names and card labels, never in perspective), the
//! pointer's hover ring, the keyboard walk's dashed ring, the flash after a live change, and the
//! compass in the bottom-right corner (the galaxy's axes and horizon as the camera sees them, the
//! focused region's direction, the scale), which labels keep clear of. Its text takes the
//! `#goverlay` rule in `app.css`: the rule's font and colour for region names, `--glab-*` for card
//! labels, knocked out of the background in `--graph-bg`. It is also where CSS colours are turned
//! into numbers for the GL.
//!
//! Each label is measured with the font it is drawn in, and its text is clipped to the rect it
//! was placed in, so what `#graph[data-labels]` says was drawn is what was drawn: the hook is
//! written from the same list.

use std::cell::RefCell;
use std::collections::HashMap;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, Element, HtmlCanvasElement};

use super::labels::{Kind, Placed, Rect};
use crate::dom;

/// One text style of the overlay: a CSS font, its size in CSS px and its ink.
struct Face {
    font: String,
    size: f64,
    ink: String,
}

/// The overlay's text styles, read from the `#goverlay` rule: region names (the rule's own font
/// and colour), card labels (`--glab-font`, `--glab-ink`), the focus's label (`--glab-focus-font`
/// in `--glab-accent`) and portal labels (the card font in `--glab-accent`).
struct Style {
    region: Face,
    card: Face,
    focus: Face,
    portal: Face,
    knockout: String,
}

impl Style {
    fn face(&self, k: Kind) -> &Face {
        match k {
            Kind::Region => &self.region,
            Kind::Focus => &self.focus,
            Kind::Portal => &self.portal,
            Kind::Card | Kind::Folder => &self.card,
        }
    }
}

/// The font size in a CSS font shorthand, in px.
fn font_px(f: &str) -> Option<f64> {
    f.split_whitespace()
        .find_map(|t| t.strip_suffix("px").and_then(|n| n.parse::<f64>().ok()))
}

/// Which width cache a kind measures in (one per font).
fn class(k: Kind) -> usize {
    match k {
        Kind::Region => 0,
        Kind::Focus => 1,
        Kind::Card | Kind::Folder | Kind::Portal => 2,
    }
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

/// The compass's box in pane CSS px, `lift` px above its corner: an obstacle for the labels.
pub fn compass_box(w: f64, h: f64, lift: f64) -> Rect {
    Rect {
        x: w - COMPASS_INSET - COMPASS_PX,
        y: h - COMPASS_INSET - COMPASS_PX - lift,
        w: COMPASS_PX,
        h: COMPASS_PX,
    }
}

pub struct Overlay {
    cv: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    style: RefCell<Style>,
    /// text widths per font (see `class`), keyed by text, cleared when the style is read again
    widths: RefCell<[HashMap<String, f64>; 3]>,
}

impl Overlay {
    /// The overlay canvas `#goverlay`, when the page has one.
    pub fn new() -> Option<Overlay> {
        let cv: HtmlCanvasElement = dom::try_id("goverlay")?.dyn_into().ok()?;
        let ctx: CanvasRenderingContext2d = cv.get_context("2d").ok().flatten()?.dyn_into().ok()?;
        let face = |font: &str, ink: &str| Face {
            font: font.into(),
            size: font_px(font).unwrap_or(10.5),
            ink: ink.into(),
        };
        let ov = Overlay {
            cv,
            ctx,
            style: RefCell::new(Style {
                region: face("600 10.5px monospace", "#999"),
                card: face("400 10px monospace", "#999"),
                focus: face("700 10.5px monospace", "#fa0"),
                portal: face("400 10px monospace", "#fa0"),
                knockout: "#000".into(),
            }),
            widths: RefCell::new(Default::default()),
        };
        ov.restyle("#000");
        Some(ov)
    }

    /// Read the text styles again (a theme or accent change): the `#goverlay` rule's font and
    /// colour and its `--glab-*` properties, and `knockout`, the background text is cut out of.
    pub fn restyle(&self, knockout: &str) {
        let cs = dom::win().get_computed_style(&self.cv).ok().flatten();
        let get = |k: &str| {
            cs.as_ref()
                .and_then(|s| s.get_property_value(k).ok())
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let mut st = self.style.borrow_mut();
        let set_font = |f: &mut Face, v: Option<String>| {
            if let Some(v) = v {
                f.size = font_px(&v).unwrap_or(f.size);
                f.font = v;
            }
        };
        set_font(&mut st.region, get("font"));
        set_font(&mut st.card, get("--glab-font"));
        set_font(&mut st.focus, get("--glab-focus-font"));
        let card_font = (st.card.font.clone(), st.card.size);
        (st.portal.font, st.portal.size) = card_font;
        if let Some(c) = get("color") {
            st.region.ink = c;
        }
        if let Some(c) = get("--glab-ink") {
            st.card.ink = c;
        }
        if let Some(c) = get("--glab-accent") {
            st.focus.ink = c.clone();
            st.portal.ink = c;
        }
        st.knockout = knockout.to_string();
        for w in self.widths.borrow_mut().iter_mut() {
            w.clear();
        }
    }

    /// The rect a label of kind `k` needs for `text`, in CSS px: its width as `measureText`
    /// gives it in the font it is drawn in (cached), with a px either side for the knockout, and
    /// a line of that font.
    pub fn measure(&self, k: Kind, text: &str) -> (f64, f64) {
        let st = self.style.borrow();
        let face = st.face(k);
        let h = (face.size * 1.25).ceil();
        let c = class(k);
        if let Some(w) = self.widths.borrow()[c].get(text) {
            return (*w, h);
        }
        self.ctx.set_font(&face.font);
        let w = self
            .ctx
            .measure_text(text)
            .map(|m| m.width())
            .unwrap_or(text.chars().count() as f64 * 6.5)
            .ceil()
            + 2.0;
        self.widths.borrow_mut()[c].insert(text.to_string(), w);
        (w, h)
    }

    /// A CSS colour as straight RGBA in 0–1: the browser canonicalises it through `fillStyle`
    /// (to `#rrggbb` or `rgba(r, g, b, a)`), and that is parsed. Unparseable gives mid grey.
    pub fn canon(&self, css: &str) -> [f32; 4] {
        self.ctx.set_fill_style_str("#808080");
        self.ctx.set_fill_style_str(css);
        let s = self.ctx.fill_style().as_string().unwrap_or_default();
        parse_color(&s).unwrap_or([0.5, 0.5, 0.5, 1.0])
    }

    /// Clear to the pane's size at `dpr`, draw the compass and exactly the labels given, and
    /// stroke the rings in `color`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        w: f64,
        h: f64,
        dpr: f64,
        color: &str,
        rings: &[Ring],
        labels: &[Placed],
        compass: Option<&Compass>,
    ) {
        let (bw, bh) = ((w * dpr).round() as u32, (h * dpr).round() as u32);
        if self.cv.width() != bw || self.cv.height() != bh {
            self.cv.set_width(bw);
            self.cv.set_height(bh);
        }
        let ctx = &self.ctx;
        ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
        ctx.clear_rect(0.0, 0.0, bw as f64, bh as f64);
        if rings.is_empty() && labels.is_empty() && compass.is_none() {
            return;
        }
        ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0).ok();
        if let Some(c) = compass {
            self.compass(w, h, c);
        }
        self.labels(labels);
        if rings.is_empty() {
            return;
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
    }

    /// Draw the compass in its box (`compass_box`) in the bottom-right corner. Each world axis is a line from the centre (dimmed when it points away from the eye); the horizon is
    /// the ground plane's unit circle; the focused region is a dot on the sphere (hollow when it
    /// lies behind); the scale's name sits under it.
    fn compass(&self, w: f64, h: f64, c: &Compass) {
        let ctx = &self.ctx;
        let Rect { x: x0, y: y0, .. } = compass_box(w, h, c.lift);
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
        let face = &st.region;
        ctx.set_font(&face.font);
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
        ctx.fill_text(c.scale, cx, y0 + COMPASS_PX - face.size * 0.6)
            .ok();
        ctx.set_text_align("start");
    }

    /// Draw each label in its rect, in its kind's font and ink, knocked out of the background
    /// and clipped to the rect.
    fn labels(&self, labels: &[Placed]) {
        if labels.is_empty() {
            return;
        }
        let st = self.style.borrow();
        let ctx = &self.ctx;
        ctx.set_text_align("start");
        ctx.set_text_baseline("middle");
        ctx.set_line_join("round");
        ctx.set_line_width(3.0);
        ctx.set_stroke_style_str(&st.knockout);
        let mut font = "";
        for p in labels {
            let face = st.face(p.kind);
            if face.font != font {
                ctx.set_font(&face.font);
                font = &face.font;
            }
            ctx.set_fill_style_str(&face.ink);
            ctx.save();
            ctx.begin_path();
            ctx.rect(p.x, p.y, p.w, p.h);
            ctx.clip();
            let (x, y) = (p.x + 1.0, p.y + p.h / 2.0);
            ctx.stroke_text(&p.text, x, y).ok();
            ctx.fill_text(&p.text, x, y).ok();
            ctx.restore();
        }
    }
}

thread_local! {
    /// The last `data-labels` and `data-label-cands` written, so a frame writes them only when
    /// they changed.
    static LAST: RefCell<(String, Option<usize>)> = const { RefCell::new((String::new(), None)) };
}

/// Write `#graph[data-labels]`, `[[x, y, w, h, kind, text], …]` in pane CSS px to 0.1, from the
/// labels the overlay drew this frame, and `#graph[data-label-cands]`, how many cards could
/// have been labelled before the cap.
pub fn publish(cv: &Element, placed: &[Placed], cands: usize) {
    let r = |v: f64| (v * 10.0).round() / 10.0;
    let rows: Vec<serde_json::Value> = placed
        .iter()
        .map(|p| serde_json::json!([r(p.x), r(p.y), r(p.w), r(p.h), p.kind.name(), p.text]))
        .collect();
    let s = serde_json::Value::Array(rows).to_string();
    LAST.with(|l| {
        let mut l = l.borrow_mut();
        if l.0 != s {
            let _ = cv.set_attribute("data-labels", &s);
            l.0 = s;
        }
        if l.1 != Some(cands) {
            let _ = cv.set_attribute("data-label-cands", &cands.to_string());
            l.1 = Some(cands);
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
