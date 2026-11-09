//! The knowledge graph: a deterministic radial layout by folder, drawn on a canvas.
//!
//! An SVG restyled through CSS on every selection and scale change stutters at ~500 elements, so
//! the picture is painted from a resolved copy of the graph's CSS rules (`.n`, `.lab`,
//! `.halo`, `.ebase`, `.ehot`, `.spotlit`, ...), once per animation frame and only when something
//! changed. Input handlers never read layout; they update the view and schedule a frame.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use std::rc::Rc;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, Path2d, PointerEvent, WheelEvent};

use crate::dom;
use crate::js::{self, esc, fmt_n, num, to_fixed};
use crate::model::{Idx, edge_group, fam_var, shape_of};
use crate::state::{self, cfg, is_collapsed};
use crate::theme;
use crate::{chrome, insp};

const R_ROOTFILES: f64 = 58.0;
const R_TOP: f64 = 190.0;
const ROW: f64 = 18.0;
const SPACING: f64 = 11.5;

fn dr_dir(d: i32) -> f64 {
    [0.0, 0.0, 100.0, 84.0, 70.0, 60.0, 54.0, 50.0][d.clamp(0, 7) as usize]
}
fn dr_file(d: i32) -> f64 {
    if d <= 1 { 54.0 } else { 44.0 }
}
fn rad(a: f64) -> f64 {
    a * PI / 180.0
}
fn polar(a: f64, r: f64) -> (f64, f64) {
    (r * rad(a).sin(), -r * rad(a).cos())
}

// ---- layout --------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct P {
    pub x: f64,
    pub y: f64,
    pub a: f64,
    pub r: f64,
    pub depth: i32,
    pub label: Option<String>,
    pub chain: Option<String>,
    pub cluster: bool,
}

#[derive(Clone, Debug)]
pub struct VEdge {
    pub s: String,
    pub t: String,
    pub g: &'static str,
    pub c: f64,
}

#[derive(Default)]
pub struct Layout {
    pub pos: HashMap<String, P>,
    pub rep: HashMap<String, String>,
    pub vis: Vec<String>,
    pub vedges: Vec<VEdge>,
    pub bbox: [f64; 4],
    pub r_out: f64,
}

impl Layout {
    /// `G.rep.get(id) ?? id`
    pub fn rep_or<'a>(&'a self, id: &'a str) -> &'a str {
        self.rep.get(id).map(|s| s.as_str()).unwrap_or(id)
    }
}

fn chain_end(idx: &Idx, id: &str) -> (String, String) {
    let mut cur = id.to_string();
    let mut label = idx.node(id).map(|n| n.name.clone()).unwrap_or_default();
    for _ in 0..12 {
        if is_collapsed(idx, &cur) {
            break;
        }
        let (dirs, files) = idx.sorted_kids(&cur, false);
        if !files.is_empty() || dirs.len() != 1 {
            break;
        }
        cur = dirs[0].clone();
        label.push('/');
        label.push_str(&idx.node(&cur).map(|n| n.name.clone()).unwrap_or_default());
    }
    (cur, label + "/")
}

fn weight(idx: &Idx, id: &str, cache: &mut HashMap<String, f64>) -> f64 {
    if let Some(w) = cache.get(id) {
        return *w;
    }
    let w = if is_collapsed(idx, id) {
        2.2
    } else {
        let (end, _) = chain_end(idx, id);
        let (dirs, files) = idx.sorted_kids(&end, false);
        let mut w = files.len() as f64 * 0.7;
        for d in dirs {
            w += weight(idx, d, cache);
        }
        w.max(1.6)
    };
    cache.insert(id.to_string(), w);
    w
}

struct Placer<'a> {
    idx: &'a Idx,
    lay: Layout,
    wc: HashMap<String, f64>,
}

impl Placer<'_> {
    fn set_pos(&mut self, id: &str, a: f64, r: f64, p: P) {
        let (x, y) = polar(a, r);
        self.lay.pos.insert(id.to_string(), P { x, y, a, r, ..p });
        self.lay.rep.insert(id.to_string(), id.to_string());
        self.lay.vis.push(id.to_string());
    }

    fn weight(&mut self, id: &str) -> f64 {
        weight(self.idx, id, &mut self.wc)
    }

    fn place_files(&mut self, files: &[String], a0: f64, a1: f64, r0: f64, depth: i32) {
        let span = (a1 - a0).max(0.4);
        let n = files.len() as f64;
        let rows = ((n * SPACING / (rad(span) * r0)).ceil()).max(1.0);
        for (k, f) in files.iter().enumerate() {
            let kf = k as f64;
            self.set_pos(
                f,
                a0 + span * (kf + 0.5) / n,
                r0 + (kf % rows) * ROW,
                P {
                    depth,
                    ..Default::default()
                },
            );
        }
    }

    fn place(&mut self, id: &str, a0: f64, a1: f64, r: f64, depth: i32) {
        let idx = self.idx;
        let (end, label) = chain_end(idx, id);
        let mid = (a0 + a1) / 2.0;
        let chain = if id != end {
            Some(id.to_string())
        } else {
            None
        };
        self.set_pos(
            &end,
            mid,
            r,
            P {
                depth,
                label: Some(label),
                chain,
                ..Default::default()
            },
        );
        let mut c = id.to_string();
        while c != end {
            self.lay.rep.insert(c.clone(), end.clone());
            let (dirs, _) = idx.sorted_kids(&c, false);
            c = dirs[0].clone();
        }
        if is_collapsed(idx, &end) {
            fn hide(idx: &Idx, x: &str, end: &str, rep: &mut HashMap<String, String>) {
                for k in idx.kids_of(x) {
                    rep.insert(k.clone(), end.to_string());
                    if idx.is_dir(k) {
                        hide(idx, k, end, rep);
                    }
                }
            }
            hide(idx, &end, &end, &mut self.lay.rep);
            self.lay.pos.get_mut(&end).unwrap().cluster = true;
            return;
        }
        let (dirs, files) = idx.sorted_kids(&end, false);
        let wf = files.len() as f64 * 0.7;
        let mut wd = 0.0;
        for d in dirs {
            wd += self.weight(d);
        }
        let tw = if wf + wd != 0.0 { wf + wd } else { 1.0 };
        let pad_a = ((a1 - a0) * 0.05).min(1.0);
        let (aa0, aa1) = (a0 + pad_a, a1 - pad_a);
        let fa1 = aa0 + (aa1 - aa0) * (wf / tw);
        if !files.is_empty() {
            self.place_files(files, aa0, fa1, r + dr_file(depth), depth + 1);
        }
        let mut cur = fa1;
        for d in dirs {
            let span = (aa1 - aa0) * self.weight(d) / tw;
            self.place(d, cur, cur + span, r + dr_dir(depth + 1), depth + 1);
            cur += span;
        }
    }
}

/// `layout()`
pub fn layout(idx: &Idx) -> Layout {
    let mut pl = Placer {
        idx,
        lay: Layout::default(),
        wc: HashMap::new(),
    };
    pl.set_pos(
        ".",
        0.0,
        0.0,
        P {
            depth: 0,
            ..Default::default()
        },
    );
    {
        let p = pl.lay.pos.get_mut(".").unwrap();
        p.x = 0.0;
        p.y = 0.0;
    }
    let (tdirs, tfiles) = idx.sorted_kids(".", false);
    let nf = tfiles.len().max(1) as f64;
    for (i, f) in tfiles.iter().enumerate() {
        pl.set_pos(
            f,
            200.0 + i as f64 * (320.0 / nf),
            R_ROOTFILES,
            P {
                depth: 1,
                ..Default::default()
            },
        );
    }
    let mut w = 0.0;
    for d in tdirs {
        w += pl.weight(d);
    }
    if w == 0.0 {
        w = 1.0;
    }
    let gap = if tdirs.len() > 1 { 1.6 } else { 0.0 };
    let total = 360.0 - gap * tdirs.len() as f64;
    let mut a = 0.0;
    for d in tdirs {
        let span = total * pl.weight(d) / w;
        pl.place(d, a, a + span, R_TOP, 1);
        a += span + gap;
    }
    let mut lay = pl.lay;
    for n in &idx.nodes {
        if !lay.rep.contains_key(&n.id) && n.t != "surface" {
            lay.rep.insert(n.id.clone(), ".".into());
        }
    }
    // surfaces on the outer ring, each at the circular mean of what reaches it
    let mut radii: Vec<f64> = lay.vis.iter().map(|id| lay.pos[id].r).collect();
    radii.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let p92 = radii
        .get((radii.len() as f64 * 0.92).floor() as usize)
        .copied()
        .filter(|v| *v != 0.0)
        .unwrap_or(R_TOP);
    let r_out = (R_TOP + 230.0).max(p92 + 74.0);
    let mut want: Vec<(String, f64)> = idx
        .nodes
        .iter()
        .filter(|n| n.t == "surface")
        .map(|n| {
            let (mut sx, mut sy, mut cnt) = (0.0, 0.0, 0.0);
            for &ei in idx.in_of(&n.id) {
                let e = &idx.edges[ei];
                if let Some(p) = lay.rep.get(&e.s).and_then(|r| lay.pos.get(r)) {
                    if p.r > 0.0 {
                        sx += rad(p.a).sin() * e.n;
                        sy += rad(p.a).cos() * e.n;
                        cnt += e.n;
                    }
                }
            }
            let ang = if cnt != 0.0 {
                (sx.atan2(sy) * 180.0 / PI + 360.0) % 360.0
            } else {
                0.0
            };
            (n.id.clone(), ang)
        })
        .collect();
    want.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(std::cmp::Ordering::Equal));
    let min_sep = (360.0 / want.len().max(1) as f64 * 0.8).min(26.0);
    for _ in 0..60 {
        let mut moved = false;
        for i in 0..want.len() {
            let j = (i + 1) % want.len();
            if want.len() < 2 {
                break;
            }
            let d = (want[j].1 - want[i].1 + 360.0) % 360.0;
            if d < min_sep {
                let push = (min_sep - d) / 2.0;
                want[i].1 = (want[i].1 - push + 360.0) % 360.0;
                want[j].1 = (want[j].1 + push) % 360.0;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    for (id, a) in &want {
        let (x, y) = polar(*a, r_out);
        lay.pos.insert(
            id.clone(),
            P {
                x,
                y,
                a: *a,
                r: r_out,
                depth: 1,
                ..Default::default()
            },
        );
        lay.rep.insert(id.clone(), id.clone());
        lay.vis.push(id.clone());
    }
    // visible edges, aggregated by representative
    let mut ve: indexmap::IndexMap<String, VEdge> = indexmap::IndexMap::new();
    for id in &lay.vis {
        let n = idx.node(id).unwrap();
        if n.t == "surface" || id == "." {
            continue;
        }
        let p = match &lay.pos[id].chain {
            Some(ch) => idx.node(ch).and_then(|x| x.parent.clone()),
            None => n.parent.clone(),
        };
        let p = p.unwrap_or_else(|| ".".into());
        let rp = lay.rep.get(&p).cloned().unwrap_or_else(|| ".".into());
        if &rp != id {
            ve.insert(
                format!("{rp}|{id}|tree"),
                VEdge {
                    s: rp,
                    t: id.clone(),
                    g: "tree",
                    c: 1.0,
                },
            );
        }
    }
    for e in &idx.edges {
        let Some(rs) = lay.rep.get(&e.s) else {
            continue;
        };
        let rt = match lay.rep.get(&e.t) {
            Some(r) => r,
            None if lay.pos.contains_key(&e.t) => &e.t,
            None => continue,
        };
        if rs == rt {
            continue;
        }
        let g = edge_group(&e.k);
        let key = format!("{rs}|{rt}|{g}");
        if let Some(v) = ve.get_mut(&key) {
            v.c += e.n;
        } else {
            ve.insert(
                key,
                VEdge {
                    s: rs.clone(),
                    t: rt.clone(),
                    g,
                    c: e.n,
                },
            );
        }
    }
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for id in &lay.vis {
        let p = &lay.pos[id];
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    let m = 70.0;
    lay.bbox = [
        x0 - m - 40.0,
        y0 - m,
        (x1 - x0) + 2.0 * m + 80.0,
        (y1 - y0) + 2.0 * m,
    ];
    lay.vedges = ve.into_values().collect();
    lay.r_out = r_out;
    lay
}

pub struct LabelGeom {
    pub x: f64,
    pub y: f64,
    pub rot: f64,
    pub anchor: &'static str,
}

fn label_geom(phi: f64, d: f64) -> LabelGeom {
    let q = ((phi % 360.0) + 360.0) % 360.0;
    let (x, y) = (rad(q).sin() * d, -rad(q).cos() * d);
    if q < 180.0 {
        LabelGeom {
            x,
            y,
            rot: q - 90.0,
            anchor: "start",
        }
    } else {
        LabelGeom {
            x,
            y,
            rot: q - 270.0,
            anchor: "end",
        }
    }
}

/// `shapePath(shape, r)`: SVG path data (the shapes the legend draws).
pub fn shape_path(shape: &str, r: f64) -> String {
    match shape {
        "square" | "hsquare" => format!(
            "M{},{}h{}v{}h{}Z",
            num(-r),
            num(-r),
            num(2.0 * r),
            num(2.0 * r),
            num(-2.0 * r)
        ),
        "diamond" => format!(
            "M0,{}L{},0L0,{}L{},0Z",
            num(-r * 1.25),
            num(r * 1.25),
            num(r * 1.25),
            num(-r * 1.25)
        ),
        "triangle" => format!(
            "M0,{}L{},{}L{},{}Z",
            num(-r * 1.2),
            num(r * 1.1),
            num(r * 0.8),
            num(-r * 1.1),
            num(r * 0.8)
        ),
        "hex" | "hexo" => {
            let mut d = String::new();
            for i in 0..6 {
                let a = PI / 6.0 + i as f64 * PI / 3.0;
                d.push(if i > 0 { 'L' } else { 'M' });
                d.push_str(&to_fixed(a.cos() * r, 2));
                d.push(',');
                d.push_str(&to_fixed(a.sin() * r, 2));
            }
            d + "Z"
        }
        _ => format!(
            "M{},0A{},{} 0 1 1 {},0A{},{} 0 1 1 {},0Z",
            num(r),
            num(r),
            num(r),
            num(-r),
            num(r),
            num(r),
            num(r)
        ),
    }
}

/// `curve(p, q, g)`
fn curve(p: (f64, f64), q: (f64, f64), g: &str) -> String {
    if g == "tree" {
        return format!(
            "M{},{}L{},{}",
            to_fixed(p.0, 1),
            to_fixed(p.1, 1),
            to_fixed(q.0, 1),
            to_fixed(q.1, 1)
        );
    }
    let (cx, cy) = ((p.0 + q.0) * 0.25, (p.1 + q.1) * 0.25);
    format!(
        "M{},{}Q{},{} {},{}",
        to_fixed(p.0, 1),
        to_fixed(p.1, 1),
        to_fixed(cx, 1),
        to_fixed(cy, 1),
        to_fixed(q.0, 1),
        to_fixed(q.1, 1)
    )
}

// ---- what gets drawn -------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Root,
    Surf,
    Dir,
    File,
}

struct VNode {
    id: String,
    x: f64,
    y: f64,
    kind: Kind,
    d1: bool,
    fam: String,
    shape: Path2d,
    fill: &'static str, // "accent" | "bg" | "fam"
    /// records and external systems: outlined in the family color, filled with the background
    hollow: bool,
    sw: f64,
    r: f64,
    halo_r: f64,
    ring_r: Option<f64>,
    cnt: Option<(String, bool)>,
    dot: Option<(f64, f64)>,
    label: String,
    lg: LabelGeom,
    /// the label offset before rounding (for declutter)
    lg_raw: (f64, f64),
    fs: f64,
    // label segment data for declutter
    ux: f64,
    uy: f64,
    len: f64,
    degree: f64,
    depth: i32,
}

struct Hot {
    path: Path2d,
    out: bool,
    g: &'static str,
    arrow: Option<(f64, f64, f64)>,
}

struct Colors {
    accent: String,
    accent_text: String,
    bg: String,
    ink: String,
    ink2: String,
    warn: String,
    edge: String,
    edge_tree: String,
    edge_hot: String,
    line: String,
    mono: String,
    fam: HashMap<String, String>,
}

impl Colors {
    fn read() -> Colors {
        let v = theme::css_var;
        let mut fam = HashMap::new();
        for f in [
            "core",
            "cards",
            "rules",
            "memory",
            "records",
            "workspace",
            "other",
            "world",
        ] {
            fam.insert(f.to_string(), v(&format!("--f-{f}")));
        }
        Colors {
            accent: v("--accent"),
            accent_text: v("--accent-text"),
            bg: v("--graph-bg"),
            ink: v("--ink"),
            ink2: v("--ink-2"),
            warn: v("--warn"),
            edge: v("--edge"),
            edge_tree: v("--edge-tree"),
            edge_hot: v("--edge-hot"),
            line: v("--line"),
            mono: v("--mono"),
            fam,
        }
    }
    fn fam(&self, f: &str) -> &str {
        self.fam.get(f).map(|s| s.as_str()).unwrap_or("#888")
    }
}

struct Drag {
    x: f64,
    y: f64,
    moved: bool,
}

struct Anim {
    k0: f64,
    x0: f64,
    y0: f64,
    k: f64,
    x: f64,
    y: f64,
    t0: f64,
    ms: f64,
}

pub struct G {
    canvas: Option<HtmlCanvasElement>,
    ctx: Option<CanvasRenderingContext2d>,
    pub k: f64,
    pub x: f64,
    pub y: f64,
    pub lay: Rc<Layout>,
    nodes: Vec<VNode>,
    at: HashMap<String, usize>,
    colors: Option<Colors>,
    base: Vec<(Path2d, &'static str)>,
    hot: Vec<Hot>,
    sel_vis: Option<usize>,
    nbr: Vec<bool>,
    spotlit: bool,
    spot: Option<(f64, f64, f64)>,
    shown: Vec<bool>,
    pub sk: f64,
    lab_sk: f64,
    decl_t: f64,
    decl_timer: i32,
    hover: Option<usize>,
    /// the node the keyboard walk is on (see `walk`)
    cursor: Option<usize>,
    flash: Vec<(usize, f64)>,
    w: f64,
    h: f64,
    dpr: f64,
    host_rect: (f64, f64, f64, f64),
    raf: i32,
    anim: Option<Anim>,
    drag: Option<Drag>,
    built: bool,
    /// offscreen layers for the spotlight's two dim levels (empty while nothing is spotlit)
    layers: Vec<(HtmlCanvasElement, CanvasRenderingContext2d)>,
}

thread_local! {
    static GS: &'static std::cell::RefCell<G> = Box::leak(Box::new(std::cell::RefCell::new(G {
        canvas: None, ctx: None, k: 1.0, x: 0.0, y: 0.0, lay: Rc::new(Layout::default()), nodes: vec![], at: HashMap::new(),
        colors: None, base: vec![], hot: vec![], sel_vis: None, nbr: vec![], spotlit: false, spot: None, shown: vec![],
        sk: 1.0, lab_sk: 0.0, decl_t: 0.0, decl_timer: 0, hover: None, cursor: None, flash: vec![], w: 0.0, h: 0.0, dpr: 1.0,
        host_rect: (0.0, 0.0, 0.0, 0.0), raf: 0, anim: None, drag: None, built: false, layers: vec![],
    })));
}

fn g() -> std::cell::RefMut<'static, G> {
    GS.with(|g| *g).borrow_mut()
}

/// The current layout (rep, pos, vis), for the rest of the client.
pub fn lay() -> Rc<Layout> {
    g().lay.clone()
}

pub fn rep_of(id: &str) -> String {
    lay().rep_or(id).to_string()
}

// ---- building ------------------------------------------------------------------------------

/// `buildGraph(flash)`
pub fn build(flash: &[String]) {
    let Some(idx) = state::idx() else { return };
    let lay = Rc::new(layout(&idx));
    let dirty = idx.dirty();
    let mut nodes = Vec::with_capacity(lay.vis.len());
    let mut at = HashMap::with_capacity(lay.vis.len());
    for id in &lay.vis {
        let n = idx.node(id).unwrap();
        let p = &lay.pos[id];
        let fam = n.fam().to_string();
        let (kind, shape, r) = match n.t.as_str() {
            "root" => (Kind::Root, "diamond", 8.0),
            "surface" => (Kind::Surf, "hexo", 8.5),
            "dir" => {
                let mut r = if p.depth == 1 { 8.0 } else { 5.6 };
                if p.cluster {
                    let cnt = idx.dinfo(id).map(|d| num(d.files)).unwrap_or_default();
                    r = if cnt.chars().count() >= 3 { 9.5 } else { 7.5 };
                }
                (Kind::Dir, "circle", r)
            }
            _ => (
                Kind::File,
                shape_of(&fam).unwrap_or("dot"),
                2.9 + (idx.degree(id).sqrt() * 0.55).min(3.4),
            ),
        };
        let cnt = if kind == Kind::Dir && p.cluster {
            let c = idx.dinfo(id).map(|d| num(d.files)).unwrap_or_default();
            let c3 = c.chars().count() >= 3;
            Some((c, c3))
        } else {
            None
        };
        let hollow = shape == "hsquare" || shape == "hexo";
        let fill = if kind == Kind::Root {
            "accent"
        } else if hollow || kind == Kind::Surf {
            "bg"
        } else {
            "fam"
        };
        // CSS: .n .shape 1.2; .n.dir/.n.root .shape 1.5; .n.surf .shape 2 (overrides the attributes)
        let sw = match kind {
            Kind::Surf => 2.0,
            Kind::Dir | Kind::Root => 1.5,
            Kind::File => 1.2,
        };
        let is_dirty = kind == Kind::File && dirty.get(id).map(|s| !s.is_empty()).unwrap_or(false);
        let lg = if id == "." {
            LabelGeom {
                x: 0.0,
                y: r + 13.0,
                rot: 0.0,
                anchor: "middle",
            }
        } else if kind == Kind::Surf {
            let (sa, ca) = (rad(p.a).sin(), rad(p.a).cos());
            let anchor = if sa > 0.25 {
                "start"
            } else if sa < -0.25 {
                "end"
            } else {
                "middle"
            };
            let mut lg = LabelGeom {
                x: sa * (r + 5.0),
                y: -ca * (r + 5.0),
                rot: 0.0,
                anchor,
            };
            if anchor == "middle" {
                lg.y = if ca > 0.0 { -(r + 9.0) } else { r + 9.0 };
            }
            lg
        } else if kind == Kind::Dir && !p.cluster {
            label_geom(p.a + 180.0, r + 4.0)
        } else {
            label_geom(p.a, r + if p.cluster { 7.5 } else { 3.5 })
        };
        let label = p.label.clone().unwrap_or_else(|| idx.name_of(id));
        let fs = if id == "." {
            13.0
        } else if kind == Kind::Surf {
            12.0
        } else if kind == Kind::Dir {
            if p.depth == 1 { 12.0 } else { 11.0 }
        } else {
            10.5
        };
        // label geometry exactly as the SVG had it (attributes were written with toFixed(1))
        let lgx = js::parse_float(&to_fixed(lg.x, 1));
        let lgy = js::parse_float(&to_fixed(lg.y, 1));
        let rot = js::parse_float(&to_fixed(lg.rot, 1));
        let shape_d = shape_path(shape, r);
        at.insert(id.clone(), nodes.len());
        nodes.push(VNode {
            id: id.clone(),
            x: js::parse_float(&to_fixed(p.x, 1)),
            y: js::parse_float(&to_fixed(p.y, 1)),
            kind,
            d1: kind == Kind::Dir && p.depth == 1,
            fam,
            shape: Path2d::new_with_path_string(&shape_d).unwrap(),
            fill,
            hollow,
            sw,
            r,
            halo_r: js::parse_float(&to_fixed(r + 4.0, 1)),
            ring_r: if cnt.is_some() { Some(r + 3.6) } else { None },
            cnt,
            dot: if is_dirty {
                Some((
                    js::parse_float(&to_fixed(r * 0.9, 1)),
                    js::parse_float(&to_fixed(-r * 0.9, 1)),
                ))
            } else {
                None
            },
            ux: rad(lg.rot).cos(),
            uy: rad(lg.rot).sin(),
            len: js::len16(&label) as f64,
            label,
            lg_raw: (lg.x, lg.y),
            lg: LabelGeom {
                x: lgx,
                y: lgy,
                rot,
                anchor: lg.anchor,
            },
            fs,
            degree: idx.degree(id),
            depth: p.depth,
        });
    }
    // base edges: one path per group
    let mut groups: [(String, &'static str); 3] = [
        (String::new(), "tree"),
        (String::new(), "link"),
        (String::new(), "surface"),
    ];
    for e in &lay.vedges {
        if let (Some(p), Some(q)) = (lay.pos.get(&e.s), lay.pos.get(&e.t)) {
            let gi = match e.g {
                "tree" => 0,
                "link" => 1,
                _ => 2,
            };
            groups[gi].0.push_str(&curve((p.x, p.y), (q.x, q.y), e.g));
        }
    }
    let base = groups
        .iter()
        .map(|(d, gname)| (Path2d::new_with_path_string(d).unwrap(), *gname))
        .collect();
    let now = dom::perf_now();
    {
        let mut gs = g();
        gs.lay = lay.clone();
        gs.flash.clear();
        for id in flash {
            let r = lay.rep_or(id).to_string();
            if let Some(&i) = at.get(&r) {
                gs.flash.push((i, now));
            }
        }
        gs.shown = vec![false; nodes.len()];
        gs.nodes = nodes;
        gs.at = at;
        gs.base = base;
        gs.colors = Some(Colors::read());
        gs.lab_sk = 0.0;
        gs.hover = None;
        gs.cursor = None;
        gs.built = true;
    }
    if let Some(t) = walk_target() {
        let mut gs = g();
        let rep = gs.lay.rep_or(&t).to_string();
        gs.cursor = gs.at.get(&rep).copied();
    }
    apply_view();
    update();
    render_legend(&idx);
}

fn render_legend(idx: &Idx) {
    let mut html = String::new();
    for f in &idx.families {
        if !idx
            .nodes
            .iter()
            .any(|n| n.family.as_deref() == Some(&f.id) && n.t != "root")
        {
            continue;
        }
        let shape = shape_of(&f.id).unwrap_or("dot");
        let hollow = shape == "hsquare" || shape == "hexo";
        html.push_str(&format!(
            "<span><svg width=\"12\" height=\"12\" viewBox=\"-6 -6 12 12\"><path d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"/></svg>{}</span>",
            shape_path(shape, 4.0),
            if hollow { "none".to_string() } else { fam_var(&f.id) },
            if hollow { fam_var(&f.id) } else { "none".to_string() },
            esc(&f.label)
        ));
    }
    html.push_str("<span style=\"color:var(--faint)\">◌ collapsed folder · double-click or E to expand</span>");
    dom::set_html(&dom::id("glegend"), &html);
}

// ---- view transform --------------------------------------------------------------------------

fn screen_scale(gs: &G) -> f64 {
    let (w, h) = if gs.w > 0.0 {
        (gs.w, gs.h)
    } else {
        (800.0, 600.0)
    };
    let b = gs.lay.bbox;
    if b[2] <= 0.0 || b[3] <= 0.0 {
        return 1.0;
    }
    (w / b[2]).min(h / b[3])
}

/// `applyView()`
pub fn apply_view() {
    let need = {
        let mut gs = g();
        let sk = screen_scale(&gs) * gs.k;
        gs.sk = sk;
        (sk / if gs.lab_sk != 0.0 { gs.lab_sk } else { 1.0 } - 1.0).abs() > 0.03
    };
    if need {
        declutter_soon();
    }
    schedule();
}

/// The viewBox mapping (xMidYMid meet): user units -> CSS px in the canvas.
fn frame(gs: &G) -> (f64, f64, f64) {
    let b = gs.lay.bbox;
    let s = screen_scale(gs);
    let (w, h) = (gs.w, gs.h);
    let ox = (w - b[2] * s) / 2.0 - b[0] * s;
    let oy = (h - b[3] * s) / 2.0 - b[1] * s;
    (s, ox, oy)
}

/// Client coordinates to viewBox user units (`svgPoint`).
fn user_point(gs: &G, cx: f64, cy: f64) -> (f64, f64) {
    let (s, ox, oy) = frame(gs);
    (
        (cx - gs.host_rect.0 - ox) / s,
        (cy - gs.host_rect.1 - oy) / s,
    )
}

/// Client coordinates to world (layout) units.
fn world_point(gs: &G, cx: f64, cy: f64) -> (f64, f64) {
    let (ux, uy) = user_point(gs, cx, cy);
    ((ux - gs.x) / gs.k, (uy - gs.y) / gs.k)
}

fn animate_to(k: f64, x: f64, y: f64, ms: f64) {
    {
        let mut gs = g();
        gs.anim = Some(Anim {
            k0: gs.k,
            x0: gs.x,
            y0: gs.y,
            k,
            x,
            y,
            t0: dom::perf_now(),
            ms,
        });
    }
    schedule();
}

/// `zoomBy(f, cx, cy)` (cx, cy in user units; the middle of the picture when absent)
pub fn zoom_by(f: f64, c: Option<(f64, f64)>) {
    {
        let mut gs = g();
        gs.anim = None;
        let k = (gs.k * f).clamp(0.4, 12.0);
        let b = gs.lay.bbox;
        let (cx, cy) = c.unwrap_or((b[0] + b[2] / 2.0, b[1] + b[3] / 2.0));
        gs.x = cx - (cx - gs.x) * (k / gs.k);
        gs.y = cy - (cy - gs.y) * (k / gs.k);
        gs.k = k;
    }
    apply_view();
}

/// `focusNode(id, k)`
pub fn focus_node(id: &str, k: Option<f64>) {
    let (target, kk) = {
        let gs = g();
        let rid = gs.lay.rep_or(id).to_string();
        let Some(p) = gs.lay.pos.get(&rid) else {
            return;
        };
        let b = gs.lay.bbox;
        let (cx, cy) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
        let kk = k.unwrap_or(gs.k.max(1.7));
        ((cx - p.x * kk, cy - p.y * kk), kk)
    };
    animate_to(kk, target.0, target.1, 420.0);
}

pub fn fit() {
    animate_to(1.0, 0.0, 0.0, 420.0);
}

pub fn view_k() -> f64 {
    g().k
}

// ---- selection: neighbors, hot edges, spotlight ------------------------------------------------

/// `nbrsOf(id)`: representatives around `id` (and how they connect).
pub fn nbrs_of(idx: &Idx, lay: &Layout, id: Option<&str>) -> HashSet<String> {
    let mut set = HashSet::new();
    let Some(id) = id else { return set };
    let r = lay.rep_or(id).to_string();
    let mut add = |x: &str| {
        let rx = match lay.rep.get(x) {
            Some(rx) => Some(rx.clone()),
            None if lay.pos.contains_key(x) => Some(x.to_string()),
            None => None,
        };
        if let Some(rx) = rx {
            if rx != r {
                set.insert(rx);
            }
        }
    };
    for &ei in idx.out_of(id) {
        add(&idx.edges[ei].t);
    }
    for &ei in idx.in_of(id) {
        add(&idx.edges[ei].s);
    }
    if let Some(n) = idx.node(id) {
        if let Some(p) = &n.parent {
            add(p);
        }
        if idx.is_dir(id) && !is_collapsed(idx, id) {
            for k in idx.kids_of(id) {
                add(k);
            }
        }
    }
    set
}

/// `hotEdges(id)`: the highlighted edges around `id` (the selection, or what the pointer is on).
fn hot_edges(idx: &Idx, gs: &mut G, center: Option<&str>) {
    gs.hot.clear();
    let Some(center) = center else { return };
    let lay = gs.lay.clone();
    let r = lay.rep_or(center).to_string();
    let Some(p) = lay.pos.get(&r) else { return };
    let mut seen = HashSet::new();
    let mut draw = |other: &str, out: bool, gname: &'static str, hot: &mut Vec<Hot>| {
        let Some(q) = lay.pos.get(other) else { return };
        if other == r {
            return;
        }
        let key = format!("{other}{}{gname}", if out { "out" } else { "in" });
        if !seen.insert(key) {
            return;
        }
        let d = if out {
            curve((p.x, p.y), (q.x, q.y), gname)
        } else {
            curve((q.x, q.y), (p.x, p.y), gname)
        };
        let arrow = if out && gname != "tree" {
            // marker-end on M p Q c q: oriented along c -> q
            let f1 = |v: f64| js::parse_float(&to_fixed(v, 1));
            let (cx, cy) = (f1((p.x + q.x) * 0.25), f1((p.y + q.y) * 0.25));
            let (ex, ey) = (f1(q.x), f1(q.y));
            Some((ex, ey, (ey - cy).atan2(ex - cx)))
        } else {
            None
        };
        hot.push(Hot {
            path: Path2d::new_with_path_string(&d).unwrap(),
            out,
            g: gname,
            arrow,
        });
    };
    let mut hot = Vec::new();
    for &ei in idx.out_of(center) {
        let e = &idx.edges[ei];
        draw(lay.rep_or(&e.t), true, edge_group(&e.k), &mut hot);
    }
    for &ei in idx.in_of(center) {
        let e = &idx.edges[ei];
        draw(lay.rep_or(&e.s), false, edge_group(&e.k), &mut hot);
    }
    if let Some(n) = idx.node(center) {
        if let Some(par) = &n.parent {
            draw(lay.rep_or(par), false, "tree", &mut hot);
        }
    }
    if idx.is_dir(center) && !is_collapsed(idx, center) {
        for k in idx.kids_of(center) {
            draw(lay.rep_or(k), true, "tree", &mut hot);
        }
    }
    gs.hot = hot;
}

/// `updateGraph()`: selection classes, spotlight, hot edges, labels, hint.
pub fn update() {
    let Some(idx) = state::idx() else { return };
    let sel = state::sel();
    let hint = {
        let mut gs = g();
        if !gs.built {
            return;
        }
        let lay = gs.lay.clone();
        let nb = nbrs_of(&idx, &lay, sel.as_deref());
        let rsel = sel.as_deref().map(|s| lay.rep_or(s).to_string());
        gs.spotlit = sel.is_some();
        gs.sel_vis = rsel.as_ref().and_then(|r| gs.at.get(r).copied());
        let nbr: Vec<bool> = gs.nodes.iter().map(|n| nb.contains(&n.id)).collect();
        gs.nbr = nbr;
        gs.spot = match (&rsel, rsel.as_ref().and_then(|r| lay.pos.get(r))) {
            (Some(r), Some(p)) => {
                let big = idx.is_dir(r) || idx.node(r).map(|n| n.t == "surface").unwrap_or(false);
                Some((p.x, p.y, if big { 210.0 } else { 150.0 }))
            }
            _ => None,
        };
        hot_edges(&idx, &mut gs, sel.as_deref());
        match (&sel, &rsel) {
            (Some(s), Some(r)) if s != r => {
                format!("{} is collapsed · E to expand", idx.name_of(r))
            }
            _ => String::new(),
        }
    };
    declutter();
    set_hint(&hint);
    schedule();
}

fn set_hint(t: &str) {
    let el = match dom::try_id("ghint") {
        Some(el) => el,
        None => {
            let el = dom::doc().create_element("div").unwrap();
            el.set_id("ghint");
            el.set_class_name("gtip");
            let _ = el.set_attribute("style", "left:auto;right:10px;top:10px;pointer-events:none");
            let _ = dom::id("v-graph").append_child(&el);
            el
        }
    };
    dom::set_hidden(&el, t.is_empty());
    dom::set_text(&el, t);
}

// ---- labels: greedy placement in screen space ------------------------------------------------

const CW: f64 = 0.61;

fn pt_seg(px: f64, py: f64, s: &[f64; 5]) -> f64 {
    let (dx, dy) = (s[2] - s[0], s[3] - s[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 != 0.0 {
        (((px - s[0]) * dx + (py - s[1]) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (s[0] + t * dx - px).hypot(s[1] + t * dy - py)
}

fn seg_cross(a: &[f64; 5], b: &[f64; 5]) -> bool {
    let o = |ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64| {
        (bx - ax) * (cy - ay) - (by - ay) * (cx - ax) > 0.0
    };
    o(b[0], b[1], b[2], b[3], a[0], a[1]) != o(b[0], b[1], b[2], b[3], a[2], a[3])
        && o(a[0], a[1], a[2], a[3], b[0], b[1]) != o(a[0], a[1], a[2], a[3], b[2], b[3])
}

fn seg_dist(a: &[f64; 5], b: &[f64; 5]) -> f64 {
    if seg_cross(a, b) {
        return 0.0;
    }
    pt_seg(a[0], a[1], b)
        .min(pt_seg(a[2], a[3], b))
        .min(pt_seg(b[0], b[1], a))
        .min(pt_seg(b[2], b[3], a))
}

/// `declutterSoon()`: at most every 140 ms while zooming, and once more 90 ms after.
fn declutter_soon() {
    let now = dom::perf_now();
    let (last, timer) = {
        let gs = g();
        (gs.decl_t, gs.decl_timer)
    };
    dom::clear_timeout(timer);
    if now - last > 140.0 {
        g().decl_t = now;
        declutter();
    }
    let h = dom::timeout(90, || {
        g().decl_t = dom::perf_now();
        declutter();
        schedule();
    });
    g().decl_timer = h;
}

/// `declutter()`: which labels show.
pub fn declutter() {
    let Some(idx) = state::idx() else { return };
    let all = cfg().labels;
    let sel = state::sel();
    let mut gs = g();
    if gs.nodes.is_empty() {
        return;
    }
    let sk = if gs.sk != 0.0 { gs.sk } else { 1.0 };
    gs.lab_sk = sk;
    let lay = gs.lay.clone();
    let rsel = sel.as_deref().map(|s| lay.rep_or(s).to_string());
    let nb = if sel.is_some() {
        nbrs_of(&idx, &lay, sel.as_deref())
    } else {
        HashSet::new()
    };
    let mut items: Vec<(i32, f64, usize)> = Vec::new();
    for (i, n) in gs.nodes.iter().enumerate() {
        let pr = if Some(&n.id) == rsel.as_ref() || n.id == "." {
            0
        } else if n.kind == Kind::Surf || (n.kind == Kind::Dir && n.depth == 1) {
            1
        } else if nb.contains(&n.id) {
            if n.kind == Kind::Dir { 2 } else { 3 }
        } else if n.kind == Kind::Dir {
            if rsel.is_some() { 6 } else { 2 }
        } else if all {
            5
        } else if rsel.is_some() || sk < 1.05 {
            continue;
        } else {
            5
        };
        items.push((pr, -n.degree, i));
    }
    items.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    const CELL: f64 = 56.0;
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let mut segs: Vec<[f64; 5]> = Vec::new();
    let mut shown = vec![false; gs.nodes.len()];
    let mut stamp = 0u32;
    let mut seen: HashMap<usize, u32> = HashMap::new();
    let cells = |s: &[f64; 5], pad: f64| {
        let x0 = ((s[0].min(s[2]) - pad) / CELL).floor() as i64;
        let x1 = ((s[0].max(s[2]) + pad) / CELL).floor() as i64;
        let y0 = ((s[1].min(s[3]) - pad) / CELL).floor() as i64;
        let y1 = ((s[1].max(s[3]) + pad) / CELL).floor() as i64;
        (x0, x1, y0, y1)
    };
    for (pr, _, i) in items {
        let n = &gs.nodes[i];
        let p = &lay.pos[&n.id];
        // measured from the layout position plus the label offset (unrounded)
        let (ox, oy) = n.lg_raw;
        let (ax, ay) = ((p.x + ox) * sk, (p.y + oy) * sk);
        let len = n.len * n.fs * CW;
        let f = match n.lg.anchor {
            "start" => [0.0, 1.0],
            "end" => [-1.0, 0.0],
            _ => [-0.5, 0.5],
        };
        let seg = [
            ax + n.ux * len * f[0],
            ay + n.uy * len * f[0],
            ax + n.ux * len * f[1],
            ay + n.uy * len * f[1],
            n.fs,
        ];
        let (x0, x1, y0, y1) = cells(&seg, n.fs);
        if pr > 0 && !all {
            stamp += 1;
            let mut ok = true;
            'outer: for gx in x0..=x1 {
                for gy in y0..=y1 {
                    if let Some(list) = grid.get(&(gx, gy)) {
                        for &j in list {
                            if seen.get(&j) == Some(&stamp) {
                                continue;
                            }
                            seen.insert(j, stamp);
                            let o = &segs[j];
                            if seg_dist(&seg, o) < (seg[4] + o[4]) * 0.46 {
                                ok = false;
                                break 'outer;
                            }
                        }
                    }
                }
            }
            if !ok {
                continue;
            }
        }
        let k = segs.len();
        segs.push(seg);
        for gx in x0..=x1 {
            for gy in y0..=y1 {
                grid.entry((gx, gy)).or_default().push(k);
            }
        }
        shown[i] = true;
    }
    gs.shown = shown;
    drop(gs);
    schedule();
}

// ---- drawing -------------------------------------------------------------------------------

/// Ask for one frame (coalesces any number of requests).
pub fn schedule() {
    let mut gs = g();
    if gs.raf != 0 || gs.canvas.is_none() {
        return;
    }
    gs.raf = dom::raf(|t| {
        g().raf = 0;
        frame_tick(t);
    });
}

fn frame_tick(now: f64) {
    // animation step (animateTo)
    let more = {
        let mut gs = g();
        if let Some(a) = &gs.anim {
            let u = ((now - a.t0) / a.ms).clamp(0.0, 1.0);
            let e = 1.0 - (1.0 - u).powi(3);
            let (k, x, y) = (
                a.k0 + (a.k - a.k0) * e,
                a.x0 + (a.x - a.x0) * e,
                a.y0 + (a.y - a.y0) * e,
            );
            let done = u >= 1.0;
            gs.k = k;
            gs.x = x;
            gs.y = y;
            if done {
                gs.anim = None;
            }
            true
        } else {
            false
        }
    };
    if more {
        apply_view();
    }
    let t0 = dom::perf_now();
    draw(now);
    record_draw(dom::perf_now() - t0);
    let again = {
        let mut gs = g();
        gs.flash.retain(|(_, t0)| now - t0 < 3200.0);
        gs.anim.is_some() || !gs.flash.is_empty()
    };
    if again {
        schedule();
    }
}

/// When a test has put an array on `window.__oktDraw`, keep the last few hundred frame times on it
/// (tests/perf/profile.mjs).
fn record_draw(ms: f64) {
    let Ok(arr) = js_sys::Reflect::get(&dom::win(), &JsValue::from_str("__oktDraw"))
        .and_then(|v| v.dyn_into::<js_sys::Array>())
    else {
        return;
    };
    arr.push(&JsValue::from_f64(ms));
    if arr.length() > 600 {
        arr.shift();
    }
}

fn set_ls(ctx: &CanvasRenderingContext2d, v: &str) {
    let _ = js_sys::Reflect::set(
        ctx,
        &JsValue::from_str("letterSpacing"),
        &JsValue::from_str(v),
    );
}

fn ease_out(t: f64) -> f64 {
    // cubic-bezier(0, 0, .58, 1): solve x(s) = t, return y(s)
    let (x1, y1, x2, y2) = (0.0, 0.0, 0.58, 1.0);
    let bez = |s: f64, p1: f64, p2: f64| {
        3.0 * (1.0 - s).powi(2) * s * p1 + 3.0 * (1.0 - s) * s * s * p2 + s * s * s
    };
    let mut lo = 0.0;
    let mut hi = 1.0;
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if bez(mid, x1, x2) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez((lo + hi) / 2.0, y1, y2)
}

fn draw(now: f64) {
    let gs = g();
    let (Some(cv), Some(ctx), Some(c)) = (&gs.canvas, &gs.ctx, &gs.colors) else {
        return;
    };
    if gs.w <= 0.0 || gs.h <= 0.0 {
        return;
    }
    let dpr = gs.dpr;
    let want_w = (gs.w * dpr).round() as u32;
    let want_h = (gs.h * dpr).round() as u32;
    if cv.width() != want_w || cv.height() != want_h {
        cv.set_width(want_w);
        cv.set_height(want_h);
    }
    let (s, ox, oy) = frame(&gs);
    let sk = s * gs.k;
    let ex = ox + gs.x * s;
    let ey = oy + gs.y * s;
    ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
    ctx.set_global_alpha(1.0);
    ctx.clear_rect(0.0, 0.0, want_w as f64, want_h as f64);
    let a = dpr * sk;
    let world = |ctx: &CanvasRenderingContext2d| {
        ctx.set_transform(a, 0.0, 0.0, a, dpr * ex, dpr * ey).ok();
    };
    let dash = |ctx: &CanvasRenderingContext2d, d: &[f64]| {
        let arr = js_sys::Array::new();
        for v in d {
            arr.push(&JsValue::from_f64(*v));
        }
        ctx.set_line_dash(&arr).ok();
    };
    world(ctx);
    // ring guides
    for r in [R_TOP, gs.lay.r_out] {
        ctx.begin_path();
        ctx.arc(0.0, 0.0, r, 0.0, PI * 2.0).ok();
        ctx.set_stroke_style_str(&c.line);
        ctx.set_line_width(0.6);
        dash(ctx, &[1.0, 5.0]);
        ctx.stroke();
    }
    dash(ctx, &[]);
    // spotlight
    if let Some((sx, sy, sr)) = gs.spot {
        if let Ok(gr) = ctx.create_radial_gradient(sx, sy, 0.0, sx, sy, sr) {
            let _ = gr.add_color_stop(0.0, &theme::hex_alpha(&c.accent, 0.22));
            let _ = gr.add_color_stop(0.6, &theme::hex_alpha(&c.accent, 0.07));
            let _ = gr.add_color_stop(1.0, &theme::hex_alpha(&c.accent, 0.0));
            ctx.set_fill_style_canvas_gradient(&gr);
            ctx.begin_path();
            ctx.arc(sx, sy, sr, 0.0, PI * 2.0).ok();
            ctx.fill();
        }
    }
    // base edges
    let eop = if gs.spotlit { 0.35 } else { 1.0 };
    for (path, gname) in &gs.base {
        let (stroke, w, d): (&str, f64, &[f64]) = match *gname {
            "tree" => (&c.edge_tree, 0.8, &[]),
            "link" => (&c.edge, 0.7, &[]),
            _ => (&c.edge, 0.7, &[2.0, 3.0]),
        };
        ctx.set_global_alpha(eop);
        ctx.set_stroke_style_str(stroke);
        ctx.set_line_width(w);
        dash(ctx, d);
        ctx.stroke_with_path(path);
    }
    ctx.set_global_alpha(1.0);
    // hot edges
    for h in &gs.hot {
        let (stroke, w, op) = if h.g == "tree" {
            (&c.edge_hot, 1.0, 0.5)
        } else if h.out {
            (&c.accent, 1.4, 0.95)
        } else {
            (&c.edge_hot, 1.4, 0.95)
        };
        ctx.set_global_alpha(op);
        ctx.set_stroke_style_str(stroke);
        ctx.set_line_width(w);
        dash(ctx, if h.g == "surface" { &[4.0, 3.0] } else { &[] });
        ctx.stroke_with_path(&h.path);
        if let Some((x, y, ang)) = h.arrow {
            dash(ctx, &[]);
            ctx.save();
            ctx.translate(x, y).ok();
            ctx.rotate(ang).ok();
            ctx.scale(0.7, 0.7).ok();
            ctx.translate(-9.0, -5.0).ok();
            ctx.begin_path();
            ctx.move_to(0.0, 0.0);
            ctx.line_to(10.0, 5.0);
            ctx.line_to(0.0, 10.0);
            ctx.close_path();
            ctx.set_fill_style_str(&c.accent);
            ctx.fill();
            ctx.restore();
        }
    }
    dash(ctx, &[]);
    ctx.set_global_alpha(1.0);
    // nodes, each with its label, in document order (later nodes paint over earlier labels).
    // Under the spotlight the other nodes are dimmed with *group* opacity (the node's shape,
    // outline and label composited together), so each dim level is painted at full
    // strength into its own layer and the layer is composited at that opacity, under the lit nodes.
    // labels keep a constant screen size (`scale(var(--ls))`, --ls = clamp(1/sk, .08, 6)); exactly
    // 1 inside the clamp, so the text keeps one transform and its glyphs stay cached while zooming
    let inv = 1.0 / sk;
    let ls_scale = if (0.08..=6.0).contains(&inv) {
        1.0
    } else {
        inv.clamp(0.08, 6.0) * sk
    };
    let mut paint = Painter {
        a,
        dpr,
        ex,
        ey,
        sk,
        ls_scale,
        now,
        font: String::new(),
        ls: "0px".into(),
    };
    let level = |i: usize, n: &VNode| -> f64 {
        let is_sel = gs.sel_vis == Some(i);
        let is_nbr = gs.nbr.get(i).copied().unwrap_or(false);
        if gs.spotlit && !is_sel && !is_nbr {
            if (n.kind == Kind::Dir && n.d1) || n.kind == Kind::Surf {
                0.4
            } else {
                0.16
            }
        } else {
            1.0
        }
    };
    if gs.spotlit {
        for (li, lvl) in [(0usize, 0.16), (1usize, 0.4)] {
            let Some((lc, lctx)) = gs.layers.get(li) else {
                continue;
            };
            if lc.width() != want_w || lc.height() != want_h {
                lc.set_width(want_w);
                lc.set_height(want_h);
            }
            lctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
            lctx.clear_rect(0.0, 0.0, want_w as f64, want_h as f64);
            let mut lp = Painter {
                font: String::new(),
                ls: "0px".into(),
                ..paint.clone_view()
            };
            let mut any = false;
            for (i, n) in gs.nodes.iter().enumerate() {
                if level(i, n) == lvl {
                    lp.node(lctx, &gs, c, i, n);
                    any = true;
                }
            }
            lp.reset(lctx);
            if any {
                ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
                ctx.set_global_alpha(lvl);
                let _ = ctx.draw_image_with_html_canvas_element(lc, 0.0, 0.0);
                ctx.set_global_alpha(1.0);
            }
        }
    } else {
        // the layers are as big as the pane: give their memory back while nothing is spotlit
        // (one layer reused for both levels would do, but costs a copy-on-write per frame)
        for (lc, _) in &gs.layers {
            if lc.width() != 0 {
                lc.set_width(0);
                lc.set_height(0);
            }
        }
    }
    for (i, n) in gs.nodes.iter().enumerate() {
        if level(i, n) == 1.0 {
            paint.node(ctx, &gs, c, i, n);
        }
    }
    paint.reset(ctx);
    ctx.set_global_alpha(1.0);
}

/// Paints one node (halo, shape, ring, count, dirty dot, label) at full group opacity.
struct Painter {
    a: f64,
    dpr: f64,
    ex: f64,
    ey: f64,
    sk: f64,
    ls_scale: f64,
    now: f64,
    font: String,
    ls: String,
}

impl Painter {
    fn clone_view(&self) -> Painter {
        Painter {
            a: self.a,
            dpr: self.dpr,
            ex: self.ex,
            ey: self.ey,
            sk: self.sk,
            ls_scale: self.ls_scale,
            now: self.now,
            font: String::new(),
            ls: "0px".into(),
        }
    }

    fn set_font(&mut self, ctx: &CanvasRenderingContext2d, f: String) {
        if f != self.font {
            ctx.set_font(&f);
            self.font = f;
        }
    }

    fn set_spacing(&mut self, ctx: &CanvasRenderingContext2d, v: &str) {
        if v != self.ls {
            set_ls(ctx, v);
            self.ls = v.into();
        }
    }

    fn reset(&mut self, ctx: &CanvasRenderingContext2d) {
        self.set_spacing(ctx, "0px");
        ctx.set_global_alpha(1.0);
    }

    fn node(&mut self, ctx: &CanvasRenderingContext2d, gs: &G, c: &Colors, i: usize, n: &VNode) {
        let (a, dpr, ex, ey, sk) = (self.a, self.dpr, self.ex, self.ey, self.sk);
        let is_sel = gs.sel_vis == Some(i);
        let is_nbr = gs.nbr.get(i).copied().unwrap_or(false);
        ctx.set_transform(a, 0.0, 0.0, a, dpr * ex + a * n.x, dpr * ey + a * n.y)
            .ok();
        // halo (selection ring; the flash animation after a change)
        let flash = gs
            .flash
            .iter()
            .find(|(fi, _)| *fi == i)
            .map(|(_, t0)| self.now - t0);
        let (halo_op, halo_w) = match flash {
            Some(dt) if dt < 3200.0 => {
                let e = ease_out((dt % 1600.0) / 1600.0);
                (1.0 - e, 6.0 - 5.0 * e)
            }
            // when the flash ends the selection ring is back
            _ => (if is_sel { 1.0 } else { 0.0 }, 2.0),
        };
        if halo_op > 0.0 {
            ctx.set_global_alpha(halo_op);
            ctx.begin_path();
            ctx.arc(0.0, 0.0, n.halo_r, 0.0, PI * 2.0).ok();
            ctx.set_stroke_style_str(&c.accent);
            ctx.set_line_width(halo_w);
            ctx.stroke();
        }
        // the keyboard walk's place: a dashed ring, as a search hit is drawn
        if gs.cursor == Some(i) {
            ctx.set_global_alpha(0.9);
            ctx.begin_path();
            ctx.arc(0.0, 0.0, n.halo_r + 1.5, 0.0, PI * 2.0).ok();
            ctx.set_stroke_style_str(&c.accent);
            ctx.set_line_width(2.0);
            set_dash(ctx, &[3.0, 2.5]);
            ctx.stroke();
            set_dash(ctx, &[]);
        }
        ctx.set_global_alpha(1.0);
        let fill: &str = match n.fill {
            "accent" => &c.accent,
            "bg" => &c.bg,
            _ => c.fam(&n.fam),
        };
        ctx.set_fill_style_str(fill);
        ctx.fill_with_path_2d(&n.shape);
        // hollow shapes (records, external systems) are outlined in their family color, as the
        // legend draws them
        ctx.set_stroke_style_str(if n.hollow { c.fam(&n.fam) } else { &c.bg });
        ctx.set_line_width(n.sw);
        ctx.stroke_with_path(&n.shape);
        if let Some(rr) = n.ring_r {
            ctx.begin_path();
            ctx.arc(0.0, 0.0, rr, 0.0, PI * 2.0).ok();
            ctx.set_stroke_style_str(c.fam(&n.fam));
            ctx.set_line_width(1.4);
            set_dash(ctx, &[3.0, 2.5]);
            ctx.stroke();
            set_dash(ctx, &[]);
        }
        if let Some((txt, c3)) = &n.cnt {
            self.set_font(
                ctx,
                format!("700 {}px {}", if *c3 { "7.5" } else { "8.5" }, c.mono),
            );
            self.set_spacing(ctx, if *c3 { "-0.3px" } else { "0px" });
            ctx.set_text_align("center");
            ctx.set_text_baseline("middle");
            ctx.set_fill_style_str(&c.bg);
            let _ = ctx.fill_text(txt, 0.0, 0.0);
        }
        if let Some((dx, dy)) = n.dot {
            ctx.begin_path();
            ctx.arc(dx, dy, 1.9, 0.0, PI * 2.0).ok();
            ctx.set_fill_style_str(&c.warn);
            ctx.fill();
        }
        // label: opacity per the .n .lab rules
        let hover = gs.hover == Some(i) || gs.cursor == Some(i);
        let lv = gs.shown.get(i).copied().unwrap_or(false);
        let lop = if n.kind == Kind::File && lv && !is_sel && !is_nbr {
            0.85
        } else if lv || hover {
            1.0
        } else {
            0.0
        };
        if lop <= 0.0 {
            return;
        }
        let x_s = ex + sk * (n.x + n.lg.x);
        let y_s = ey + sk * (n.y + n.lg.y);
        if x_s < -400.0 || x_s > gs.w + 400.0 || y_s < -400.0 || y_s > gs.h + 400.0 {
            return;
        }
        // fill, weight, size, spacing per the cascade
        let (fill, weight): (&str, &str) = match n.kind {
            Kind::Root => (&c.accent_text, if is_sel { "700" } else { "800" }),
            Kind::Surf => (&c.accent_text, if is_sel { "700" } else { "600" }),
            Kind::Dir => {
                let f: &str = if !n.d1 {
                    &c.ink2
                } else if is_sel {
                    &c.accent_text
                } else {
                    &c.ink
                };
                (f, if is_sel { "700" } else { "600" })
            }
            Kind::File => (
                if is_sel { &c.accent_text } else { &c.ink2 },
                if is_sel { "700" } else { "400" },
            ),
        };
        let size = match n.kind {
            Kind::Root => "13",
            Kind::Surf => "12",
            Kind::Dir => {
                if n.d1 {
                    "12"
                } else {
                    "11"
                }
            }
            Kind::File => "10.5",
        };
        self.set_font(ctx, format!("{weight} {size}px {}", c.mono));
        self.set_spacing(
            ctx,
            if n.kind == Kind::Dir && n.d1 {
                "0.36px"
            } else {
                "0px"
            },
        );
        ctx.set_transform(dpr, 0.0, 0.0, dpr, dpr * x_s, dpr * y_s)
            .ok();
        ctx.rotate(n.lg.rot * PI / 180.0).ok();
        if self.ls_scale != 1.0 {
            ctx.scale(self.ls_scale, self.ls_scale).ok();
        }
        ctx.set_global_alpha(lop);
        ctx.set_text_align(match n.lg.anchor {
            "middle" => "center",
            "end" => "end",
            _ => "start",
        });
        ctx.set_text_baseline("middle");
        ctx.set_line_join("round");
        ctx.set_line_width(3.0);
        ctx.set_stroke_style_str(&c.bg);
        let _ = ctx.stroke_text(&n.label, 0.0, 0.0);
        ctx.set_fill_style_str(fill);
        let _ = ctx.fill_text(&n.label, 0.0, 0.0);
    }
}

fn set_dash(ctx: &CanvasRenderingContext2d, d: &[f64]) {
    let arr = js_sys::Array::new();
    for v in d {
        arr.push(&JsValue::from_f64(*v));
    }
    ctx.set_line_dash(&arr).ok();
}

// ---- keyboard: walking connections, framing the selection ------------------------------------

struct Walk {
    from: String,
    list: Vec<(String, &'static str)>,
    i: Option<usize>,
}

thread_local! {
    static WALK: RefCell<Option<Walk>> = const { RefCell::new(None) };
}

/// The node the walk is on, while it belongs to the current selection.
pub fn walk_target() -> Option<String> {
    let sel = state::sel()?;
    WALK.with(|w| {
        let w = w.borrow();
        let w = w.as_ref().filter(|w| w.from == sel)?;
        w.i.map(|i| w.list[i].0.clone())
    })
}

/// j and k in the graph: step through the selection's connections in the order the inspector
/// lists them (`insp::walk_list`), marking the one reached with a dashed ring, its label and its
/// card, and saying where the walk is in the hint. Enter or space goes there; esc stops.
pub fn walk(step: i64) {
    let Some(idx) = state::idx() else { return };
    let Some(sel) = state::sel() else {
        chrome::toast(
            "GRAPH",
            "Select something first; j and k then step through its connections",
            true,
        );
        return;
    };
    let got = WALK.with(|w| {
        let mut w = w.borrow_mut();
        if w.as_ref().is_none_or(|w| w.from != sel) {
            *w = Some(Walk {
                from: sel.clone(),
                list: insp::walk_list(&idx, &sel),
                i: None,
            });
        }
        let w = w.as_mut().unwrap();
        let n = w.list.len() as i64;
        if n == 0 {
            return None;
        }
        let i = match w.i {
            None if step > 0 => 0,
            None => n - 1,
            Some(i) => (i as i64 + step).rem_euclid(n),
        } as usize;
        w.i = Some(i);
        Some((w.list[i].0.clone(), w.list[i].1, i + 1, n as usize))
    });
    let Some((target, label, at, of)) = got else {
        chrome::toast(
            "GRAPH",
            &format!("{} has no connections", idx.name_of(&sel)),
            true,
        );
        return;
    };
    let (place, k, w, h) = {
        let mut gs = g();
        let rep = gs.lay.rep_or(&target).to_string();
        gs.cursor = gs.at.get(&rep).copied();
        let (s, ox, oy) = frame(&gs);
        let sk = s * gs.k;
        let (ex, ey) = (ox + gs.x * s, oy + gs.y * s);
        let place = gs
            .cursor
            .map(|i| (ex + sk * gs.nodes[i].x, ey + sk * gs.nodes[i].y));
        (place, gs.k, gs.w, gs.h)
    };
    // keep it in view: centre it, at the same zoom, when it is near or past an edge
    if let Some((x, y)) = place {
        if x < 60.0 || y < 60.0 || x > w - 60.0 || y > h - 60.0 {
            focus_node(&target, Some(k));
        }
    }
    insp::mark_chip(Some(&target));
    let shown = if target.ends_with('/') {
        target.clone()
    } else {
        idx.name_of(&target)
    };
    set_hint(&format!("{at}/{of} · {label} · {shown} · ⏎ go · esc"));
    schedule();
}

/// Forget the walk (the selection moved, or esc). True when there was one to forget.
pub fn walk_reset() -> bool {
    let had = WALK.with(|w| w.borrow_mut().take()).is_some();
    let drawn = g().cursor.take().is_some();
    if had || drawn {
        insp::mark_chip(None);
        schedule();
    }
    had || drawn
}

/// esc in the graph: stop walking, and put the hint back.
pub fn walk_stop() -> bool {
    if walk_target().is_none() {
        return false;
    }
    walk_reset();
    update();
    true
}

/// z: zoom so the selection and everything it touches fill the view (0 fits the whole graph).
pub fn frame_selection() {
    let Some(idx) = state::idx() else { return };
    let Some(sel) = state::sel() else {
        fit();
        return;
    };
    let target = {
        let gs = g();
        let lay = gs.lay.clone();
        let r = lay.rep_or(&sel).to_string();
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for id in std::iter::once(r).chain(nbrs_of(&idx, &lay, Some(&sel))) {
            if let Some(p) = lay.pos.get(&id) {
                pts.push((p.x, p.y));
            }
        }
        if pts.is_empty() || gs.w <= 0.0 {
            return;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (x, y) in &pts {
            x0 = x0.min(*x);
            y0 = y0.min(*y);
            x1 = x1.max(*x);
            y1 = y1.max(*y);
        }
        // room for labels, and never closer than a small neighborhood
        let pad = 60.0;
        let (bw, bh) = (
            (x1 - x0 + 2.0 * pad).max(240.0),
            (y1 - y0 + 2.0 * pad).max(240.0),
        );
        let s = screen_scale(&gs);
        let k = ((gs.w / s) / bw).min((gs.h / s) / bh).clamp(0.4, 12.0);
        let b = gs.lay.bbox;
        let (cx, cy) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
        let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        (k, cx - mx * k, cy - my * k)
    };
    animate_to(target.0, target.1, target.2, 420.0);
}

// ---- input -----------------------------------------------------------------------------------

/// The node under client point (cx, cy): the topmost whose halo reaches it.
fn hit(gs: &G, cx: f64, cy: f64) -> Option<usize> {
    if gs.w <= 0.0 {
        return None;
    }
    let (wx, wy) = world_point(gs, cx, cy);
    for (i, n) in gs.nodes.iter().enumerate().rev() {
        let (dx, dy) = (wx - n.x, wy - n.y);
        let reach = n.r.max(n.ring_r.unwrap_or(0.0)) + 5.0;
        if dx * dx + dy * dy <= reach * reach {
            return Some(i);
        }
    }
    None
}

fn tip_html(idx: &Idx, id: &str) -> String {
    let n = idx.node(id).unwrap();
    let info = if n.t == "dir" {
        format!(
            "{} files{}",
            fmt_n(idx.dinfo(id).map(|d| d.files).unwrap_or(0.0)),
            if is_collapsed(idx, id) {
                " · collapsed"
            } else {
                ""
            }
        )
    } else if n.t == "surface" {
        format!("{} files reach it", idx.in_of(id).len())
    } else {
        let t = if !n.type_s().is_empty() {
            n.type_s()
        } else {
            n.kind()
        };
        format!(
            "{t}{}",
            if n.lt0() != 0.0 {
                format!(" · changed {}", crate::fmt::ago_text(n.lt0()))
            } else {
                String::new()
            }
        )
    };
    let p = if n.t == "surface" {
        "external system".to_string()
    } else if id == "." {
        idx.kb.display.clone()
    } else {
        id.to_string()
    };
    format!(
        "<b>{}</b><div class=\"p\">{}</div><div class=\"m\">{}</div>",
        esc(&idx.label_of(id)),
        esc(&p),
        esc(&info)
    )
}

fn set_hover(new: Option<usize>, cx: f64, cy: f64) {
    let Some(idx) = state::idx() else { return };
    let tip = dom::id("gtip");
    let (old, changed_to) = {
        let mut gs = g();
        let old = gs.hover;
        if old == new {
            (old, None)
        } else {
            gs.hover = new;
            (old, Some(new))
        }
    };
    if let Some(new) = changed_to {
        if old.is_some() {
            // pointerout: hide the tip, back to the selection's edges
            dom::set_hidden(&tip, true);
            let sel = state::sel();
            hot_edges(&idx, &mut g(), sel.as_deref());
        }
        if let Some(i) = new {
            let id = g().nodes[i].id.clone();
            dom::set_html(&tip, &tip_html(&idx, &id));
            dom::set_hidden(&tip, false);
            hot_edges(&idx, &mut g(), Some(&id));
        }
        let canvas = g().canvas.clone();
        if let Some(cv) = canvas {
            dom::toggle(&cv, "onnode", new.is_some());
        }
        schedule();
    }
    if !dom::hidden(&tip) {
        // beside the pointer, on its other side where the pane runs out, and kept inside the pane
        let (left, top, w, h) = g().host_rect;
        let size = tip.get_bounding_client_rect();
        let (tw, th) = (size.width(), size.height());
        let mut x = cx - left + 14.0;
        let mut y = cy - top + 14.0;
        if x + tw > w {
            x = cx - left - 14.0 - tw;
        }
        if y + th > h {
            y = cy - top - 14.0 - th;
        }
        let x = x.min(w - tw - 4.0).max(4.0);
        let y = y.min(h - th - 4.0).max(4.0);
        dom::set_style(&tip, "left", &format!("{}px", num(x)));
        dom::set_style(&tip, "top", &format!("{}px", num(y)));
    }
}

/// Toggle the "all labels" display (`settings.labels`).
pub fn toggle_labels() {
    {
        let mut c = state::cfg_mut();
        c.labels = !c.labels;
    }
    state::save_settings();
    let on = cfg().labels;
    if let Some(b) = dom::q(".gtools [data-z=labels]") {
        dom::toggle(&b, "on", on);
    }
    declutter();
}

/// `toggleCollapse(id, force)`
pub fn toggle_collapse(id: &str, force: Option<bool>) {
    let Some(idx) = state::idx() else { return };
    if !idx.is_dir(id) || id == "." {
        return;
    }
    let now = is_collapsed(&idx, id);
    let next = force.unwrap_or(!now);
    state::cfg_mut().collapsed.insert(id.to_string(), next);
    state::save_settings();
    build(&[]);
    if let Some(sel) = state::sel() {
        focus_node(&sel, Some(view_k()));
    }
    crate::chrome::toast(
        if next { "Collapsed" } else { "Expanded" },
        &idx.name_of(id),
        false,
    );
}

/// Fit the canvas to its pane: its size, and its place on the page for hit-testing (a pane can move
/// without resizing, which the ResizeObserver doesn't see).
pub fn fit_pane() {
    let host = dom::id("v-graph");
    let r = host.get_bounding_client_rect();
    let mut gs = g();
    gs.w = r.width();
    gs.h = r.height();
    gs.host_rect = (r.left(), r.top(), r.width(), r.height());
    gs.dpr = dom::win().device_pixel_ratio();
    drop(gs);
    apply_view();
}

/// `wireGraph()`: create the canvas and its input handling.
pub fn wire() {
    let cv: HtmlCanvasElement = dom::id("graph").dyn_into().expect("canvas#graph");
    let ctx: CanvasRenderingContext2d = cv
        .get_context("2d")
        .ok()
        .flatten()
        .unwrap()
        .dyn_into()
        .unwrap();
    let layers = (0..2)
        .filter_map(|_| {
            let c: HtmlCanvasElement = dom::doc().create_element("canvas").ok()?.dyn_into().ok()?;
            c.set_width(0);
            c.set_height(0);
            let x: CanvasRenderingContext2d =
                c.get_context("2d").ok().flatten()?.dyn_into().ok()?;
            Some((c, x))
        })
        .collect();
    {
        let mut gs = g();
        gs.canvas = Some(cv.clone());
        gs.ctx = Some(ctx);
        gs.layers = layers;
        gs.dpr = dom::win().device_pixel_ratio();
    }
    // size: follow the pane
    let ro_cb = wasm_bindgen::closure::Closure::<dyn FnMut(js_sys::Array)>::new(
        move |_entries: js_sys::Array| fit_pane(),
    );
    if let Ok(ro) = web_sys::ResizeObserver::new(ro_cb.as_ref().unchecked_ref()) {
        ro.observe(&dom::id("v-graph"));
    }
    ro_cb.forget();
    dom::on(dom::win().as_ref(), "scroll", |_: web_sys::Event| fit_pane());
    let cvt: &web_sys::EventTarget = cv.as_ref();
    dom::on_active(cvt, "wheel", |e: WheelEvent| {
        e.prevent_default();
        let p = user_point(&g(), e.client_x() as f64, e.client_y() as f64);
        let f = (-e.delta_y() * if e.ctrl_key() { 0.012 } else { 0.0016 }).exp();
        zoom_by(f, Some(p));
    });
    dom::on(cvt, "pointerdown", |e: PointerEvent| {
        if e.button() != 0 {
            return;
        }
        g().drag = Some(Drag {
            x: e.client_x() as f64,
            y: e.client_y() as f64,
            moved: false,
        });
    });
    let cv2 = cv.clone();
    dom::on(
        dom::win().as_ref(),
        "pointermove",
        move |e: PointerEvent| {
            let (cx, cy) = (e.client_x() as f64, e.client_y() as f64);
            let dragging = g().drag.is_some();
            if dragging {
                let start = {
                    let gs = g();
                    let d = gs.drag.as_ref().unwrap();
                    (d.x, d.y, d.moved)
                };
                if !start.2 && (cx - start.0).hypot(cy - start.1) < 4.0 {
                    return;
                }
                if !start.2 {
                    g().drag.as_mut().unwrap().moved = true;
                    dom::add_class(&cv2, "panning");
                    let _ = cv2.set_pointer_capture(e.pointer_id());
                    set_hover(None, cx, cy);
                }
                {
                    let mut gs = g();
                    gs.anim = None;
                    let (s, _, _) = frame(&gs);
                    let d = gs.drag.as_mut().unwrap();
                    let (dx, dy) = ((cx - d.x) / s, (cy - d.y) / s);
                    d.x = cx;
                    d.y = cy;
                    gs.x += dx;
                    gs.y += dy;
                }
                apply_view();
                return;
            }
            // hover: only over the canvas
            let over = e
                .target()
                .map(|t| &t == AsRef::<web_sys::EventTarget>::as_ref(&cv2))
                .unwrap_or(false);
            let h = if over { hit(&g(), cx, cy) } else { None };
            set_hover(h, cx, cy);
        },
    );
    let cv3 = cv.clone();
    dom::on(dom::win().as_ref(), "pointerup", move |e: PointerEvent| {
        let Some(d) = g().drag.take() else { return };
        dom::remove_class(&cv3, "panning");
        if d.moved {
            return;
        }
        let on_canvas = e
            .target()
            .map(|t| &t == AsRef::<web_sys::EventTarget>::as_ref(&cv3))
            .unwrap_or(false);
        if !on_canvas {
            return;
        }
        let h = hit(&g(), e.client_x() as f64, e.client_y() as f64);
        match h {
            Some(i) => {
                let id = g().nodes[i].id.clone();
                crate::nav::select(
                    Some(&id),
                    crate::nav::SelOpts {
                        from: "graph",
                        ..Default::default()
                    },
                );
            }
            None => {
                if state::sel().is_some() {
                    crate::nav::select(
                        None,
                        crate::nav::SelOpts {
                            from: "graph",
                            ..Default::default()
                        },
                    );
                }
            }
        }
    });
    dom::on(cvt, "dblclick", |e: web_sys::MouseEvent| {
        let h = hit(&g(), e.client_x() as f64, e.client_y() as f64);
        match h {
            Some(i) => {
                let id = g().nodes[i].id.clone();
                let isdir = state::idx().map(|x| x.is_dir(&id)).unwrap_or(false);
                if isdir && id != "." {
                    toggle_collapse(&id, None);
                }
            }
            None => fit(),
        }
    });
    let cv4 = cv.clone();
    dom::on(cvt, "pointerleave", move |e: PointerEvent| {
        if g().drag.as_ref().map(|d| d.moved).unwrap_or(false) {
            return;
        }
        set_hover(None, e.client_x() as f64, e.client_y() as f64);
        dom::remove_class(&cv4, "onnode");
    });
    for b in dom::qa(".gtools button") {
        let z = dom::data(&b, "z").unwrap_or_default();
        dom::on(b.as_ref(), "click", move |_: web_sys::MouseEvent| {
            match z.as_str() {
                "in" => zoom_by(1.35, None),
                "out" => zoom_by(1.0 / 1.35, None),
                "fit" => fit(),
                "labels" => toggle_labels(),
                _ => {}
            }
        });
    }
    fit_pane();
}
