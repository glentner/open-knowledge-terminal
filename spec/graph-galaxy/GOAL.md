# GOAL — The graph as a navigable 3D space: galaxy, system, focus

> **Origin spec.** The *what* and *why*: the locked contract `code-review` grades against.
> The *how* lives in [`PLAN.md`](PLAN.md) and [`TECH.md`](TECH.md) (written by `code-plan`).
> Edit requirements here; do **not** silently drift them during build.

- **slug:** graph-galaxy
- **kind:** feature
- **appetite:** big

## Problem

Geoffrey, 2026-11-08 (story 3, verbatim in part): "Our earlier work on this is excellent. It's fast
and performant. It does it's job of negotiating polar space around the center with angles that
spread regions of the graph. ... from a functionality standpoint we're getting limited practical use
now out of the graph view because it's so busy it's hard to really appreciate anything we're looking
at. For example, If I start searching "pearc28" and then want to use j/k and the other key bindings
to navigate down into that region of the workspace in the graph, I value the connections, but it's
just not something I can discern with the way we're displaying everything. ... Right now we have 360
degress of angular rotation to spread content and things are still crowded. ... The way we're
rotating things in the current graph, and the sizes, make it impossible to move through so I give up
and just pop over to the Files view, which has it's own drawbacks."

So the graph has stopped helping him navigate. The behaviour is pre-existing, not a regression.
Evidence, confirmed at `8c07189`:

- `okt-client/src/graph.rs` is "a deterministic radial layout by folder, drawn on a canvas" (its
  module doc, line 1): `R_TOP = 190.0` (`:24`), `dr_dir()` (`:28`), `polar()` (`:37`), painted with
  a 2D context. All 360° are spent and the picture is still crowded; the seed counted 913 cards in
  the KB, plus their folders.
- Inactive parts are faded, never removed, so the busy picture persists at every zoom.
- `z` (`okt-client/src/keys.rs:517`) calls `graph::frame_selection()` (`graph.rs:1942`): a 2D
  bounding-box zoom over the selection and its neighbours, with no notion of orientation, and no
  notion of a search's matches.
- Regions are top-level folders. A project like pearc28 spans `project/`, `workspace/pearc28/`,
  `memory/docket/` and `decision/`, so "the pearc28 region" is not one place in today's layout.

He asked for ambition: "full 3D, playing with both theta and psi angles, distance/size, and having
some courage with the changes to visibility", with a galaxy-to-solar-system zoom, `z` solving "a more
interesting kind of rotation in both angles", `j`/`k` still walking connection points, and `<space>`
warping to another region "like clicking in a street-view in a maps app". "I'm not looking for us to
lean too much into the space analogy in terms of visual disign language. But as a metaphore I think
this is much more powerful. ... Don't be shy, let's do something impressive."

## Outcome / vision

Three scales and a camera that does the work. The space metaphor shapes structure and motion only;
the visual language stays okt's.

- **Galaxy,** the outermost scale: each region (a top-level folder) is one cluster, its cards a dense
  cloud. Between regions, one bundled edge per linked pair, weighted by link count. Labels: region
  names only.
- **System:** one region fills the pane in 3D. Subfolders are sub-clusters, folder depth is distance
  from the region's core, and card size carries importance. Other regions stay visible at the rim, in
  their true direction, as labelled clusters with the bundles that reach them: the ways out.
- **Focus:** the selection and its links, labelled, with the camera turned to separate them.

**The camera** is spherical: azimuth θ, elevation ψ and distance, fitted to the graph's pane, never
the window (the graph will live in panes of any shape once pane-arrangements lands). `z` solves the
angle, not only the zoom: it looks along the focus set's smallest principal axis, so the plane the set
spreads in most faces the screen, then sets the distance to fit the set to about 70% of the pane.

**Walking** stays on `j`/`k`, now in on-screen angular order like a clock hand. A target in another
region shows as a portal: the edge runs out to that region's cluster at the rim. The go key flies
there: an eased flight of about 600 ms that rises toward galaxy scale mid-flight and descends on the
target, so the trip itself shows where you went. `[` comes back.

**Visibility, with courage.** Visibility is decided by graph distance from the focus, and most of
the graph is not drawn:

| Tier | Cards | Drawn as |
|---|---|---|
| 0 | the focus | full colour, label, halo |
| 1 | its direct links | full colour, labelled within the label budget |
| 2 | two hops away | dim points, no labels; an edge only on a path to the focus |
| 3+ | the rest of the focused region | faint points, no edges |
| — | every other region | one cluster glyph (centroid, a soft halo sized by card count, the region's name) and the bundle to it |

A "show all" toggle brings back the full picture for when the whole is the point, and replaces the
retired 2D radial layout as the spectacle.

Also in scope, as outcome items the review checks against the running app:

- **Depth cues:** perspective size, and alpha and saturation falling with depth. No blur.
- **Size carries importance** (link count, or the log of line count; the plan picks); index cards
  and folders read larger.
- **A compass** in a corner of the pane: the galaxy's orientation and where the camera is in it.
- **The layout** is hierarchical: regions on a Fibonacci sphere, subfolders recursively inside each
  region's cap, depth as radius, plus a light, deterministic relaxation. No runtime physics. Where
  the few root-level cards sit is the plan's call.
- **The renderer** is chosen in TECH by a frame-time benchmark (Clarifications), not here.

## Acceptance criteria (the contract)

"Focus set" means the cards `z` and selection frame: the selection and the connections the walk
steps through, or a results list's matches (R4). "Results list" means okt's current FIND or other
Results list. Performance and camera checks run in the throwaway environment (`tests/env.sh`, on
6581, never 6580).

- **R1** — WHEN the graph view shows with nothing selected, the graph SHALL show galaxy scale: one
  cluster per region, one bundled edge per pair of regions with links between them, its weight
  rising with the link count, and labels for region names only.
- **R2** — WHEN a folder (a region's top-level folder or a subfolder) is selected, the graph SHALL
  show system scale on that folder's region: the region drawn in 3D filling the pane, its subfolders
  as sub-clusters, folder depth as distance from the region's core, and every other region at the
  rim in its true direction as a labelled cluster with its bundle. WHEN the camera zooms out past
  focus scale, the graph SHALL cross to system scale and then to galaxy scale; WHEN it zooms in at
  galaxy scale, the graph SHALL enter system scale on the region nearest the pane's centre. Crossing
  scale by zoom SHALL NOT change the selection.
- **R3** — WHEN a card is selected, the camera SHALL ease to focus scale on it, looking along the
  focus set's smallest principal axis, at the distance that fits the focus set to about 70% of the
  graph pane.
- **R4** — WHEN `z` is pressed with the Results view active, the graph SHALL frame the results
  list's matches, wherever they are and across regions; WHEN `z` is pressed otherwise, it SHALL frame
  the selection and its links. In both cases the camera SHALL re-solve its orientation and distance
  for that set, and no card's layout position SHALL change. Selecting a match from Results SHALL make
  that card the focus, as any selection does (R3).
- **R5** — WHEN `j` or `k` is pressed during a walk in the graph, the selection's walk mark SHALL
  step to the next or previous connection in on-screen angular order, highlighting the edge and its
  target; a target in another region SHALL show as a portal, an edge to that region's cluster at the
  rim labelled with the target's path. The inspector's connection list SHALL mark the row of the
  walk's current target, so the two stay linked although their orders differ.
- **R6** — WHEN Enter or space is pressed during a walk, the graph SHALL select the marked target by
  an eased flight of about 600 ms that rises toward galaxy scale and descends on the target, and SHALL
  push a browser history entry for it; WHEN `[` is pressed after a warp or any focus change, the
  camera SHALL ease back to the previous focus, and `]` SHALL go forward again. With no walk, space
  and Enter keep today's meaning.
- **R7** — The graph SHALL orbit the camera on pointer drag and zoom it on wheel or pinch; it SHALL
  orbit from the keyboard on keys free in `keys.rs` today (the plan picks them and proves them free:
  in the graph, ArrowUp and ArrowDown already walk); and `-`, `+`/`=` and `0` SHALL keep their zoom,
  zoom and fit meanings, applied to camera distance. No other existing key SHALL change meaning.
  README § Keys and the `?` list SHALL describe the new keys and the angular walk order, replacing
  README's "in the order the inspector lists them".
- **R8** — WHILE "show all" is off, the graph SHALL draw cards by the visibility tiers in Outcome:
  cards outside the focused region and outside the focus set only as region clusters, and only the
  edges that touch the focus set or bundle two regions. WHEN "show all" is on, the graph SHALL draw
  every card and every edge.
- **R9** — The graph SHALL label at most 24 cards at once (region names not counted), filled by tier
  and then importance, drawn flat on an overlay rather than in perspective, with no two labels
  overlapping on screen.
- **R10** — The layout SHALL be deterministic: the same KB SHALL give the same card positions on every
  load, AND adding one card SHALL NOT move any card outside that card's folder.
- **R11** — The graph SHALL hold a p95 frame time ≤ 16.7 ms during a scripted orbit and a scripted
  warp, measured with `tests/perf/profile.mjs` (or an extension of it) on the `tests/env.sh` clone of
  the KB, in headed Chromium on mac-m4. WHERE `prefers-reduced-motion` is set, every camera move SHALL
  cut to its end state instead of easing.
- **R12** — `tests/ui/keys.mjs` SHALL pass inside `tests/env.sh`, extended to cover `z`, `j`/`k`, the
  warp on space and Enter, `[` after a warp, and the keyboard orbit keys in the graph.

## Non-goals (no-gos)

- **Gather** (a temporary constellation that pulls a cross-region search's matches together and
  releases them on exit): a follow-up candidate, not this cycle.
- **Project-centric regions** (a pearc28 cluster spanning folders): regions stay top-level folders.
- **No `/api/index` change.** `okt-server/src/index.rs` `snapshot()` is high-blast-radius.
- **No space-themed styling.** okt's visual language stays.
- **No runtime physics.** The layout's relaxation is deterministic and runs at layout time.
- **No blur** as a depth cue.
- **No 2D radial mode.** The radial layout is retired; "show all" covers the full picture.
- **No pane system.** The camera fits its pane; making panes placeable is `issues/pane-arrangements.md`.
- **No renderer choice in this GOAL.** TECH makes it from the benchmark.

## Clarifications

- **Q:** Split into a pilot and follow-ups? — **A:** No. One big cycle with the seed's full vision;
  the size circuit-breaker is waived ("Don't be shy, let's do something impressive"). (2026-11-08,
  foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous.)
- **Q:** Search sets that span regions: frame them in place, or gather them? — **A:** Frame in place.
  `z` frames the matches where they are and the camera solves the angle that separates them; the
  layout is untouched. Gather is a Non-goal and a follow-up candidate. (2026-11-08, foreman ruling
  under decision 2026-11-08-okt-dream-inbox-autonomous.)
- **Q:** Renderer: Canvas 2D or WebGL2? — **A:** Decided in TECH by a frame-time benchmark of both
  candidates, (a) Canvas 2D with software projection and (b) WebGL2 via `web-sys` with a 2D overlay
  for labels, on the full live KB content (the `tests/env.sh` clone of `~/KB` on 6581, never the
  server on 6580), on mac-m4, in headed Chromium (GPU), because headless software GL does not
  represent the display. WebGL2 is preferred if both pass, for headroom, depth and glow. The plan's
  research SHALL record the benchmark and its numbers. (2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous.)
- **Q:** Layout? — **A:** Hierarchical: regions on a Fibonacci sphere, subfolders recursively inside
  each region's cap, depth as radius, plus a light, deterministic relaxation. No runtime physics.
  (2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous.)
- **Q:** What is a region? — **A:** The top-level folder, for this cycle. Project-centric clusters
  are a Non-goal. (2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous.)
- **Q:** Keys: what orbits, what warps, what returns? — **A:** No existing key changes meaning except
  as named here. During a walk, Enter and space already go to the marked connection; in the new graph
  that go is the warp flight. Back is okt's existing `[` (browser history back; `]` forward). Every
  warp and every focus change pushes a history entry, and `[` returns with an eased camera move. With
  no walk, space and Enter keep today's meaning. Pointer drag orbits, wheel or pinch zooms; keyboard
  orbit uses keys free in `keys.rs` today, which the plan picks and proves free (ArrowLeft/ArrowRight
  for azimuth and Shift+ArrowUp/Down for elevation are suggestions). `-`, `+`/`=` and `0` keep their
  zoom meaning, applied to camera distance. README § Keys and the `?` list are updated. (2026-11-08,
  foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous.)
- **Q:** Does the 2D radial layout survive as a mode? — **A:** No, it is retired; the "show all"
  toggle covers the full-picture spectacle. (2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous.)
- **Q:** How is "60 fps" measured, and what does reduced motion mean? — **A:** Frame time is measured
  with `tests/perf/profile.mjs` (or an extension of it) during a scripted orbit and a scripted warp on
  the `tests/env.sh` clone, in headed Chromium on mac-m4; the target is p95 frame time ≤ 16.7 ms.
  Reduced motion means camera moves cut. (2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous.)
- **Q:** Which of the seed's design rules carry into the contract? — **A:** All of them, as
  requirements or explicit outcome items: the visibility tiers, the label budget (~24), depth cues
  without blur, size as importance, stable positions, the compass, and "show all". The seed's
  non-goals stay: no `/api/index` change, no space-themed styling, no runtime physics. The camera
  fits its pane, not the window. (2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous.)
- **Q:** What brings system scale: selection, zoom, or both? — **A:** Both. Selection sets the
  scale: selecting a region's top-level folder (or a subfolder) brings system scale on that region,
  and selecting a card brings focus scale. Zoom crosses to the neighbouring scale: zooming out from
  focus reaches system and then galaxy, and zooming in at galaxy enters the region nearest the pane
  centre. Crossing scale by zoom does not change the selection. This is the story's "galaxy view at
  the outer-most distance, then we zoom into a solarsystem". (2026-11-08, foreman ruling under
  decision 2026-11-08-okt-dream-inbox-autonomous.)
- **Q:** With a results list and a selection both present, what does `z` frame? — **A:** The FIND or
  Paths matches when `z` is pressed with the Results view active (the list is what is on screen);
  otherwise the selection and its links. Both branches solve the angle without moving any card.
  Selecting a match from Results makes that card the focus as usual. (2026-11-08, foreman ruling
  under decision 2026-11-08-okt-dream-inbox-autonomous.)
- **Q:** The shaper's three readings of the seed: angular walk order, a hard label cap, focus-set
  cards always drawn? — **A:** All accepted. The walk steps in on-screen angular order, and the
  inspector's connection list marks the row of the walk's current target so the two stay linked;
  README § Keys' "in the order the inspector lists them" is updated. The cap of 24 card labels is
  hard. Focus-set cards are always drawn, in any region. (2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous.)

## Related materials

- Seed: [`issues/graph-galaxy.md`](../../issues/graph-galaxy.md) (promoted; `status: adopted:graph-galaxy`)
- Geoffrey's words: story 3 in the KB, `workspace/open-knowledge-terminal/stories.md` (chat, 2026-11-08).
- Related seed: [`issues/pane-arrangements.md`](../../issues/pane-arrangements.md).
- Source: `okt-client/src/graph.rs` (layout, paint, walk, `frame_selection()`),
  `okt-client/static/app.css` (the graph rules; changes with `graph.rs`, § Conventions),
  `okt-client/src/keys.rs` (`z` at `:517`, the graph walk at `:404`–`:432`, `[`/`]`, `0`, `+`/`=`,
  `-`), `okt-client/src/state.rs` (camera or selection state would touch it: high-blast-radius,
  § Invariants), `README.md` § Keys.
- Tests: `tests/perf/profile.mjs`, `tests/ui/keys.mjs`, `tests/env.sh`.
