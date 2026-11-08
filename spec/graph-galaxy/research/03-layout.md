# Research 03: layout math

Topic: a deterministic 3D hierarchical layout that meets R10, with a proof sketch, constants, and
tests. Prototyped in a throwaway Python lab on the real `~/KB` tree (944 `.md` cards, 21 regions,
145 folders, max depth 8, the largest flat folder 147 cards, `workspace/` with 22 subfolders).

## 1. The constraint, stated exactly

R10's second clause forbids pure set-based spreading. Let `f(S, x)` be the position of folder `x`
among sibling set `S`. If adding any sibling `y` leaves every `x` unchanged, then by induction from
`{x}`, `f(S, x) = f({x}, x)`: each folder's direction depends on itself alone. Count-based
Fibonacci (slot `i` of `N`, sorted by name) moves **every** region when `N` changes, and sibling
repulsion between subfolders moves a sibling's whole subtree. Both break R10.

Pure hashing passes R10 but clumps. In the lab, hashing the 21 region names onto the sphere gave a
min separation of 1-3° (0° when quantised to a 64-slot lattice, because two regions shared a slot).
Hand-picking the salt for today's KB reached only 22-25°, and each new region still had a ~45%
chance of landing on an occupied slot. A perfect Fibonacci-21 sphere gives 39.4°.

**The way out: give siblings an order that a newcomer can never jump.** `/api/index` already sends
`ft` for every file: the time of its first commit, following renames (`index.rs:855-858`, built
from the full `git log` in `history()`, which `build()` runs synchronously before any snapshot).
Define

    birth(X) = min ft over all files under folder X   (missing ft ⇒ +∞)
    key(X)   = (birth(X), fnv64(X), X)                 // total order

A newly added card has no `ft`, or the latest one, so no existing folder's birth changes, and a
folder it creates sorts **last** among its siblings. Siblings are then placed **greedily in key
order**: each takes the slot on a fixed candidate lattice that lies farthest from the slots its
older siblings took. Older siblings never see younger ones, so a newcomer cannot displace them.
That keeps the Fibonacci sphere and gives real spread. Client cost: one field,
`pub ft: Option<f64>` on `model::Node` (`#[serde(default)]`). There is no `/api/index` change.

## 2. Algorithm

Input: the `Idx` node list, reduced to `LNode { id, parent, is_dir, ft }`. The layout SHALL NOT
read edges, link counts, line counts or render sizes, because adding a card changes other cards'
link counts. Size-as-importance stays render-only. Output: `BTreeMap<id, [f64; 3]>` plus a frame
per folder (`anchor`, `axis`, `depth`, `core`).

**Hash.** FNV-1a 64 over the path's UTF-8 bytes, then the splitmix64 finalizer. A stream
`u(h, k) = (splitmix64(h + k·0x9E3779B97F4A7C15) >> 11) · 2⁻⁵³` gives values in [0, 1). Use
`u64::wrapping_*` throughout. Never use `RandomState`/`DefaultHasher`, and never iterate a
`HashMap`; use sorted `Vec`s or a `BTreeMap`.

**Regions (depth 1).**
- Lattice: `L_i = fib(i, M)` with `M = 256`, where `z = 1 − (2i+1)/M`, `φ = i·π(3−√5)`.
- Directions: `greedy(regions by key, L)`. The first region takes slot `fnv(name) % M`. Each later
  region takes `argmax_i min_{taken t} angle(L_i, t)`, scanning `i` upward with a strict `>`, so
  ties go to the lowest index.
- Core: `C_R = R_GAL · u_R`, with `R_GAL = 900`.
- Lab, min separation of today's 21 regions by `M`: 32 → 34.6°, 64 → 29.3°, **256 → 36.9°**,
  512 → 33.1°. Greedy is noisy, so tune `M` on the real tree. After adding 10 hypothetical new
  regions it is still ≥24°, and the original 21 stay put.

**Subfolders (depth d ≥ 2), recursively.**
- Parent `P` has axis `v_P`. Cap half-angle for its children: `α(d) = A1 · κ^(d−2)`, with
  `A1 = 55°` and `κ = 0.6` (55°, 33°, 20°, 12°, ...).
- Candidates: a fixed Fibonacci cap lattice of `K = 48` points around `v_P`:
  `cosθ_i = 1 − (i+½)/K · (1 − cos α)`, `φ_i = i·GA`, expressed in the ONB of `v_P` (Duff et al.
  2017, branchless and a pure function of `v_P`).
- Child axis: `v_c = greedy(children of P by key, caps)`.
- Depth is distance from the core. `A_c = C_R + r(d)·v_c`, `r(1) = 0`,
  `r(d) = Σ_{k=2..d} Δ_k`, `Δ = [90, 70, 56, 46, 40, 36, 33, 30, ...]`. Deepest today:
  `r(8) ≈ 371`.
- More than `K` siblings: wrap to slot `i mod K` plus a hash jitter of 0.3 of the lattice spacing.
  No folder has more than 22 today.
- The folder node sits at `A_F`.

**Cards in folder F.**
- `n = |cards(F)|`, cloud radius `s_F = S0·∛n + DMIN`, with `S0 = 7` and `DMIN = 10`. Depending on
  `n` is allowed: only F's cards move.
- Seed each card from `h = fnv(path)`: direction `norm(2u(h,0..2) − 1)`, radius `s_F·∛u(h,3)`.
- `index.md` is pinned at `A_F`, a fixed obstacle in the relaxation.

**Relaxation, within F only.** Cards of other folders and folder anchors never take part.

    for it in 0..16:                        // ITERS = 16, Jacobi (accumulate, then apply)
      disp = 0
      for i < j in path-sorted order:
        d = |p_j − p_i|
        if 1e-9 < d < DMIN: m = ½(DMIN − d)/d; disp_i −= m(p_j − p_i); disp_j += m(p_j − p_i)
      for unpinned i: p_i += disp_i; if |p_i − A_F| > s_F: p_i = A_F + s_F·unit(p_i − A_F)

- Cost: `Σ n_F²/2 · 16`, about 0.5M distance evaluations for this KB. CPython does the whole
  layout in 144 ms, so expect low single-digit ms in WASM. Add no spatial grid until a folder
  passes ~500 cards.

**Root-level cards (recommended).** The four root cards sit at the galaxy centre: `"."` has its
anchor at the origin, `index.md` pinned there, and the rest in a cloud of radius ~21, relaxed among
themselves. They read as the core the regions orbit, and they cost nothing in R10, because the root
folder holds only root cards.

**Lab quality (age-greedy vs pure hash).**

| Metric | Age-greedy | Pure hash |
|---|---|---|
| Region min separation | 29.3° (M=64) | 0° |
| Sibling-folder min separation | 14.0° | 1.3° |
| Cross-folder card pairs closer than DMIN | 3 (6 of 944 cards) | 34 (50 cards) |
| Within-folder min distance | 9.90 (DMIN 10) | 9.90 (DMIN 10) |

**Float determinism.** `+ − × ÷ √` are IEEE-exact on both targets. `sin`, `cos` and `acos` go
through different libms natively (Apple) and in wasm32 (Rust's libm port), so native and WASM may
differ in the last ulp. R10 concerns one binary, so the tests compare native with native. If the
plan wants native and WASM bitwise equal, it can use the `libm` crate on both. For greedy, compare
angles as `dot` (a larger min-dot is worse), which needs no `acos`.

## 3. Proof sketch (R10)

Invariants, true by construction:

- **I1.** `axis(X)` depends only on `axis(parent(X))`, `depth(X)`, and the slots of X's
  **older** siblings (lower key). `anchor(X)` depends only on `core`, `depth(X)` and `axis(X)`.
- **I2.** The positions of F's cards depend only on `anchor(F)` and the set of F's cards.
- **I3.** Nothing reads edges, sizes or counts from outside F.

Add card `c` in folder F, possibly creating new folders `G_1 ⊂ ... ⊂ G_k = F`, possibly a new
region. Then:

1. Every existing folder X keeps its birth, since `ft(c) = ∞` or is the newest.
2. Every new `G_j` has birth `∞`, so it sorts after every committed sibling.
3. By induction from the root, each existing X has the same parent axis and the same older
   siblings, so by I1 its axis and anchor are unchanged.
4. For each existing folder Y ≠ F, the card set and anchor are unchanged, so by I2 its cards are
   unchanged.
5. Only F's cards can move. ∎

**Precondition:** existing folders are committed. Among never-committed sibling folders (births
tied at ∞, ordered by hash), a new one can displace another uncommitted sibling. The same holds
for a KB with no git history at all. State this in TECH as the known limit.

Out of scope for R10, which covers additions only: moving a long-lived card into a new folder
(renames carry `ft`), or deleting a folder's oldest file. Either can reorder younger siblings.

## 4. Tests (pure Rust, `cargo test -p okt-client`)

`cargo test -p okt-client` runs natively today: 6 tests pass in 10 s with
`CARGO_TARGET_DIR` set. The crate is `cdylib`, and tests already live in `#[cfg(test)] mod tests`
in `js.rs` and `model.rs`. Put the layout in its own module (e.g. `okt-client/src/layout3.rs`),
importing only `std`. It needs no `web_sys`, `js_sys` or `crate::js`.

- **Fixture builder.** A synthetic tree shaped like the KB: 21 regions with distinct births, a
  147-card flat folder, a depth-8 chain, a 22-subfolder region, and 4 root cards with
  `index.md`s. Use a hand-rolled splitmix RNG, since the crate has no `rand`. An optional test
  gated on `OKT_INDEX_JSON` (like `parse_log_sample`) runs the same assertions on a real snapshot.
- **`deterministic`.** Run `layout(nodes)` twice and assert every coordinate is equal by
  `f64::to_bits`.
- **`order_independent`.** Shuffle the input with the seeded RNG and assert bitwise equality. The
  lab confirmed this.
- **`r10_*`, one test per case.** Add one card with `ft = None` at each of these places:
  - (a) an existing folder;
  - (b) a new subfolder of an existing folder;
  - (c) a new two-deep chain;
  - (d) a new region that sorts first by name (`aa…`) and one that sorts last (`zz…`);
  - (e) the root.

  Assert bitwise that every pre-existing node, folder nodes included, whose folder is not the new
  card's folder is unchanged. The lab found 0 nodes moved outside in all six cases; inside, 134 of
  135 `contact/` cards moved, and 3 at the root.
- **`r10_property`.** Make 200 random single-card additions on the fixture and apply the same
  assertion.
- **`spread`.** Region min angle ≥ 25° on the 21-region fixture, within-folder min distance
  ≥ 0.95·DMIN, and no NaN or infinite coordinates.
- **`no_edge_input`.** Enforced by the signature, since `layout` takes no edges. Leave a comment so
  nobody "improves" it with link-weighted placement.
