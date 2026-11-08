//! The knowledge graph: every card where the deterministic 3D layout puts it (`space`), drawn by
//! WebGL2 (`gl`) under a spherical camera (`cam`), with a flat overlay canvas (`overlay`) for the
//! marks that must stay crisp. The graph tokens in `app.css` (`--graph-*`, the family colours,
//! the accent) are the colours the renderer paints with, read once per build and on an accent
//! change.
//!
//! Frames are drawn on demand. Input handlers never draw: every camera move goes through
//! `cam_to`, which starts an ease (or cuts, under reduced motion) and schedules a frame. The frame
//! evaluates the pose, projects and sorts the drawn points, draws, and only then, with no graph
//! state borrowed, writes the test hooks.

mod cam;
mod gl;
mod overlay;
mod space;

use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::rc::Rc;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{HtmlCanvasElement, PointerEvent, WheelEvent};

use crate::dom;
use crate::js::{esc, fmt_n, num, to_fixed};
use crate::model::{Idx, Node, edge_group, fam_var, shape_of};
use crate::state::{self, cfg, is_collapsed};
use crate::theme;
use crate::{chrome, insp};
use cam::{Pane, Pose};

/// A card's world radius before its links add to it (the layout spaces cards 10 apart).
const CARD_R: f64 = 2.2;
/// The first view: a little to the side and above, so depth reads from the start.
const THETA0: f64 = -30.0 * PI / 180.0;
const PSI0: f64 = 20.0 * PI / 180.0;
/// One orbit key press turns the azimuth this far, one tilt key press the elevation.
pub const ORBIT_DEG: f64 = 15.0;
pub const TILT_DEG: f64 = 10.0;
/// A selected card is approached to at least the distance that fits this radius around it.
const FOCUS_RADIUS: f64 = 120.0;
/// Room around fitted cards for their marks, in world units.
const FIT_PAD: f64 = 12.0;

// ---- collapse: which node stands for each id -------------------------------------------------

/// Which node stands for each id under the collapse settings and single-child folder chains
/// (`rep`). The keys and actions that open and close folders ask it which folder they act on.
#[derive(Default)]
pub struct Layout {
    pub rep: HashMap<String, String>,
}

impl Layout {
    /// `G.rep.get(id) ?? id`
    pub fn rep_or<'a>(&'a self, id: &'a str) -> &'a str {
        self.rep.get(id).map(|s| s.as_str()).unwrap_or(id)
    }
}

/// The last folder of a chain of folders that each hold exactly one folder and no files.
fn chain_end(idx: &Idx, id: &str) -> String {
    let mut cur = id.to_string();
    for _ in 0..12 {
        if is_collapsed(idx, &cur) {
            break;
        }
        let (dirs, files) = idx.sorted_kids(&cur, false);
        if !files.is_empty() || dirs.len() != 1 {
            break;
        }
        cur = dirs[0].clone();
    }
    cur
}

fn reps(idx: &Idx) -> Layout {
    fn hide(idx: &Idx, x: &str, end: &str, rep: &mut HashMap<String, String>) {
        for k in idx.kids_of(x) {
            rep.insert(k.clone(), end.to_string());
            if idx.is_dir(k) {
                hide(idx, k, end, rep);
            }
        }
    }
    fn walk(idx: &Idx, id: &str, rep: &mut HashMap<String, String>) {
        let end = chain_end(idx, id);
        let mut c = id.to_string();
        while c != end {
            rep.insert(c.clone(), end.clone());
            let (dirs, _) = idx.sorted_kids(&c, false);
            c = dirs[0].clone();
        }
        rep.insert(end.clone(), end.clone());
        if is_collapsed(idx, &end) {
            hide(idx, &end, &end, rep);
            return;
        }
        let (dirs, files) = idx.sorted_kids(&end, false);
        for f in files {
            rep.insert(f.clone(), f.clone());
        }
        for d in dirs {
            walk(idx, d, rep);
        }
    }
    let mut rep = HashMap::new();
    rep.insert(".".to_string(), ".".to_string());
    let (tdirs, tfiles) = idx.sorted_kids(".", false);
    for f in tfiles {
        rep.insert(f.clone(), f.clone());
    }
    for d in tdirs {
        walk(idx, d, &mut rep);
    }
    for n in &idx.nodes {
        if !rep.contains_key(&n.id) {
            let r = if n.t == "surface" {
                n.id.clone()
            } else {
                ".".into()
            };
            rep.insert(n.id.clone(), r);
        }
    }
    Layout { rep }
}

/// `shapePath(shape, r)`: SVG path data (the files tree draws its family marks with it).
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

// ---- what gets drawn -------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Root,
    Surf,
    Dir,
    File,
}

/// How one node is drawn, whatever is selected.
struct NodeVis {
    kind: Kind,
    fam: String,
    /// records and external systems: an outline, as the legend draws them
    hollow: bool,
    /// world radius: cards grow with their links, folders with their files
    r: f64,
}

fn vis_of(idx: &Idx, n: &Node) -> NodeVis {
    let fam = n.fam().to_string();
    let shape = shape_of(&fam).unwrap_or("dot");
    let (kind, r) = match n.t.as_str() {
        "root" => (Kind::Root, CARD_R * 3.0),
        "surface" => (Kind::Surf, CARD_R * 2.2),
        "dir" => {
            let files = idx.dinfo(&n.id).map(|d| d.files).unwrap_or(0.0);
            (Kind::Dir, CARD_R * (1.4 + 0.3 * (1.0 + files).ln()))
        }
        _ => {
            let index = n.id == "index.md" || n.id.ends_with("/index.md");
            let r = CARD_R * (1.0 + 0.35 * (1.0 + idx.degree(&n.id)).ln());
            (Kind::File, if index { r * 1.4 } else { r })
        }
    };
    NodeVis {
        kind,
        hollow: kind == Kind::Surf || shape == "hsquare" || shape == "hexo",
        fam,
        r,
    }
}

/// The graph's colours, as the GL takes them (straight RGBA in 0–1).
struct Colors {
    bg: [f32; 4],
    fog: [f32; 4],
    accent: [f32; 4],
    halo: [f32; 4],
    edge: [f32; 4],
    edge_hot: [f32; 4],
    fam: HashMap<String, [f32; 4]>,
    /// the accent as CSS, for the overlay's strokes
    accent_css: String,
}

impl Colors {
    fn read(ov: &overlay::Overlay) -> Colors {
        let v = theme::css_var;
        let c = |name: &str| ov.canon(&v(name));
        let fam = [
            "core",
            "cards",
            "rules",
            "memory",
            "records",
            "workspace",
            "other",
            "world",
        ]
        .iter()
        .map(|f| (f.to_string(), c(&format!("--f-{f}"))))
        .collect();
        Colors {
            bg: c("--graph-bg"),
            fog: c("--graph-fog"),
            accent: c("--accent"),
            halo: c("--graph-halo"),
            edge: c("--graph-edge"),
            edge_hot: c("--edge-hot"),
            fam,
            accent_css: v("--accent"),
        }
    }
    fn fam(&self, f: &str) -> [f32; 4] {
        self.fam
            .get(f)
            .or_else(|| self.fam.get("other"))
            .copied()
            .unwrap_or([0.55, 0.55, 0.55, 1.0])
    }
}

/// A camera move in flight, from the pose a frame showed when it started.
#[derive(Clone, Copy)]
struct CamAnim {
    from: Pose,
    to: Pose,
    t0: f64,
    ms: f64,
}

#[derive(Clone, Copy, PartialEq)]
enum Move {
    /// straight to the end pose
    Cut,
    /// eased over this many milliseconds (a cut under reduced motion)
    Ease(f64),
}

/// What a resize re-fits while `autofit` holds: the whole graph, or a set of points.
enum Fit {
    Galaxy,
    Points(Vec<cam::V3>),
}

struct Drag {
    x: f64,
    y: f64,
    moved: bool,
}

/// A point drawn in the last frame, in pane CSS px, for picking.
struct Pick {
    x: f64,
    y: f64,
    r: f64,
    i: usize,
}

pub struct G {
    canvas: Option<HtmlCanvasElement>,
    gl: Option<gl::Renderer>,
    ov: Option<overlay::Overlay>,
    lay: Rc<Layout>,
    /// the 3D layout of every node in the index, aligned with `ids`
    space: Rc<space::Space>,
    ids: Rc<Vec<String>>,
    at: HashMap<String, usize>,
    nodes: Vec<NodeVis>,
    /// every link edge whose two ends are nodes, and whether it reaches an external system
    edges: Vec<(u32, u32, bool)>,
    colors: Option<Colors>,
    /// what the GL was last given to draw
    inst: Rc<gl::Instances>,
    /// `inst` still has to reach the GL (it had no context when it changed)
    inst_dirty: bool,
    sel: Option<usize>,
    pose: Pose,
    anim: Option<CamAnim>,
    /// re-fit on resize: set by a fit, cleared by a manual zoom
    autofit: bool,
    fit: Fit,
    /// camera moves started, for `data-move`
    moves: u64,
    picks: Vec<Pick>,
    hover: Option<usize>,
    /// the node the keyboard walk is on (see `walk`)
    cursor: Option<usize>,
    flash: Vec<(usize, f64)>,
    w: f64,
    h: f64,
    dpr: f64,
    host_rect: (f64, f64, f64, f64),
    raf: i32,
    drag: Option<Drag>,
    built: bool,
}

impl G {
    /// The pane, or a stand-in while it is hidden, so fit distances stay defined.
    fn pane(&self) -> Pane {
        if self.w > 0.0 && self.h > 0.0 {
            Pane {
                w: self.w,
                h: self.h,
            }
        } else {
            Pane { w: 800.0, h: 600.0 }
        }
    }
    /// The distance that fits the whole graph.
    fn d_galaxy(&self) -> f64 {
        cam::fit_sphere(self.space.radius.max(space::DMIN), self.pane())
    }
    /// The closest the camera comes: a small neighbourhood of one card fills the pane.
    fn d_min(&self) -> f64 {
        cam::fit_sphere(1.5 * space::DMIN, self.pane())
    }
    /// Where the camera is going: the end of the move in flight, or where it is.
    fn dest(&self) -> Pose {
        self.anim.map(|a| a.to).unwrap_or(self.pose)
    }
}

thread_local! {
    static GS: &'static RefCell<G> = Box::leak(Box::new(RefCell::new(G {
        canvas: None, gl: None, ov: None, lay: Rc::new(Layout::default()),
        space: Rc::new(space::Space::default()), ids: Rc::new(vec![]), at: HashMap::new(),
        nodes: vec![], edges: vec![], colors: None, inst: Rc::new(gl::Instances::default()),
        inst_dirty: false, sel: None,
        pose: Pose { target: [0.0; 3], theta: THETA0, psi: PSI0, ln_d: 8.0 },
        anim: None, autofit: true, fit: Fit::Galaxy, moves: 0, picks: vec![], hover: None,
        cursor: None, flash: vec![], w: 0.0, h: 0.0, dpr: 1.0, host_rect: (0.0, 0.0, 0.0, 0.0),
        raf: 0, drag: None, built: false,
    })));
}

fn g() -> std::cell::RefMut<'static, G> {
    GS.with(|g| *g).borrow_mut()
}

fn lay() -> Rc<Layout> {
    g().lay.clone()
}

pub fn rep_of(id: &str) -> String {
    lay().rep_or(id).to_string()
}

// ---- building ------------------------------------------------------------------------------

/// `buildGraph(flash)`: lay out the index and hand it to the renderer. The camera is kept.
pub fn build(flash: &[String]) {
    let Some(idx) = state::idx() else { return };
    let lnodes: Vec<space::LNode> = idx
        .nodes
        .iter()
        .map(|n| space::LNode {
            id: n.id.clone(),
            parent: n.parent.clone(),
            is_dir: n.t == "dir" || n.t == "root",
            ft: n.ft,
        })
        .collect();
    let sp = Rc::new(space::layout(&lnodes));
    let lay = Rc::new(reps(&idx));
    let ids: Vec<String> = idx.nodes.iter().map(|n| n.id.clone()).collect();
    let at: HashMap<String, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();
    let nodes: Vec<NodeVis> = idx.nodes.iter().map(|n| vis_of(&idx, n)).collect();
    let edges: Vec<(u32, u32, bool)> = idx
        .edges
        .iter()
        .filter_map(|e| {
            let surface = edge_group(&e.k) == "surface";
            Some((*at.get(&e.s)? as u32, *at.get(&e.t)? as u32, surface))
        })
        .collect();
    let now = dom::perf_now();
    let flashed: Vec<(usize, f64)> = flash
        .iter()
        .filter_map(|id| at.get(id).map(|&i| (i, now)))
        .collect();
    let colors = {
        let gs = g();
        gs.ov.as_ref().map(Colors::read)
    };
    {
        let mut gs = g();
        gs.lay = lay;
        gs.space = sp.clone();
        gs.ids = Rc::new(ids);
        gs.at = at;
        gs.nodes = nodes;
        gs.edges = edges;
        if colors.is_some() {
            gs.colors = colors;
        }
        gs.flash = flashed;
        gs.hover = None;
        gs.cursor = None;
        gs.picks.clear();
        let d_g = gs.d_galaxy();
        if !gs.built {
            gs.pose = Pose {
                target: [0.0; 3],
                theta: THETA0,
                psi: PSI0,
                ln_d: d_g.ln(),
            };
            gs.autofit = true;
            gs.fit = Fit::Galaxy;
        } else {
            let (lo, hi) = (gs.d_min(), 1.5 * d_g);
            gs.pose = cam::clamp(&gs.pose, lo, hi);
        }
        gs.built = true;
    }
    if let Some(t) = walk_target() {
        let mut gs = g();
        gs.cursor = gs.at.get(&t).copied();
    }
    update();
    publish_space(&lnodes, &sp, idx.files().count());
    render_legend(&idx);
}

/// Publish the 3D layout for tests: `#graph[data-layout]` (its fingerprint) and
/// `#graph[data-total]` (cards in the index). Runs with no graph state borrowed, after the
/// positions reached the renderer and `window.__oktPos`.
fn publish_space(nodes: &[space::LNode], sp: &space::Space, total: usize) {
    if let Some(cv) = dom::try_id("graph") {
        let _ = cv.set_attribute("data-layout", &format!("{:016x}", space::digest(nodes, sp)));
        let _ = cv.set_attribute("data-total", &total.to_string());
    }
}

/// What the GL draws for the current selection: every node, the selection haloed, and every link
/// edge, the selection's own last (on top) with out-links in the accent; links to external
/// systems are fainter.
fn instances(gs: &G) -> gl::Instances {
    let grey = [0.55, 0.55, 0.55, 1.0];
    let (accent, halo, edge, edge_hot) = gs
        .colors
        .as_ref()
        .map(|c| (c.accent, c.halo, c.edge, c.edge_hot))
        .unwrap_or((grey, grey, [0.5, 0.5, 0.5, 0.2], [0.8, 0.8, 0.8, 0.6]));
    let n = gs.nodes.len();
    let mut inst = gl::Instances {
        pos: Vec::with_capacity(n * 4),
        col: Vec::with_capacity(n * 4),
        ends: Vec::with_capacity(gs.edges.len() * 2),
        ecol: Vec::with_capacity(gs.edges.len() * 5),
    };
    for (i, (p, v)) in gs.space.pos.iter().zip(&gs.nodes).enumerate() {
        inst.pos
            .extend([p[0] as f32, p[1] as f32, p[2] as f32, v.r as f32]);
        let fam = gs.colors.as_ref().map(|c| c.fam(&v.fam)).unwrap_or(grey);
        let (rgb, st) = if Some(i) == gs.sel {
            (halo, gl::style::HALO)
        } else {
            match v.kind {
                Kind::Root => (accent, gl::style::DISC),
                Kind::Surf => (fam, gl::style::RING),
                Kind::Dir => (fam, gl::style::FOLDER),
                Kind::File if v.hollow => (fam, gl::style::RING),
                Kind::File => (fam, gl::style::DISC),
            }
        };
        inst.col.extend([rgb[0], rgb[1], rgb[2], st]);
    }
    let mut hot: Vec<(u32, u32, [f32; 4], f32)> = Vec::new();
    for &(s, t, surface) in &gs.edges {
        if Some(s as usize) == gs.sel {
            hot.push((s, t, [accent[0], accent[1], accent[2], 0.95], 1.6));
        } else if Some(t as usize) == gs.sel {
            hot.push((s, t, edge_hot, 1.4));
        } else {
            let a = if surface { edge[3] * 0.6 } else { edge[3] };
            inst.ends.extend([s, t]);
            inst.ecol.extend([edge[0], edge[1], edge[2], a, 1.0]);
        }
    }
    for (s, t, c, w) in hot {
        inst.ends.extend([s, t]);
        inst.ecol.extend([c[0], c[1], c[2], c[3], w]);
    }
    inst
}

/// Hand the current instances to the GL, and to `window.__oktPos` when a test asked for them.
fn sync_instances() {
    let (ids, inst) = {
        let mut gs = g();
        let inst = gs.inst.clone();
        let ok = gs.gl.as_mut().map(|r| r.upload(&inst)).unwrap_or(false);
        gs.inst_dirty = !ok;
        (gs.ids.clone(), inst)
    };
    gl::publish_positions(&ids, &inst);
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
            "<span><svg width=\"12\" height=\"12\" viewBox=\"-6 -6 12 12\"><circle r=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"/></svg>{}</span>",
            if hollow { "3.4" } else { "4" },
            if hollow { "none".to_string() } else { fam_var(&f.id) },
            if hollow { fam_var(&f.id) } else { "none".to_string() },
            esc(&f.label)
        ));
    }
    html.push_str(
        "<span style=\"color:var(--faint)\">drag to orbit · wheel to zoom · 0 fits</span>",
    );
    dom::set_html(&dom::id("glegend"), &html);
}

/// Re-read the graph's colours (the accent changed) and redraw with them.
pub fn recolor() {
    let colors = {
        let gs = g();
        gs.ov.as_ref().map(Colors::read)
    };
    if colors.is_some() {
        g().colors = colors;
    }
    update();
}

// ---- the camera ----------------------------------------------------------------------------

/// `(prefers-reduced-motion: reduce)`, read at the start of every move so a live OS toggle
/// counts (the `chrome::is_standalone` pattern).
fn reduced_motion() -> bool {
    dom::win()
        .match_media("(prefers-reduced-motion: reduce)")
        .ok()
        .flatten()
        .map(|m| m.matches())
        .unwrap_or(false)
}

/// Every camera move goes through here. A cut (`Move::Cut`; any move under reduced motion, or
/// while the graph is hidden) puts the camera at `to` at once; an ease starts from the pose the
/// last frame showed, so nothing snaps. `data-move` is bumped before this returns, so a test can
/// wait for a move to start before it waits for `data-anim="0"`.
fn cam_to(to: Pose, mv: Move) {
    let reduced = reduced_motion();
    let n = {
        let mut gs = g();
        let to = cam::clamp(&to, gs.d_min(), 1.5 * gs.d_galaxy());
        gs.moves += 1;
        match mv {
            Move::Ease(ms) if !reduced && gs.w > 0.0 => {
                let from = gs.pose;
                gs.anim = Some(CamAnim {
                    from,
                    to,
                    t0: dom::perf_now(),
                    ms,
                });
            }
            _ => {
                gs.pose = to;
                gs.anim = None;
            }
        }
        gs.moves
    };
    if let Some(cv) = dom::try_id("graph") {
        let _ = cv.set_attribute("data-move", &n.to_string());
    }
    schedule();
}

/// `0`, the fit button and a double-click on empty space: the whole graph, from where the camera
/// looks now. The selection is kept.
pub fn fit() {
    let to = {
        let mut gs = g();
        gs.autofit = true;
        gs.fit = Fit::Galaxy;
        Pose {
            target: [0.0; 3],
            ln_d: gs.d_galaxy().ln(),
            ..gs.dest()
        }
    };
    cam_to(to, Move::Ease(cam::EASE_MS));
}

/// `+`/`=` (f > 1, closer) and `-` (f < 1, farther): the camera distance, about the target.
/// Repeated presses chain from where the last one was going.
pub fn zoom_by(f: f64) {
    let to = {
        let mut gs = g();
        gs.autofit = false;
        let p = gs.dest();
        Pose {
            ln_d: p.ln_d - f.ln(),
            ..p
        }
    };
    cam_to(to, Move::Ease(cam::KEY_MS));
}

/// The orbit keys: turn the azimuth and tilt the elevation by degrees, about the target.
pub fn orbit_by(d_theta: f64, d_psi: f64) {
    let to = {
        let gs = g();
        cam::orbit(&gs.dest(), d_theta.to_radians(), d_psi.to_radians())
    };
    cam_to(to, Move::Ease(cam::KEY_MS));
}

/// Ease the camera onto a node, at distance `d` or close enough to see its neighbourhood.
pub fn focus_node(id: &str, d: Option<f64>) {
    let to = {
        let mut gs = g();
        let Some(&i) = gs.at.get(id) else { return };
        let target = gs.space.pos[i];
        let dest = gs.dest();
        let near = cam::fit_sphere(FOCUS_RADIUS, gs.pane()).ln();
        gs.autofit = false;
        Pose {
            target,
            ln_d: d.map(f64::ln).unwrap_or(dest.ln_d.min(near)),
            ..dest
        }
    };
    cam_to(to, Move::Ease(cam::EASE_MS));
}

// ---- selection -----------------------------------------------------------------------------

/// `updateGraph()`: the selection changed (or what is drawn did): new instances, the hint.
pub fn update() {
    if state::idx().is_none() {
        return;
    }
    let sel = state::sel();
    let hint = {
        let mut gs = g();
        if !gs.built {
            return;
        }
        let si = sel.as_deref().and_then(|s| gs.at.get(s).copied());
        gs.sel = si;
        gs.inst = Rc::new(instances(&gs));
        let rsel = sel.as_deref().map(|s| gs.lay.rep_or(s).to_string());
        match (&sel, &rsel) {
            (Some(s), Some(r)) if s != r => {
                let name = state::idx().map(|i| i.name_of(r)).unwrap_or_default();
                format!("{name} is collapsed · E to expand")
            }
            _ => String::new(),
        }
    };
    sync_instances();
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
    let t0 = dom::perf_now();
    let out = {
        let mut gs = g();
        if let Some(a) = gs.anim {
            let u = ((now - a.t0) / a.ms).clamp(0.0, 1.0);
            if u >= 1.0 {
                gs.pose = a.to;
                gs.anim = None;
            } else {
                gs.pose = cam::ease_pose(&a.from, &a.to, cam::smootherstep(u));
            }
        }
        render(&mut gs, now)
    };
    if let Some((drawn, uploaded)) = out {
        if let Some(cv) = dom::try_id("graph") {
            gl::publish(&cv, &drawn);
        }
        if let Some((ids, inst)) = uploaded {
            gl::publish_positions(&ids, &inst);
        }
        record_draw(dom::perf_now() - t0);
    }
    let again = {
        let mut gs = g();
        gs.flash.retain(|(_, t0)| now - t0 < 3200.0);
        gs.anim.is_some() || !gs.flash.is_empty()
    };
    if again {
        schedule();
    }
}

type Uploaded = Option<(Rc<Vec<String>>, Rc<gl::Instances>)>;

/// Draw one frame at the current pose: project every node, sort far to near, draw the GL scene
/// and the overlay's rings, and keep what was drawn for picking and for the hooks.
fn render(gs: &mut G, now: f64) -> Option<(gl::Drawn, Uploaded)> {
    if gs.w <= 0.0 || gs.h <= 0.0 || !gs.built {
        return None;
    }
    let cv = gs.canvas.clone()?;
    let (bw, bh) = (
        (gs.w * gs.dpr).round() as u32,
        (gs.h * gs.dpr).round() as u32,
    );
    if cv.width() != bw || cv.height() != bh {
        cv.set_width(bw);
        cv.set_height(bh);
    }
    let uploaded = if gs.inst_dirty {
        let inst = gs.inst.clone();
        let ok = gs.gl.as_mut().map(|r| r.upload(&inst)).unwrap_or(false);
        gs.inst_dirty = !ok;
        ok.then(|| (gs.ids.clone(), inst))
    } else {
        None
    };
    let pane = gs.pane();
    let pose = gs.pose;
    let unit = cam::px_per_unit(pane);
    let mut items: Vec<(f64, u32, f64, f64)> = Vec::with_capacity(gs.nodes.len());
    for (i, p) in gs.space.pos.iter().enumerate() {
        if let Some([x, y, s]) = cam::project(*p, &pose, pane) {
            items.push((s, i as u32, x, y));
        }
    }
    items.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let (near, far) = items
        .iter()
        .fold((f64::MAX, 0.0f64), |(n, f), it| (n.min(it.0), f.max(it.0)));
    let order: Vec<u32> = items.iter().map(|it| it.1).collect();
    let cards = items
        .iter()
        .filter(|it| gs.nodes[it.1 as usize].kind == Kind::File)
        .count();
    let picks: Vec<Pick> = items
        .iter()
        .rev()
        .map(|&(s, i, x, y)| Pick {
            x,
            y,
            r: (gs.nodes[i as usize].r * unit / s).max(1.6),
            i: i as usize,
        })
        .collect();
    let (bg, fog, accent_css) = gs
        .colors
        .as_ref()
        .map(|c| (c.bg, c.fog, c.accent_css.clone()))
        .unwrap_or(([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0], "#ffa028".into()));
    let vp = cam::view_proj(&pose, pane, gs.space.radius).map(|v| v as f32);
    let edges = gs.gl.as_ref().and_then(|r| {
        r.draw(&gl::Frame {
            vp,
            order: &order,
            px: (bw, bh),
            scale: (unit * gs.dpr) as f32,
            dpr: gs.dpr as f32,
            depth: (near as f32, far as f32),
            bg,
            fog,
        })
    });
    // the overlay: hover, the walk's place, and the flash after a live change
    let at = |i: usize| picks.iter().find(|p| p.i == i).map(|p| (p.x, p.y, p.r));
    let mut rings = Vec::new();
    for &(i, t0) in &gs.flash {
        let dt = now - t0;
        if let (true, Some((x, y, r))) = (dt < 3200.0, at(i)) {
            let e = ease_out((dt % 1600.0) / 1600.0);
            rings.push(overlay::Ring {
                x,
                y,
                r: r + 4.0,
                width: 6.0 - 5.0 * e,
                alpha: 1.0 - e,
                dashed: false,
            });
        }
    }
    if let Some((x, y, r)) = gs.hover.and_then(at) {
        rings.push(overlay::Ring {
            x,
            y,
            r: r + 4.0,
            width: 1.5,
            alpha: 0.8,
            dashed: false,
        });
    }
    if let Some((x, y, r)) = gs.cursor.and_then(at) {
        rings.push(overlay::Ring {
            x,
            y,
            r: r + 5.5,
            width: 2.0,
            alpha: 0.9,
            dashed: true,
        });
    }
    if let Some(ov) = &gs.ov {
        ov.draw(gs.w, gs.h, gs.dpr, &accent_css, &rings);
    }
    gs.picks = picks;
    let edges = edges?;
    let drawn = gl::Drawn {
        scale: if gs.sel.is_some() { "focus" } else { "galaxy" },
        theta_deg: pose.theta.to_degrees(),
        psi_deg: pose.psi.to_degrees(),
        dist: pose.d(),
        anim: gs.anim.is_some(),
        vp,
        cards,
        edges,
        pos_hash: gs.gl.as_ref().map(|r| r.pos_hash()).unwrap_or(0),
        dist_galaxy: gs.d_galaxy(),
        focus: gs.sel.map(|i| gs.ids[i].clone()).unwrap_or_default(),
    };
    Some((drawn, uploaded))
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
/// lists them (`insp::walk_list`), marking the one reached with a dashed ring and its card, and
/// saying where the walk is in the hint. Enter or space goes there; esc stops.
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
    let (place, d, w, h) = {
        let mut gs = g();
        gs.cursor = gs.at.get(&target).copied();
        let place = gs
            .cursor
            .and_then(|i| cam::project(gs.space.pos[i], &gs.pose, gs.pane()));
        (place, gs.dest().d(), gs.w, gs.h)
    };
    // keep it in view: centre it, at the same distance, when it is near or past an edge
    let off =
        place.is_none_or(|p| p[0] < 60.0 || p[1] < 60.0 || p[0] > w - 60.0 || p[1] > h - 60.0);
    if off && w > 0.0 {
        focus_node(&target, Some(d));
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

/// z: bring the selection and the connections the walk steps through into view, fitted to the
/// pane from where the camera looks now (0 fits the whole graph).
pub fn frame_selection() {
    let Some(idx) = state::idx() else { return };
    let Some(sel) = state::sel() else {
        fit();
        return;
    };
    let members = insp::walk_list(&idx, &sel);
    let to = {
        let mut gs = g();
        let Some(&i) = gs.at.get(&sel) else { return };
        let target = gs.space.pos[i];
        let mut pts = vec![target];
        for (m, _) in &members {
            if let Some(&j) = gs.at.get(m) {
                pts.push(gs.space.pos[j]);
            }
        }
        let dest = gs.dest();
        let d = cam::fit_distance(
            &pts,
            target,
            dest.theta,
            dest.psi,
            gs.pane(),
            FIT_PAD,
            gs.d_min(),
        );
        gs.autofit = true;
        gs.fit = Fit::Points(pts);
        Pose {
            target,
            ln_d: d.ln(),
            ..dest
        }
    };
    cam_to(to, Move::Ease(cam::EASE_MS));
}

// ---- input -----------------------------------------------------------------------------------

/// The node under client point (cx, cy): the nearest drawn there in the last frame.
fn hit(gs: &G, cx: f64, cy: f64) -> Option<usize> {
    let (x, y) = (cx - gs.host_rect.0, cy - gs.host_rect.1);
    gs.picks
        .iter()
        .find(|p| (p.x - x).hypot(p.y - y) <= p.r.max(3.0) + 4.0)
        .map(|p| p.i)
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
    let changed = {
        let mut gs = g();
        let changed = gs.hover != new;
        gs.hover = new;
        changed
    };
    if changed {
        match new {
            Some(i) => {
                let id = g().ids[i].clone();
                dom::set_html(&tip, &tip_html(&idx, &id));
                dom::set_hidden(&tip, false);
            }
            None => dom::set_hidden(&tip, true),
        }
        let canvas = g().canvas.clone();
        if let Some(cv) = canvas {
            dom::toggle(&cv, "onnode", new.is_some());
        }
        schedule();
    }
    if !dom::hidden(&tip) {
        let (left, top, w, h) = g().host_rect;
        let mut x = cx - left + 14.0;
        let mut y = cy - top + 14.0;
        if x + 300.0 > w {
            x -= 320.0;
        }
        if y + 70.0 > h {
            y -= 80.0;
        }
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
    schedule();
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
    crate::chrome::toast(
        if next { "Collapsed" } else { "Expanded" },
        &idx.name_of(id),
        false,
    );
}

/// Follow the pane: its size, where it is on the page, and the device pixel ratio. A pane that
/// changed size while `autofit` holds is re-fitted, as a cut.
fn resize() {
    let host = dom::id("v-graph");
    let r = host.get_bounding_client_rect();
    let refit = {
        let mut gs = g();
        let changed = gs.w != r.width() || gs.h != r.height();
        gs.w = r.width();
        gs.h = r.height();
        gs.host_rect = (r.left(), r.top(), r.width(), r.height());
        gs.dpr = dom::win().device_pixel_ratio();
        if changed && gs.autofit && gs.built && gs.w > 0.0 && gs.anim.is_none() {
            let p = gs.pose;
            let d = match &gs.fit {
                Fit::Galaxy => gs.d_galaxy(),
                Fit::Points(pts) => cam::fit_distance(
                    pts,
                    p.target,
                    p.theta,
                    p.psi,
                    gs.pane(),
                    FIT_PAD,
                    gs.d_min(),
                ),
            };
            Some(Pose { ln_d: d.ln(), ..p })
        } else {
            None
        }
    };
    match refit {
        Some(to) => cam_to(to, Move::Cut),
        None => schedule(),
    }
}

/// `wireGraph()`: the GL context, the overlay, and the graph's input handling.
pub fn wire() {
    let cv: HtmlCanvasElement = dom::id("graph").dyn_into().expect("canvas#graph");
    let renderer = gl::Renderer::new(&cv);
    if renderer.is_none() {
        dom::log_err(
            "graph",
            "WebGL2 is not available; the graph cannot be drawn",
        );
    }
    {
        let mut gs = g();
        gs.canvas = Some(cv.clone());
        gs.gl = renderer;
        gs.ov = overlay::Overlay::new();
        gs.dpr = dom::win().device_pixel_ratio();
    }
    let cvt: &web_sys::EventTarget = cv.as_ref();
    // a lost context (sleep, a GPU switch) stops the drawing; a restored one is rebuilt from G
    dom::on(cvt, "webglcontextlost", |e: web_sys::Event| {
        e.prevent_default();
        if let Some(r) = g().gl.as_mut() {
            r.lost();
        }
    });
    dom::on(cvt, "webglcontextrestored", |_: web_sys::Event| {
        if let Some(r) = g().gl.as_mut() {
            r.restore();
        }
        sync_instances();
        schedule();
    });
    // size: follow the pane
    let ro_cb = wasm_bindgen::closure::Closure::<dyn FnMut(js_sys::Array)>::new(
        move |_entries: js_sys::Array| resize(),
    );
    if let Ok(ro) = web_sys::ResizeObserver::new(ro_cb.as_ref().unchecked_ref()) {
        ro.observe(&dom::id("v-graph"));
    }
    ro_cb.forget();
    dom::on(dom::win().as_ref(), "scroll", |_: web_sys::Event| resize());
    // wheel and pinch: the camera distance, about the target
    dom::on_active(cvt, "wheel", |e: WheelEvent| {
        e.prevent_default();
        let to = {
            let mut gs = g();
            gs.autofit = false;
            let dl = cam::wheel_delta(e.delta_y(), e.delta_mode(), e.ctrl_key(), gs.h);
            Pose {
                ln_d: gs.pose.ln_d + dl,
                ..gs.pose
            }
        };
        cam_to(to, Move::Cut);
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
            let start = g().drag.as_ref().map(|d| (d.x, d.y, d.moved));
            if let Some((x0, y0, moved)) = start {
                if !moved && (cx - x0).hypot(cy - y0) < 4.0 {
                    return;
                }
                if !moved {
                    g().drag.as_mut().unwrap().moved = true;
                    dom::add_class(&cv2, "orbiting");
                    let _ = cv2.set_pointer_capture(e.pointer_id());
                    set_hover(None, cx, cy);
                }
                // drag orbits: across the pane's short side turns half a circle
                let to = {
                    let mut gs = g();
                    let m = gs.w.min(gs.h).max(1.0);
                    let d = gs.drag.as_mut().unwrap();
                    let (dx, dy) = (cx - d.x, cy - d.y);
                    d.x = cx;
                    d.y = cy;
                    cam::orbit(&gs.pose, -dx / m * PI, dy / m * PI)
                };
                cam_to(to, Move::Cut);
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
        dom::remove_class(&cv3, "orbiting");
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
                let id = g().ids[i].clone();
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
                let id = g().ids[i].clone();
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
                "in" => zoom_by(cam::ZOOM_STEP),
                "out" => zoom_by(1.0 / cam::ZOOM_STEP),
                "fit" => fit(),
                "labels" => toggle_labels(),
                _ => {}
            }
        });
    }
    resize();
}
