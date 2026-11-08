//! The knowledge graph: every card where the deterministic 3D layout puts it (`space`), drawn by
//! WebGL2 (`gl`) under a spherical camera (`cam`), with a flat overlay canvas (`overlay`) for the
//! marks that must stay crisp. The graph tokens in `app.css` (`--graph-*`, the family colours,
//! the accent) are the colours the renderer paints with, read once per build and on an accent
//! change.
//!
//! The camera works at three scales. At galaxy scale each region (a top-level folder) is one
//! glyph, a soft halo sized by its card count, and every pair of linked regions one bundle. At
//! system scale one region is fitted to the pane with its subfolders as glyphs, and the other
//! regions sit on a rim inside the pane's edge, in their true direction, with the bundles that
//! reach them. Focus scale is a card and its links. A selection sets the scale (a folder brings
//! its region to system scale, a card focus, nothing the galaxy); zooming crosses to the
//! neighbouring scale and never changes the selection. Selecting a card turns the camera to look
//! along the smallest principal axis of the card and its links in its region, fitted to the pane;
//! its links in other regions run out to their regions' glyphs on the rim (portals).
//!
//! Which cards are drawn (`vis`) is decided apart from the scale, by link hops from the focus:
//! the focus and its links in any region, two hops dim and the rest of its region faint, nothing
//! else but the region glyphs; or every card and link under "show all".
//!
//! Frames are drawn on demand. Input handlers never draw: every camera move goes through
//! `cam_to`, which starts an ease (or cuts, under reduced motion) and schedules a frame. The frame
//! evaluates the pose, projects and sorts the drawn points, draws, and only then, with no graph
//! state borrowed, writes the test hooks.

mod cam;
mod gl;
mod overlay;
mod space;
mod vis;

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
use cam::{Pane, Pose, Scale};

/// A card's world radius before its links add to it (the layout spaces cards 10 apart).
const CARD_R: f64 = 2.2;
/// However close the camera comes, a point is drawn no larger than this, in CSS px: a tight
/// focus set fitted to the pane shows its arrangement, not a few giant discs.
const POINT_MAX_PX: f64 = 22.0;
/// The first view: a little to the side and above, so depth reads from the start.
const THETA0: f64 = -30.0 * PI / 180.0;
const PSI0: f64 = 20.0 * PI / 180.0;
/// One orbit key press turns the azimuth this far, one tilt key press the elevation.
pub const ORBIT_DEG: f64 = 15.0;
pub const TILT_DEG: f64 = 10.0;
/// The scale bands take a card's own approach as fitting this radius around it.
const FOCUS_RADIUS: f64 = 120.0;
/// Room around a fitted region's nodes for their marks, in world units.
const FIT_PAD: f64 = 12.0;
/// Room around a focus fit's cards: a card's radius (the flat labels find their own room).
const FOCUS_PAD: f64 = CARD_R;
/// A region glyph's halo: this world radius plus this much per square root of its cards.
const GLYPH_R0: f64 = 14.0;
const GLYPH_K: f64 = 6.0;
/// A subfolder's glyph at system scale, likewise per square root of its files.
const SUB_R0: f64 = 6.0;
const SUB_K: f64 = 3.0;
/// A glyph's dot at its core, and a rim glyph, in CSS px.
const DOT_PX: f64 = 2.5;
const RIM_PX: f64 = 5.0;
/// Segments per bundle curve.
const BUNDLE_SEGS: usize = 12;

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
    /// files under a folder
    files: f64,
}

fn vis_of(idx: &Idx, n: &Node) -> NodeVis {
    let fam = n.fam().to_string();
    let shape = shape_of(&fam).unwrap_or("dot");
    let files = idx.dinfo(&n.id).map(|d| d.files).unwrap_or(0.0);
    let (kind, r) = match n.t.as_str() {
        "root" => (Kind::Root, CARD_R * 3.0),
        "surface" => (Kind::Surf, CARD_R * 2.2),
        "dir" => (Kind::Dir, CARD_R * (1.4 + 0.3 * (1.0 + files).ln())),
        _ => {
            let index = n.id == "index.md" || n.id.ends_with("/index.md");
            (
                Kind::File,
                CARD_R * vis::importance(idx.degree(&n.id), index),
            )
        }
    };
    NodeVis {
        kind,
        hollow: kind == Kind::Surf || shape == "hsquare" || shape == "hexo",
        fam,
        r,
        files,
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
    bundle: [f32; 4],
    fam: HashMap<String, [f32; 4]>,
    /// the accent as CSS, for the overlay's strokes
    accent_css: String,
    /// a region halo's peak alpha (`--graph-glyph-alpha`)
    glyph_alpha: f32,
    /// how much of a card two hops out shows, and of the rest of the focused region
    /// (`--graph-t2-alpha`, `--graph-t3-alpha`)
    tier_alpha: (f32, f32),
    /// the compass's ink and its far side, as CSS (`--graph-compass`, `--graph-compass-far`)
    compass: (String, String),
    /// how far inside the pane's edge the rim runs, CSS px (`--graph-rim-inset`)
    rim_inset: f64,
    /// a dark background: halos add light rather than cover
    dark: bool,
}

impl Colors {
    /// Read the graph tokens, and give the overlay its text style.
    fn read(ov: &overlay::Overlay) -> Colors {
        let v = theme::css_var;
        ov.restyle(&v("--graph-bg"));
        let c = |name: &str| ov.canon(&v(name));
        let bg = c("--graph-bg");
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
        let number = |name: &str, or: f64| {
            v(name)
                .trim_end_matches("px")
                .trim()
                .parse::<f64>()
                .unwrap_or(or)
        };
        Colors {
            dark: 0.2126 * bg[0] + 0.7152 * bg[1] + 0.0722 * bg[2] < 0.5,
            bg,
            fog: c("--graph-fog"),
            accent: c("--accent"),
            halo: c("--graph-halo"),
            edge: c("--graph-edge"),
            edge_hot: c("--edge-hot"),
            bundle: c("--graph-bundle"),
            fam,
            accent_css: v("--accent"),
            glyph_alpha: number("--graph-glyph-alpha", 0.3) as f32,
            tier_alpha: (
                number("--graph-t2-alpha", 0.55) as f32,
                number("--graph-t3-alpha", 0.22) as f32,
            ),
            compass: (v("--graph-compass"), v("--graph-compass-far")),
            rim_inset: number("--graph-rim-inset", 34.0),
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

/// A camera move in flight, from the pose a frame showed when it started. The rim weight (0 at
/// galaxy scale, 1 where other regions sit on the rim) eases with it, so glyphs slide between
/// their true projection and the rim.
#[derive(Clone, Copy)]
struct CamAnim {
    from: Pose,
    to: Pose,
    t0: f64,
    ms: f64,
    rim0: f64,
    rim1: f64,
    /// a move across the system/galaxy band: a zoom adjusts its distance instead of cancelling it
    cross: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Move {
    /// straight to the end pose
    Cut,
    /// eased over this many milliseconds (a cut under reduced motion)
    Ease(f64),
    /// eased like `Ease`, across the system/galaxy band (the target travels between the origin
    /// and a region): a zoom during it moves both of its ends rather than stranding the target
    Cross(f64),
}

/// How far toward the rim the other regions' glyphs sit at a scale.
fn rim_of(s: Scale) -> f64 {
    if s == Scale::Galaxy { 0.0 } else { 1.0 }
}

/// A bundle between two regions, tessellated into texels from `first` on.
struct Bundle {
    a: usize,
    b: usize,
    first: u32,
    /// CSS px
    w: f32,
    alpha: f32,
}

/// What a resize re-fits while `autofit` holds: the whole graph, or a set of points with the
/// room kept around them.
enum Fit {
    Galaxy,
    Points(Vec<cam::V3>, f64),
}

/// A focus link in a region out of focus: its edge runs from the focus to the card when the card
/// is on the pane, otherwise to its region's glyph on the rim.
struct Portal {
    from: usize,
    to: usize,
    region: usize,
    rgba: [f32; 4],
    /// CSS px
    w: f32,
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
    /// regions, adjacency and region-pair counts; region indices agree with `space.regions`
    vis: Rc<vis::Vis>,
    /// each region's folder node
    region_node: Vec<Option<usize>>,
    /// external systems in the index (the `@` glyph shows when there are any)
    n_surf: usize,
    bundles: Vec<Bundle>,
    bundle_pts: Vec<cam::V3>,
    colors: Option<Colors>,
    /// what the GL was last given to draw
    inst: Rc<gl::Instances>,
    /// the nodes drawn as points, decided when the selection, scale or show all changes
    drawn: Rc<Vec<u32>>,
    /// for each node under a collapsed folder, that folder (its tier-3 cards fold into it)
    fold: Vec<Option<usize>>,
    /// a selected folder's own children: tier 1 with the folder's links
    kids: Vec<usize>,
    /// the focus links drawn out to the rim, decided with `drawn`
    portals: Vec<Portal>,
    /// the cards the last card solve fitted, for `data-focus-ids`
    focus_ids: Rc<Vec<String>>,
    /// the active region's subfolders, drawn as glyphs
    subs: Vec<usize>,
    show_all: bool,
    scale: Scale,
    /// the region the camera is at, at system and focus scale
    region: Option<usize>,
    /// where the other regions' glyphs sit, 0 (true projection) to 1 (the rim)
    rim: f64,
    /// the selection the camera last followed
    sel_id: Option<String>,
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
    /// the pane chrome the rim and the compass keep clear of: the toolbar's right edge, the
    /// legend's height and its right edge, CSS px from the pane's left, bottom and left edges
    chrome: (f64, f64, f64),
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
    /// The distance a selected card is approached to.
    fn d_focus(&self) -> f64 {
        cam::fit_sphere(FOCUS_RADIUS, self.pane())
    }
    /// The distance that fits a region whatever the orientation.
    fn d_system(&self, r: usize) -> f64 {
        let radius = self.space.regions.get(r).map(|g| g.radius).unwrap_or(0.0);
        cam::fit_sphere(radius.max(space::DMIN), self.pane())
    }
    /// The bands between the scales: around the active region, or a typical region (the
    /// geometric mean of their fits) when there is none.
    fn bands(&self) -> cam::Bands {
        let d_s = match self.region {
            Some(r) => self.d_system(r),
            None => {
                let ls: Vec<f64> = (0..self.space.regions.len())
                    .filter(|&r| self.vis.is_region(r))
                    .map(|r| self.d_system(r).ln())
                    .collect();
                if ls.is_empty() {
                    self.d_galaxy() / 3.0
                } else {
                    (ls.iter().sum::<f64>() / ls.len() as f64).exp()
                }
            }
        };
        cam::bands(self.d_focus(), d_s, self.d_galaxy())
    }
    /// How far in a zoom may cross: focus when a card is selected in the active region,
    /// otherwise system.
    fn innermost(&self) -> Scale {
        match (self.sel, self.region) {
            (Some(i), Some(r))
                if self.scale != Scale::Galaxy
                    && self.nodes.get(i).is_some_and(|v| v.kind == Kind::File)
                    && self.vis.region.get(i) == Some(&r) =>
            {
                Scale::Focus
            }
            _ => Scale::System,
        }
    }
    /// A node's region, when it is a top-level folder's (not the root's cards or a surface).
    fn region_of(&self, i: usize) -> Option<usize> {
        self.vis
            .region
            .get(i)
            .copied()
            .filter(|&r| self.vis.is_region(r))
    }
    /// Fit region `r` from `theta`/`psi`: its core as the target, and the distance that fits
    /// every node in it; also the points fitted, for a resize.
    fn region_fit(&self, r: usize, theta: f64, psi: f64) -> (cam::V3, f64, Vec<cam::V3>) {
        let target = self
            .space
            .regions
            .get(r)
            .map(|g| g.core)
            .unwrap_or([0.0; 3]);
        let pts: Vec<cam::V3> = self
            .vis
            .region
            .iter()
            .zip(&self.space.pos)
            .filter(|(g, _)| **g == r)
            .map(|(_, p)| *p)
            .collect();
        let d = cam::fit_distance(&pts, target, theta, psi, self.pane(), FIT_PAD, self.d_min());
        (target, d, pts)
    }
    /// The camera for a selected card: the card as the target, looking along the smallest
    /// principal axis of the card and its links in its own region (from the side the camera is
    /// on), at the distance that fits them to the pane. Its links in other regions are left to
    /// their portals on the rim. Also the points fitted and their ids.
    fn card_view(&self, i: usize) -> (Pose, Vec<cam::V3>, Vec<String>) {
        let r = self.vis.region.get(i).copied();
        let mut set = vec![i];
        if let Some(adj) = self.vis.adj.get(i) {
            set.extend(adj.iter().copied().filter(|&j| {
                j != i
                    && self.vis.region.get(j).copied() == r
                    && self.nodes.get(j).is_some_and(|v| v.kind != Kind::Root)
            }));
        }
        let pts: Vec<cam::V3> = set.iter().map(|&j| self.space.pos[j]).collect();
        let ids = set.iter().map(|&j| self.ids[j].clone()).collect();
        let dest = self.dest();
        let dir = cam::solve_view(&pts, cam::basis(dest.theta, dest.psi).dir);
        let (theta, psi) = cam::angles(dir, dest.theta);
        let target = self.space.pos[i];
        let d = cam::fit_distance(
            &pts,
            target,
            theta,
            psi,
            self.pane(),
            FOCUS_PAD,
            self.d_min(),
        );
        let to = Pose {
            target,
            theta,
            psi,
            ln_d: d.ln(),
        };
        (to, pts, ids)
    }
    /// Set the scale and the active region; true when either changed.
    fn set_scale(&mut self, scale: Scale, region: Option<usize>) -> bool {
        let changed = self.scale != scale || self.region != region;
        self.scale = scale;
        self.region = region;
        changed
    }
}

thread_local! {
    static GS: &'static RefCell<G> = Box::leak(Box::new(RefCell::new(G {
        canvas: None, gl: None, ov: None, lay: Rc::new(Layout::default()),
        space: Rc::new(space::Space::default()), ids: Rc::new(vec![]), at: HashMap::new(),
        nodes: vec![], edges: vec![], vis: Rc::new(vis::Vis::default()), region_node: vec![],
        n_surf: 0, bundles: vec![], bundle_pts: vec![], colors: None,
        inst: Rc::new(gl::Instances::default()), drawn: Rc::new(vec![]), subs: vec![],
        fold: vec![], kids: vec![], portals: vec![], focus_ids: Rc::new(vec![]),
        show_all: false, scale: Scale::Galaxy, region: None, rim: 0.0, sel_id: None,
        inst_dirty: false, sel: None,
        pose: Pose { target: [0.0; 3], theta: THETA0, psi: PSI0, ln_d: 8.0 },
        anim: None, autofit: true, fit: Fit::Galaxy, moves: 0, picks: vec![], hover: None,
        cursor: None, flash: vec![], w: 0.0, h: 0.0, dpr: 1.0, host_rect: (0.0, 0.0, 0.0, 0.0),
        chrome: (0.0, 0.0, 0.0),
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
    let vnodes: Vec<vis::VNode> = idx
        .nodes
        .iter()
        .map(|n| vis::VNode {
            id: &n.id,
            surface: n.t == "surface",
            file: n.t == "file",
        })
        .collect();
    let vedges: Vec<(usize, usize, f64)> = idx
        .edges
        .iter()
        .filter_map(|e| Some((*at.get(&e.s)?, *at.get(&e.t)?, e.n)))
        .collect();
    let order: Vec<String> = sp.regions.iter().map(|r| r.id.clone()).collect();
    let vis = Rc::new(vis::build(&vnodes, &vedges, &order));
    let region_node: Vec<Option<usize>> = vis.regions.iter().map(|k| at.get(k).copied()).collect();
    let n_surf = idx.nodes.iter().filter(|n| n.t == "surface").count();
    let (bundles, bundle_pts) = tessellate(&vis, &sp, ids.len());
    // the collapsed folder each node folds into, if any
    let fold: Vec<Option<usize>> = ids
        .iter()
        .map(|id| {
            let r = lay.rep_or(id);
            (r != id && is_collapsed(&idx, r))
                .then(|| at.get(r).copied())
                .flatten()
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
        // the active region keeps its folder across a rebuild, wherever its index moved
        let active = gs.region.and_then(|r| gs.vis.regions.get(r).cloned());
        gs.region = active.and_then(|k| vis.regions.iter().position(|x| *x == k));
        gs.lay = lay;
        gs.space = sp.clone();
        gs.ids = Rc::new(ids);
        gs.at = at;
        gs.nodes = nodes;
        gs.edges = edges;
        gs.vis = vis;
        gs.region_node = region_node;
        gs.fold = fold;
        gs.n_surf = n_surf;
        gs.bundles = bundles;
        gs.bundle_pts = bundle_pts;
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
    let chrome = chrome_insets();
    g().chrome = chrome;
}

/// How far the toolbar reaches in from the pane's left edge, and the legend up from its bottom
/// and in from its left edge (zero for any when it is not shown), so rim glyphs and the compass
/// keep clear of them.
fn chrome_insets() -> (f64, f64, f64) {
    let Some(host) = dom::try_id("v-graph") else {
        return (0.0, 0.0, 0.0);
    };
    let hr = host.get_bounding_client_rect();
    let left = dom::q(".gtools")
        .map(|t| t.get_bounding_client_rect())
        .filter(|r| r.width() > 0.0)
        .map(|r| r.right() - hr.left())
        .unwrap_or(0.0);
    let legend = dom::try_id("glegend")
        .map(|l| l.get_bounding_client_rect())
        .filter(|r| r.width() > 0.0);
    let bottom = legend
        .as_ref()
        .map(|r| hr.bottom() - r.top())
        .unwrap_or(0.0);
    let right = legend
        .as_ref()
        .map(|r| r.right() - hr.left())
        .unwrap_or(0.0);
    (left.max(0.0), bottom.max(0.0), right.max(0.0))
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

/// One quadratic Bézier per linked region pair, from core to core with its control point pulled
/// halfway toward the origin, as `BUNDLE_SEGS + 1` points placed after the `first` texels.
fn tessellate(vis: &vis::Vis, sp: &space::Space, first: usize) -> (Vec<Bundle>, Vec<cam::V3>) {
    let n_max = vis.pairs.iter().map(|p| p.n).fold(0.0, f64::max);
    let mut bundles = Vec::with_capacity(vis.pairs.len());
    let mut pts = Vec::with_capacity(vis.pairs.len() * (BUNDLE_SEGS + 1));
    for p in &vis.pairs {
        let (Some(a), Some(b)) = (sp.regions.get(p.a), sp.regions.get(p.b)) else {
            continue;
        };
        let (a0, b0) = (a.core, b.core);
        let c = [
            (a0[0] + b0[0]) * 0.25,
            (a0[1] + b0[1]) * 0.25,
            (a0[2] + b0[2]) * 0.25,
        ];
        let at = (first + pts.len()) as u32;
        for k in 0..=BUNDLE_SEGS {
            let t = k as f64 / BUNDLE_SEGS as f64;
            let (u, m, v) = ((1.0 - t) * (1.0 - t), 2.0 * (1.0 - t) * t, t * t);
            pts.push([
                u * a0[0] + m * c[0] + v * b0[0],
                u * a0[1] + m * c[1] + v * b0[1],
                u * a0[2] + m * c[2] + v * b0[2],
            ]);
        }
        let (w, alpha) = vis::bundle_weight(p.n, n_max);
        bundles.push(Bundle {
            a: p.a,
            b: p.b,
            first: at,
            w: w as f32,
            alpha: alpha as f32,
        });
    }
    (bundles, pts)
}

/// What one change of the tiers hands the GL.
struct Tiered {
    inst: gl::Instances,
    /// the nodes drawn as points
    drawn: Vec<u32>,
    /// the active region's subfolders, drawn as glyphs
    subs: Vec<usize>,
    portals: Vec<Portal>,
}

/// Where a node's tier rides: its colour texel's fourth channel holds `style + TIER_K·tier`, with
/// tier 4 for a node that is not drawn, so the hooks count tiers from what was uploaded.
const TIER_K: f32 = 8.0;

/// A node's tier code as uploaded (0–3 drawn, 4 not), from its colour texel's fourth channel.
fn tier_code(packed: f32) -> usize {
    ((packed + 0.5) / TIER_K).floor() as usize
}

/// What is drawn, decided when the selection, the scale or show all changes (never per frame).
/// The tiers decide (`vis::tiers`): the focus and its links in any region, two hops and the rest
/// of the focused region inside it, collapsed folders standing for their tier-3 cards, or every
/// card under show all. With nothing selected, the active region's folders are drawn at system
/// scale. The edges are those the tier rule draws (`vis::edge_kind`): the focus's own in the
/// accent (out) or `--edge-hot` (in), between its links faint, out to two hops dim; its links in
/// regions out of focus are kept apart as portals, drawn per frame. The bundles between regions
/// are uploaded unless show all.
fn instances(gs: &G) -> Tiered {
    let grey = [0.55, 0.55, 0.55, 1.0];
    let (accent, halo, edge, edge_hot, bundle, (t2a, _)) = gs
        .colors
        .as_ref()
        .map(|c| (c.accent, c.halo, c.edge, c.edge_hot, c.bundle, c.tier_alpha))
        .unwrap_or((
            grey,
            grey,
            [0.5, 0.5, 0.5, 0.2],
            [0.8, 0.8, 0.8, 0.6],
            grey,
            (0.55, 0.22),
        ));
    let n = gs.nodes.len();
    let sel = gs.sel;
    let centre: Vec<usize> = sel.into_iter().collect();
    let regions: Vec<usize> = sel
        .and_then(|i| gs.vis.region.get(i).copied())
        .into_iter()
        .collect();
    let kids: &[usize] = if sel.is_some_and(|i| gs.nodes[i].kind == Kind::Dir) {
        &gs.kids
    } else {
        &[]
    };
    let folder: Vec<bool> = gs
        .nodes
        .iter()
        .map(|v| matches!(v.kind, Kind::Dir | Kind::Root))
        .collect();
    let focus = vis::Focus {
        centre: &centre,
        kids,
        regions: &regions,
    };
    let tier = vis::tiers(&gs.vis, &focus, &folder, &gs.fold, gs.show_all);
    let active = if gs.scale == Scale::Galaxy {
        None
    } else {
        gs.region
    };
    let on: Vec<bool> = (0..n)
        .map(|i| {
            let k = gs.nodes[i].kind;
            let r = gs.vis.region.get(i).copied();
            k != Kind::Root
                && (tier[i] != vis::HIDDEN
                    || (sel.is_none() && k == Kind::Dir && r.is_some() && r == active))
        })
        .collect();
    let drawn: Vec<u32> = (0..n).filter(|&i| on[i]).map(|i| i as u32).collect();
    let subs: Vec<usize> = match active {
        Some(r) => (0..n)
            .filter(|&i| {
                gs.nodes[i].kind == Kind::Dir
                    && gs.vis.region.get(i) == Some(&r)
                    && gs.region_node.get(r).copied().flatten() != Some(i)
            })
            .collect(),
        None => Vec::new(),
    };
    let extra = gs.bundle_pts.len();
    let mut inst = gl::Instances {
        pos: Vec::with_capacity((n + extra) * 4),
        col: Vec::with_capacity((n + extra) * 4),
        ends: Vec::with_capacity(gs.edges.len() * 2),
        ecol: Vec::with_capacity(gs.edges.len() * 6),
        n_links: 0,
        n_bundles: 0,
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
        let code = if !on[i] { 4 } else { tier[i].min(3) };
        inst.col
            .extend([rgb[0], rgb[1], rgb[2], st + TIER_K * code as f32]);
    }
    for p in &gs.bundle_pts {
        inst.pos
            .extend([p[0] as f32, p[1] as f32, p[2] as f32, 0.0]);
        inst.col.extend([0.0, 0.0, 0.0, TIER_K * 4.0]);
    }
    let link = gl::group::LINK;
    let mut hot: Vec<(u32, u32, [f32; 4], f32)> = Vec::new();
    let mut portals = Vec::new();
    for &(s, t, surface) in &gs.edges {
        let (si, ti) = (s as usize, t as usize);
        if !gs.show_all && !(on[si] && on[ti]) {
            continue;
        }
        let Some(kind) = vis::edge_kind(&gs.vis, &tier, &regions, si, ti, gs.show_all) else {
            continue;
        };
        let (out, inn) = (Some(si) == sel, Some(ti) == sel);
        let a = if surface { edge[3] * 0.6 } else { edge[3] };
        let (c, w) = match kind {
            _ if out => ([accent[0], accent[1], accent[2], 0.95], 1.6),
            _ if inn => (edge_hot, 1.4),
            vis::EdgeKind::Hot | vis::EdgeKind::Portal => (edge_hot, 1.4),
            vis::EdgeKind::Faint => ([edge[0], edge[1], edge[2], a * 0.8], 1.0),
            vis::EdgeKind::Dim => ([edge[0], edge[1], edge[2], a * t2a], 1.0),
            vis::EdgeKind::Plain => ([edge[0], edge[1], edge[2], a], 1.0),
        };
        match kind {
            vis::EdgeKind::Portal => {
                let (from, to) = if out { (si, ti) } else { (ti, si) };
                portals.push(Portal {
                    from,
                    to,
                    region: gs.vis.region[to],
                    rgba: c,
                    w,
                });
            }
            _ if out || inn => hot.push((s, t, c, w)),
            _ => {
                inst.ends.extend([s, t]);
                inst.ecol.extend([c[0], c[1], c[2], c[3], w, link]);
            }
        }
    }
    for (s, t, c, w) in hot {
        inst.ends.extend([s, t]);
        inst.ecol.extend([c[0], c[1], c[2], c[3], w, link]);
    }
    inst.n_links = inst.ends.len() / 2;
    if !gs.show_all {
        for b in &gs.bundles {
            for k in 0..BUNDLE_SEGS as u32 {
                inst.ends.extend([b.first + k, b.first + k + 1]);
                inst.ecol.extend([
                    bundle[0],
                    bundle[1],
                    bundle[2],
                    b.alpha,
                    b.w,
                    gl::group::BUNDLE,
                ]);
            }
        }
        inst.n_bundles = gs.bundles.len();
    }
    Tiered {
        inst,
        drawn,
        subs,
        portals,
    }
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
        "<span style=\"color:var(--faint)\">drag to orbit · wheel to zoom across scales · 0 fits</span>",
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
        let rim1 = rim_of(gs.scale);
        match mv {
            Move::Ease(ms) | Move::Cross(ms) if !reduced && gs.w > 0.0 => {
                let (from, rim0) = (gs.pose, gs.rim);
                gs.anim = Some(CamAnim {
                    from,
                    to,
                    t0: dom::perf_now(),
                    ms,
                    rim0,
                    rim1,
                    cross: matches!(mv, Move::Cross(_)),
                });
            }
            _ => {
                gs.pose = to;
                gs.anim = None;
                gs.rim = rim1;
            }
        }
        gs.moves
    };
    if let Some(cv) = dom::try_id("graph") {
        let _ = cv.set_attribute("data-move", &n.to_string());
    }
    schedule();
}

/// `0`, the fit button, a double-click on empty space, and an empty selection: the whole graph at
/// galaxy scale, from where the camera looks now. The selection is kept.
pub fn fit() {
    let (to, rescaled) = {
        let mut gs = g();
        gs.autofit = true;
        gs.fit = Fit::Galaxy;
        let rescaled = gs.set_scale(Scale::Galaxy, None);
        let to = Pose {
            target: [0.0; 3],
            ln_d: gs.d_galaxy().ln(),
            ..gs.dest()
        };
        (to, rescaled)
    };
    cam_to(to, Move::Cross(cam::EASE_MS));
    if rescaled {
        retier();
    }
}

/// `+`/`=` (f > 1, closer) and `-` (f < 1, farther): the camera distance, about the target.
/// Repeated presses chain from where the last one was going.
pub fn zoom_by(f: f64) {
    zoom(-f.ln(), true);
}

/// A user zoom by `dl` in `ln d`: the wheel or a pinch (from where the camera is, as a cut) or a
/// key (`keyed`: from where it is going, eased). Only user zooms consult the bands, and crossing
/// one changes the scale, never the selection:
/// - out past system: the target eases to the origin (galaxy);
/// - in at galaxy: the region nearest the pane centre becomes the active region, and the camera
///   eases onto it (system);
/// - in at system, with a card selected in the active region: back onto the card (focus);
/// - out from focus: system, about the same target.
///
/// A zoom during a band-crossing ease moves both of its ends instead of cancelling it.
fn zoom(dl: f64, keyed: bool) {
    enum Act {
        Adjusted,
        Plain(Pose),
        Crossed(Pose, Move),
    }
    let act = {
        let mut gs = g();
        gs.autofit = false;
        let (lo, hi) = (gs.d_min().ln(), (1.5 * gs.d_galaxy()).ln());
        let crossing = gs.anim.filter(|a| a.cross);
        let (base, ln_d) = match crossing {
            Some(mut a) => {
                a.from.ln_d = (a.from.ln_d + dl).clamp(lo, hi);
                a.to.ln_d = (a.to.ln_d + dl).clamp(lo, hi);
                gs.anim = Some(a);
                (a.to, a.to.ln_d)
            }
            None => {
                let p = if keyed { gs.dest() } else { gs.pose };
                (p, (p.ln_d + dl).clamp(lo, hi))
            }
        };
        let prev = gs.scale;
        let next = cam::classify(ln_d.exp(), &gs.bands(), prev, gs.innermost());
        let landing = match next {
            _ if next == prev => None,
            Scale::Galaxy => {
                gs.set_scale(Scale::Galaxy, None);
                Some((
                    Pose {
                        target: [0.0; 3],
                        ln_d,
                        ..base
                    },
                    Move::Cross(cam::EASE_MS),
                ))
            }
            Scale::System if prev == Scale::Galaxy => {
                let cands: Vec<(usize, cam::V3)> = (0..gs.space.regions.len())
                    .filter(|&r| gs.vis.is_region(r))
                    .map(|r| (r, gs.space.regions[r].core))
                    .collect();
                let pick = {
                    let c: Vec<cam::Candidate> = cands
                        .iter()
                        .map(|&(r, core)| cam::Candidate {
                            core,
                            cards: gs.vis.cards.get(r).copied().unwrap_or(0),
                            name: &gs.vis.regions[r],
                        })
                        .collect();
                    cam::nearest_to_centre(&c, &gs.pose, gs.pane())
                };
                pick.map(|k| {
                    let r = cands[k].0;
                    let (target, d, _) = gs.region_fit(r, base.theta, base.psi);
                    gs.set_scale(Scale::System, Some(r));
                    (
                        Pose {
                            target,
                            ln_d: ln_d.min(d.ln()),
                            ..base
                        },
                        Move::Cross(cam::EASE_MS),
                    )
                })
            }
            Scale::System => {
                let r = gs.region;
                gs.set_scale(Scale::System, r);
                Some((Pose { ln_d, ..base }, plain(keyed)))
            }
            Scale::Focus => gs.sel.map(|i| {
                let r = gs.region_of(i);
                gs.set_scale(Scale::Focus, r);
                let (to, _, _) = gs.card_view(i);
                (
                    Pose {
                        ln_d: ln_d.min(to.ln_d),
                        ..to
                    },
                    Move::Ease(cam::EASE_MS),
                )
            }),
        };
        match (landing, crossing) {
            (Some((to, mv)), _) => Act::Crossed(to, mv),
            (None, Some(_)) => Act::Adjusted,
            (None, None) => Act::Plain(Pose { ln_d, ..base }),
        }
    };
    match act {
        Act::Adjusted => schedule(),
        Act::Plain(to) => cam_to(to, plain(keyed)),
        Act::Crossed(to, mv) => {
            cam_to(to, mv);
            retier();
        }
    }
}

/// A zoom that crosses no band: a key eases, the wheel cuts.
fn plain(keyed: bool) -> Move {
    if keyed {
        Move::Ease(cam::KEY_MS)
    } else {
        Move::Cut
    }
}

/// The orbit keys: turn the azimuth and tilt the elevation by degrees, about the target.
pub fn orbit_by(d_theta: f64, d_psi: f64) {
    let to = {
        let gs = g();
        cam::orbit(&gs.dest(), d_theta.to_radians(), d_psi.to_radians())
    };
    cam_to(to, Move::Ease(cam::KEY_MS));
}

/// Ease the camera onto a node: at distance `d`, keeping the scale and the angles, or, for a
/// selection (`d` is `None`), at the scale the selection sets: a card is solved for (focus), a
/// folder brings its region to system scale, the root the galaxy.
pub fn focus_node(id: &str, d: Option<f64>) {
    let Some(d) = d else {
        match id {
            "." => fit(),
            _ if state::idx().is_some_and(|x| x.is_dir(id)) => frame_region(id),
            _ => frame_card(id),
        }
        return;
    };
    let to = {
        let mut gs = g();
        let Some(&i) = gs.at.get(id) else { return };
        gs.autofit = false;
        Pose {
            target: gs.space.pos[i],
            ln_d: d.ln(),
            ..gs.dest()
        }
    };
    cam_to(to, Move::Ease(cam::EASE_MS));
}

/// A card is selected: focus scale, the camera turned and fitted to the card and its links in its
/// region (`card_view`), eased while the graph is shown and cut while it is hidden.
fn frame_card(id: &str) {
    let (to, rescaled) = {
        let mut gs = g();
        let Some(&i) = gs.at.get(id) else { return };
        let (to, pts, ids) = gs.card_view(i);
        gs.autofit = true;
        gs.fit = Fit::Points(pts, FOCUS_PAD);
        gs.focus_ids = Rc::new(ids);
        let r = gs.region_of(i);
        (to, gs.set_scale(Scale::Focus, r))
    };
    cam_to(to, Move::Ease(cam::EASE_MS));
    if rescaled {
        retier();
    }
}

/// A folder is selected: its region, fitted to the pane from where the camera looks now, at
/// system scale.
fn frame_region(id: &str) {
    let plan = {
        let mut gs = g();
        let Some(&i) = gs.at.get(id) else { return };
        gs.region_of(i).map(|r| {
            let dest = gs.dest();
            let (target, d, pts) = gs.region_fit(r, dest.theta, dest.psi);
            gs.autofit = true;
            gs.fit = Fit::Points(pts, FIT_PAD);
            let rescaled = gs.set_scale(Scale::System, Some(r));
            (
                Pose {
                    target,
                    ln_d: d.ln(),
                    ..dest
                },
                rescaled,
            )
        })
    };
    match plan {
        Some((to, rescaled)) => {
            cam_to(to, Move::Cross(cam::EASE_MS));
            if rescaled {
                retier();
            }
        }
        None => fit(),
    }
}

// ---- selection -----------------------------------------------------------------------------

/// `updateGraph()`: the selection changed (or what is drawn did): the camera follows a new
/// selection, from any source, to the scale it sets (a card solved for at focus scale, a folder's
/// region at system scale, no selection the galaxy), then new instances and the hint.
pub fn update() {
    let Some(idx) = state::idx() else { return };
    let sel = state::sel();
    let kids: Vec<String> = match sel.as_deref() {
        Some(s) if s != "." && idx.is_dir(s) => idx.kids_of(s).to_vec(),
        _ => Vec::new(),
    };
    let moved = {
        let mut gs = g();
        if !gs.built {
            return;
        }
        gs.sel = sel.as_deref().and_then(|s| gs.at.get(s).copied());
        gs.kids = kids.iter().filter_map(|k| gs.at.get(k).copied()).collect();
        let moved = gs.sel_id != sel;
        gs.sel_id = sel.clone();
        moved
    };
    if moved {
        match sel.as_deref() {
            None | Some(".") => fit(),
            Some(s) if idx.is_dir(s) => frame_region(s),
            Some(s) => frame_card(s),
        }
    }
    retier();
    let hint = {
        let gs = g();
        let rsel = sel.as_deref().map(|s| gs.lay.rep_or(s).to_string());
        match (&sel, &rsel) {
            (Some(s), Some(r)) if s != r => {
                let name = state::idx().map(|i| i.name_of(r)).unwrap_or_default();
                format!("{name} is collapsed · E to expand")
            }
            _ => String::new(),
        }
    };
    set_hint(&hint);
}

/// Decide again what is drawn (the selection, the scale or show all changed) and hand it to the
/// GL.
fn retier() {
    let show_all = cfg().show_all;
    {
        let mut gs = g();
        if !gs.built {
            return;
        }
        gs.show_all = show_all;
        let t = instances(&gs);
        gs.inst = Rc::new(t.inst);
        gs.drawn = Rc::new(t.drawn);
        gs.subs = t.subs;
        gs.portals = t.portals;
    }
    sync_instances();
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
                gs.rim = a.rim1;
                gs.anim = None;
            } else {
                let e = cam::smootherstep(u);
                gs.pose = cam::ease_pose(&a.from, &a.to, e);
                gs.rim = a.rim0 + (a.rim1 - a.rim0) * e;
            }
        }
        render(&mut gs, now)
    };
    if let Some((drawn, uploaded, labels)) = out {
        if let Some(cv) = dom::try_id("graph") {
            gl::publish(&cv, &drawn);
            overlay::publish(&cv, &labels);
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

/// The glyphs of one frame: screen-space marks for the GL, names for the overlay, picks, and
/// what was drawn, for the hooks.
#[derive(Default)]
struct Glyphs {
    under: Vec<gl::Mark>,
    halos: Vec<gl::Mark>,
    marks: Vec<gl::Mark>,
    names: Vec<(usize, overlay::Name)>,
    picks: Vec<Pick>,
    regions: usize,
    rim: usize,
    rim_bundles: usize,
    /// view depths of the glyphs in front of the camera
    depths: Vec<f64>,
    /// where each region's glyph was drawn, in pane CSS px (portals run out to it)
    at: Vec<Option<[f64; 2]>>,
}

fn rgba(c: [f32; 4], a: f64) -> [f32; 4] {
    [c[0], c[1], c[2], (c[3] as f64 * a) as f32]
}

fn lerp2(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn mark(p: [f64; 2], r: f64, style: f32, rgba: [f32; 4]) -> gl::Mark {
    let a = [p[0] as f32, p[1] as f32];
    gl::Mark {
        a,
        b: a,
        r: r as f32,
        style,
        rgba,
    }
}

/// The region glyphs for this frame. At galaxy scale (rim weight 0) each region is a soft halo at
/// its projected core, sized by its cards; where other regions sit on the rim (weight 1) each is a
/// small disc on the rim at its true direction, hollow when its core is behind the camera, with a
/// line from the active region's core; between, every glyph slides from one place to the other.
/// The active region keeps its glyph at its core, and its subfolders get glyphs of their own. The
/// root's cards and the external systems get one unlabelled glyph each, never on the rim.
fn glyphs(gs: &G, pose: &Pose, pane: Pane, unit: f64) -> Glyphs {
    let mut out = Glyphs::default();
    let Some(c) = gs.colors.as_ref() else {
        return out;
    };
    let w = gs.rim;
    let ga = c.glyph_alpha as f64;
    let inset = cam::Insets {
        left: c.rim_inset.max(gs.chrome.0 + 14.0),
        top: c.rim_inset,
        right: c.rim_inset,
        bottom: c.rim_inset.max(gs.chrome.1 + 14.0),
    };
    let centre = [pane.w / 2.0, pane.h / 2.0];
    let regions = gs.space.regions.len().min(gs.vis.regions.len());
    // where each region's glyph landed, for the lines out to the rim
    let mut at: Vec<Option<[f64; 2]>> = vec![None; regions];
    let fam_of = |r: usize| {
        gs.region_node
            .get(r)
            .copied()
            .flatten()
            .map(|i| c.fam(&gs.nodes[i].fam))
            .unwrap_or_else(|| c.fam("other"))
    };
    let halo_px = |world: f64, s: f64| (world * unit / s).max(DOT_PX * 2.0);
    for r in (0..regions).filter(|&r| gs.vis.is_region(r)) {
        let core = gs.space.regions[r].core;
        let cards = gs.vis.cards.get(r).copied().unwrap_or(0) as f64;
        let col = fam_of(r);
        let world = GLYPH_R0 + GLYPH_K * cards.sqrt();
        let proj = cam::project(core, pose, pane);
        let active = gs.region == Some(r) && gs.scale != Scale::Galaxy;
        let (p, alpha, radius) = if active {
            let Some([x, y, s]) = proj else { continue };
            out.depths.push(s);
            out.halos.push(mark(
                [x, y],
                halo_px(world, s),
                gl::mark::HALO,
                rgba(col, ga * 0.6),
            ));
            ([x, y], 1.0, DOT_PX)
        } else {
            let rp = cam::rim_point(core, pose, pane, inset);
            let (p, front) = match proj {
                Some([x, y, s]) => {
                    out.depths.push(s);
                    if w < 1.0 {
                        let hr = halo_px(world, s) * (1.0 - w) + RIM_PX * w;
                        out.halos.push(mark(
                            lerp2([x, y], rp, w),
                            hr,
                            gl::mark::HALO,
                            rgba(col, ga * (1.0 - w)),
                        ));
                    }
                    (lerp2([x, y], rp, w), true)
                }
                None => (rp, false),
            };
            let alpha = if front { 1.0 } else { w };
            if alpha < 0.02 {
                continue;
            }
            let radius = DOT_PX + (RIM_PX + 0.25 * cards.sqrt().min(8.0) - DOT_PX) * w;
            if w > 0.5 {
                out.rim += 1;
            }
            let style = if front {
                gl::mark::DISC
            } else {
                gl::mark::RING
            };
            out.marks
                .push(mark(p, radius, style, rgba(col, 0.95 * alpha)));
            (p, alpha, radius)
        };
        if active {
            out.marks
                .push(mark(p, radius, gl::mark::DISC, rgba(col, 0.95)));
        }
        out.regions += 1;
        at[r] = Some(p);
        if let Some(i) = gs.region_node.get(r).copied().flatten() {
            out.picks.push(Pick {
                x: p[0],
                y: p[1],
                r: radius + 4.0,
                i,
            });
        }
        if alpha > 0.5 {
            // a name leans toward the pane centre once its glyph is on the rim
            let (dx, dy) = (centre[0] - p[0], centre[1] - p[1]);
            let l = dx.hypot(dy).max(1e-9);
            let lean = if active { 0.0 } else { w };
            out.names.push((
                cards as usize,
                overlay::Name {
                    x: p[0],
                    y: p[1],
                    r: radius,
                    inward: [dx / l * lean + (1.0 - lean), dy / l * lean],
                    text: gs.vis.regions[r].trim_end_matches('/').to_string(),
                },
            ));
        }
    }
    // the active region's subfolders
    for &i in &gs.subs {
        if let Some([x, y, s]) = cam::project(gs.space.pos[i], pose, pane) {
            let v = &gs.nodes[i];
            let world = SUB_R0 + SUB_K * v.files.sqrt();
            out.halos.push(mark(
                [x, y],
                halo_px(world, s),
                gl::mark::HALO,
                rgba(c.fam(&v.fam), ga * 0.5 * w),
            ));
        }
    }
    // lines from the active region's core out to the others' glyphs
    let active = gs.region.filter(|_| w > 0.0 && !gs.show_all);
    if let Some((a, from)) = active.and_then(|a| Some((a, at.get(a).copied().flatten()?))) {
        for b in gs.bundles.iter().filter(|b| b.a == a || b.b == a) {
            let other = if b.a == a { b.b } else { b.a };
            if let Some(to) = at.get(other).copied().flatten() {
                out.under.push(gl::Mark {
                    a: [from[0] as f32, from[1] as f32],
                    b: [to[0] as f32, to[1] as f32],
                    r: b.w * 0.5,
                    style: gl::mark::DISC,
                    rgba: rgba(c.bundle, b.alpha as f64 * w),
                });
                out.rim_bundles += 1;
            }
        }
    }
    // the root's cards at the centre, and the external systems: one unlabelled glyph each
    let root = gs.region_node.first().copied().flatten();
    let n_root = gs.vis.cards.first().copied().unwrap_or(0) as f64;
    let mut pseudo = vec![(
        [0.0; 3],
        c.accent,
        GLYPH_R0 * 0.7 + 4.0 * n_root.sqrt(),
        gl::mark::DISC,
        root.or_else(|| gs.at.get(".").copied()),
    )];
    if let (true, Some(sr)) = (gs.n_surf > 0, gs.space.regions.get(1)) {
        let world = GLYPH_R0 * 0.7 + 4.0 * (gs.n_surf as f64).sqrt();
        pseudo.push((sr.core, c.fam("world"), world, gl::mark::RING, None));
    }
    for (p, col, world, style, pick) in pseudo {
        if let Some([x, y, s]) = cam::project(p, pose, pane) {
            out.depths.push(s);
            out.halos.push(mark(
                [x, y],
                halo_px(world, s),
                gl::mark::HALO,
                rgba(col, ga * 0.7),
            ));
            out.marks
                .push(mark([x, y], DOT_PX + 0.5, style, rgba(col, 0.9)));
            if let Some(i) = pick {
                out.picks.push(Pick {
                    x,
                    y,
                    r: DOT_PX + 4.0,
                    i,
                });
            }
        }
    }
    out.at = at;
    out
}

/// Draw one frame at the current pose: project the drawn points, sort them far to near, place the
/// glyphs, draw the GL scene and the overlay, and keep what was drawn for picking and the hooks.
fn render(gs: &mut G, now: f64) -> Option<(gl::Drawn, Uploaded, Vec<overlay::Placed>)> {
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
    let drawn = gs.drawn.clone();
    let mut items: Vec<(f64, u32, f64, f64)> = Vec::with_capacity(drawn.len());
    // points behind the camera are drawn too (the GL drops them), first, so the draw list is the
    // whole tier set wherever the camera is
    let mut behind: Vec<u32> = Vec::new();
    for &i in drawn.iter() {
        match cam::project(gs.space.pos[i as usize], &pose, pane) {
            Some([x, y, s]) => items.push((s, i, x, y)),
            None => behind.push(i),
        }
    }
    items.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut gly = glyphs(gs, &pose, pane, unit);
    let (near, far) = items
        .iter()
        .map(|it| it.0)
        .chain(gly.depths.iter().copied())
        .fold((f64::MAX, 0.0f64), |(n, f), s| (n.min(s), f.max(s)));
    let (near, far) = if near > far { (1.0, 2.0) } else { (near, far) };
    let order: Vec<u32> = behind
        .iter()
        .copied()
        .chain(items.iter().map(|it| it.1))
        .collect();
    // the cards in the draw list: by tier (as uploaded) and by region
    let mut tiers = [0usize; 4];
    let mut by_region: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for &i in &order {
        let i = i as usize;
        if gs.nodes[i].kind != Kind::File {
            continue;
        }
        if let Some(t) = gs
            .inst
            .col
            .get(i * 4 + 3)
            .and_then(|c| tiers.get_mut(tier_code(*c)))
        {
            *t += 1;
        }
        if let Some(k) = gs.vis.region.get(i).and_then(|&r| gs.vis.regions.get(r)) {
            *by_region.entry(k.as_str()).or_insert(0) += 1;
        }
    }
    let cards = tiers.iter().sum::<usize>();
    let focus_n = gs
        .inst
        .col
        .as_chunks::<4>()
        .0
        .iter()
        .take(gs.nodes.len())
        .filter(|c| tier_code(c[3]) <= 1)
        .count();
    // the focus's halo: a soft glow around its point, light added on a dark background
    if let (Some(c), Some(&(s, _, x, y))) = (
        gs.colors.as_ref(),
        gs.sel
            .and_then(|i| items.iter().find(|it| it.1 as usize == i)),
    ) {
        let rpx = (gs.sel.map(|i| gs.nodes[i].r).unwrap_or(CARD_R) * unit / s).min(POINT_MAX_PX);
        gly.halos.push(mark(
            [x, y],
            (rpx * 3.0).max(14.0),
            gl::mark::HALO,
            rgba(c.halo, 0.32),
        ));
    }
    // portals: from the focus to the card when it is on the pane, else to its region's glyph
    let mut n_portals = 0;
    for p in &gs.portals {
        let Some([fx, fy, _]) = cam::project(gs.space.pos[p.from], &pose, pane) else {
            continue;
        };
        let to = cam::project(gs.space.pos[p.to], &pose, pane)
            .filter(|q| q[0] >= 0.0 && q[1] >= 0.0 && q[0] <= pane.w && q[1] <= pane.h)
            .map(|q| [q[0], q[1]])
            .or_else(|| gly.at.get(p.region).copied().flatten());
        if let Some(to) = to {
            gly.under.push(gl::Mark {
                a: [fx as f32, fy as f32],
                b: [to[0] as f32, to[1] as f32],
                r: p.w * 0.5,
                style: gl::mark::DISC,
                rgba: p.rgba,
            });
            n_portals += 1;
        }
    }
    let mut picks: Vec<Pick> = items
        .iter()
        .rev()
        .map(|&(s, i, x, y)| Pick {
            x,
            y,
            r: (gs.nodes[i as usize].r * unit / s).clamp(1.6, POINT_MAX_PX),
            i: i as usize,
        })
        .collect();
    picks.extend(gly.picks);
    let (bg, fog, accent_css, additive, tier_alpha) = gs
        .colors
        .as_ref()
        .map(|c| (c.bg, c.fog, c.accent_css.clone(), c.dark, c.tier_alpha))
        .unwrap_or((
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            "#ffa028".into(),
            true,
            (0.55, 0.22),
        ));
    let vp = cam::view_proj(&pose, pane, gs.space.radius).map(|v| v as f32);
    let fade = if gs.show_all {
        0.0
    } else {
        (1.0 - gs.rim) as f32
    };
    let counts = gs.gl.as_ref().and_then(|r| {
        r.draw(&gl::Frame {
            vp,
            order: &order,
            px: (bw, bh),
            scale: (unit * gs.dpr) as f32,
            dpr: gs.dpr as f32,
            depth: (near as f32, far as f32),
            bg,
            fog,
            bundle_fade: fade,
            tier_alpha,
            r_max: POINT_MAX_PX as f32,
            under: &gly.under,
            halos: &gly.halos,
            marks: &gly.marks,
            additive,
        })
    });
    // the overlay: region names (the larger regions first), hover, the walk's place, the flash
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
    let mut names = gly.names;
    names.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.text.cmp(&b.1.text)));
    let names: Vec<overlay::Name> = names.into_iter().map(|(_, n)| n).collect();
    // the compass: the galaxy's axes and horizon as the camera sees them, the focused region's
    // direction, and the scale
    let b = cam::basis(pose.theta, pose.psi);
    let compass = gs.colors.as_ref().map(|c| overlay::Compass {
        r: b.r,
        u: b.u,
        dir: b.dir,
        region: gs
            .region
            .filter(|_| gs.scale != Scale::Galaxy)
            .and_then(|r| gs.space.regions.get(r))
            .map(|g| g.core),
        scale: gs.scale.name(),
        // above the legend when the legend reaches under the compass
        lift: if gs.chrome.2 > gs.w - overlay::COMPASS_INSET - overlay::COMPASS_PX - 4.0 {
            gs.chrome.1
        } else {
            0.0
        },
        ink: c.compass.0.clone(),
        far: c.compass.1.clone(),
        accent: accent_css.clone(),
    });
    let labels = match &gs.ov {
        Some(ov) => ov.draw(
            gs.w,
            gs.h,
            gs.dpr,
            &accent_css,
            &rings,
            &names,
            compass.as_ref(),
        ),
        None => Vec::new(),
    };
    gs.picks = picks;
    let (edges, gl_bundles) = counts?;
    let drawn = gl::Drawn {
        scale: gs.scale.name(),
        region: gs
            .region
            .filter(|_| gs.scale != Scale::Galaxy)
            .and_then(|r| gs.vis.regions.get(r).cloned())
            .unwrap_or_default(),
        show_all: gs.show_all,
        regions: gly.regions,
        rim: gly.rim,
        bundles: if gl_bundles > 0 {
            gl_bundles
        } else {
            gly.rim_bundles
        },
        theta_deg: pose.theta.to_degrees(),
        psi_deg: pose.psi.to_degrees(),
        dist: pose.d(),
        anim: gs.anim.is_some(),
        vp,
        cards,
        edges: edges + n_portals,
        pos_hash: gs.gl.as_ref().map(|r| r.pos_hash()).unwrap_or(0),
        dist_galaxy: gs.d_galaxy(),
        focus: gs.sel.map(|i| gs.ids[i].clone()).unwrap_or_default(),
        focus_n,
        focus_ids: serde_json::to_string(&*gs.focus_ids).unwrap_or_default(),
        tiers,
        drawn_regions: serde_json::to_string(&by_region).unwrap_or_default(),
    };
    Some((drawn, uploaded, labels))
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
    let (to, rescaled) = {
        let mut gs = g();
        let Some(&i) = gs.at.get(&sel) else { return };
        let r = gs.region_of(i);
        let rescaled = gs.set_scale(Scale::Focus, r);
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
        gs.fit = Fit::Points(pts, FIT_PAD);
        let to = Pose {
            target,
            ln_d: d.ln(),
            ..dest
        };
        (to, rescaled)
    };
    cam_to(to, Move::Ease(cam::EASE_MS));
    if rescaled {
        retier();
    }
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

/// Toggle "show all" (`settings.showAll`): every card and every link edge, at every scale,
/// whatever is collapsed.
pub fn toggle_show_all() {
    {
        let mut c = state::cfg_mut();
        c.show_all = !c.show_all;
    }
    state::save_settings();
    mark_show_all();
    retier();
}

/// The toolbar's show-all button follows the setting.
fn mark_show_all() {
    let on = cfg().show_all;
    if let Some(b) = dom::q(".gtools [data-z=all]") {
        dom::toggle(&b, "on", on);
    }
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
    let chrome = chrome_insets();
    let refit = {
        let mut gs = g();
        gs.chrome = chrome;
        let changed = gs.w != r.width() || gs.h != r.height();
        gs.w = r.width();
        gs.h = r.height();
        gs.host_rect = (r.left(), r.top(), r.width(), r.height());
        gs.dpr = dom::win().device_pixel_ratio();
        if changed && gs.autofit && gs.built && gs.w > 0.0 && gs.anim.is_none() {
            let p = gs.pose;
            let d = match &gs.fit {
                Fit::Galaxy => gs.d_galaxy(),
                Fit::Points(pts, pad) => {
                    cam::fit_distance(pts, p.target, p.theta, p.psi, gs.pane(), *pad, gs.d_min())
                }
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
        let h = g().h;
        zoom(
            cam::wheel_delta(e.delta_y(), e.delta_mode(), e.ctrl_key(), h),
            false,
        );
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
                "all" => toggle_show_all(),
                _ => {}
            }
        });
    }
    mark_show_all();
    resize();
}
