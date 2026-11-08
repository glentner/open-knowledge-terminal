//! What the graph draws, as pure data: the region each node belongs to, who links to whom, how
//! many links join each pair of regions, and how heavily the bundle between them is drawn.
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
    #[cfg_attr(not(test), allow(dead_code))]
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
}
