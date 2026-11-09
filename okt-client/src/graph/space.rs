//! The graph's 3D layout: a pure function from the folder tree to a position for every node.
//!
//! Regions (the top-level folders) sit on a sphere of radius `R_GAL`; each subfolder sits inside
//! a cap around its parent's axis, at a distance from its region's core that grows with depth;
//! cards sit in a cloud around their folder's anchor, relaxed against each other only. Root-level
//! cards form the core at the origin, and surfaces (`@…` nodes) form a pseudo-region `@` whose
//! slot is reserved before any region is placed.
//!
//! **Stability.** The same tree gives the same positions, bit for bit, whatever order the nodes
//! arrive in, and adding a card moves no card outside the card's own folder. Two rules carry that:
//!
//! - Siblings are placed greedily in a fixed order, each taking the free lattice slot farthest
//!   from the slots its *older* siblings took, so a sibling never sees a younger one. The order
//!   is `(birth, path)`: `birth` is the earliest first-commit time (`ft`) of any file under the
//!   folder, and a folder with no committed file has birth +∞, after every committed sibling, ties
//!   broken by path. A new card has no `ft`, so it changes no existing folder's birth, and a
//!   folder it creates sorts after its committed siblings.
//! - `layout` reads the tree and `ft` only. It takes no edges, link counts, line counts or render
//!   sizes on purpose: adding a card changes other cards' link counts, so link-weighted placement
//!   would move cards far from the new one. Size-as-importance belongs to the renderer.
//!
//! Known exceptions, where an addition can move cards in other folders: a folder authored earlier
//! on another host that lands later (its older first commit sorts it before existing siblings); an
//! old card renamed into a new folder (it carries its old first commit); and, among several
//! uncommitted sibling folders, a new one whose path sorts before an existing one. Deleting a
//! folder's oldest file can also reorder its younger siblings.
//!
//! Floating point: `+ − × ÷ √` are exact everywhere, but `sin`/`cos` come from the platform's
//! libm, so positions are bitwise stable within one binary, not across native and WebAssembly.
//! Every iteration runs over sorted `Vec`s or a `BTreeMap`; nothing iterates a hashed collection.

use std::cmp::Ordering;
use std::collections::BTreeMap;

pub type V = [f64; 3];

/// One node of the tree, as the layout sees it.
#[derive(Clone, Debug)]
pub struct LNode {
    pub id: String,
    pub parent: Option<String>,
    pub is_dir: bool,
    /// first-commit time of a file (`None` when uncommitted, and for folders)
    pub ft: Option<f64>,
}

/// Where a folder sits: its anchor, the axis its subfolders spread around, its depth (the root
/// is 0, a region 1) and its region's core.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub anchor: V,
    pub axis: V,
    pub depth: u32,
    pub core: V,
}

/// A region: the root core `.`, the surfaces `@`, or a top-level folder.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub id: String,
    pub core: V,
    pub axis: V,
    /// the farthest of its nodes from `core`
    pub radius: f64,
}

/// The layout. Every per-node vector is aligned with the input slice.
#[derive(Clone, Debug, Default)]
pub struct Space {
    pub pos: Vec<V>,
    /// `Some` for folders and the root
    pub frame: Vec<Option<Frame>>,
    /// index into `regions`
    pub region: Vec<usize>,
    /// `.` first, then `@`, then the top-level folders in placement order
    pub regions: Vec<Region>,
    /// the farthest node from the origin
    pub radius: f64,
}

// The layout's signature is part of its stability contract: no edge or count input.
const _: fn(&[LNode]) -> Space = layout;

pub const R_GAL: f64 = 900.0;
const M_REGIONS: usize = 256;
const K_CAP: usize = 48;
const CAP_A1_DEG: f64 = 55.0;
const CAP_KAPPA: f64 = 0.6;
const DELTA: [f64; 8] = [90.0, 70.0, 56.0, 46.0, 40.0, 36.0, 33.0, 30.0];
const S0: f64 = 7.0;
pub const DMIN: f64 = 10.0;
const ITERS: usize = 16;
const S_SURF: f64 = 60.0;
const WRAP_JITTER: f64 = 0.3;
const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653; // π(3 − √5)

// ---- hashing -------------------------------------------------------------------------------

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The path's hash: FNV-1a 64 over its UTF-8 bytes, then the splitmix64 finalizer.
fn hash(path: &str) -> u64 {
    splitmix(fnv1a(path.as_bytes()))
}

/// The `k`-th value in [0, 1) of the stream seeded by `h`.
fn unit(h: u64, k: u64) -> f64 {
    (splitmix(h.wrapping_add(k.wrapping_mul(0x9e37_79b9_7f4a_7c15))) >> 11) as f64
        * (1.0 / (1u64 << 53) as f64)
}

// ---- vectors -------------------------------------------------------------------------------

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: V, k: f64) -> V {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn len(a: V) -> f64 {
    dot(a, a).sqrt()
}
fn unit_or(a: V, fallback: V) -> V {
    let l = len(a);
    if l > 1e-12 { mul(a, 1.0 / l) } else { fallback }
}

/// A random unit vector from the hash stream (uniform in the cube, normalised).
fn hash_dir(h: u64, k0: u64) -> V {
    let v = [
        2.0 * unit(h, k0) - 1.0,
        2.0 * unit(h, k0 + 1) - 1.0,
        2.0 * unit(h, k0 + 2) - 1.0,
    ];
    unit_or(v, [0.0, 0.0, 1.0])
}

/// An orthonormal basis completing `n` (Duff et al. 2017: branchless, a pure function of `n`).
fn onb(n: V) -> (V, V) {
    let sign = if n[2] >= 0.0 { 1.0 } else { -1.0 };
    let a = -1.0 / (sign + n[2]);
    let b = n[0] * n[1] * a;
    (
        [1.0 + sign * n[0] * n[0] * a, sign * b, -sign * n[0]],
        [b, sign + n[1] * n[1] * a, -n[1]],
    )
}

// ---- lattices ------------------------------------------------------------------------------

/// `m` points spread evenly over the unit sphere.
fn fib_sphere(m: usize) -> Vec<V> {
    (0..m)
        .map(|i| {
            let z = 1.0 - (2 * i + 1) as f64 / m as f64;
            let r = (1.0 - z * z).max(0.0).sqrt();
            let phi = i as f64 * GOLDEN_ANGLE;
            [r * phi.cos(), r * phi.sin(), z]
        })
        .collect()
}

/// `K_CAP` points spread evenly over the cap of half-angle `alpha` around `axis`.
fn fib_cap(axis: V, alpha: f64) -> Vec<V> {
    let (b1, b2) = onb(axis);
    let span = 1.0 - alpha.cos();
    (0..K_CAP)
        .map(|i| {
            let c = 1.0 - (i as f64 + 0.5) / K_CAP as f64 * span;
            let s = (1.0 - c * c).max(0.0).sqrt();
            let phi = i as f64 * GOLDEN_ANGLE;
            add(
                add(mul(b1, s * phi.cos()), mul(b2, s * phi.sin())),
                mul(axis, c),
            )
        })
        .collect()
}

/// Greedy placement on a lattice: `taken` holds the slots of the older siblings, in placement
/// order. With nothing taken, the newcomer takes `first`; otherwise the free slot whose nearest
/// taken slot is farthest (compared by dot product, so no `acos`), the lowest index on a tie.
/// Once every slot is taken, the `k`-th sibling shares the slot of sibling `k mod len`, nudged
/// by a hash jitter of `WRAP_JITTER` lattice spacings. Returns the direction and records the
/// slot.
fn greedy(lat: &[V], taken: &mut Vec<usize>, first: usize, spacing: f64, h: u64) -> V {
    if taken.is_empty() {
        taken.push(first % lat.len());
        return lat[first % lat.len()];
    }
    if taken.len() >= lat.len() {
        let slot = taken[taken.len() % lat.len()];
        taken.push(slot);
        let j = mul(hash_dir(h, 8), WRAP_JITTER * spacing);
        return unit_or(add(lat[slot], j), lat[slot]);
    }
    let mut best = usize::MAX;
    let mut best_d = f64::INFINITY;
    for (i, p) in lat.iter().enumerate() {
        if taken.contains(&i) {
            continue;
        }
        let near = taken
            .iter()
            .map(|&t| dot(*p, lat[t]))
            .fold(f64::NEG_INFINITY, f64::max);
        if near < best_d {
            best_d = near;
            best = i;
        }
    }
    taken.push(best);
    lat[best]
}

/// Half-angle of the cap a folder at depth `d - 1` spreads its children (depth `d`) over.
fn cap_alpha(d: u32) -> f64 {
    (CAP_A1_DEG * CAP_KAPPA.powi(d as i32 - 2)).to_radians()
}

/// Distance from the region core of a folder at depth `d`.
fn depth_radius(d: u32) -> f64 {
    (2..=d as usize)
        .map(|k| DELTA[(k - 2).min(DELTA.len() - 1)])
        .sum()
}

/// Sibling order: oldest birth first, +∞ (uncommitted) last, ties by path.
fn by_birth(a: (f64, &str), b: (f64, &str)) -> Ordering {
    a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1))
}

// ---- the layout ----------------------------------------------------------------------------

struct Tree<'a> {
    nodes: &'a [LNode],
    /// per folder index: (child folders, child files), each sorted by id
    kids: BTreeMap<usize, (Vec<usize>, Vec<usize>)>,
    birth: Vec<f64>,
}

impl Tree<'_> {
    fn birth_of(&mut self, dir: usize) -> f64 {
        let (dirs, files) = self.kids.get(&dir).cloned().unwrap_or_default();
        let mut b = f64::INFINITY;
        for f in files {
            if let Some(t) = self.nodes[f].ft.filter(|t| t.is_finite()) {
                b = b.min(t);
            }
        }
        for d in dirs {
            b = b.min(self.birth_of(d));
        }
        self.birth[dir] = b;
        b
    }

    /// Child folders of `dir` in placement order.
    fn ordered_dirs(&self, dir: usize) -> Vec<usize> {
        let mut dirs = self.kids.get(&dir).map(|k| k.0.clone()).unwrap_or_default();
        dirs.sort_by(|&a, &b| {
            by_birth(
                (self.birth[a], &self.nodes[a].id),
                (self.birth[b], &self.nodes[b].id),
            )
        });
        dirs
    }

    fn files(&self, dir: usize) -> &[usize] {
        self.kids.get(&dir).map(|k| k.1.as_slice()).unwrap_or(&[])
    }
}

fn is_index(id: &str) -> bool {
    id.rsplit('/')
        .next()
        .unwrap_or(id)
        .eq_ignore_ascii_case("index.md")
}

/// Cards of one folder: seeded from their path hashes in a cloud around `anchor`, `index.md`
/// pinned at the anchor, then relaxed against each other only (Jacobi: displacements are summed
/// over all pairs, then applied), and kept inside the cloud.
fn place_cards(nodes: &[LNode], files: &[usize], anchor: V, pos: &mut [V]) {
    let n = files.len();
    if n == 0 {
        return;
    }
    let s = S0 * (n as f64).cbrt() + DMIN;
    let mut p: Vec<V> = Vec::with_capacity(n);
    let mut pinned: Vec<bool> = Vec::with_capacity(n);
    for &f in files {
        let id = &nodes[f].id;
        if is_index(id) {
            p.push(anchor);
            pinned.push(true);
        } else {
            let h = hash(id);
            let r = s * unit(h, 3).cbrt();
            p.push(add(anchor, mul(hash_dir(h, 0), r)));
            pinned.push(false);
        }
    }
    let mut disp = vec![[0.0; 3]; n];
    for _ in 0..ITERS {
        for d in disp.iter_mut() {
            *d = [0.0; 3];
        }
        for i in 0..n {
            for j in i + 1..n {
                let d = sub(p[j], p[i]);
                let l = len(d);
                if l > 1e-9 && l < DMIN {
                    let m = mul(d, 0.5 * (DMIN - l) / l);
                    disp[i] = sub(disp[i], m);
                    disp[j] = add(disp[j], m);
                }
            }
        }
        for i in 0..n {
            if pinned[i] {
                continue;
            }
            let q = add(p[i], disp[i]);
            let off = sub(q, anchor);
            let l = len(off);
            p[i] = if l > s {
                add(anchor, mul(off, s / l))
            } else {
                q
            };
        }
    }
    for (k, &f) in files.iter().enumerate() {
        pos[f] = p[k];
    }
}

/// Lay out the tree. See the module doc for what is guaranteed.
pub fn layout(nodes: &[LNode]) -> Space {
    let n = nodes.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| nodes[a].id.cmp(&nodes[b].id));
    let by_id: BTreeMap<&str, usize> = order.iter().map(|&i| (nodes[i].id.as_str(), i)).collect();

    // The root is the parentless folder (the first by id if there are several); surfaces are the
    // parentless files. Anything else whose parent is missing or not a folder hangs off the root.
    let root = order
        .iter()
        .copied()
        .find(|&i| nodes[i].is_dir && nodes[i].parent.is_none());
    let mut surfaces = Vec::new();
    let mut kids: BTreeMap<usize, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    for &i in &order {
        if Some(i) == root {
            continue;
        }
        let up = nodes[i]
            .parent
            .as_deref()
            .and_then(|p| by_id.get(p).copied())
            .filter(|&p| nodes[p].is_dir && p != i);
        let orphan = nodes[i].parent.is_none() && !nodes[i].is_dir;
        let Some(p) = up.or(if orphan { None } else { root }) else {
            surfaces.push(i);
            continue;
        };
        let e = kids.entry(p).or_default();
        if nodes[i].is_dir {
            e.0.push(i)
        } else {
            e.1.push(i)
        }
    }
    let mut tree = Tree {
        nodes,
        kids,
        birth: vec![f64::INFINITY; n],
    };
    if let Some(r) = root {
        tree.birth_of(r);
    }

    let mut sp = Space {
        pos: vec![[0.0; 3]; n],
        frame: vec![None; n],
        region: vec![0; n],
        regions: Vec::new(),
        radius: 0.0,
    };
    let z = [0.0, 0.0, 1.0];
    sp.regions.push(Region {
        id: ".".into(),
        core: [0.0; 3],
        axis: z,
        radius: 0.0,
    });

    // The surfaces' slot is reserved first, so neither a new surface nor a new region moves the
    // other.
    let sphere = fib_sphere(M_REGIONS);
    let sphere_spacing = (4.0 * std::f64::consts::PI / M_REGIONS as f64).sqrt();
    let mut taken = Vec::new();
    let at = greedy(
        &sphere,
        &mut taken,
        (hash("@") % M_REGIONS as u64) as usize,
        sphere_spacing,
        0,
    );
    let surf_core = mul(at, R_GAL);
    sp.regions.push(Region {
        id: "@".into(),
        core: surf_core,
        axis: at,
        radius: 0.0,
    });
    for &i in &surfaces {
        let h = hash(&nodes[i].id);
        sp.pos[i] = add(surf_core, mul(hash_dir(h, 0), S_SURF * unit(h, 3).cbrt()));
        sp.region[i] = 1;
    }

    let Some(root) = root else {
        finish(&mut sp);
        return sp;
    };
    sp.frame[root] = Some(Frame {
        anchor: [0.0; 3],
        axis: z,
        depth: 0,
        core: [0.0; 3],
    });
    place_cards(nodes, tree.files(root), [0.0; 3], &mut sp.pos);
    for &f in tree.files(root) {
        sp.region[f] = 0;
    }

    for r in tree.ordered_dirs(root) {
        let h = hash(&nodes[r].id);
        let axis = greedy(
            &sphere,
            &mut taken,
            (h % M_REGIONS as u64) as usize,
            sphere_spacing,
            h,
        );
        let core = mul(axis, R_GAL);
        let ri = sp.regions.len();
        sp.regions.push(Region {
            id: nodes[r].id.clone(),
            core,
            axis,
            radius: 0.0,
        });
        // depth-first, each folder's children placed in birth order inside its cap
        let mut stack = vec![(
            r,
            Frame {
                anchor: core,
                axis,
                depth: 1,
                core,
            },
        )];
        while let Some((dir, fr)) = stack.pop() {
            sp.pos[dir] = fr.anchor;
            sp.frame[dir] = Some(fr);
            sp.region[dir] = ri;
            place_cards(nodes, tree.files(dir), fr.anchor, &mut sp.pos);
            for &f in tree.files(dir) {
                sp.region[f] = ri;
            }
            let d = fr.depth + 1;
            let alpha = cap_alpha(d);
            let cap = fib_cap(fr.axis, alpha);
            let spacing = (2.0 * std::f64::consts::PI * (1.0 - alpha.cos()) / K_CAP as f64).sqrt();
            let mut taken_cap = Vec::new();
            for c in tree.ordered_dirs(dir) {
                let hc = hash(&nodes[c].id);
                let v = greedy(
                    &cap,
                    &mut taken_cap,
                    (hc % K_CAP as u64) as usize,
                    spacing,
                    hc,
                );
                let anchor = add(core, mul(v, depth_radius(d)));
                stack.push((
                    c,
                    Frame {
                        anchor,
                        axis: v,
                        depth: d,
                        core,
                    },
                ));
            }
        }
    }
    finish(&mut sp);
    sp
}

fn finish(sp: &mut Space) {
    for (i, p) in sp.pos.iter().enumerate() {
        let r = &mut sp.regions[sp.region[i]];
        r.radius = r.radius.max(len(sub(*p, r.core)));
        sp.radius = sp.radius.max(len(*p));
    }
}

/// The layout's fingerprint: FNV-1a 64 over each node's id and its position quantised to 1e-6,
/// in id order, so it does not depend on the order the nodes arrived in.
pub fn digest(nodes: &[LNode], sp: &Space) -> u64 {
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| nodes[a].id.cmp(&nodes[b].id));
    let mut buf = Vec::new();
    for i in order {
        buf.extend_from_slice(nodes[i].id.as_bytes());
        buf.push(0);
        for c in sp.pos[i] {
            buf.extend_from_slice(&((c * 1e6).round() as i64).to_le_bytes());
        }
    }
    fnv1a(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// splitmix64 as a seeded generator (the crate has no `rand`).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(1);
            splitmix(self.0)
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    fn dir(id: &str, parent: &str) -> LNode {
        LNode {
            id: id.into(),
            parent: Some(parent.into()),
            is_dir: true,
            ft: None,
        }
    }
    fn card(id: &str, parent: &str, ft: Option<f64>) -> LNode {
        LNode {
            id: id.into(),
            parent: Some(parent.into()),
            is_dir: false,
            ft,
        }
    }
    fn surface(id: &str) -> LNode {
        LNode {
            id: id.into(),
            parent: None,
            is_dir: false,
            ft: None,
        }
    }

    /// A tree shaped like the KB: 4 root cards, 21 regions with distinct births (not in name
    /// order), a 147-card flat folder, a depth-8 chain, a region with 22 subfolders, and 3
    /// surfaces. Every folder holds at least one committed card, so every folder has a birth.
    fn fixture() -> Vec<LNode> {
        let mut rng = Rng(7);
        let mut v = vec![LNode {
            id: ".".into(),
            parent: None,
            is_dir: true,
            ft: None,
        }];
        let mut t = 1.0e9;
        let mut tick = |rng: &mut Rng| {
            t += 1.0 + rng.below(5000) as f64;
            Some(t)
        };
        for name in ["index.md", "AGENTS.md", "log.md", "README.md"] {
            v.push(card(name, ".", tick(&mut rng)));
        }
        // births in a shuffled order of names
        let mut names: Vec<String> = (0..21).map(|i| format!("m{i:02}")).collect();
        for i in (1..names.len()).rev() {
            names.swap(i, rng.below(i + 1));
        }
        for (k, r) in names.iter().enumerate() {
            v.push(dir(r, "."));
            let cards = match k {
                0 => 147,
                _ => 3 + rng.below(30),
            };
            v.push(card(&format!("{r}/index.md"), r, tick(&mut rng)));
            for c in 1..cards {
                v.push(card(&format!("{r}/c{c:03}.md"), r, tick(&mut rng)));
            }
            match k {
                1 => {
                    // a chain down to depth 8
                    let mut p = r.clone();
                    for d in 2..=8 {
                        let c = format!("{p}/d{d}");
                        v.push(dir(&c, &p));
                        for j in 0..3 {
                            v.push(card(&format!("{c}/k{j}.md"), &c, tick(&mut rng)));
                        }
                        p = c;
                    }
                }
                2 => {
                    for s in 0..22 {
                        let c = format!("{r}/s{s:02}");
                        v.push(dir(&c, r));
                        for j in 0..(2 + rng.below(6)) {
                            v.push(card(&format!("{c}/k{j}.md"), &c, tick(&mut rng)));
                        }
                    }
                }
                _ => {
                    for s in 0..rng.below(4) {
                        let c = format!("{r}/s{s}");
                        v.push(dir(&c, r));
                        for j in 0..(1 + rng.below(8)) {
                            v.push(card(&format!("{c}/k{j}.md"), &c, tick(&mut rng)));
                        }
                        if rng.below(2) == 0 {
                            let g = format!("{c}/g");
                            v.push(dir(&g, &c));
                            v.push(card(&format!("{g}/k.md"), &g, tick(&mut rng)));
                        }
                    }
                }
            }
        }
        for s in ["@slack", "@email", "@github"] {
            v.push(surface(s));
        }
        v
    }

    fn by_id(nodes: &[LNode], sp: &Space) -> BTreeMap<String, V> {
        nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.clone(), sp.pos[i]))
            .collect()
    }

    fn bits(p: V) -> [u64; 3] {
        [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]
    }

    /// Add `extra` to `base`; every pre-existing node except the cards of `folder` must keep its
    /// position bit for bit (folder nodes, `index.md`s and surfaces included).
    fn assert_local(base: &[LNode], extra: &[LNode], folder: &str) {
        let before = by_id(base, &layout(base));
        let mut all = base.to_vec();
        all.extend_from_slice(extra);
        let after = by_id(&all, &layout(&all));
        let mut checked = 0;
        for n in base {
            if !n.is_dir && n.parent.as_deref() == Some(folder) {
                continue;
            }
            assert_eq!(
                bits(before[&n.id]),
                bits(after[&n.id]),
                "{} moved when adding {:?} to {folder}",
                n.id,
                extra.iter().map(|e| &e.id).collect::<Vec<_>>()
            );
            checked += 1;
        }
        assert!(checked > base.len() / 2, "only {checked} nodes compared");
        for e in extra {
            assert!(
                after[&e.id].iter().all(|c| c.is_finite()),
                "{} not finite",
                e.id
            );
        }
    }

    #[test]
    fn deterministic() {
        let f = fixture();
        let (a, b) = (layout(&f), layout(&f));
        assert_eq!(a.pos.len(), f.len());
        for i in 0..f.len() {
            assert_eq!(bits(a.pos[i]), bits(b.pos[i]), "{}", f[i].id);
        }
        assert_eq!(digest(&f, &a), digest(&f, &b));
    }

    #[test]
    fn order_independent() {
        let f = fixture();
        let want = by_id(&f, &layout(&f));
        let h = digest(&f, &layout(&f));
        for seed in 1..=5 {
            let mut g = f.clone();
            let mut rng = Rng(seed * 1_000_003);
            for i in (1..g.len()).rev() {
                g.swap(i, rng.below(i + 1));
            }
            let sp = layout(&g);
            let got = by_id(&g, &sp);
            for (id, p) in &want {
                assert_eq!(bits(*p), bits(got[id]), "{id} with seed {seed}");
            }
            assert_eq!(digest(&g, &sp), h);
        }
    }

    #[test]
    fn add_existing_folder() {
        let f = fixture();
        assert_local(&f, &[card("m05/zz-new.md", "m05", None)], "m05");
        assert_local(&f, &[card("m05/aa-new.md", "m05", None)], "m05");
        // a subfolder, and the deepest folder of the chain
        let sub = f
            .iter()
            .find(|n| n.is_dir && n.id.ends_with("/s0"))
            .unwrap()
            .id
            .clone();
        assert_local(&f, &[card(&format!("{sub}/new.md"), &sub, None)], &sub);
        let deep = f
            .iter()
            .find(|n| n.is_dir && n.id.ends_with("/d8"))
            .unwrap()
            .id
            .clone();
        assert_local(&f, &[card(&format!("{deep}/new.md"), &deep, None)], &deep);
    }

    #[test]
    fn add_new_subfolder() {
        let f = fixture();
        assert_local(
            &f,
            &[dir("m05/aa", "m05"), card("m05/aa/new.md", "m05/aa", None)],
            "m05/aa",
        );
        // a parent that already has 22 subfolders
        let r = f
            .iter()
            .find(|n| n.id.ends_with("/s21"))
            .unwrap()
            .parent
            .clone()
            .unwrap();
        let s = format!("{r}/aa");
        assert_local(&f, &[dir(&s, &r), card(&format!("{s}/x.md"), &s, None)], &s);
    }

    #[test]
    fn add_new_chain() {
        let f = fixture();
        assert_local(
            &f,
            &[
                dir("m09/new", "m09"),
                dir("m09/new/deeper", "m09/new"),
                card("m09/new/deeper/x.md", "m09/new/deeper", None),
            ],
            "m09/new/deeper",
        );
    }

    #[test]
    fn add_new_region_first() {
        let f = fixture();
        assert_local(
            &f,
            &[dir("aa-new", "."), card("aa-new/x.md", "aa-new", None)],
            "aa-new",
        );
    }

    #[test]
    fn add_new_region_last() {
        let f = fixture();
        assert_local(
            &f,
            &[dir("zz-new", "."), card("zz-new/x.md", "zz-new", None)],
            "zz-new",
        );
    }

    #[test]
    fn add_root() {
        let f = fixture();
        assert_local(&f, &[card("aa-root.md", ".", None)], ".");
        assert_local(&f, &[card("zz-root.md", ".", None)], ".");
    }

    #[test]
    fn add_new_surface() {
        let f = fixture();
        // no cards are exempt: a surface belongs to no folder
        assert_local(&f, &[surface("@aaa")], "");
        let none: Vec<LNode> = f
            .iter()
            .filter(|n| !n.id.starts_with('@'))
            .cloned()
            .collect();
        assert_local(&none, &[surface("@first")], "");
    }

    #[test]
    fn uncommitted_after_committed() {
        // the order itself
        let mut keys = vec![
            (f64::INFINITY, "zzz"),
            (5.0, "b"),
            (f64::INFINITY, "aaa"),
            (3.0, "c"),
            (5.0, "a"),
        ];
        keys.sort_by(|a, b| by_birth(*a, *b));
        let names: Vec<&str> = keys.iter().map(|k| k.1).collect();
        assert_eq!(names, ["c", "a", "b", "aaa", "zzz"]);

        // in the layout: a folder with no committed file, named to sort before every committed
        // sibling, still takes its slot after all of them
        let f = fixture();
        let frames = |nodes: &[LNode]| -> BTreeMap<String, Frame> {
            let sp = layout(nodes);
            nodes
                .iter()
                .enumerate()
                .filter_map(|(i, n)| sp.frame[i].map(|fr| (n.id.clone(), fr)))
                .collect()
        };
        let base = frames(&f);
        let mut one = f.clone();
        one.extend([dir("!aaa", "."), card("!aaa/x.md", "!aaa", None)]);
        one.extend([
            dir("m05/!aaa", "m05"),
            card("m05/!aaa/x.md", "m05/!aaa", None),
        ]);
        let with_one = frames(&one);
        for (id, fr) in &base {
            assert_eq!(*fr, with_one[id], "{id} moved for an uncommitted sibling");
        }
        // two uncommitted siblings: the one first by path keeps its slot when the other arrives
        let mut two = one.clone();
        two.extend([
            dir("m05/!bbb", "m05"),
            card("m05/!bbb/x.md", "m05/!bbb", None),
        ]);
        let with_two = frames(&two);
        assert_eq!(with_one["m05/!aaa"], with_two["m05/!aaa"]);
        let mut rev = f.clone();
        rev.extend([
            dir("m05/!bbb", "m05"),
            card("m05/!bbb/x.md", "m05/!bbb", None),
        ]);
        let alone = frames(&rev);
        assert_ne!(alone["m05/!bbb"], with_two["m05/!bbb"], "!bbb ignored !aaa");
    }

    #[test]
    fn add_property() {
        let f = fixture();
        let dirs: Vec<String> = f
            .iter()
            .filter(|n| n.is_dir)
            .map(|n| n.id.clone())
            .collect();
        let mut rng = Rng(42);
        for k in 0..200 {
            let extra: Vec<LNode>;
            let folder: String;
            match rng.below(4) {
                0 => {
                    let d = &dirs[rng.below(dirs.len())];
                    let id = if d == "." {
                        format!("p{k}.md")
                    } else {
                        format!("{d}/p{k}.md")
                    };
                    folder = d.clone();
                    extra = vec![card(&id, d, None)];
                }
                1 => {
                    let d = &dirs[rng.below(dirs.len())];
                    let s = if d == "." {
                        format!("p{k}")
                    } else {
                        format!("{d}/p{k}")
                    };
                    folder = s.clone();
                    extra = vec![dir(&s, d), card(&format!("{s}/x.md"), &s, None)];
                }
                2 => {
                    let s = format!("{}{k}", ["a", "n", "z", "0"][rng.below(4)]);
                    folder = s.clone();
                    extra = vec![dir(&s, "."), card(&format!("{s}/x.md"), &s, None)];
                }
                _ => {
                    folder = ".".into();
                    extra = vec![card(&format!("q{k}.md"), ".", None)];
                }
            }
            assert_local(&f, &extra, &folder);
        }
    }

    #[test]
    fn spread() {
        let f = fixture();
        let sp = layout(&f);
        assert!(
            sp.pos.iter().flatten().all(|c| c.is_finite()),
            "a coordinate is not finite"
        );
        assert!(sp.radius.is_finite() && sp.radius > R_GAL);
        // regions: the 21 top-level folders, at least 25° apart
        let axes: Vec<V> = sp.regions.iter().skip(2).map(|r| r.axis).collect();
        assert_eq!(axes.len(), 21);
        let mut min_deg = 180.0f64;
        for i in 0..axes.len() {
            for j in i + 1..axes.len() {
                min_deg = min_deg.min(dot(axes[i], axes[j]).clamp(-1.0, 1.0).acos().to_degrees());
            }
        }
        assert!(min_deg >= 25.0, "regions only {min_deg:.1}° apart");
        // cards of one folder at least 0.95 × DMIN apart
        let mut groups: BTreeMap<&str, Vec<V>> = BTreeMap::new();
        for (i, n) in f.iter().enumerate() {
            if let (false, Some(p)) = (n.is_dir, n.parent.as_deref()) {
                groups.entry(p).or_default().push(sp.pos[i]);
            }
        }
        assert!(groups.values().any(|g| g.len() >= 147));
        let mut min_d = f64::INFINITY;
        for g in groups.values() {
            for i in 0..g.len() {
                for j in i + 1..g.len() {
                    min_d = min_d.min(len(sub(g[i], g[j])));
                }
            }
        }
        assert!(
            min_d >= 0.95 * DMIN,
            "cards of one folder only {min_d:.2} apart"
        );
    }
}
