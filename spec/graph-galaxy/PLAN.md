# PLAN — The graph as a navigable 3D space: galaxy, system, focus

> **Status:** Draft for review · **Last updated:** 2026-11-08
> **Authoritative technical design.** The *how*. Vision/contract is [`GOAL.md`](GOAL.md);
> the phased executable roadmap is [`TECH.md`](TECH.md). Backing detail is in
> [`research/`](research/). Every design element traces to a GOAL R-ID.

## 1. Summary

Replace the 2D radial canvas with a **WebGL2 scene and a flat 2D overlay**. The benchmark picked
WebGL2: both candidates passed, with WebGL2 drawing in 0.1–0.3 ms of JS per frame at full KB scale
([05](research/05-renderer-benchmark.md)). The scene is drawn from a **deterministic hierarchical
3D layout** that provably moves nothing outside a new card's folder. A **spherical camera** solves
its own angle and distance and travels by eased moves and a rising warp flight. **Visibility tiers**
by graph distance mean most of the graph is not drawn. Everything that is math (layout, camera,
tiers, label placement, walk order) lives in pure modules tested natively by `cargo test`. The
browser-facing parts publish their state as `data-*` attributes, so `keys.mjs` and the perf
harness can grade them without reading pixels. Seven vertical phases; the appetite is big and the
size breaker is waived (GOAL Clarifications).

## 2. Design

### 2.1 Module layout (`okt-client/src`)

`graph.rs` stays the module root, keeping its file name and its public API. It gains a `graph/`
directory of submodules:

| Module | Pure? | Holds |
|---|---|---|
| `graph.rs` | no | `G` state in the existing `GS` thread-local, wiring, the public API (`wire`, `build`, `update`, `focus_node`, `fit`, `zoom_by`, `frame_selection`, `walk*`, `rep_of`, `toggle_collapse`, `toggle_show_all`, `shape_path`, `schedule`), the frame clock, input, `data-*` hooks |
| `graph/space.rs` | yes, `std` only | the layout: `layout(&[LNode]) -> Space` |
| `graph/cam.rs` | yes | pose, basis, projection, matrices, Jacobi eigen, view solve, fit, easing, warp path, scale classification, wheel normalisation |
| `graph/vis.rs` | yes | regions, adjacency, region-pair counts, BFS tiers, edge selection, importance, angular walk order |
| `graph/labels.rs` | yes | the greedy flat label placer over screen rects |
| `graph/gl.rs` | no | the WebGL2 renderer: programs, VAOs, instance buffers, passes, context loss |
| `graph/overlay.rs` | no | the 2D overlay canvas: labels, region names, portal labels, compass |

`AGENTS.md` widens to match: the Map names `graph/`, and the coupled core reads "`app.css` with
`graph.rs` and `okt-client/src/graph/`". Only the coupled core widens; the high-blast-radius list is
untouched.

### 2.2 Layout (`graph/space.rs`), R10

As in [03](research/03-layout.md), with these settled values:

- **Input:** `LNode { id, parent, is_dir, ft }` from `Idx`. `model::Node` gains
  `#[serde(default)] pub ft: Option<f64>`, which `/api/index` already sends (`index.rs:857`), so
  there is no server change.
- **Order of siblings:** `key(X) = (birth(X), X)`, where `birth` is the minimum `ft` of the files
  under X. A folder with no committed file has birth +∞, so it sorts after every committed sibling,
  and ties are broken by path (GOAL Clarification on R10, 2026-11-08).
- **Regions:** placed greedily in key order on a 256-point Fibonacci lattice, at `R_GAL = 900`. The
  surfaces' slot is taken first (below), so every region, the oldest included, takes the free slot
  farthest from those already taken, compared by dot product. The first-slot rule survives on the cap
  lattices: a folder's oldest child takes slot `fnv % 48`. (Amended in build P1, 2026-11-08: the oldest
  region took `fnv % 256` before the surfaces' slot was reserved.)
- **Subfolders:** a 48-point cap lattice around the parent axis, with half-angle `55°·0.6^(d−2)`.
  Distance from the region core is `Σ Δ_k`, with `Δ = [90, 70, 56, 46, 40, 36, 33, 30…]`.
- **Cards:** seeded by the path hash inside a cloud of radius `7·∛n + 10`, with `index.md` pinned at
  the folder anchor. 16 Jacobi relaxation passes run within the folder only, at minimum spacing 10.
- **Root cards:** at the galaxy centre.
- **Surfaces:** pseudo-region `@`. Its slot is reserved on the region lattice before any region
  (slot `fnv("@") % 256`), whether or not a surface exists. Surfaces sit at pure hash positions
  within a fixed radius of 60 around its core, with no relaxation, so a new surface moves nothing
  else. (Amended in build P1, 2026-11-08: with birth +∞ the `@` pseudo-region sorted among the
  uncommitted regions by path, so a new uncommitted region whose name sorts before `@`, a digit
  for one, moved the surfaces, and the first surface moved every uncommitted region after it.)
- **No edge input:** the layout takes none (enforced by its signature, and a comment says why).
- **Determinism:** FNV-1a 64 plus splitmix64; sorted `Vec`/`BTreeMap` only, never a `HashMap`
  iteration. Collapse never moves positions.
- **Output:** `Space { pos: Vec<[f64;3]>, frame: per dir (anchor, axis, depth, core), region
  of each node, bounding radius per region and for the galaxy }`. `G` keeps it in an `Rc`.
- **Layout hash:** FNV-1a over positions quantised to 1e-6, computed once per build. It is published
  as `data-layout`.

### 2.3 Camera (`graph/cam.rs`), R2, R3, R4, R6, R7, R11

From [04](research/04-camera.md), with one correction (digest § 3):

- **Pose:** `{target, θ, ψ, ln d}` with +Y up and ψ clamped to ±85°. Closed-form basis, 40° vertical
  FOV, aspect from the pane rect (never the window), device pixels applied at the end. One
  `project()` serves picking, labels and hooks; `view_proj()` serves GL.
- **View solve:** uniform covariance, then cyclic Jacobi.
  - The view direction is the **exact** smallest eigenvector, with its sign chosen toward the
    current view.
  - The set is degenerate when n ≤ 2, `tr C` ≈ 0, collinear (`λ₁ < 1e-9·tr C`), or the two
    smallest eigenvalues are within 10% of each other (`λ₁/λ₀ < 1.1`, a ratio test, never a
    fraction of the trace, which would call a well-defined axis degenerate). Then choose the
    direction in the near-degenerate eigenspace (the plane of e₀ and e₁, or the plane perpendicular
    to the segment) that is closest to the current view. A single point keeps θ and ψ.
- **Fit:** the closed form at F = 0.70, with pad = drawn radius plus label room. `d_min` comes from
  a minimum radius of 1.5 × spacing.
- **Targets:**
  - card selection or `z`: the selected card;
  - folder: its region core;
  - none: the galaxy origin;
  - `z` from Results: the midpoint of the matches' view-plane bounding box at their mean depth.
- **Moves:** `CamAnim { from, to, t0, ms, kind }` replaces `Anim`.
  - `Ease`: 450 ms smootherstep. θ takes the short arc; `ln d` moves linearly.
  - `Warp`: 600 ms on the van Wijk–Nuij path (ρ = √2), with the eased parameter, θ/ψ eased over the
    same `u`, and the per-frame distance capped at the galaxy fit.
  - A new move starts from the pose evaluated this frame (no snaps). Drag, wheel and `+`/`−` cancel
    any move in flight.
- **Reduced motion:** `fn reduced_motion()` reads `matchMedia('(prefers-reduced-motion: reduce)')`
  at the start of every move, with the `chrome.rs:60` pattern. When it is set, `pose = to` and one
  frame is scheduled. Every camera move goes through one entry point, `cam_to(to, kind)`, so no path
  bypasses the rule.
- **Scales:** `classify(d, bands, prev, innermost)`, consulted **only on user zoom input**
  (wheel, pinch, `+`, `−`). After any solved move (a selection, `z`, `0`, a flight, `[`/`]`) the
  scale is the move's own kind (focus, system or galaxy), whatever the distance, so a wide
  Results fit stays at focus scale.
  - The band boundaries are `B = √(d_inner·d_outer)` with ±15% hysteresis.
  - Selection sets the innermost scale: a card gives focus, a folder gives system, no selection
    gives system (reached only by zooming in).
  - Zooming in at galaxy picks the region whose projected core is nearest the pane centre (ties:
    card count, then name), marks it the *active region*, and eases the target there.
  - Zooming out past the system/galaxy band eases the target to the origin. Wheel input during that
    ease adjusts `ln d` of both ends instead of cancelling it, so the target never strands.
  - "Nearest the pane centre" considers regions whose core is in front of the camera first; a region
    behind is chosen only when none is in front.
  - Zooming in at system scale crosses to focus scale only when a card is selected in the active
    region (it re-solves R3); otherwise the distance clamps at `d_min`.
  - Crossings never call `nav::select` (R2).
- **Input:**
  - drag: Δθ = −dx/min(W,H)·π, Δψ = +dy/min(W,H)·π, keeping the 4 px dead zone;
  - wheel: normalised by deltaMode, `ln d += dy·k` with k = 0.0016, or 0.012 when `ctrlKey` (pinch);
    each event clamps to |Δ| ≤ 0.5 and d stays in `[d_min, 1.5·d_G]`;
  - keys: ←/→ ±15° azimuth and PageUp/PageDown ±10° elevation, each eased over 140 ms, only while
    the graph view is active and `document.activeElement` is neither inside the inspector
    (`#insp`) nor an input. Otherwise the key falls through to the browser, so a focused inspector
    still scrolls natively (row D1);
    - *2026-11-08, review cycle 1 (M1):* the test is where the user last pressed a pointer, not
      `document.activeElement`: a click in the inspector's text focuses nothing, so the focus test
      took PageUp/PageDown from a reader. A pointerdown inside `#insp` hands ←/→ and
      PageUp/PageDown to the browser until the next pointerdown in the graph pane (`#v-graph`);
      focus inside `#insp` or in an input still hands them over too.
  - `+`/`=` and `−`: `ln d ∓ ln 1.35`;
  - `0`: always the galaxy fit, keeping the selection, as today's `0` fits the whole graph
    whatever is selected (`graph.rs:1006`). The `.gtools` fit button and a double-click on empty
    space do the same;
  - zoom centres on the target (row D3).
  - A selection while the graph view is **visible** eases, whatever its source (graph click,
    inspector chip, pin, `n`/`N`); while the graph is hidden it cuts.
  - Resize re-fits as a cut when `autofit` is set; otherwise d is kept.

### 2.4 Scales, rim and drawing, R1, R2, R8

- **Scale is presentation, tiers are content.** The tier set (§ 2.5) decides which cards and edges
  are drawn at every scale; the scale decides the camera, the glyph and rim presentation and the
  label candidates. Zooming out to galaxy with a card still selected keeps its tiers drawn (R8 holds
  while show all is off), with the other regions back at their true positions.
- **Root and surfaces** (row D11): the root cards sit at the galaxy centre as one unlabelled core
  glyph, and the surfaces pseudo-region `@` as one unlabelled glyph. Neither is a region: neither
  counts in `data-regions`, gets a name label, or gets a bundle. Links from root cards or surfaces
  appear only as tier edges when a focus touches them.
- **Galaxy** (no selection, or zoomed out):
  - Each region is a **cluster glyph** at its core: a soft halo whose radius grows with
    √(card count), in the region's family colour, with the region's name.
  - Its cards are **not** drawn individually: the glyph is the cluster (R8: outside the focus,
    cards appear only as region clusters). The glyph's halo is a soft radial fill whose radius and
    density grow with the card count, which reads as the Outcome's "dense cloud" without drawing
    cards.
  - One **bundle** per region pair with links: a quadratic Bézier with its control point pulled 50%
    toward the origin, width `w0 + w1·ln(1+n)` and alpha rising with `ln(1+n)/ln(1+n_max)`. The
    geometry is tessellated once per build.
  - Labels are region names only.
- **System** (a folder is selected, or zoom-in):
  - The focused region is drawn in 3D and fitted to the pane.
  - Subfolders draw as sub-cluster glyphs at their anchors. Cards are tier 3 points, or tier 0/1
    when the selected folder is tier 0.
  - Every other region is a **rim glyph** (digest § 4) carrying its name and the bundle to it.
- **Focus** (a card is selected): the tiers of [07](research/07-labels-picking-visibility.md) § 1:
  - tier 0 is the card, with a static halo;
  - tier 1 is its links, in full colour;
  - tier 2 is dim, with edges only on a path to the focus;
  - tier 3+ is the rest of the region, faint, with no edges;
  - other regions are rim glyphs with their bundles.

  Focus-set cards are drawn in any region, at their true positions. When one is off-pane, its edge
  is drawn to its region's rim glyph instead: a **portal**.
- **"Show all"** (`Settings.show_all`): every card and every link edge at true positions, with
  depth cues, in place of the retired radial spectacle. The tier colours still mark the focus.
  Toggled from `.gtools [data-z=all]` and the settings DISPLAY row. R9's label cap still holds.
- **Collapse** (row D12): a collapsed folder's **tier-3** cards are drawn as its glyph, which shows
  the count. Focus-set cards (tiers 0 and 1) are always drawn (Clarifications), and **show all
  overrides collapse** (R8: every card). The collapse settings keep today's semantics, auto-collapse
  threshold included. Space, Enter, `E`, EXPAND/COLLAPSE ALL and the toast keep today's meaning
  (R6: "with no walk, space and Enter keep today's meaning").
  - *2026-11-08, review cycle 1 (L2):* the count is drawn as the 2D graph drew it, with show all
    off: a ring round the folder's point in its family colour and the file count beside it, on
    the overlay (`--glab-count-*`). A card with uncommitted changes gets its dot back, at its
    point's upper right (`--graph-dirty`).
- **Depth cues:**
  - perspective size;
  - saturation and alpha falling over the drawn set's depth range;
  - an aerial fade toward `--graph-bg`;
  - no blur.
- **Size:** `r0·(1+0.35·ln(1+deg))`, ×1.4 for `index.md`; folders sized by file count.
- **Compass:** on the overlay, bottom-right (56 px, inset 10 px).
  - The world axes rotated by the view, with the far axis dimmed.
  - A horizon ring, with a dot for the focused region's direction.
  - The scale word.
  - It is a label obstacle.
- **GL passes**, with no depth test and premultiplied alpha:
  1. bundles and edges as line quads;
  2. points as instanced quads, CPU-sorted back to front by view depth each frame (about 1.3k
     indices, < 0.1 ms);
  3. halos and glyph halos, additive only when `--graph-bg` luma < 0.5.

  Instance data is re-uploaded only when the tier set changes. Per frame only the uniforms and the
  sorted index order change.
- **Context loss:** a `webglcontextlost` listener stops the frames. `webglcontextrestored` recompiles
  and re-uploads from `G`.
- **Colours:** `Colors::read` canonicalises each custom property through the overlay's `fillStyle`,
  then parses it to `[f32;4]`.
  - New tokens join the graph block of `app.css`: `--graph-halo`, `--graph-bundle`, `--graph-fog`,
    `--graph-t2-alpha`, `--graph-t3-alpha`, plus the overlay label rules.
  - Accent changes now rebuild the graph colours as well (fixing the stale accent noted in brief 01).
  - The spotlight layers and `.spotlit` rules retire.

### 2.5 Visibility (`graph/vis.rs`), R8

Computed once per index load:
- `ix`;
- `adj` over all link edges, both directions;
- `region` per node;
- region-pair link counts, summing `n`.

Computed on every focus change, never per frame: BFS to 2 hops from the focus set, with folders
terminal. The tier for each node is:

```
t = hop(c)
if c ∈ F or t == 1          -> t                  // focus set: any region
elif region(c) ∈ R_F        -> min(t, 3)          // ... but no tier 3 when |R_F| > 1
else                        -> HIDDEN             // folded into its region glyph
```

R_F is the set of focused regions:
- empty with no selection;
- the selection's region for a card or folder;
- the matches' regions for a `z` from Results.

Edges are drawn as follows:
- tier 0–1: hot, with out-links in the accent and the walk target's edge thicker;
- tier 1–1: faint;
- tier 1–2: dim;
- portals: to the rim glyph;
- bundles: all pairs at galaxy scale, otherwise the pairs touching R_F;
- nothing else.

In show-all, nothing is hidden and every edge draws.

### 2.6 Labels (`graph/labels.rs`, `graph/overlay.rs`), R9

- **Candidates:** tier 0, then tier 1 (portals included). Galaxy scale has no card candidates. In
  show-all every card is a candidate.
- **Order:** `(tier, walk target first, −importance, path)`.
- **Placement:** the walk target's label first (a portal's included), so R5's "labelled with the
  target's path" can never be crowded out; then region names (exempt from the cap; a colliding one
  is dropped for that frame); then cards, until 24 non-region labels are placed. Every non-region
  kind (card, portal) counts against the 24. Each card tries its last anchor, then right, left,
  above, below.
  - *2026-11-08, review cycle 1 (M3):* a region name with no free side is not dropped while there
    is room for it: it moves out along its lean (toward the pane centre), sliding sideways, and a
    leader line joins it to its glyph (`--glab-leader`), so every rim cluster is named (R2). The
    free test, and so R9's no-overlap, is the same one.
- **Collisions:** AABBs with 2 px padding. The obstacles are the compass, `.gtools`, `#glegend`,
  `#ghint` and the tier-0 disc.
- **Motion:** the label set is frozen during motion and re-solved 150 ms after rest. A frozen label
  that fits no anchor hides for that frame.
- **Text:** a `measureText` cache keyed by text and font, cleared on build. Card text is the file
  name without `.md`. A portal label is the KB path, middle-ellipsised to 34 characters.
- **One list:** the overlay draws exactly the `Placed` list, measuring each rect with the same
  `measureText` metrics and DPR it draws with, and clips each label's text to its rect; `data-labels`
  is written from that list (`[x,y,w,h,kind,text]`), and `data-label-cands` counts the candidates.
- **Purity:** `labels::place(cands, obstacles, pane) -> Vec<Placed>` is pure, so native tests prove
  `count ≤ 24` and pairwise non-overlap over random scenes.

### 2.7 Walk, `z`, warp and history (`keys.rs`, `nav.rs`, `insp.rs`), R4, R5, R6

- **Walk order:**
  - Membership stays `insp::walk_list(idx, sel)`, so `N` and the `#ghint` `i/N · label · name · ⏎ go
    · esc` format survive.
  - `vis::angular_order(focus_px, targets_px)` sorts clockwise from 12 o'clock by `atan2`. Ties go
    to screen distance, then path.
  - A portal target uses its rim glyph position.
  - The order is computed from the **destination** pose (`anim.to` while a move runs), so a
    mid-ease projection never reorders it, and is frozen for the walk. A user orbit or zoom during
    the walk re-sorts it and re-finds the current target by id. A target behind the camera uses its
    rim direction, never a flipped projection.
  - The camera holds still during j/k. A target that is off-pane but in the region triggers a small
    re-fit; the frozen order stands.
- **Inspector mark:** `insp::mark_chip(Some(target))` is unchanged; it marks by id. `insp::render()`
  re-applies the mark from `graph::walk_target()`. A target beyond the 40-chip cap gets no mark; this
  is pre-existing and recorded as `issues/walk-mark-chip-cap.md` (§ 5).
  - *2026-11-08, review cycle 1 (M2):* a target past the cap gets its chip shown at the end of its
    group and marked; that chip goes again at the next mark, so the group is back to its usual
    length once the walk moves on or stops. The seed and its ROADMAP entry are retired.
- **`z` (`keys.rs:517`):** capture `s().view == "list"` and clone `S.list` **before** switching the
  view, then drop the borrow.
  - Results active: the focus set is the FIND files and names, or the Paths items, and the camera
    solves for it.
  - Otherwise: the R3 solve for the selection.
  - Either way it goes through `cam_to`, and positions never change (`data-layout` stays constant).
- **Warp:** in the walk branch at `keys.rs:413`, Enter/space calls `nav::select(Some(t),
  SelOpts { warp: true, .. })`.
  - `nav::select` hands the camera move to `graph::focus_node_with(id, Move::Warp)`.
  - The `from != "graph"` guard at `nav.rs:56` goes, because R3 moves the camera on every
    selection: a graph click eases, an off-view selection cuts.
- **Flight shape:** the van Wijk–Nuij path, with a floor on the rise: the peak distance is at least
  `max(2·max(d₀, d₁), d_system)`, capped at the galaxy fit, so even an in-region hop visibly rises
  "toward galaxy scale". Duration 600 ms.
- **History:** before `nav(true)` pushes, `nav` calls `replace_state` on the current entry with
  `{pose: graph::pose_json()}`, which stores the **destination** pose of a move in flight, never a
  mid-flight pose. On `popstate`, `apply_hash` reads `history.state.pose` **after its
  last await** (§ 1) and calls `graph::cam_to(pose, Ease)`, falling back to the R3 solve. It also
  calls `graph::walk_reset()` when the selection changes. It reads the hash and `history.state`
  together, at one moment after the await, so a second `[` in between can't pair one entry's hash
  with another's pose. The hash format and dedupe stay unchanged. "Focus change" means a selection
  change (row D10).
  `[`/`]` keep `history.back`/`forward`.

### 2.8 Hooks for tests (`graph.rs`)

`data-*` attributes on `#graph` are written at the end of a frame, only when their rounded value
changes. No CSS selects on them. They are written **inside `gl.rs` and `overlay.rs` from what was
drawn**: the evaluated pose and `view_proj` of the frame, the uploaded instance buffer (its count,
and an FNV hash of its positions as `data-pos-hash`), and the overlay's own placed rects. Never from
`anim.to`, the tier map or a recomputation beside the draw. `cam_to` bumps `data-move`
synchronously, so a test can wait for a move to start before it waits for `data-anim="0"`.

| Attribute | Value |
|---|---|
| `data-scale` | `galaxy`, `system` or `focus` |
| `data-theta`, `data-psi` | degrees, to 0.1 |
| `data-dist` | 3 significant figures |
| `data-anim` | `0` or `1` |
| `data-focus`, `data-focus-n` | the focus and the size of the focus set |
| `data-walk`, `data-walk-i`, `data-walk-n` | the walk target and its position |
| `data-walk-angle` | the target's on-screen angle |
| `data-layout` | the layout hash |
| `data-drawn` | number of cards drawn |
| `data-total` | number of cards in the index |
| `data-labels` | JSON `[[x,y,w,h,kind],…]` of the placed labels |
| `data-showall` | `0` or `1` |
| `data-rim` | number of rim glyphs |
| `data-bundles` | number of bundles drawn |
| `data-regions` | number of region glyphs drawn |
| `data-drawn-regions` | JSON `{region: drawn card count}` |
| `data-tiers` | `n0,n1,n2,n3`: drawn cards per tier |
| `data-axis-err` | degrees between the view direction and the exact smallest eigenvector of the last solve (`-` when degenerate) |
| `data-fit` | the largest \|ndc\| of the fitted members after the last solve |
| `data-peak` | the highest camera distance of the last flight |
| `data-move` | a counter `cam_to` bumps synchronously on every move |
| `data-view` | the 16-float view-projection matrix the last frame drew with |
| `data-pos-hash` | FNV hash of the uploaded position buffer |
| `data-region` | the active region |
| `data-dist-galaxy` | the galaxy fit distance |
| `data-edges` | edges drawn (tier edges, or every edge in show all; bundles counted apart) |
| `data-label-cands` | label candidates before the cap |
| `data-focus-ids` | JSON ids of the focus set the last solve used |
| `window.__oktPos` | when a test installs it (an empty object), the client fills it with `{id: [x,y,z]}` from the uploaded positions on each upload (from the `Space` in P1), the `__oktDraw` pattern |

`window.__oktDraw` survives.

### 2.9 Docs and help, R7

- **README § Keys** (`README.md:79-82`):
  - j/k step "clockwise around the selection, in screen order";
  - Enter/space "fly there", and `[` flies back;
    - *2026-11-08, review cycle 1 (L1):* `[` eases straight back (a solved move, no flight), and
      README and the `?` list say so;
  - `z` frames the selection and its links, or the Results matches;
  - ←/→ orbit, PageUp/PageDown tilt;
  - `+`/`−`/`0` act on camera distance;
  - drag orbits, wheel and pinch zoom.
- **`?` list** (`chrome.rs:469-528`): the same rows, plus the GRAPH `<dl>`.
- **Doc comments** that restate the order (`keys.rs:400`, `graph.rs:1849`, `insp.rs:484`) are
  updated in the same commit.
- **README:9-11** loses "radial" and "panning".

### Requirement → design map

| R-ID | Design element(s) that satisfy it |
|---|---|
| R1 | § 2.4 galaxy: region glyphs at cores, region-pair bundles weighted by `ln(1+n)` (`vis.rs` pair counts), region-name labels only (§ 2.6 has no card candidates at galaxy scale); camera fits the galaxy when nothing is selected |
| R2 | § 2.3 `classify` bands, innermost scale from the selection, zoom-in picks the region nearest the centre; § 2.4 system: region fitted, subfolder glyphs, depth as radius (`space.rs` Σ Δ), rim glyphs in true direction with bundles; crossings never select |
| R3 | § 2.3 view solve (exact smallest axis), 70% fit of the focus-set members in the selection's region, with other regions' members as rim portals (as amended), `cam_to(Ease)` on every card selection (the `nav.rs:56` guard removed) |
| R4 | § 2.7 `z`: Results matches (captured before the view switch) or the selection's set; re-solve via `cam_to`; `data-layout` constant |
| R5 | § 2.7 angular order in `vis.rs`, re-sorted per step; portals to rim glyphs labelled with the target path (§ 2.6); `mark_chip` by id, re-applied after `insp::render()` |
| R6 | § 2.7 warp: `SelOpts.warp` → `cam_to(Warp)` 600 ms van Wijk–Nuij path rising toward galaxy; history push with the pose stashed; popstate eases to the stored pose; no walk → today's space/Enter |
| R7 | § 2.3 drag orbit, wheel and pinch zoom, ←/→ and PageUp/PageDown (proved free, [02](research/02-keys-history.md) § 2), `-`/`+`/`0` on distance; § 2.9 README and `?` |
| R8 | § 2.5 tiers and edge rule; § 2.4 show-all draws every card and edge; `data-drawn` vs `data-total` |
| R9 | § 2.6 placer: ≤ 24 card labels, region names exempt, flat overlay, no overlap; native property tests and the `data-labels` rect check |
| R10 | § 2.2 age-ordered greedy lattices (uncommitted folders last, ties by path, as amended), folder-local relaxation, no edge input; native determinism, order-independence and per-case locality tests; `data-layout` stable across reloads |
| R11 | § 2.3 one `cam_to` entry with the reduced-motion cut; WebGL2 static buffers; `profile.mjs --graph` headed orbit and warp scenarios with a p95 ≤ 16.7 ms verdict (§ 6) |
| R12 | § 2.8 hooks; `keys.mjs` extended per phase with named checks (§ 6) |

## 3. Invariant gate (AGENTS.md constitution check)

Checked against `AGENTS.md` § Invariants before research (Step 2) and again against this design.

- **High-blast-radius list:**
  - `okt-client/src/state.rs` **is touched**, and Trigger 2 fired. The edit is one field: `labels`
    becomes `show_all`, persisted as `showAll` (row D4). No new borrow paths: the graph reads
    `cfg().show_all` the way it reads `cfg().labels` today.
  - `okt-server/src/http.rs` is not touched: WebGL2 needs no CSP change ([05](research/05-renderer-benchmark.md)).
  - `okt-server/src/index.rs` is not touched: `ft` is already in `snapshot()` (GOAL non-goal honoured).
  - `launch.rs` is not touched.
- **§ 1 Client re-entrancy** (the main risk for a frame clock):
  - `frame_tick` takes one short `g()` borrow to evaluate the pose and write the GL frame, then
    drops it before any DOM hook write that could dispatch.
  - On-arrival work (scale change, tier rebuild, walk re-mark) runs after the borrow is dropped.
  - Handlers clone what they need out of `g()`, `s()` and `cfg()` before calling `nav::select`,
    `insp::mark_chip` or `focus_node`.
  - `apply_hash` reads `history.state` and touches the camera only after its last await.
  - The `z` handler drops its `s()` borrow before switching the view.
  - Since `g()` is always `borrow_mut`, the public API never calls itself while holding it.
    `cam_to` and `walk` are borrow-scoped internally.
- **§ 2 Pygments:** not touched.
- **§ Conventions:**
  - `app.css` changes in every phase that changes the graph's paint (the new tokens, the overlay
    label rules, the retired `.spotlit`/`.ebase` rules). The gate checks this per phase.
  - No R#/P# ids in source or tests; the gate scans added lines.
  - README, `?` and doc comments change in the same commit as the keys they describe.
  - The version is untouched.

### Deviation justifications

Each row was read against the wording of every R-ID it touches. Rows marked *reading* choose an
interpretation the GOAL leaves open and go to the PLAN/TECH sign-off.

| # | Deviation | Why needed | Simpler alternative rejected because |
|---|-----------|-----------|--------------------------------------|
| D1 | *Reading (R7):* elevation orbits on **PageUp/PageDown**, not the Clarification's suggested Shift+ArrowUp/Down | `keys.rs` never tests Shift, so Shift+↑/↓ arrives as `ArrowUp`/`ArrowDown` and **already walks** (`keys.rs:405`/`:409`). Using it would change an existing key's meaning, which R7 forbids. The Clarification marks the keys as suggestions | Gating the walk on `!shift` changes Shift+↑/↓'s meaning; letter pairs (`<`/`>`, `w`/`x`) are free but weaker mnemonics. PageUp/PageDown is fn+↑/↓ on a Mac keyboard, so it rides the vertical arrows. The orbit keys act **only** while the graph view is active and focus is neither inside the inspector nor in an input, so native scrolling of a focused inspector is unchanged (foreman ruling, 2026-11-08) |
| D2 | *Reading (R9):* the existing **"ALL LABELS"** setting and `.gtools` button retire; the slot becomes the **"show all"** toggle | R9's 24-label cap is hard (Clarifications), so an all-labels mode cannot survive. The toggle is a button, not a key, so R7's key rule does not apply | Keeping "all labels" as a silent no-op leaves a lie in the UI. Keeping it working breaks R9 |
| D3 | Wheel and pinch zoom **about the camera target**, not the pointer | Orbit and zoom must share one centre, or the next orbit swings the scene off-pane. R7 only requires "zoom it on wheel or pinch" | Pointer-anchored zoom in 3D needs a ray-cast target change on every wheel event, which fights R3's centred focus. Recorded as a possible follow-up in `issues/graph-zoom-to-cursor.md` (§ 5) |
| D4 | `state.rs` (high-blast-radius) changes: `Settings.labels` → `Settings.show_all`, key `showAll` | "Show all" is an Outcome item and R8 needs it. Persisting it like the setting it replaces keeps the user's choice across reloads | A graph-local, unpersisted toggle avoids `state.rs` but still has to remove the retired `labels` field there; the edit is unavoidable. An old `labels` key lands in `extra` and is ignored |
| D5 | *Resolved by GOAL Clarification (2026-11-08); not a deviation.* R3 and R4's selection branch now read: fit the focus-set members in the selection's region, with members in other regions drawn as portals at the rim; `z` from Results fits all matches in place | — | — |
| D6 | *Reading (R8):* a `z`-from-Results focus spanning several regions draws **no tier 3** | R8's tier 3 is "the rest of *the* focused region" (singular). Drawing several whole regions would bring back the busy picture R8 exists to remove | Drawing tier 3 for every matched region is a literal plural reading that defeats the tiering for broad searches |
| D7 | *Reading (R9):* **portal labels count** against the 24 | R9 exempts only region names; a portal label names a card (R5: "labelled with the target's path") | Exempting portals would let a hub with many cross-region links exceed the cap |
| D8 | *Resolved by GOAL Clarification (2026-11-08); not a deviation.* R10 now promises locality for a card added to an existing folder, and for a new folder younger by first commit than its siblings, or uncommitted. Uncommitted folders sort after every committed sibling, ties by path. The Clarification names the known exceptions | — | — |
| D10 | *Reading (R6):* a "focus change" is a **selection change**. `z` (either branch) and a zoom crossing reframe without changing the focus, so they push no history entry. `[` then returns to the previous selection's stored pose, and view switches (`g`, `f`, …) keep today's history entries | R4 says selecting a match "SHALL make that card the focus, as any selection does", and R2 says crossings "SHALL NOT change the selection". Both tie "focus" to the selection. The Clarification says every focus change pushes, which selection changes already do | Pushing on every `z` would put entries that don't change the URL hash into history; the hash dedupe at `nav.rs:163` exists to stop exactly that |
| D11 | *Reading (R1):* root cards and surfaces are **not regions**. Each is an unlabelled glyph with no bundle. The compass's scale word is pane chrome, not a graph label | R1 says "one cluster per region" and "labels for region names only". The GOAL defines a region as a top-level folder; root cards and the `@` surfaces are neither | Treating root and surfaces as regions adds non-folder clusters and labels that R1 doesn't allow |
| D12 | *Reading (R8 with R6):* a **collapsed** folder's tier-3 cards draw as its glyph. Focus-set cards always draw, and show all overrides collapse | R6 keeps space's folder toggle ("with no walk, space and Enter keep today's meaning"), so collapse must still mean something in the graph. Folding only the least relevant tier honours both | Ignoring collapse empties space's meaning in the graph, which R6 forbids. Folding focus-set cards breaks the Clarification that focus-set cards are always drawn |
| D9 | New module directory `okt-client/src/graph/`, with `AGENTS.md`'s coupled core widened to include it | Over 3k lines of renderer, math and overlay don't belong in one file; the pure modules must be testable on their own | One 4k-line `graph.rs` hides the pure math from review and testing |

Figures: D1's lines are cited from `keys.rs` as read. The 0–3° pure-hash clumping behind the R10
ruling is the lab's measure on 21 regions; the age-greedy 29–37° comes from the design's own
256-point lattice. No row states a time bound.

## 4. Rabbit holes (resolved)

- **Which renderer?** → WebGL2 by the binding headed benchmark: rAF p95 7.6–7.8 ms for both
  candidates at full KB scale, JS draw 0.1–0.3 ms for WebGL2 against 0.8–2.5 ms for Canvas 2D, and
  only WebGL2 holding flat at 10× ([05](research/05-renderer-benchmark.md)).
- **Can a spread-out layout keep R10?** → yes, by ordering siblings by git birth and placing them
  greedily on fixed lattices. The proof sketch and the lab are in [03](research/03-layout.md).
  Pure hashing passes R10 but clumps.
- **Degenerate principal axes** → Jacobi plus the degenerate-eigenspace rule, verified in a lab over
  20k sets ([04](research/04-camera.md)), corrected to keep the exact axis when it is defined
  (digest § 3).
- **Which keys are free?** → proven per key ([02](research/02-keys-history.md) § 2): Shift is
  never tested, which killed the GOAL's suggestion.
- **Testing a GL canvas without pixels** → `data-*` hooks derived from the draw lists, plus one
  canvas-screenshot liveness check ([06](research/06-harness.md)).
- **Measuring R11 truthfully** → headless is SwiftShader at 60 Hz, and its idle p95 is already
  16.8 ms, so the gate runs headed. The display here is 144 Hz, and the gate prints the measured
  refresh ([06](research/06-harness.md)).
- **D5 and D8** → ruled by the foreman (2026-11-08) and amended into GOAL.md as R3, R4 and R10, with
  dated Clarifications.
- **Labels that never overlap during motion** → frozen set, per-frame anchor re-pick, hide on
  conflict ([07](research/07-labels-picking-visibility.md)).
- **Adversarial design review** (Step 5, Trigger 3: R11 and R12 deliverables are test harness):
  see § 4.1.

### 4.1 Adversarial review

One fresh reviewer ran on the GOAL, this design and the drafted gates, with the prototype to poke
(2026-11-08). It reported 27 findings and wrote no file. What changed:

- **`0` changed meaning** (HIGH, R7): the draft made `0` re-fit the focus set. It is now always the
  galaxy fit, keeping the selection, as today (§ 2.3).
- **Gates that lie** (HIGH): a builder writes the named checks and the gate only grepped labels.
  - Every row of `verify/checks.txt` now carries its predicate, with tolerances, for review to grade.
    Labels match at column 0.
  - Hooks are written from the drawn buffers (§ 2.8), not from `anim.to` or the tier map.
  - Waits use the synchronous `data-move` counter, so no check reads the pose before a move starts.
  - Ease and reduced-motion checks read a timeline of hook writes:
    - eases need at least 3 intermediate values over at least 100 ms;
    - under reduced motion, every move writes exactly once, across every move kind.
  - Counts come from `/api/index`, never from the client.
  - Label checks need more than 24 candidates and at least 12 placed labels.
  - A portal check is added.
  - Angular order and the R3 axis are computed **by the test**, from `window.__oktPos` and
    `data-view`.
  - R10 gains an end-to-end probe: the test writes cards into the env.sh clone and waits for the
    live reload. The reload check now shuffles the index.
- **Frame-time gate** (HIGH, R11):
  - `--graph` reads `$OKT` only and refuses 6580.
  - The gate re-derives the verdict from per-scenario JSON:
    - target 6581, headed, an Apple GPU renderer;
    - refresh ≤ 8.5 ms, else exit 75 (environment);
    - at least 600 frames (900 animated frames for the warp);
    - draws ≥ 0.9 × frames;
    - p95 ≤ 16.7.
  - Any page error fails the run.
  - An `orbit_all` scenario covers show all.
  - The perf step runs in every phase from P2.
- **Static checks** (HIGH, MEDIUM):
  - The checks now grade the index and fail on untracked or unstaged source.
  - The base is the **oldest** plan commit.
  - The app.css rule is per phase and keyed on the paint files.
  - Pure modules ban hashed collections and crate imports.
  - The layout signature is pinned by a `const _: fn(&[LNode]) -> Space = layout;` line.
  - The radial code must be gone, not renamed.
  - The spec-id scan ignores `::CONST` paths.
  - Native tests are required **by exact name** (`verify/required.txt`), not by count.
- **Gate mechanics:**
  - Self-test step skipping needs an explicit `--self-test` flag and exits 3.
  - Unknown modes are rejected.
  - The leftover check diffs PIDs started by this run.
  - A busy port polls for up to 5 min, then exits 75.
  - The ledger's exemptions narrow to deviation-table rows and the ledger item, and its phrase list
    widens.
- **Design fixes:**
  - Galaxy draws region glyphs, not card clouds. Tiers apply at every scale.
  - Show all overrides collapse.
  - Scale after a solved move is the move's kind, so a wide Results fit isn't called galaxy.
  - Wheel input during a crossing ease no longer strands the target.
  - "Nearest the centre" prefers regions in front of the camera.
  - The degenerate-axis rule uses a ratio test.
  - The walk order comes from the destination pose and is frozen per walk.
  - Flights have a minimum rise.
  - History stores the destination pose and reads hash and pose together.
  - The walk target's label is placed first.
  - A selection eases whenever the graph is visible.
  - A stress check covers interleaved moves for § 1 borrow panics, including console "already
    borrowed" text.
  - New rows D10–D12; D1 records the `#ibody` scroll cost.
- **To the foreman:** D5 (the focus-set fit) and D8 (R10's precondition) were contract questions,
  not readings. The foreman ruled both (a), and they went into GOAL.md as amended R3, R4 and R10
  with dated Clarifications, committed on their own before this plan. The foreman also ruled D1's
  focus guard, which has a check row.

## 5. Risks & open questions

- **Q1 and Q2: ruled.** Both went to (a) and are now GOAL Clarifications (2026-11-08). R10's
  known exceptions are named there: an earlier-authored folder landing from another host, an old
  card renamed into a new folder, and a new uncommitted folder whose path sorts before an existing
  uncommitted sibling.
- **Frame time under the real app.** The benchmark ran the renderer alone. Mitigation: the
  `--graph` perf step runs in every phase from P2, so a regression shows in the phase that causes
  it. On a display slower than 120 Hz, the gate exits 75 (environment) rather than judging a p95
  that vsync jitter alone pushes to 16.8 ms.
- **Shared host.** Headed runs open a window on Geoffrey's display. Every gate closes its browser in
  `finally` and asserts no leftovers with `pgrep -fl 'target/fast/okt'` and
  `pgrep -fl ms-playwright`.
- **Port 6581 contention.** env.sh exits 2 when the port is busy. The gate polls for up to 5 min,
  then exits 75 with "port busy", never as a test failure.
- **Context loss** on sleep/wake: handled (§ 2.4) but hard to test. P2 tests it with
  `WEBGL_lose_context` in `keys.mjs`.
- **Seeds this cycle writes,** each `unshaped`, with a `ROADMAP.md` entry, in the phase named:
  - `issues/walk-mark-chip-cap.md`: the inspector caps chips at 40 per group, so a walk target past
    the cap gets no mark. Pre-existing. Written in P6.
    - *2026-11-08:* fixed in review cycle 1 (M2); the seed and its ROADMAP entry are retired.
  - `issues/graph-zoom-to-cursor.md`: zoom about the cursor (D3). Written in P2.
  - `issues/perf-profile-points.md`: `profile.mjs`'s legacy mode throws on an empty `points.json`
    and defaults to 6580. Pre-existing, out of scope. Written in P7.
  - Gather (a GOAL non-goal) already has its home in the GOAL; no seed is written here.

## 6. Verification strategy

Every phase's `verify:` is `sh spec/graph-galaxy/verify/gate.sh P<n>`, which runs the profile's
`verify`: `cargo test -p okt-server -p okt-pygments -p okt-client`, plus
`tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"'`. The gate adds three things around it:
named, predicate-carrying checks; tests required by exact name; and a headed frame-time verdict it
re-derives itself. Its header states the steps, the footprint and the exit codes.

1. **Static** (`verify/static.sh`), graded on the index against the oldest plan commit:
   - committed state (nothing untracked or unstaged under the source trees);
   - no spec ids in added source lines;
   - app.css changes with the paint files in each phase;
   - pure-module rules and the pinned layout signature;
   - per-phase file and text facts;
   - in P7, the ledger (`verify/ledger.sh`).
2. **Native tests:** the full profile run, then every name in `verify/required.txt` up to the
   phase, in one `cargo test -p okt-client -- --exact …`. They must exist, run without being ignored,
   and pass. TECH's phase bodies say what each asserts.
3. **`keys.mjs` in env.sh:** it must exit 0, and every label in `verify/checks.txt` up to the phase
   must open an `ok   ` line. Each row carries the predicate the check implements, which review
   grades. The checks read the hooks of § 2.8 plus `window.__oktPos`, compute expected counts,
   angles and axes from `/api/index` themselves, and keep the existing 33 checks green.
4. **Frame time, every phase from P2:** `tests/env.sh sh -c 'node "$OKT_ROOT/tests/perf/profile.mjs" --graph'`.
   - **Run:** headed, 1600×1000 at DPR 2, launched with `--disable-backgrounding-occluded-windows`.
   - **Scenarios:** `orbit` (≥ 600 frames), `orbit_all` (show all on), and from P7 `warp` (six round
     trips, at least 900 frames recorded only while `data-anim="1"`).
   - **Output:** `target=`, `mode=headed`, `renderer=` (WebGL unmasked), `refresh_ms=`, then one JSON
     line per scenario: `{"scenario":…,"frames":…,"draws":…,"p50":…,"p95":…,"max":…}`. After that,
     `PASS <scenario> p95=<x> <= 16.7` or `FAIL …` (ASCII).
   - **Exit:** non-zero on a FAIL or any page error.
   - **Gate:** re-derives everything from those lines, and exits 75 on a display slower than
     120 Hz.
5. **Cleanup:** no process the run started is left (PID diff); leftovers are killed and the gate
   goes red.

Modes:
- `--phase-only`: the phase's own rows (build's fail-once mutation);
- `--static`: the pre-push part;
- `--self-test STEPS`: plan dry runs only; exits 3.

A warm phase gate takes about 2–4 min (prototype dry run: step 2 34 s, step 3 91 s with a cold
client build).

---

*Backing research: [`research/00-digest.md`](research/00-digest.md).*
