# 00 — Research digest: graph-galaxy

Seven briefs, fanned out in parallel on 2026-11-08 (full fan-out: `appetite: big`, and Trigger 2,
since the design touches `okt-client/src/state.rs`). This file is the consolidated set of
decisions; where briefs disagree, the ruling here wins and says why.

| Brief | Topic | One-line outcome |
|---|---|---|
| [01](01-todays-graph.md) | Today's graph | Graph state lives in `graph.rs`'s own `GS` thread-local, not in `S`; public fn names survive, the radial layout and 2D camera retire, collapse and the walk re-home |
| [02](02-keys-history.md) | Keys and history | ←/→ and PageUp/PageDown are proved free in the graph; Shift+↑/↓ is **not** free (Shift is never tested, so it walks); history state object is unused |
| [03](03-layout.md) | Layout math | Age-ordered greedy slots on fixed lattices meet R10's locality proof; one new client field, `Node.ft` |
| [04](04-camera.md) | Camera math | Pure `cam` module: θ/ψ/ln d pose, Jacobi eigen, closed-form 70% fit, smootherstep eases, van Wijk–Nuij warp |
| [05](05-renderer-benchmark.md) | Renderer benchmark | Both pass headed on mac-m4 at 144 Hz (rAF p95 7.6–7.8 ms); **WebGL2 wins** by the ruling; no CSP change |
| [06](06-harness.md) | Measurement harness | `data-*` hooks on `#graph`; R11 must run headed (headless is SwiftShader at 60 Hz and fails idle) |
| [07](07-labels-picking-visibility.md) | Labels, picking, visibility | Tiers by BFS on focus change; greedy flat label placer, cap 24 incl. portals; CPU picking |

## Decisions

1. **Renderer: WebGL2 via `web-sys`, plus a 2D overlay canvas** for labels, region names, portal
   labels and the compass. Benchmark (brief 05, headed Chromium 158, ANGLE Metal on Apple M4 Max,
   144 Hz panel, 1200×900 CSS px at DPR 2, the env.sh clone: 1269 files, 149 folders, 3361 edges,
   23 regions):

   | renderer | scenario | rAF p50 / p95 / max (ms) | JS draw p50 / p95 / max (ms) |
   |---|---|---|---|
   | Canvas 2D | show all | 6.9 / 7.6 / 7.9 | 1.8 / 2.5 / 3.4 |
   | Canvas 2D | tiered focus | 6.9 / 7.8 / 7.9 | 0.8 / 1.1 / 1.5 |
   | WebGL2 + overlay | show all | 6.9 / 7.8 / 7.9 | 0.1 / 0.3 / 0.3 |
   | WebGL2 + overlay | tiered focus | 6.9 / 7.8 / 7.9 | 0.1 / 0.2 / 0.5 |
   | Canvas 2D | KB × 10 | 6.9 / 13.9 / 14.6 | 7.1 / 7.6 / 8.1 |
   | WebGL2 + overlay | KB × 10 | 6.9 / 7.5 / 7.8 | 0.1 / 0.2 / 0.3 |

   Both pass ≤ 16.7 ms p95, so the Clarification picks WebGL2. Instanced quads (not `gl.POINTS`),
   points CPU-sorted back to front with premultiplied "over" blending (brief 07 over brief 05's
   additive-only stand-in: additive glow vanishes on the paper theme), additive halos only when
   `--graph-bg` is dark. web-sys 0.3.106 already matches the wasm-bindgen 0.2.129 pin; add the
   WebGL2 features listed in brief 05. No CSP change (no directive governs WebGL; `blob:` workers
   stay unused). Handle `webglcontextlost`/`restored` from static buffers.

2. **Layout** (brief 03): hierarchical, pure, `std`-only module. Sibling folders take slots on a
   fixed lattice (256-point Fibonacci sphere for regions, 48-point cap lattice per parent) greedily
   in **birth order** (`birth = min ft` under the folder; ties by FNV-1a 64 of the path), so a
   newcomer never displaces an older sibling. Depth as radius from the region core; cards seeded
   by path hash and relaxed **only within their folder**; `index.md` pinned at the folder anchor.
   Root cards at the galaxy centre. The layout reads no edges, counts or sizes from outside a
   folder. Two additions of mine, both inside the brief's invariants:
   - **Surfaces** (`@sid` nodes, `parent: null`, `family: world`, only present when some card links
     them) form a pseudo-region `@` placed like any region with birth +∞; their positions are pure
     path hashes inside its cap, **unrelaxed**, so a new surface moves no other node.
   - **Collapse never moves anything**: it changes what is drawn (the folder's glyph stands for its
     cards), never positions.
   Limit: R10 then holds only when a new card's folders are younger, by git author time, than
   their siblings. The adversarial review broadened the failure cases (author dates from a second
   host, renamed old cards) and held that this amends R10, so it went to the foreman as Q2. The
   ruling (2026-11-08) amended R10 and hardened the order: uncommitted folders sort after every
   committed sibling, ties by path.

3. **Camera** (brief 04, one correction): pose `{target, θ, ψ, ln d}`, ψ clamped ±85°, 40° vertical
   FOV, pane aspect, shared `project()`. **Correction:** brief 04's always-on turning bias
   (κ = 0.02) tilts the solved axis off the true smallest principal axis (0.8° on a tilted slab),
   which R3's wording does not allow. The plan uses the **exact** smallest eigenvector whenever it is
   well defined, and applies the "nearest the current view" rule only inside a degenerate
   eigenspace (λ₀ and λ₁ within 10% of each other relative to tr C, n ≤ 2, or collinear), plus the
   hemisphere rule for the sign. Fit: brief 04's closed form at F = 0.70. Eases 450 ms smootherstep;
   warp 600 ms on the van Wijk–Nuij path, capped at the galaxy fit. Reduced motion read per move
   through `matchMedia`, cut to the end pose.

4. **Rim** (my synthesis of R2, R5 and the Outcome): at system and focus scale every other region is
   a glyph on a rim ring inside the pane edge, at the angle of its true direction (its centroid in
   view coordinates), solid when in front of the camera and hollow when behind. Bundles and portal
   edges run from the focused region's core or the focus card to those rim glyphs. Across the
   galaxy↔system band, glyphs interpolate between their true projection and the rim.

5. **Focus set for the R3 fit**: the selection plus its walk members **inside the selection's
   region**; members in other regions are drawn (Clarifications: focus-set cards are always drawn)
   and reached through their portal at the rim, which is inside the pane by construction. Fitting
   cross-region members in place would put the camera at galaxy distance for most cards and
   contradict the Outcome's portals. `z` from the Results view (R4) fits **all** matches in place.
   The adversarial review held that this redefines the GOAL's term "focus set", so it went to the foreman as Q1. The
   ruling (2026-11-08) amended R3 and R4 to this reading.

6. **Visibility** (brief 07; after the review, tiers apply at every scale and galaxy draws region
   glyphs, not card clouds): BFS ≤ 2 hops over link edges from the focus set, folders terminal;
   tier rule and edge rule as brief 07 states. A results focus spanning regions draws no tier 3.
   Region-pair link counts are precomputed per index load. Collapsed folders draw as their glyph.

7. **Labels** (brief 07): ≤ 24 card labels, region names exempt; portal labels count. Greedy flat
   placement with region names first, frozen set during motion, full re-solve 150 ms after rest,
   cached `measureText`. The placer is a pure function over rects, so `cargo test` can prove the cap
   and the no-overlap property.

8. **Size**: link degree, `r0·(1 + 0.35·ln(1+deg))`, ×1.4 for `index.md`, folders by file count;
   render-only (never fed to the layout).

9. **Keys** (brief 02): azimuth **ArrowLeft/ArrowRight**, elevation **PageUp/PageDown** (fn+↑/↓
   on a Mac keyboard), both proved free while the graph view is active. The GOAL's
   Shift+ArrowUp/Down suggestion is rejected because it already walks (row D1). `-`, `+`/`=`, `0`
   act on camera distance (`0` = galaxy fit). The walk order is owned by the graph: the
   `insp::walk_list` membership, sorted by on-screen angle around the focus, re-sorted on each step
   and re-found by id; `insp::mark_chip` marks by id, re-applied after `insp::render()`.

10. **History**: every focus change already pushes; before each push the current entry's
    `history.state` gets the current camera pose (`replace_state`), and `popstate` eases to the
    popped entry's stored pose, falling back to the R3 solve. The hash format is unchanged.

11. **Settings**: the "ALL LABELS" toggle (`Settings.labels`, `.gtools [data-z=labels]`) contradicts
    R9's hard cap and becomes the **"show all"** toggle, persisted as `Settings.show_all` under the
    key `showAll`. This is the design's only `state.rs` edit (row D4). Camera, scale and walk state
    stay in `graph.rs`'s thread-locals, never in `S`.

12. **Harness** (brief 06): `data-*` attributes on `#graph`, written at frame end on change, derived
    from the same lists that feed GL and the overlay. R11 extends `tests/perf/profile.mjs` with a
    `--graph` mode (headed by default; headless prints numbers with no verdict) rather than a new
    file, so the GOAL's "profile.mjs (or an extension of it)" reads literally. R12 extends
    `tests/ui/keys.mjs`. A canvas screenshot check guards against a blank GL canvas behind green
    attributes.

## Contradictions resolved

- **Additive vs over blending** (05 vs 07): over, with additive halos on dark themes only.
- **Turning bias** (04 vs R3's wording): exact axis, with the bias only for degenerate sets.
- **New `tests/perf/graph.mjs`** (06) vs **extend `profile.mjs`** (GOAL wording): extend it with a
  mode; `points.json` stays out of scope because the mode never reads it.
- **`npm install` vs `PLAYWRIGHT_MODULE`** (06): both work; TECH's setup names `npm install` in
  `tests/` (the profile's own `setup`), with `PLAYWRIGHT_MODULE` as the fallback.
