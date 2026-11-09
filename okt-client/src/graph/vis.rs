//! What the graph draws, as pure data: the region each node belongs to, who links to whom, how
//! many links join each pair of regions, how heavily the bundle between them is drawn, and, for a
//! focus, which tier each node is drawn at and which edges are drawn.
//!
//! **Tiers** count link hops from the focus: 0 the focus, 1 its direct links, 2 two hops, 3 the
//! rest of the focused region. Hops never pass through a folder, or every card in it would be two
//! hops from anything linked to it. The focus set (tiers 0 and 1) is drawn in any region; tiers 2
//! and 3 only inside the focused region, and tier 3 only when there is one focused region (several,
//! as for a search's matches, would bring every card of each back). Everything else folds into
//! its region's glyph. A collapsed folder folds its tier-3 cards into its own point; "show all"
//! hides nothing.
//!
//! A region is a top-level folder. The root's own cards (the core, `.`) and the external systems
//! (`@`) are pseudo-regions: each gets a glyph of its own, but neither counts as a region, gets a
//! name, or gets a bundle, so their links appear only when a focus reaches them.
//!
//! It uses `std` only and iterates sorted collections only, so `cargo test` runs it natively and
//! the result never depends on hashing.

use std::collections::BTreeMap;

/// The pseudo-region of the root's own cards.
pub const ROOT: &str = ".";
/// The pseudo-region of the external systems.
pub const SURFACES: &str = "@";

/// One node, as visibility sees it.
#[derive(Clone, Copy, Debug)]
pub struct VNode<'a> {
    pub id: &'a str,
    /// an external system (`@…`)
    pub surface: bool,
    /// a card (a file), as opposed to a folder, the root or a surface
    pub file: bool,
}

/// The links between two regions, `a < b`, summed over every edge's link count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pair {
    pub a: usize,
    pub b: usize,
    pub n: f64,
}

/// Visibility data, computed once per index load.
#[derive(Clone, Debug, Default)]
pub struct Vis {
    /// node id to index (a focus set's ids resolve through it)
    #[cfg_attr(not(test), allow(dead_code))]
    pub ix: BTreeMap<String, usize>,
    /// every node's neighbours over link edges, both directions, sorted and distinct (the walk
    /// out from a focus runs over it)
    pub adj: Vec<Vec<usize>>,
    /// each node's region, as an index into `regions`
    pub region: Vec<usize>,
    /// region keys: the given order, then any key a node named that the order lacked
    pub regions: Vec<String>,
    /// cards (files) per region
    pub cards: Vec<usize>,
    /// region pairs joined by at least one link, root and surfaces excluded, ordered by `(a, b)`
    pub pairs: Vec<Pair>,
}

impl Vis {
    /// A top-level folder, as opposed to the root's cards or the external systems.
    pub fn is_region(&self, r: usize) -> bool {
        self.regions
            .get(r)
            .is_some_and(|k| k != ROOT && k != SURFACES)
    }
}

/// The region key of a node: its top-level folder (with the trailing `/` folder ids carry), the
/// root core `.` for the root and its own cards, or `@` for an external system.
pub fn region_key(id: &str, surface: bool) -> &str {
    if surface {
        return SURFACES;
    }
    match id.find('/') {
        Some(k) => &id[..=k],
        None => ROOT,
    }
}

/// Regions, adjacency and region-pair counts. `edges` are `(source, target, link count)` by node
/// index; `order` is the region order the layout placed them in, so region indices agree with it.
pub fn build(nodes: &[VNode], edges: &[(usize, usize, f64)], order: &[String]) -> Vis {
    let ix: BTreeMap<String, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.to_string(), i))
        .collect();
    let mut regions: Vec<String> = order.to_vec();
    let mut rix: BTreeMap<String, usize> = regions
        .iter()
        .enumerate()
        .map(|(i, k)| (k.clone(), i))
        .collect();
    let mut region = Vec::with_capacity(nodes.len());
    for n in nodes {
        let k = region_key(n.id, n.surface);
        let r = match rix.get(k) {
            Some(&r) => r,
            None => {
                regions.push(k.to_string());
                rix.insert(k.to_string(), regions.len() - 1);
                regions.len() - 1
            }
        };
        region.push(r);
    }
    let mut cards = vec![0; regions.len()];
    for (n, &r) in nodes.iter().zip(&region) {
        if n.file {
            cards[r] += 1;
        }
    }
    let mut adj = vec![Vec::new(); nodes.len()];
    let mut sums: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    let pseudo = |r: usize| regions[r] == ROOT || regions[r] == SURFACES;
    for &(s, t, n) in edges {
        if s >= nodes.len() || t >= nodes.len() {
            continue;
        }
        adj[s].push(t);
        adj[t].push(s);
        let (a, b) = (region[s], region[t]);
        if a != b && !pseudo(a) && !pseudo(b) {
            *sums.entry((a.min(b), a.max(b))).or_insert(0.0) += n;
        }
    }
    for a in &mut adj {
        a.sort_unstable();
        a.dedup();
    }
    let pairs = sums
        .into_iter()
        .map(|((a, b), n)| Pair { a, b, n })
        .collect();
    Vis {
        ix,
        adj,
        region,
        regions,
        cards,
        pairs,
    }
}

/// How heavily a bundle of `n` links is drawn, when the heaviest bundle carries `n_max`: its width
/// in CSS px, `w0 + w1·ln(1+n)`, and its alpha, rising with `ln(1+n) / ln(1+n_max)`. Both rise
/// with the count.
pub fn bundle_weight(n: f64, n_max: f64) -> (f64, f64) {
    let l = (1.0 + n.max(0.0)).ln();
    let lmax = (1.0 + n_max.max(n).max(0.0)).ln();
    let frac = if lmax > 0.0 { l / lmax } else { 0.0 };
    (0.8 + 0.9 * l, 0.10 + 0.55 * frac)
}

/// Not drawn: folded into its region's glyph, or into a collapsed folder's point.
pub const HIDDEN: u8 = u8::MAX;

/// What is in focus: the nodes at its centre (tier 0), nodes that join tier 1 without a link (a
/// selected folder's own children), and the focused regions. No centre and no regions is no
/// selection: nothing but the glyphs is drawn. `links` says whether the centre's links join tier
/// 1: they do for a selection; a results list's matches framed in place are the whole focus set,
/// so nothing past them is drawn but the rest of a single focused region.
#[derive(Clone, Copy, Debug)]
pub struct Focus<'a> {
    pub centre: &'a [usize],
    pub kids: &'a [usize],
    pub regions: &'a [usize],
    pub links: bool,
}

/// Each node's tier for a focus, [`HIDDEN`] when it is not drawn. `folder` marks the nodes a hop
/// may reach but never pass through (folders and the root); `fold` names, for a node under a
/// collapsed folder, that folder. Under `show_all` every node is drawn, at its hop count capped at
/// 3, whatever is collapsed.
pub fn tiers(
    vis: &Vis,
    f: &Focus,
    folder: &[bool],
    fold: &[Option<usize>],
    show_all: bool,
) -> Vec<u8> {
    let n = vis.adj.len();
    let mut hop = vec![u8::MAX; n];
    let mut ring: Vec<usize> = Vec::new();
    for &c in f.centre.iter().filter(|&&c| c < n) {
        if hop[c] != 0 {
            hop[c] = 0;
            ring.push(c);
        }
    }
    // the centre is where the walk starts, so a focused folder's own links count; past it,
    // folders end a path
    let mut next: Vec<usize> = Vec::new();
    for &c in ring.iter().filter(|_| f.links) {
        for &m in &vis.adj[c] {
            if hop[m] == u8::MAX {
                hop[m] = 1;
                next.push(m);
            }
        }
    }
    for &k in f.kids.iter().filter(|&&k| k < n) {
        if hop[k] == u8::MAX {
            hop[k] = 1;
            next.push(k);
        }
    }
    for &c in &next {
        if folder.get(c).copied().unwrap_or(false) {
            continue;
        }
        for &m in &vis.adj[c] {
            if hop[m] == u8::MAX {
                hop[m] = 2;
            }
        }
    }
    let one = f.regions.len() == 1;
    let mut t: Vec<u8> = (0..n)
        .map(|c| {
            let h = hop[c];
            if show_all {
                h.min(3)
            } else if h <= 1 {
                h
            } else if f.regions.contains(&vis.region[c]) {
                match h {
                    2 => 2,
                    _ if one => 3,
                    _ => HIDDEN,
                }
            } else {
                HIDDEN
            }
        })
        .collect();
    if !show_all {
        for c in 0..n {
            if let (3, Some(Some(by))) = (t[c], fold.get(c)) {
                t[c] = HIDDEN;
                if t[*by] == HIDDEN {
                    t[*by] = 3;
                }
            }
        }
    }
    t
}

/// How an edge between two nodes is drawn, by their tiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    /// the focus to its links (or between two focus nodes): full weight
    Hot,
    /// between two of the focus's links
    Faint,
    /// from a link to a card two hops out: the path to the focus
    Dim,
    /// the focus to a link in a region out of focus: drawn out to that region's glyph on the rim
    Portal,
    /// under "show all", any other edge
    Plain,
}

/// Whether and how the edge `a`–`b` is drawn, given every node's tier: only edges that touch the
/// focus set, so never one between two tier-2 cards or from a tier-3 card, and never one whose
/// end is not drawn. A hot edge to a top-level folder's card outside the focused `regions` is a
/// portal. Under `show_all` every edge is drawn.
pub fn edge_kind(
    vis: &Vis,
    t: &[u8],
    regions: &[usize],
    a: usize,
    b: usize,
    show_all: bool,
) -> Option<EdgeKind> {
    let (ta, tb) = (*t.get(a)?, *t.get(b)?);
    let (lo, hi) = (ta.min(tb), ta.max(tb));
    let kind = match (lo, hi) {
        _ if hi == HIDDEN => None,
        (0, 0) | (0, 1) => Some(EdgeKind::Hot),
        (1, 1) => Some(EdgeKind::Faint),
        (1, 2) => Some(EdgeKind::Dim),
        _ => None,
    };
    if show_all {
        return Some(kind.unwrap_or(EdgeKind::Plain));
    }
    if kind == Some(EdgeKind::Hot) && lo != hi {
        let far = if ta == 1 { a } else { b };
        let r = vis.region[far];
        if vis.is_region(r) && !regions.contains(&r) {
            return Some(EdgeKind::Portal);
        }
    }
    kind
}

/// A walk target as the screen shows it: where its point projects (`None` behind the camera),
/// where it meets the rim, and whether it is a portal (a link in a region out of focus).
#[derive(Clone, Copy, Debug)]
pub struct Target<'a> {
    pub px: Option<[f64; 2]>,
    pub rim: [f64; 2],
    pub portal: bool,
    pub path: &'a str,
}

impl Target<'_> {
    /// Where the walk sees it: a portal, or a target behind the camera, sits at its rim point;
    /// anything else at its point.
    pub fn at(&self) -> [f64; 2] {
        match self.px {
            Some(p) if !self.portal => p,
            _ => self.rim,
        }
    }
}

/// The on-screen angle of `p` around `focus`, both in pane px (y down): clockwise from 12
/// o'clock, in radians in `[0, 2π)`.
pub fn screen_angle(focus: [f64; 2], p: [f64; 2]) -> f64 {
    let (dx, dy) = (p[0] - focus[0], p[1] - focus[1]);
    if dx == 0.0 && dy == 0.0 {
        return 0.0;
    }
    let a = dx.atan2(-dy);
    if a < 0.0 {
        a + std::f64::consts::TAU
    } else {
        a
    }
}

/// The order the walk steps through `targets`: clockwise around the focus from 12 o'clock, by
/// [`screen_angle`] of where each shows ([`Target::at`]). Angles equal to a nanoradian tie, and
/// ties go to the nearer target, then the path. Returns indices into `targets`.
pub fn angular_order(focus_px: [f64; 2], targets: &[Target]) -> Vec<usize> {
    let key = |t: &Target| {
        let p = t.at();
        let a = (screen_angle(focus_px, p) * 1e9).round() as i64;
        (a, (p[0] - focus_px[0]).hypot(p[1] - focus_px[1]))
    };
    let keys: Vec<(i64, f64)> = targets.iter().map(key).collect();
    let mut order: Vec<usize> = (0..targets.len()).collect();
    order.sort_by(|&a, &b| {
        keys[a]
            .0
            .cmp(&keys[b].0)
            .then(keys[a].1.total_cmp(&keys[b].1))
            .then_with(|| targets[a].path.cmp(targets[b].path))
    });
    order
}

/// How much larger than a bare card a card is drawn: it grows with the log of its links, and an
/// index card is larger still.
pub fn importance(degree: f64, index: bool) -> f64 {
    let k = 1.0 + 0.35 * (1.0 + degree.max(0.0)).ln();
    if index { k * 1.4 } else { k }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> VNode<'_> {
        VNode {
            id,
            surface: id.starts_with('@'),
            file: !id.ends_with('/') && id != "." && !id.starts_with('@'),
        }
    }

    fn fixture() -> (Vec<VNode<'static>>, Vec<String>) {
        let ids = [
            ".",
            "AGENTS.md",
            "@slack",
            "a/",
            "a/x.md",
            "a/b/",
            "a/b/y.md",
            "a/b/c/z.md",
            "c/",
            "c/w.md",
            "d/",
            "d/v.md",
        ];
        let order = [".", "@", "c/", "a/", "d/"].map(String::from).to_vec();
        (ids.iter().map(|i| node(i)).collect(), order)
    }

    #[test]
    fn region_of_nodes() {
        assert_eq!(region_key(".", false), ROOT);
        assert_eq!(region_key("AGENTS.md", false), ROOT);
        assert_eq!(region_key("@slack", true), SURFACES);
        assert_eq!(region_key("a/", false), "a/");
        assert_eq!(region_key("a/x.md", false), "a/");
        assert_eq!(region_key("a/b/c/z.md", false), "a/");
        let (nodes, order) = fixture();
        let v = build(&nodes, &[], &order);
        let key = |id: &str| v.regions[v.region[v.ix[id]]].as_str();
        assert_eq!(key("."), ".");
        assert_eq!(key("AGENTS.md"), ".");
        assert_eq!(key("@slack"), "@");
        for id in ["a/", "a/x.md", "a/b/", "a/b/y.md", "a/b/c/z.md"] {
            assert_eq!(key(id), "a/", "{id}");
        }
        assert_eq!(key("c/w.md"), "c/");
        // region indices follow the layout's order
        assert_eq!(v.region[v.ix["a/x.md"]], 3);
        assert_eq!(v.region[v.ix["c/w.md"]], 2);
        assert!(!v.is_region(0) && !v.is_region(1) && v.is_region(2) && v.is_region(4));
        // cards per region count files only
        assert_eq!(v.cards, vec![1, 0, 1, 3, 1]);
        // a key the order lacks is appended, not dropped
        let v2 = build(&[node("e/q.md")], &[], &order);
        assert_eq!(v2.regions.last().map(String::as_str), Some("e/"));
    }

    #[test]
    fn pair_counts() {
        let (nodes, order) = fixture();
        let i = |id: &str| nodes.iter().position(|n| n.id == id).unwrap();
        let edges = vec![
            (i("a/x.md"), i("c/w.md"), 2.0),
            (i("c/w.md"), i("a/b/y.md"), 3.0), // the other direction joins the same pair
            (i("a/b/c/z.md"), i("c/w.md"), 1.0),
            (i("d/v.md"), i("a/x.md"), 4.0),
            (i("a/x.md"), i("a/b/y.md"), 9.0), // same region: no bundle
            (i("a/x.md"), i("AGENTS.md"), 5.0), // root: excluded
            (i("c/w.md"), i("@slack"), 7.0),   // surface: excluded
            (i("AGENTS.md"), i("@slack"), 1.0), // both pseudo
        ];
        let v = build(&nodes, &edges, &order);
        // keyed (min, max) of region indices: c/ is 2, a/ is 3, d/ is 4
        assert_eq!(
            v.pairs,
            vec![Pair { a: 2, b: 3, n: 6.0 }, Pair { a: 3, b: 4, n: 4.0 },]
        );
        assert!(
            v.pairs
                .iter()
                .all(|p| p.a < p.b && v.is_region(p.a) && v.is_region(p.b))
        );
        // adjacency runs both ways, distinct and sorted
        let ax = i("a/x.md");
        assert_eq!(
            v.adj[ax],
            vec![i("AGENTS.md"), i("a/b/y.md"), i("c/w.md"), i("d/v.md")]
        );
        assert!(v.adj[i("c/w.md")].contains(&ax));
    }

    #[test]
    fn bundle_weight() {
        let n_max = 400.0;
        let mut last = super::bundle_weight(0.0, n_max);
        for n in [1.0, 2.0, 3.0, 5.0, 10.0, 40.0, 100.0, 400.0] {
            let w = super::bundle_weight(n, n_max);
            assert!(w.0 > last.0, "width rises at {n}");
            assert!(w.1 > last.1, "alpha rises at {n}");
            assert!(w.1 <= 1.0 && w.0.is_finite());
            last = w;
        }
        let top = super::bundle_weight(n_max, n_max);
        assert!(
            (top.1 - 0.65).abs() < 1e-12,
            "the heaviest bundle takes the full alpha"
        );
        // a lone bundle is its own maximum
        assert!((super::bundle_weight(3.0, 0.0).1 - 0.65).abs() < 1e-12);
    }

    /// A small KB for the tier tests: the focus `a/f.md` links to `a/l.md` (same region) and
    /// `c/x.md` (another region); `a/l.md` links on to `a/two.md` and `c/two.md`; `c/x.md` links
    /// on to `d/two.md`; `a/far.md` and `c/far.md` are linked to nothing; `a/f.md` also links to
    /// the folder `a/s/`, which links to `a/behind.md`, and to the surface `@slack` and the root
    /// card `AGENTS.md`.
    struct Kb {
        nodes: Vec<VNode<'static>>,
        vis: Vis,
        folder: Vec<bool>,
    }

    impl Kb {
        fn new() -> Kb {
            let ids = [
                ".",
                "AGENTS.md",
                "@slack",
                "a/",
                "a/f.md",
                "a/l.md",
                "a/two.md",
                "a/far.md",
                "a/s/",
                "a/s/in.md",
                "a/behind.md",
                "c/",
                "c/x.md",
                "c/two.md",
                "c/far.md",
                "d/",
                "d/two.md",
            ];
            let nodes: Vec<VNode> = ids.iter().map(|i| node(i)).collect();
            let i = |id: &str| nodes.iter().position(|n| n.id == id).unwrap();
            let edges = [
                ("a/f.md", "a/l.md"),
                ("c/x.md", "a/f.md"),
                ("a/l.md", "a/two.md"),
                ("a/l.md", "c/two.md"),
                ("c/x.md", "d/two.md"),
                ("a/f.md", "a/s/"),
                ("a/s/", "a/behind.md"),
                ("a/f.md", "@slack"),
                ("AGENTS.md", "a/f.md"),
                ("a/l.md", "c/x.md"),
                ("a/two.md", "c/two.md"),
                ("a/two.md", "a/far.md"),
            ]
            .map(|(s, t)| (i(s), i(t), 1.0))
            .to_vec();
            let order = [".", "@", "a/", "c/", "d/"].map(String::from).to_vec();
            let vis = build(&nodes, &edges, &order);
            let folder = nodes
                .iter()
                .map(|n| n.id.ends_with('/') || n.id == ".")
                .collect();
            Kb { nodes, vis, folder }
        }
        fn i(&self, id: &str) -> usize {
            self.vis.ix[id]
        }
        fn r(&self, id: &str) -> usize {
            self.vis.region[self.i(id)]
        }
        /// Tiers for a focus on `centre`, in `regions`, nothing collapsed.
        fn tiers(&self, centre: &[&str], regions: &[&str], show_all: bool) -> Vec<u8> {
            let c: Vec<usize> = centre.iter().map(|x| self.i(x)).collect();
            let r: Vec<usize> = regions.iter().map(|x| self.r(x)).collect();
            let fold = vec![None; self.nodes.len()];
            let f = Focus {
                centre: &c,
                kids: &[],
                regions: &r,
                links: true,
            };
            tiers(&self.vis, &f, &self.folder, &fold, show_all)
        }
    }

    #[test]
    fn hidden_two_hop_other_region() {
        let kb = Kb::new();
        let t = kb.tiers(&["a/f.md"], &["a/f.md"], false);
        let at = |id: &str| t[kb.i(id)];
        assert_eq!(at("a/f.md"), 0);
        assert_eq!(at("a/l.md"), 1);
        // two hops out: drawn dim in the focused region, folded into its glyph elsewhere
        assert_eq!(at("a/two.md"), 2);
        assert_eq!(at("c/two.md"), HIDDEN);
        assert_eq!(at("d/two.md"), HIDDEN);
        // the rest of the focused region is tier 3; the rest of every other region is not drawn
        assert_eq!(at("a/far.md"), 3);
        assert_eq!(at("c/far.md"), HIDDEN);
    }

    #[test]
    fn focus_set_any_region() {
        let kb = Kb::new();
        let t = kb.tiers(&["a/f.md"], &["a/f.md"], false);
        // direct links are drawn wherever they are: another region, a surface, a root card
        for id in ["c/x.md", "@slack", "AGENTS.md", "a/l.md", "a/s/"] {
            assert_eq!(t[kb.i(id)], 1, "{id}");
        }
        // and so is the focus itself, in any region, whatever regions are focused
        let t = kb.tiers(&["c/x.md"], &["a/f.md"], false);
        assert_eq!(t[kb.i("c/x.md")], 0);
        assert_eq!(t[kb.i("d/two.md")], 1);
        assert_eq!(t[kb.i("a/f.md")], 1);
    }

    #[test]
    fn folders_terminal() {
        let kb = Kb::new();
        let t = kb.tiers(&["a/f.md"], &["a/f.md"], false);
        // a/behind.md is two hops out only through the folder a/s/, which ends the path
        assert_eq!(t[kb.i("a/s/")], 1);
        assert_eq!(t[kb.i("a/behind.md")], 3);
        // a focused folder still reaches out over its own links, and its children are tier 1
        let c = [kb.i("a/s/")];
        let kids = [kb.i("a/s/in.md")];
        let r = [kb.r("a/s/")];
        let fold = vec![None; kb.nodes.len()];
        let f = Focus {
            centre: &c,
            kids: &kids,
            regions: &r,
            links: true,
        };
        let t = tiers(&kb.vis, &f, &kb.folder, &fold, false);
        assert_eq!(t[kb.i("a/s/")], 0);
        assert_eq!(t[kb.i("a/s/in.md")], 1);
        assert_eq!(t[kb.i("a/behind.md")], 1);
        assert_eq!(t[kb.i("a/f.md")], 1);
        // from a/f.md (tier 1, a card) the walk goes on to two hops
        assert_eq!(t[kb.i("a/l.md")], 2);
        // a root card's links are tier 1; past them, another region's cards are not drawn
        let t = kb.tiers(&["AGENTS.md"], &["AGENTS.md"], false);
        assert_eq!(t[kb.i("a/f.md")], 1);
        assert_eq!(t[kb.i("a/l.md")], HIDDEN, "two hops, another region");
    }

    #[test]
    fn no_tier3_multi_region() {
        let kb = Kb::new();
        let single = kb.tiers(&["a/f.md"], &["a/f.md"], false);
        assert_eq!(single[kb.i("a/far.md")], 3);
        // a focus over two regions (a search's matches): no tier 3 in either, tier 2 still drawn
        let multi = kb.tiers(&["a/f.md", "c/far.md"], &["a/f.md", "c/far.md"], false);
        assert_eq!(multi[kb.i("a/far.md")], HIDDEN);
        assert_eq!(multi[kb.i("a/behind.md")], HIDDEN);
        assert_eq!(multi[kb.i("a/two.md")], 2);
        assert_eq!(multi[kb.i("c/two.md")], 2);
        assert_eq!(multi[kb.i("c/far.md")], 0);
        assert!(multi.iter().all(|&t| t != 3));
    }

    #[test]
    fn edge_rule() {
        let kb = Kb::new();
        let regions = [kb.r("a/f.md")];
        let t = kb.tiers(&["a/f.md"], &["a/f.md"], false);
        let e = |a: &str, b: &str| edge_kind(&kb.vis, &t, &regions, kb.i(a), kb.i(b), false);
        assert_eq!(e("a/f.md", "a/l.md"), Some(EdgeKind::Hot));
        assert_eq!(e("a/l.md", "a/f.md"), Some(EdgeKind::Hot));
        assert_eq!(e("a/l.md", "c/x.md"), Some(EdgeKind::Faint));
        assert_eq!(e("a/l.md", "a/two.md"), Some(EdgeKind::Dim));
        // a link in another region runs out to the rim; a surface or a root card does not
        assert_eq!(e("c/x.md", "a/f.md"), Some(EdgeKind::Portal));
        assert_eq!(e("a/f.md", "@slack"), Some(EdgeKind::Hot));
        assert_eq!(e("AGENTS.md", "a/f.md"), Some(EdgeKind::Hot));
        // no edge to a card not drawn, none between two tier-2 cards, none from tier 3
        assert_eq!(e("a/l.md", "c/two.md"), None);
        assert_eq!(e("c/x.md", "d/two.md"), None);
        assert_eq!(e("a/two.md", "a/far.md"), None);
        assert_eq!(e("a/s/", "a/behind.md"), None);
        let t2 = vec![2u8; kb.nodes.len()];
        assert_eq!(edge_kind(&kb.vis, &t2, &regions, 0, 1, false), None);
        let t3 = vec![3u8; kb.nodes.len()];
        assert_eq!(edge_kind(&kb.vis, &t3, &regions, 0, 1, false), None);
        // with the focus's region and c/ both focused, c/x.md is no portal
        let both = [kb.r("a/f.md"), kb.r("c/x.md")];
        assert_eq!(
            edge_kind(&kb.vis, &t, &both, kb.i("c/x.md"), kb.i("a/f.md"), false),
            Some(EdgeKind::Hot)
        );
    }

    #[test]
    fn show_all_hides_nothing() {
        let kb = Kb::new();
        let n = kb.nodes.len();
        // everything under a/ folded into a/s/ ... and c/far.md folded too: show all ignores it
        let mut fold = vec![None; n];
        for id in ["a/far.md", "a/behind.md", "c/far.md"] {
            fold[kb.i(id)] = Some(kb.i("a/s/"));
        }
        let c = [kb.i("a/f.md")];
        let r = [kb.r("a/f.md")];
        let f = Focus {
            centre: &c,
            kids: &[],
            regions: &r,
            links: true,
        };
        let t = tiers(&kb.vis, &f, &kb.folder, &fold, true);
        assert!(t.iter().all(|&x| x <= 3), "{t:?}");
        assert_eq!(t[kb.i("a/f.md")], 0);
        assert_eq!(t[kb.i("c/two.md")], 2);
        assert_eq!(t[kb.i("c/far.md")], 3);
        // with nothing selected too
        let none = Focus {
            centre: &[],
            kids: &[],
            regions: &[],
            links: true,
        };
        let t0 = tiers(&kb.vis, &none, &kb.folder, &fold, true);
        assert!(t0.iter().all(|&x| x == 3));
        // and every edge is drawn
        for (a, b) in [
            (0, 1),
            (kb.i("a/two.md"), kb.i("a/far.md")),
            (kb.i("c/x.md"), kb.i("d/two.md")),
        ] {
            assert!(edge_kind(&kb.vis, &t, &r, a, b, true).is_some());
            assert!(edge_kind(&kb.vis, &t0, &r, a, b, true).is_some());
        }
        // off, nothing selected draws no card at all
        let off = tiers(&kb.vis, &none, &kb.folder, &fold, false);
        assert!(off.iter().all(|&x| x == HIDDEN));
    }

    #[test]
    fn collapse_folds_tier3_only() {
        let kb = Kb::new();
        let n = kb.nodes.len();
        let mut fold = vec![None; n];
        // a/ collapsed into its folder point: its tier-3 cards fold, the focus set never does
        for id in [
            "a/far.md",
            "a/behind.md",
            "a/l.md",
            "a/f.md",
            "a/two.md",
            "a/s/in.md",
        ] {
            fold[kb.i(id)] = Some(kb.i("a/"));
        }
        let c = [kb.i("a/f.md")];
        let r = [kb.r("a/f.md")];
        let f = Focus {
            centre: &c,
            kids: &[],
            regions: &r,
            links: true,
        };
        let t = tiers(&kb.vis, &f, &kb.folder, &fold, false);
        assert_eq!(t[kb.i("a/far.md")], HIDDEN);
        assert_eq!(t[kb.i("a/behind.md")], HIDDEN);
        assert_eq!(t[kb.i("a/s/in.md")], HIDDEN);
        assert_eq!(t[kb.i("a/")], 3, "the collapsed folder stands for them");
        assert_eq!(t[kb.i("a/f.md")], 0);
        assert_eq!(t[kb.i("a/l.md")], 1);
        assert_eq!(t[kb.i("a/two.md")], 2);
    }

    #[test]
    fn importance_grows() {
        assert_eq!(importance(0.0, false), 1.0);
        assert!(importance(10.0, false) > importance(1.0, false));
        assert!((importance(3.0, true) / importance(3.0, false) - 1.4).abs() < 1e-12);
    }

    fn tgt(px: Option<[f64; 2]>, rim: [f64; 2], portal: bool, path: &str) -> Target<'_> {
        Target {
            px,
            rim,
            portal,
            path,
        }
    }

    #[test]
    fn angular_clockwise() {
        let f = [100.0, 100.0];
        // 12, 3, 6 and 9 o'clock and the diagonals between, given out of order
        let pts = [
            ("w", [60.0, 100.0]),
            ("s", [100.0, 150.0]),
            ("ne", [130.0, 70.0]),
            ("n", [100.0, 40.0]),
            ("nw", [70.0, 70.0]),
            ("e", [180.0, 100.0]),
            ("se", [120.0, 120.0]),
            ("sw", [80.0, 130.0]),
        ];
        let ts: Vec<Target> = pts
            .iter()
            .map(|(id, p)| tgt(Some(*p), [0.0, 0.0], false, id))
            .collect();
        let got: Vec<&str> = angular_order(f, &ts).iter().map(|&k| pts[k].0).collect();
        assert_eq!(got, ["n", "ne", "e", "se", "s", "sw", "w", "nw"]);
        // the angles themselves: 0 at 12 o'clock, a quarter turn at 3, rising clockwise
        let a = |p: [f64; 2]| screen_angle(f, p);
        assert_eq!(a([100.0, 40.0]), 0.0);
        assert!((a([180.0, 100.0]) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!((a([100.0, 150.0]) - std::f64::consts::PI).abs() < 1e-12);
        assert!((a([60.0, 100.0]) - 1.5 * std::f64::consts::PI).abs() < 1e-12);
        // just left of 12 o'clock is the end of the turn, not its start
        assert!(a([99.999, 40.0]) > 6.28 && a([99.999, 40.0]) < std::f64::consts::TAU);
        // every angle is in [0, 2π) over a full circle of points
        for k in 0..360 {
            let t = (k as f64).to_radians();
            let v = a([100.0 + 50.0 * t.sin(), 100.0 - 50.0 * t.cos()]);
            assert!((0.0..std::f64::consts::TAU).contains(&v), "{k}: {v}");
        }
        assert!(angular_order(f, &[]).is_empty());
    }

    #[test]
    fn angular_ties() {
        let f = [0.0, 0.0];
        // three on one ray (3 o'clock): the nearer first; two at the same place: by path
        let ts = [
            tgt(Some([30.0, 0.0]), [0.0; 2], false, "far"),
            tgt(Some([10.0, 0.0]), [0.0; 2], false, "near"),
            tgt(Some([20.0, 0.0]), [0.0; 2], false, "mid-b"),
            tgt(Some([20.0, 0.0]), [0.0; 2], false, "mid-a"),
            tgt(Some([0.0, -5.0]), [0.0; 2], false, "top"),
        ];
        let got: Vec<&str> = angular_order(f, &ts).iter().map(|&k| ts[k].path).collect();
        assert_eq!(got, ["top", "near", "mid-a", "mid-b", "far"]);
        // the order does not depend on the order given
        let mut rev = ts.to_vec();
        rev.reverse();
        let got2: Vec<&str> = angular_order(f, &rev)
            .iter()
            .map(|&k| rev[k].path)
            .collect();
        assert_eq!(got2, got);
    }

    #[test]
    fn angular_portal_rim() {
        let f = [400.0, 300.0];
        // a portal projects at 1 o'clock but its region's glyph is on the rim at 9: it sorts at 9
        let portal = tgt(Some([450.0, 200.0]), [30.0, 300.0], true, "b/portal.md");
        // a card behind the camera has no point: it sorts at its rim point (6 o'clock)
        let behind = tgt(None, [400.0, 570.0], false, "a/behind.md");
        // an ordinary card at 3 o'clock, whatever its rim point says
        let card = tgt(Some([500.0, 300.0]), [400.0, 30.0], false, "a/card.md");
        assert_eq!(portal.at(), [30.0, 300.0]);
        assert_eq!(behind.at(), [400.0, 570.0]);
        assert_eq!(card.at(), [500.0, 300.0]);
        let ts = [portal, behind, card];
        let got: Vec<&str> = angular_order(f, &ts).iter().map(|&k| ts[k].path).collect();
        assert_eq!(got, ["a/card.md", "a/behind.md", "b/portal.md"]);
        // a portal behind the camera sits at its rim point too
        let gone = tgt(None, [400.0, 30.0], true, "c/gone.md");
        assert_eq!(gone.at(), [400.0, 30.0]);
        let ts = [portal, gone, card];
        let got: Vec<&str> = angular_order(f, &ts).iter().map(|&k| ts[k].path).collect();
        assert_eq!(got, ["c/gone.md", "a/card.md", "b/portal.md"]);
    }

    #[test]
    fn matches_without_links() {
        let (nodes, order) = fixture();
        let i = |id: &str| nodes.iter().position(|n| n.id == id).unwrap();
        let edges = vec![
            (i("a/x.md"), i("c/w.md"), 1.0),
            (i("a/x.md"), i("a/b/y.md"), 1.0),
            (i("c/w.md"), i("d/v.md"), 1.0),
        ];
        let v = build(&nodes, &edges, &order);
        let folder: Vec<bool> = nodes.iter().map(|n| !n.file && !n.surface).collect();
        let fold = vec![None; nodes.len()];
        // two matches in two regions: they alone are drawn, their links are not
        let c = [i("a/x.md"), i("d/v.md")];
        let r = [v.region[c[0]], v.region[c[1]]];
        let f = Focus {
            centre: &c,
            kids: &[],
            regions: &r,
            links: false,
        };
        let t = tiers(&v, &f, &folder, &fold, false);
        for (k, n) in nodes.iter().enumerate() {
            let want = if c.contains(&k) { 0 } else { HIDDEN };
            assert_eq!(t[k], want, "{}", n.id);
        }
        // one match: the rest of its region is drawn faint, nothing else
        let c1 = [i("a/x.md")];
        let r1 = [v.region[c1[0]]];
        let f1 = Focus {
            centre: &c1,
            kids: &[],
            regions: &r1,
            links: false,
        };
        let t1 = tiers(&v, &f1, &folder, &fold, false);
        assert_eq!(t1[i("a/x.md")], 0);
        assert_eq!(t1[i("a/b/y.md")], 3);
        assert_eq!(t1[i("c/w.md")], HIDDEN);
    }
}
