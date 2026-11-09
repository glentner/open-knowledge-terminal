---
slug: graph-galaxy
title: The graph as a navigable 3D space (galaxy, system, focus)
kind: feature
appetite: big
status: in_progress
branch: feat/graph-galaxy
base: main
current_phase: P7
last_updated: '2026-11-08'
phases:
- id: P1
  name: Deterministic 3D layout
  status: done
  satisfies:
  - R10
  depends_on: []
  parallel: false
  hammerable: false
  hill: uphill
  attempts: 1
  verify: sh spec/graph-galaxy/verify/gate.sh P1
- id: P2
  name: WebGL2 scene and the orbit camera
  status: done
  satisfies:
  - R7
  - R11
  depends_on:
  - P1
  parallel: false
  hammerable: false
  hill: uphill
  verify: sh spec/graph-galaxy/verify/gate.sh P2
- id: P3
  name: Galaxy and system scales, show all
  status: done
  satisfies:
  - R1
  - R2
  depends_on:
  - P2
  parallel: false
  hammerable: false
  hill: uphill
  verify: sh spec/graph-galaxy/verify/gate.sh P3
- id: P4
  name: Focus scale and visibility tiers
  status: done
  satisfies:
  - R3
  - R8
  depends_on:
  - P3
  parallel: false
  hammerable: false
  hill: uphill
  attempts: 1
  verify: sh spec/graph-galaxy/verify/gate.sh P4
- id: P5
  name: Flat label overlay and compass
  status: done
  satisfies:
  - R9
  depends_on:
  - P4
  parallel: false
  hammerable: false
  hill: uphill
  verify: sh spec/graph-galaxy/verify/gate.sh P5
- id: P6
  name: z framing and the angular walk with portals
  status: done
  satisfies:
  - R4
  - R5
  - R7
  depends_on:
  - P5
  parallel: false
  hammerable: false
  hill: uphill
  verify: sh spec/graph-galaxy/verify/gate.sh P6
- id: P7
  name: Warp flight, history, frame-time gate and ledger
  status: pending
  satisfies:
  - R6
  - R11
  - R12
  depends_on:
  - P6
  parallel: false
  hammerable: false
  hill: uphill
  verify: sh spec/graph-galaxy/verify/gate.sh P7
review:
  last_reviewed_commit: ''
  verdict: none
  blocked_reason: ''
  cycle: 0
---

# TECH.md — The graph as a navigable 3D space (galaxy, system, focus)

The **context engine and finite-state machine** for building this feature. The YAML
frontmatter above is the resume ground-truth (read it from the cycle worktree with
`uv run --with pyyaml python "$KB/bin/factory/next_phase.py" spec/graph-galaxy/TECH.md`); the
per-phase checklists below are the work. `code-build` executes the next actionable phase, runs its
`verify:` command, updates state via
`uv run --with pyyaml python "$KB/bin/factory/set_phase.py" …`, and makes one atomic code+state commit.

- **Vision / requirements (locked):** [`GOAL.md`](GOAL.md) — R-IDs are the contract.
- **Authoritative design:** [`PLAN.md`](PLAN.md).
- **Backing research:** [`research/00-digest.md`](research/00-digest.md) + briefs.

## Frontmatter field reference

- `status` (top): `planned | in_progress | blocked | in_review | done` (`done` is stamped by
  `code-publish` after confirmation, just before landing — the terminal state of the retained record)
- `appetite`: `small | big` — caps phase count and build-iteration budget (circuit breaker).
- `branch`, `base`: the cycle's branch (prefix per GOAL `kind`, from the profile's `branches`) and the
  profile's `base`.
- phase `status`: `pending | in_progress | done | blocked`
- `satisfies`: GOAL R-IDs this phase delivers (traceability anchor for `code-review`).
  **Artifact-deliverable R-IDs.** Some requirements are met by a *committed document* — a research
  audit, a protocol doc, an assessment — not by CLI-observable behaviour. Attach such an R-ID to the
  phase that produces and commits the artifact, and give that phase a content `verify:`
  (`test -f …`, a `grep -q` for the sections that must exist) rather than forcing a CLI drive.
  **Say so in the phase body**, because it changes who can grade it: `code-review` blinds its reviewer
  to all of `spec/`, so an R-ID whose evidence lives there is graded by the *orchestrator*, not the
  blind reviewer. Naming it in `TECH.md` is what lets the review skill route it correctly instead of
  reporting an unverifiable requirement.
- `depends_on`: phase ids that must be `done` first (a phase is actionable only when met).
- `parallel`: `true` only for genuinely independent, non-coupled work (docs, tests, isolated leaf
  modules). Any phase touching the profile's `coupled core` is always `false`.
- `hammerable`: `false` marks a correctness/security phase that scope-hammering must **never** cut.
- `hill`: `uphill` (still figuring it out) → `crest` (unknowns resolved) → `downhill` (just
  executing). A phase stuck `uphill` across builds is a raised hand → escalate to the human.
- `attempts`: durable failed-verify counter (absent = 0), bumped by `set_phase.py --phase P<n>
  --record-attempt` on every red verify gate; `next_phase.py` warns at ≥3 — the circuit breaker
  runs on this file, not on session memory.
- `verify`: the exact command that proves the phase (prefer driving the real CLI, not just tests —
  wrap CLI drives in the profile's `throwaway environment` helper, `<helper> sh -c "…"`, so they hit
  a disposable instance, never the developer's real data). A gate carrying `$( )` or quoting the
  calling shell would mangle is set with `set_phase.py --verify-file PATH`, not `--verify`.
- `review.cycle`: completed review passes, auto-incremented by every `set_phase.py --verdict …`;
  REVIEW.md's "Cycle {n}" mirrors it and the ≤3-cycle bound is graded against it. `--verdict approved`
  also clears `review.blocked_reason` unless one is passed explicitly.

## Conventions (apply to every phase)

- Commit conventions, code style, and load-bearing invariants come from the repo's `AGENTS.md` — it
  is the constitution. Walk the sections of its § Invariants that this change touches; that is the
  curated footgun checklist.
- One phase per `code-build` invocation by default; one atomic commit containing **both** the code and
  the `TECH.md` state change. The branch commit subject is code-build's (`$KB/skill/code-build.md` →
  Step 6), not restated here; branch commits are squashed away at `code-publish` per the profile's
  `landing`. Trailers follow the profile's `branch commits`.
- The last phase carries the standing last-phase items: the deferral ledger (`code-plan` Step 6.8),
  and every item the previous cycle's last phase carried for a project-level obligation (mpsearch's
  `## Times` quote, taken as `code-build` Step 4 says). A plan that drops one says why in `PLAN.md`.
  This repo's previous cycle carried no project-level last-phase item, so only the ledger applies.
- A PR body (when the profile's `landing` opens one) ends with an attribution naming the actual
  harness, model, and variant.
- A CLI/feature change obeys the profile's `same-commit rules` (§ Conventions in `AGENTS.md`): the
  help text, docs, or generated files it names change **in the same commit**.

### This cycle's gate (every phase)

- Every `verify:` is `sh spec/graph-galaxy/verify/gate.sh P<n>`, run from the worktree root **after
  staging** (`git add -A`): the static checks read the index and fail on untracked or unstaged source.
  The gate's header states its steps, footprint and exit codes (0 green, 1 red, 75 environment: port
  6581 busy for 5 min or a display slower than 120 Hz). Build's fail-once mutation uses
  `--phase-only`; the pre-push check uses `--static`.
- It runs with the Bash sandbox **off** (env.sh binds 6581, never 6580) and needs, exported by the
  caller: `WASM_BINDGEN` (the Cargo.lock pin, 0.2.129), `PLAYWRIGHT_BROWSERS_PATH`, and either
  `npm install` in `tests/` (the profile's `setup`) or `PLAYWRIGHT_MODULE` pointing at a Playwright
  `index.mjs`. Steps 3–4 open Chromium on the host's display; step 4 is headed, from P2 on.
- **Named checks:** `verify/checks.txt` rows are `phase|label|predicate`. Write each check in
  `tests/ui/keys.mjs` with exactly that label as its printed prefix, implementing exactly that
  predicate; its header defines "waits", "timeline", "positions" and "projects". The predicates are
  the contract code-review grades; the gate only sees the labels.
- **Required tests:** `verify/required.txt` names every native test a phase adds. They run by exact
  name and must exist, not be ignored, and pass. Each phase body says what each one asserts.
- **Hooks:** PLAN § 2.8. They are written inside `gl.rs` and `overlay.rs` from what was drawn, never
  from `anim.to`, the tier map or a parallel computation. `cam_to` bumps `data-move` synchronously.
- **Frame time:** PLAN § 6 step 4 fixes `profile.mjs --graph`'s output lines; the gate re-derives
  the verdict from them.
- **Measured** on the prototype dry run (2026-11-08, mac-m4): step 2 took 34 s with warm
  dependencies; step 3 took 91 s, including a cold client build and the 33 existing checks. A warm
  phase gate is about 2–4 min.
- `AGENTS.md` § 1 applies everywhere: no borrow of `S`, settings or `GS`/`WALK` across a call that can
  re-enter (focus, blur, dispatch, `nav::select`, `insp::render`, `mark_chip`) or an `await`. `g()`
  is always `borrow_mut`, so a nested call panics. keys.mjs fails on a page error or a console message
  holding "panicked at" or "already borrowed" (add the console listener in P1).

---

## Phase P1 — Deterministic 3D layout
**Satisfies:** R10 · **Depends on:** —
**Goal:** the layout of PLAN § 2.2 exists as a pure, natively tested module, computed on every build.
Its hash and positions are visible in the running app, stable across a shuffled reload, and adding
cards to the throwaway KB moves nothing outside their folders. Nothing is drawn from it yet; the 2D
graph keeps painting.

- [x] `okt-client/src/model.rs`: `Node` gains `#[serde(default)] pub ft: Option<f64>`.
- [x] `okt-client/src/graph/space.rs`, with `graph.rs` declaring `mod space;`. It uses `std` only and
      no hashed collections. It holds `LNode`, `Space`, `pub fn layout(nodes: &[LNode]) -> Space`,
      and the pinning line `const _: fn(&[LNode]) -> Space = layout;`.
  - Contents: the hash, birth order, region and cap lattices, depth radii, card seeding,
    folder-local relaxation, root cards at the centre, and surfaces as the pseudo-region `@` with
    unrelaxed hash positions.
  - Sibling order is `(birth, path)`. A folder with no committed file has birth +∞, so it sorts
    after every committed sibling, with ties by path (GOAL Clarification on R10).
  - A comment says why `layout` takes no edges or counts, and names R10's known exceptions from the
    Clarification.
  - *Amended in build (2026-11-08):* the `@` slot is reserved on the region lattice before any
    region, and surfaces sit within a fixed radius, so surfaces and regions never displace each
    other (PLAN § 2.2, Regions and Surfaces).
- [x] Tests in `space.rs` (names in `verify/required.txt`). Each R10 test asserts bitwise equality
      for **every** pre-existing node outside the new card's folders: cards, folder nodes and
      surfaces. The fixture gives every committed folder an `ft`.
  - `deterministic`: run twice, compare bitwise.
  - `order_independent`: seeded shuffles of nodes.
  - `r10_existing_folder`.
  - `r10_new_subfolder`.
  - `r10_new_chain`: two new folders deep.
  - `r10_new_region_first` and `r10_new_region_last`: names sorting before and after every region.
  - `r10_root`.
  - `r10_new_surface`: a new surface node moves nothing else.
  - `uncommitted_after_committed`: a folder without `ft` takes its slot after every committed
    sibling, whatever its name, and two uncommitted siblings order by path.
  - `r10_property`: 200 random single additions, mixing existing folders, new subfolders, new
    regions and root.
  - `spread`: region min angle ≥ 25° on a 21-region fixture, within-folder min distance ≥ 0.95 × 10,
    no NaN.
- [x] `graph.rs` `build()`:
  - Build `LNode`s from the `Idx`, call `space::layout`, and keep the `Space` in `G` (an `Rc`).
  - Set `#graph[data-layout]` (FNV-1a over positions quantised to 1e-6) and `#graph[data-total]`.
  - Fill `window.__oktPos` when a test installed it.
  - The 2D paint is unchanged.
- [x] `tests/ui/keys.mjs`: the P1 rows of `verify/checks.txt`, plus a console listener that fails the
      run on "panicked at" or "already borrowed". The probe writes go to `$OKT_KB`, the env.sh clone,
      never `~/KB`.
- **Verify:** `sh spec/graph-galaxy/verify/gate.sh P1`.
- **Touches:** `okt-client/src/model.rs`, `okt-client/src/graph.rs`, `okt-client/src/graph/space.rs`,
  `tests/ui/keys.mjs`. No `app.css` change: P1 changes no paint.

## Phase P2 — WebGL2 scene and the orbit camera
**Satisfies:** R7, R11 (orbit) · **Depends on:** P1
**Goal:** the radial 2D canvas is gone. Every card and edge of the layout is drawn in WebGL2 with
depth cues, under a spherical camera.
- It orbits by drag and keys, and zooms by wheel, pinch and `+`/`=`/`−`/`0`.
- It has hooks, picking, reduced-motion cuts and context-loss recovery.
- The headed `orbit` and `orbit_all` scenarios hold p95 ≤ 16.7 ms.

- [x] `okt-client/Cargo.toml`: add the web-sys features `WebGl2RenderingContext`, `WebGlProgram`,
      `WebGlShader`, `WebGlBuffer`, `WebGlUniformLocation`, `WebGlVertexArrayObject`,
      `WebGlContextAttributes`, `WebGlContextEvent`, `WebglLoseContext`.
- [x] `graph/cam.rs` (pure): the pose, basis, `project`, `view_proj`, Jacobi, easing, wheel
      normalisation, and `cam_to`'s pose math. This phase's tests:
  - `basis_orthonormal`: orthonormal, right-handed, up.y > 0 over a θ/ψ grid.
  - `project_matches_matrices`: `project` equals `P·V` within 1e-9, and the target maps to the pane
    centre.
  - `jacobi_residual_and_determinism`: residual ≤ 1e-12·λmax, orthonormal, bitwise repeatable, on
    random, diagonal, repeated-root and zero matrices.
  - `wrap_pi_short_arc`.
  - `wheel_delta_modes`: line, page, ctrl factors and the clamp.
  - `ease_endpoints_and_monotone`: smootherstep endpoints exact and monotone; `ln d` linear.
- [x] `graph/gl.rs`:
  - programs, VAO, instanced quads for points (sorted back to front per frame), line quads for edges,
    premultiplied blending, uniforms per frame;
  - instance re-upload only on a tier change;
  - `webglcontextlost` and `webglcontextrestored`;
  - writes the drawn-state hooks.
- [x] `graph/overlay.rs`: a pointer-transparent second canvas over `#graph`, sized to the pane at DPR.
      This phase draws only the hover state.
- [x] `graph.rs`:
  - **Retire:** the radial layout (`fn layout(idx: &Idx)`, `polar`, `R_TOP`, `dr_dir`,
    `label_geom`), the 2D scene context, the ring guides and the spotlight layers.
  - **Camera:** `G` gains `pose`, `CamAnim` and `autofit`. Every move goes through one
    `cam_to(to, kind)`, which bumps `data-move` and reads `reduced_motion()` at the start (new, the
    `chrome.rs:60` `match_media` pattern). `frame_tick` evaluates the pose under one short borrow
    and drops it before writing hooks.
  - **Input:**
    - pointer drag orbits, keeping the 4 px dead zone;
    - wheel and pinch zoom about the target (row D3);
    - `0`, the fit button and a double-click on empty space fit the galaxy and keep the selection;
    - a double-click on a folder toggles collapse;
    - CPU picking over the last frame's projected points: hover shows `#gtip`, a click selects.
  - **API:** `fit`, `zoom_by`, `focus_node` and `frame_selection` keep their names. For now
    `frame_selection` fits the selection and its walk members; P4 and P6 make it the real solve.
  - **Hooks:** those of PLAN § 2.8 that exist so far. `data-scale` reads `focus` or `galaxy` until P3.
    `window.__oktDraw` is kept.
  - **Colours:** `Colors::read` is canonicalised through the overlay `fillStyle`. An accent change
    rebuilds the colours.
- [x] `keys.rs` graph block (before the global match, after the walk arms):
  - ArrowLeft/ArrowRight turn the azimuth ±15° and PageUp/PageDown tilt ±10°, each eased over 140 ms,
    only while the graph view is active and `document.activeElement` is neither inside `#insp` nor an
    input. Otherwise the key falls through, so a focused inspector scrolls natively (row D1).
  - ArrowUp/ArrowDown keep walking.
  - `+`/`=` and `-` act on `ln d`; `0` is the galaxy fit.
- [x] `app.css` graph block:
  - retire `.spotlit`, `.ebase`, `.ehot`, `.ring-guide` and `.n .lab`;
  - add the overlay canvas rule and the colour tokens the GL reads;
  - keep `.gtools`, `.glegend` and `.gtip`.
- [x] Docs:
  - `README.md` § Keys, and the `chrome.rs` `?` list and GRAPH `<dl>`: ←/→ orbit, PageUp/PageDown
    tilt, `+`/`=`/`−`/`0` on camera distance, drag orbits, wheel and pinch zoom.
  - `README.md:9-11` loses "radial" and "panning".
  - `AGENTS.md` Map and profile: the coupled core names `okt-client/src/graph/` (row D9).
- [x] `tests/ui/keys.mjs`: the P2 rows of `verify/checks.txt`.
- [x] `tests/perf/profile.mjs`: add a `--graph` mode, parsed before `points.json` is read; the legacy
      mode is untouched.
  - **Target:** `process.env.OKT` only, never argv. It exits 2 when `OKT` is unset or names 6580.
  - **Browser:** headed by default (`--headless` prints numbers with no verdict), with
    `--disable-backgrounding-occluded-windows`, 1600×1000 at DPR 2.
  - **Run:** prints the lines PLAN § 6 step 4 fixes. It calibrates the refresh for 1 s, warms up
    with one untimed orbit, then runs:
    - `orbit`: ≥ 5 s and ≥ 600 frames of ArrowRight presses every 16 ms, then PageUp, then a
      circular drag;
    - `orbit_all`: the same with show all on. Until P3 adds the toggle, every card is drawn
      already.
  - **Recording:** rAF intervals and `__oktDraw` counts, in the page.
  - **Exit:** 1 on any FAIL or any page error. The browser closes in `finally`.
- [x] Seed `issues/graph-zoom-to-cursor.md` (`unshaped`, from the ISSUE template) and its
      `ROADMAP.md` entry: zoom about the pointer, the alternative row D3 deferred.
- *Amended in build (2026-11-08):*
  - `Cargo.toml` also takes `WebGlTexture`. Each node's position, radius, colour and style live in
    two RGBA32F textures that the instanced quads read with `texelFetch`, so a frame uploads only
    its uniforms and the sorted point order (one `u32` per point). Edges are instanced line quads
    that read both ends from the same texture.
  - `window.__oktPos` is filled from the uploaded (f32) positions on every upload, and the upload
    runs at build and on a selection change, not at the next frame, so the hook is current while
    the graph is hidden. `publish_space` keeps only `data-layout` and `data-total`.
  - `focus_node(id, d)` keeps its name; its second argument is now a camera distance.
    `zoom_by(f)` drops the 2D anchor argument (the `keys.rs` callers change with it).
  - Drag and wheel are moves too: they go through `cam_to` as cuts, so they bump `data-move`.
  - `data-scale` reads `focus` with a selection and `galaxy` without. `data-drawn` counts the
    cards in front of the near plane, so close to a card, cards behind the camera are not counted.
  - Collapse draws nothing different until P4 (row D12): every card is drawn. `rep_of` and the
    collapse settings keep their meaning for E, space and EXPAND/COLLAPSE, and the legend's
    collapsed-folder note gives way to the orbit and zoom hints.
  - The "ALL LABELS" toggle still flips its setting and button, and draws nothing until P3
    replaces it with show all.
  - `theme.rs` loses `hex_alpha`, dead since the spotlight retired (outside the Touches list).
  - `profile.mjs --graph` drains `window.__oktDraw` every frame, because the client keeps only
    its last 600 entries and a scenario runs longer. Its drag is a pointer move at a time, so the
    graph draws on nearly every frame.
  - In `keys.mjs`, a screenshot's "colours" are 4-bit-per-channel buckets, and the screenshot also
    hides `.gtools`, `#glegend`, `#ghint` and `#gtip`, so only the GL canvas counts.
- **Verify:** `sh spec/graph-galaxy/verify/gate.sh P2`.
- **Touches:** `okt-client/Cargo.toml`, `okt-client/src/graph.rs`,
  `okt-client/src/graph/{cam,gl,overlay}.rs`, `okt-client/src/keys.rs`, `okt-client/src/chrome.rs`,
  `okt-client/src/actions.rs` (accent rebuild), `okt-client/static/{app.css,index.html}`,
  `README.md`, `AGENTS.md`, `tests/ui/keys.mjs`, `tests/perf/profile.mjs`,
  `issues/graph-zoom-to-cursor.md`, `ROADMAP.md`.

## Phase P3 — Galaxy and system scales, show all
**Satisfies:** R1, R2 · **Depends on:** P2
**Goal:**
- With nothing selected, the graph is a galaxy of region glyphs joined by weighted bundles, with
  region names only.
- Selecting a folder brings its region to system scale, with the other regions on the rim.
- Zoom crosses scales without touching the selection.
- "Show all" replaces "all labels" and draws every card and edge.

- [x] `graph/vis.rs` (pure): `ix`, `adj`, `region` per node (root `.` and surfaces `@` as
      pseudo-regions, row D11), region-pair link counts summing `n`, bundle weights. Tests:
  - `region_of_nodes`: root, surface and nested cards.
  - `pair_counts`: sums of `n` keyed `(min, max)`, with root and surfaces excluded.
  - `bundle_weight`: rises with the count.
- [x] `graph/cam.rs`: `classify` and `nearest_to_centre`. Tests:
  - `classify_hysteresis`: no flip-flop at ±10% around a band; respects the innermost cap.
  - `nearest_region_front_first`: a region behind the camera loses to any in front; ties go to
    count, then name.
- [x] `graph.rs`:
  - **Scale:** the scale state of PLAN § 2.3 (from the move kind after a solve, from `classify` on
    zoom input) and the active region. Zoom-in at galaxy picks the in-front region nearest the
    centre; zoom-out past the band eases the target to the origin, and wheel input adjusts that ease
    rather than cancelling it. Crossings never call `nav::select`.
  - **Galaxy:** region glyphs only (halo by count) and Bézier bundles tessellated once per build.
    The root core and `@` glyphs are unlabelled.
  - **System:** the region fitted, subfolder glyphs at their anchors, other regions as rim glyphs
    (solid in front, hollow behind) with their bundles. Glyph positions interpolate across the band.
    A folder selection fits its region.
  - **Tiers at every scale** (PLAN § 2.4). Until P4 the tier rule is: the selected folder's region
    is drawn, every other card is hidden.
  - **Hooks:** `data-scale`, `data-region`, `data-regions`, `data-rim`, `data-bundles`, `data-edges`,
    `data-dist-galaxy`.
- [x] `graph/overlay.rs`: region names at glyphs and rim glyphs; no card labels at galaxy scale.
- [x] Show all (rows D2, D4):
  - `state.rs`: `Settings.labels` becomes `Settings.show_all`, under the key `showAll`, in load and
    save. An old `labels` key falls into `extra`.
  - `graph::toggle_labels` becomes `toggle_show_all`. The `.gtools` button becomes `data-z="all"`,
    and the `chrome.rs` DISPLAY row says SHOW ALL.
  - Show all draws every card and every link edge at true positions, overriding collapse (row D12).
- [x] `app.css`: glyph, bundle and rim tokens; the region-name overlay rule.
- [x] `tests/ui/keys.mjs`: the P3 rows.
- *Amended in build (2026-11-08):*
  - Glyphs are drawn by the GL, not the overlay: one screen-space "mark" program (a capsule per
    instance, a disc when its two ends meet) draws region halos, rim discs and rings, the root and
    `@` glyphs, subfolder halos, and the lines out to the rim, a few dozen uploaded per frame. The
    overlay draws only the region names. Halos are an even glow with a soft edge (flat to 70% of
    the radius), so a region reads as one cloud even where its own cards cover the middle.
  - Bundles are tessellated into extra texels after the nodes (12 segments each) and drawn as edges
    of their own group; one uniform fades them as the rim weight rises, so they hand over to the
    rim lines across the band. The rim weight eases with the camera move that changes the scale.
  - The rim runs `--graph-rim-inset` inside the pane, and further in where the toolbar and the
    legend are (their rects are read on resize and build), so no rim glyph sits under them.
    `cam::rim_point` is pure and has its own test, `rim_points_hug_the_edge`.
  - `fit()`, a folder selection and a zoom crossing are `Move::Cross` eases: each moves the target
    across the system/galaxy band, so a zoom during one moves both of its ends instead of
    stranding the target. Zooming in at galaxy eases onto the region at the lesser of the asked
    distance and the region's fit.
  - `update()` follows a selection change: nothing selected (or the root) eases to the galaxy fit,
    a folder fits its region. So a graph click on a region glyph or a folder reaches system scale
    while `nav.rs` is untouched; `nav`'s `focus_node` for a folder repeats the same fit. A card's
    own move stays `focus_node`'s until P4.
  - Region glyphs are pickable (a click selects the region's folder); the root glyph selects the
    root. Until P4 the link edges drawn without show all are the selection's own whose other end is
    drawn. Show all draws no bundles: every link edge is drawn instead.
  - `Cargo.toml` takes `TextMetrics` (the overlay measures the names it places). The `?` GRAPH list
    gains the scales and the show-all button.
- **Verify:** `sh spec/graph-galaxy/verify/gate.sh P3`.
- **Touches:** `okt-client/src/graph.rs`, `okt-client/src/graph/{vis,cam,overlay,gl}.rs`,
  `okt-client/src/state.rs`, `okt-client/src/chrome.rs`, `okt-client/static/{app.css,index.html}`,
  `okt-client/Cargo.toml`, `tests/ui/keys.mjs`.

## Phase P4 — Focus scale and visibility tiers
**Satisfies:** R3, R8 · **Depends on:** P3
**Goal:**
- Selecting a card eases the camera to focus scale, along the focus set's exact smallest principal
  axis, fitted to 70% of the pane.
- Only the tiers are drawn: the focus haloed, depth cues on, portals reaching the rim.
- The compass shows where the camera is.

- [x] `graph/vis.rs`: BFS tiers (folders terminal; the rule of PLAN § 2.5, including no tier 3 for a
      multi-region focus, row D6), the edge selection, importance, and collapse folding tier 3 only
      (row D12). Tests:
  - `hidden_two_hop_other_region`.
  - `focus_set_any_region`.
  - `folders_terminal`.
  - `no_tier3_multi_region`.
  - `edge_rule`: no tier-2/2 or tier-3 edges, and portals to the rim.
  - `show_all_hides_nothing`.
- [x] `graph/cam.rs`: `solve_view` and `fit_distance`. Tests:
  - `solve_view_exact_axis`: an anisotropic set returns e₀ within 1e-9, sign toward the view.
  - `solve_view_degenerate`: isotropic keeps the view; two points; collinear; coplanar normal;
    coincident; and λ = (1, 1.5, 7.5) is **not** degenerate.
  - `angles_round_trip`: includes the pole clamp.
  - `fit_fills_seventy_percent`: max |ndc| = 0.70 ± 1e-9 at aspects 0.5, 1 and 2.5; a near point
    raises d; a single point gives `d_min`.
- [x] `graph.rs`:
  - **Card selection:** `nav::select` triggers the R3 solve via `cam_to(Ease)`. The fit input is the
    focus-set members in the selection's region; members in other regions are drawn as rim portals
    (amended R3). It eases while the graph is visible and cuts while it is hidden; the
    `from != "graph"` guard at `nav.rs:56` goes.
  - **Folder selection** keeps P3's region fit.
  - **Portals:** an off-pane focus-set card's edge is drawn to its region's rim glyph.
  - **Look:** a static focus halo (additive on dark backgrounds only); depth cues (saturation,
    alpha, an aerial fade toward `--graph-bg`); size by degree.
  - **Hooks:** `data-focus`, `data-focus-n`, `data-focus-ids`, `data-tiers`, `data-drawn-regions`.
- [x] `graph/overlay.rs`: the compass, bottom-right (axes, horizon ring, focused-region dot, scale
      word).
- [x] `app.css`: `--graph-halo`, `--graph-fog`, `--graph-t2-alpha`, `--graph-t3-alpha`, compass rules.
- [x] `tests/ui/keys.mjs`: the P4 rows.
- *Amended in build (2026-11-08):*
  - **Selection moves:** `update()` runs the card solve for a selection from any source (a graph
    click, `nav::select`, `apply_hash`), as it already ran the folder and empty fits, so both
    `focus_node(i, None)` calls in `nav.rs` go: `select`'s guarded one and `apply_hash`'s
    duplicate. `focus_node(id, None)` for a card runs the same solve (the files view's space);
    `focus_node(id, Some(d))` keeps the target's angles and is the walk's re-centre.
  - **The fit:** a card's fit set is the card and its links in its own region (`vis.adj`, the
    walk list's members), padded by a card's radius rather than a region's 12 units, so a tight
    set still fills 0.6–0.8 of the pane. A point is drawn no larger than 22 CSS px
    (`POINT_MAX_PX`, a uniform the GL and the picking share), so a small fitted set shows its
    arrangement, not a few giant discs. A zoom-in crossing from system to focus re-solves with
    the asked distance as its cap.
  - **Tiers:** a focused folder's children seed tier 1 (its walk list's "Subfolder" and "In this
    folder" members); the root seeds none. With nothing selected, the active region's folders are
    still drawn as points at system scale (P3's look); with a selection the tiers alone decide.
    `vis::importance` holds the size rule `vis_of` uses.
  - **Portals** are decided with the tiers: a hot edge whose link is in a top-level folder out of
    focus. Each frame draws it as a screen-space line from the focus to the card when the card
    projects inside the pane, otherwise to its region's glyph where it was drawn.
  - **Hooks:** a node's tier rides in its colour texel with its style (`style + 8·tier`, 4 when not
    drawn), so `data-tiers` and `data-focus-n` (nodes uploaded at tier 0 or 1) count from the
    uploaded buffer. `data-drawn` now counts the cards in the draw list, the whole tier set,
    behind-the-camera points included (they go first in the order and the GL drops them), which
    replaces P2's "in front of the near plane": at focus scale another region's links can sit
    behind the camera, and R8's count must hold wherever it is. `data-edges` adds the portals
    drawn. `data-focus-ids` is the last card solve's fit set. `data-axis-err` and `data-fit`
    (PLAN § 2.8) are not written; the axis check computes from `data-view`.
  - **Compass:** it sits above the legend when the legend reaches under it, and region names keep
    clear of its box. Its colours are `--graph-compass` and `--graph-compass-far`.
  - **The walk target's thicker edge** (PLAN § 2.5) moves to P6, with the walk.
  - `keys.mjs`: `KEYS_VERBOSE=1` prints passing checks' detail. The env.sh KB's hub has no
    well-defined axis (λ₁/λ₀ = 1.095), so the axis check also solves for the most connected card
    whose set has λ₁/λ₀ ≥ 1.25, and the axis itself is always graded.
- **Verify:** `sh spec/graph-galaxy/verify/gate.sh P4`.
- **Touches:** `okt-client/src/graph.rs`, `okt-client/src/graph/{vis,gl,overlay,cam}.rs`,
  `okt-client/src/nav.rs`, `okt-client/static/app.css`, `tests/ui/keys.mjs`.

## Phase P5 — Flat label overlay and compass
**Satisfies:** R9 · **Depends on:** P4
**Goal:** at most 24 card labels, filled by tier then importance, flat on the overlay. They never
overlap each other, region names or the pane chrome, at rest or in motion. The walk target's label
is placed first.

- [x] `graph/labels.rs` (pure): `const MAX_CARD_LABELS: usize = 24;` and
      `place(cands, obstacles, pane, last_anchor) -> Vec<Placed>`. Tests:
  - `cap_24`: at most 24 non-region labels over random scenes of up to 2000 candidates.
  - `no_overlap`: all placed rects pairwise disjoint, region names included.
  - `fill_order`: tier, walk target, importance, path; a colliding region name is dropped.
  - `walk_target_before_regions`: the walk target's label survives a colliding region name.
- [x] `graph/overlay.rs`:
  - draws exactly the `Placed` list, measured with the metrics and DPR it draws with, each label's
    text clipped to its rect;
  - writes `data-labels` (`[x,y,w,h,kind,text]`) and `data-label-cands` from that list;
  - keeps a `measureText` cache;
  - freezes the label set during motion and re-solves 150 ms after rest;
  - card text drops `.md`; portal text is middle-ellipsised to 34 characters (row D7: portals count).
- [x] `app.css`: overlay label rules (size, weight, knockout in `--graph-bg`).
- [x] `tests/ui/keys.mjs`: the P5 rows.
- *Amended in build (2026-11-08):*
  - **Compass:** P4 already drew it (bottom right, above the legend when the legend reaches under
    it). P5 draws no second one: `overlay::compass_box` gives its box, and every label, region
    names included, keeps clear of it.
  - **One placer:** `labels::place(cands, obstacles, pane, last_anchor)` places the region names
    too (they moved out of `overlay.rs`), the active region's name first, then the larger regions.
    `last_anchor` is a lookup closure (the module is pure: no hashed collections). A `Cand`
    carries its kind, tier, walk flag, importance (the card's drawn radius), path and lean;
    `labels::order` is the fill order. `labels::ellipsize_middle` cuts portal paths, keeping the
    first folder and the file name when they fit. Two tests beyond the required four:
    `last_side_first` and `portal_text_ellipsis`.
  - **Kinds** in `data-labels`: `region`, `focus` (the selection), `card`, `folder`, `portal`.
    Every kind but `region` counts against the 24.
  - **Candidates:** the drawn nodes at tier 0 and 1 (cards, folders, external systems; never the
    root), or every drawn node under show all. Galaxy scale has none, except under show all with a
    selection (still capped at 24). `data-label-cands` counts the candidates in front of the
    camera.
  - *Foreman ruling (2026-11-08), follow-up commit:* R1 is unconditional. With nothing selected,
    the galaxy labels region names only, show all or not: R8's show all is about drawing, not
    labels. The first P5 commit let show all name cards at the galaxy fit with nothing selected;
    the follow-up stops it, and `verify/checks.txt` gains the P5 row "show all at the galaxy
    labels region names only" (nothing selected, show all on, at the galaxy fit: data-scale
    galaxy, data-label-cands 0, every data-labels entry a region).
  - **Where a label sits:** at its point when the point is on the pane. A portal whose card is
    off the pane is labelled at its region's glyph, leaning toward the pane centre; a card off the
    pane is not labelled.
  - **Obstacles:** the `.gtools` and `#glegend` rects (read at build and on resize), `#ghint`'s
    (read when its text changes), the compass box and the focus's disc.
  - **Motion:** a frame whose pose differs from the last is motion. The labelled set is chosen
    again when it is stale (the tiers, the walk target, the pane, the hint or the fonts changed)
    and once the pose has been still for 150 ms; the frame loop keeps running until that choice
    is made. In between, only the chosen cards are placed again, with the walk target always
    among them.
  - **Text:** each rect is the `measureText` width in the label's own font plus a px either side
    for the knockout stroke; the text is clipped to it. Region names are now drawn in `--ink`
    (they were `--ink-2`, which card labels take), so the two read apart.
  - Not built, and not in PLAN § 2.6: the research brief's 120 ms fade-in and clickable labels.
- **Verify:** `sh spec/graph-galaxy/verify/gate.sh P5`.
- **Touches:** `okt-client/src/graph/{labels,overlay}.rs`, `okt-client/src/graph.rs`,
  `okt-client/static/app.css`, `tests/ui/keys.mjs`.

## Phase P6 — z framing and the angular walk with portals
**Satisfies:** R4, R5, R7 (docs of the walk order) · **Depends on:** P5
**Goal:** `z` re-solves the camera for the selection and its links, or for the Results matches,
without moving a card. `j`/`k` step clockwise on screen, with the inspector's row marked. Portals
carry the target's path.

- [x] `keys.rs` `z`:
  - Capture `s().view == "list"` and clone `S.list` before switching to the graph, then drop the
    borrow.
  - With Results active, the focus set is the FIND files and names, or the Paths items, and the
    camera solves for all of them in place. Otherwise it runs the R3 solve (the in-region members
    fitted, the others as rim portals, per amended R4).
  - Both call `graph::frame_set(ids)` via `cam_to`, with the scale kept at focus (PLAN § 2.3). No
    history push (row D10).
- [x] `graph/vis.rs`: `angular_order(focus_px, targets)`, clockwise from 12 o'clock; ties go to
      distance, then path; portal and behind-camera targets sit at their rim point. Tests:
  - `angular_clockwise`.
  - `angular_ties`.
  - `angular_portal_rim`.
- [x] `graph.rs` walk:
  - The walk target's edge is drawn thicker (PLAN § 2.5; moved here from P4).
  - Membership comes from `insp::walk_list`. The order is computed from the destination pose and
    frozen per walk; a user orbit or zoom re-sorts it and re-finds the target by id.
  - The camera holds still during j/k, except a small re-fit for an off-pane target in the region.
  - Hooks: `data-walk`, `data-walk-i`, `data-walk-n`, `data-walk-angle`.
  - The `#ghint` format is kept. The walk target's portal label carries its path.
- [x] `insp.rs`: `render()` re-applies `mark_chip(graph::walk_target())`, with no `GS`/`WALK` borrow
      held across it. Update the doc comment.
- [x] Docs:
  - `README.md` § Keys and the `?` list: j/k step clockwise in screen order, replacing "in the order
    the inspector lists them"; `z` frames the selection and its links, or the Results matches.
  - Doc comments at `keys.rs:400`, the `graph.rs` walk and `insp.rs:484`.
- [x] Seed `issues/walk-mark-chip-cap.md` (`unshaped`) and its `ROADMAP.md` entry. The inspector caps
      chips at 40 per group, so a walk target past the cap gets no mark. This is pre-existing and not
      fixed in this cycle.
- [x] `tests/ui/keys.mjs`: the P6 rows.
- *Amended in build (2026-11-08):*
  - **`z`:** the Results branch calls `graph::frame_set(ids)`; the selection branch keeps the name
    `frame_selection` and solves through `solve_for(i, members)`, which `card_view` now calls with
    the card's links, so a card is solved exactly as a selection is and a folder with its walk
    members in its region. Both ease through `cam_to` at focus scale. Neither pushes a history
    entry; a `z` pressed in another view keeps today's view switch and its entry (row D10: view
    switches keep theirs).
  - **The Results focus** is `G::matches`: the matches are tier 0 and the whole focus set. Their
    links are not drawn (`vis::Focus` gains `links`, false for it; the test
    `matches_without_links`), and one matched region draws the rest of it at tier 3, so
    `data-focus-n` (tier 0 and 1 as uploaded) is the match count. The target is the middle of the
    matches' view-plane extent at their mean depth (`view_centre`), the active region the one
    holding the most matches. A selection change, `0`, a `z` without Results or a walk ends it; a
    rebuild keeps it by id.
  - **Walk order:** `vis::angular_order(focus_px, targets)` takes `vis::Target { px, rim, portal,
    path }` and sorts by `vis::screen_angle` of `Target::at()`, angles tied to the nanoradian. The
    graph places a card at its point (on or off the pane), a portal whose card is off the pane at
    its region's glyph (at the scale's rim weight), and a card behind the camera at the rim in its
    own direction. The walk records the destination pose its order came from; a later step under
    another destination re-sorts and re-finds the target by id, and the walk's own re-fit
    updates the record instead.
  - **Re-fit:** a target that is not at the rim and lies within 40 px of the pane's edge, or behind
    the camera, moves the camera out (same target and angles) just far enough to fit the selection
    and the target; root and surface targets count as in the region, since they have no rim
    glyph.
  - **The thicker edge** is drawn per frame as a screen-space mark in the accent,
    `--graph-walk-w` wide (3 px), from the selection to where the target shows, only when the two
    are linked (a folder's own children have no edge); a step uploads nothing. The hooks
    `data-walk`, `data-walk-i`, `data-walk-n` and `data-walk-angle` (degrees, clockwise from 12)
    are written by `gl::publish` from the frame (`gl.rs`, outside the Touches list).
  - **Labels:** the walk target's portal label is its whole path (other portals stay
    middle-ellipsised to 34 characters, row D7), and a portal target behind the camera is still
    labelled at its region's glyph.
  - **The `?` GRAPH list** gains a portals row.
  - **`keys.mjs`:** the env.sh hub (`contact/index.md`) has 138 chips in one Connections group, so
    an angular walk can start past the chip cap. The existing GRAPH walk checks and the P2
    ArrowDown row now walk from `walker`, the most connected card whose every group fits 40 chips.
    The P6 clockwise row walks the hub, and at a target past the cap asserts that no chip is
    marked (the seed's defect), and that targets with a chip have theirs marked. The `z` re-solve
    row uses the most connected card whose set has λ₁/λ₀ ≥ 1.25, as the P4 axis row does: the hub's
    set is degenerate (1.095), and a degenerate solve can land near the orbited view.
- **Verify:** `sh spec/graph-galaxy/verify/gate.sh P6`.
- **Touches:** `okt-client/src/keys.rs`, `okt-client/src/graph.rs`, `okt-client/src/graph/vis.rs`,
  `okt-client/src/insp.rs`, `okt-client/src/chrome.rs`, `README.md`, `okt-client/static/app.css`,
  `tests/ui/keys.mjs`, `issues/walk-mark-chip-cap.md`, `ROADMAP.md`.

## Phase P7 — Warp flight, history, frame-time gate and ledger
**Satisfies:** R6, R11, R12 · **Depends on:** P6
**Goal:**
- Enter or space during a walk flies to the target on the rising 600 ms path and pushes a history
  entry.
- `[` eases back to the stored pose and `]` goes forward.
- Reduced motion cuts every move.
- Interleaved moves never panic a borrow.
- The headed orbit and warp hold p95 ≤ 16.7 ms.
- The cycle's records are consistent.

- [ ] `keys.rs` walk branch: Enter/space call `nav::select(Some(t), SelOpts { warp: true, .. })`.
      With no walk they keep today's meaning (folder toggle, content tab).
- [ ] `nav.rs`:
  - add `SelOpts.warp`;
  - before `nav(true)` pushes, `replace_state` the current entry with `{pose}`, the destination pose
    of any move in flight;
  - `apply_hash` reads the hash and `history.state.pose` together, after its last await, then calls
    `graph::cam_to(pose, Ease)` (fallback: the R3 solve) and `graph::walk_reset()` when the
    selection changed;
  - the hash format and its dedupe are unchanged.
- [ ] `graph.rs`: `focus_node_with(id, Move::Warp)` calls `cam_to(Warp)`: van Wijk–Nuij, 600 ms, with
      the minimum rise of PLAN § 2.7 and a cap at the galaxy fit. The hook `data-peak` records the
      flight's peak.
- [ ] `graph/cam.rs`: test `warp_endpoints_and_rise`: endpoints exact; peak ≥ max(2·max(d₀, d₁),
      d_system) and ≤ d_G; D = 0 falls back to a log-lerp with the same floor.
- [ ] Docs: `README.md` § Keys and the `?` list say Enter or space flies there, `[` flies back and
      `]` goes forward.
- [ ] `tests/perf/profile.mjs --graph`: add `warp`, six round trips of `j`, space (alternating with
      Enter), wait, `[`, wait. Frames are recorded only while `data-anim="1"`, and the scenario needs
      at least 900 of them.
- [ ] `tests/ui/keys.mjs`: the P7 rows.
- [ ] Seed `issues/perf-profile-points.md` (`unshaped`) and its `ROADMAP.md` entry. The legacy
      `profile.mjs` mode throws on the empty `points.json` and defaults to 6580. This is pre-existing
      and not fixed in this cycle.
- [ ] The deferral ledger: walk every earlier phase body here and `PLAN.md`. Each deferred item names
      an `issues/` seed with a `ROADMAP.md` entry (`verify/ledger.sh`, run by the gate).
- **Verify:** `sh spec/graph-galaxy/verify/gate.sh P7`.
- **Touches:** `okt-client/src/keys.rs`, `okt-client/src/nav.rs`, `okt-client/src/graph.rs`,
  `okt-client/src/graph/cam.rs`, `okt-client/src/chrome.rs`, `README.md`, `tests/ui/keys.mjs`,
  `tests/perf/profile.mjs`, `issues/perf-profile-points.md`, `ROADMAP.md`.

---

## How `code-build` drives this

1. `next_phase.py` prints the next actionable phase (statuses are authoritative; the
   `current_phase` pointer is reconciled against them).
2. Pre-flight: the cycle worktree is clean, on `branch`, and `base` is reachable.
3. Execute every `[ ]` in the phase (consult `PLAN.md` / `research/` for detail).
4. Run the phase's `verify:` command — never advance on a checkbox alone.
5. Amend this file freely if reality diverges (regenerate frontmatter with `set_phase.py`; note the
   amendment in the commit body). STOP and escalate only on a **`GOAL.md` contradiction**.
6. Mark the phase `done`, advance `current_phase`, `--touch`; one `[{category}]` commit; stop and report.
