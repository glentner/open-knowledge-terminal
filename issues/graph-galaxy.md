---
status: unshaped
kind: feature
appetite: big
---

# The graph as a navigable 3D space: galaxy, system, focus

## Problem

Geoffrey, 2026-11-08: the graph is fast and its radial layout does its job, but it no longer helps
him navigate. Everything is drawn at once, so after a search ("pearc28") he can't discern the
connections he walks with `j`/`k`, gives up, and switches to Files.

Evidence at `8d2f001`:
- `okt-client/src/graph.rs` is a deterministic radial layout by folder, painted on a 2D canvas:
  `polar(a, r)`, rings at `R_TOP = 190` and `dr_dir(depth)`. All 360° are spent and the picture is
  still crowded. The KB tracks 913 cards, plus their folders.
- Inactive parts are faded, never removed, so the busy picture persists at every zoom.
- `z` (`okt-client/src/keys.rs:517`) calls `graph::frame_selection()`: a 2D zoom-to-fit with no
  notion of orientation.
- Regions are folders. A project like pearc28 spans `project/`, `workspace/pearc28/`,
  `memory/docket/` and `decision/`, so "the pearc28 region" is not one place in today's layout.

## Why it was deferred

Not deferred: Geoffrey's request, the first cycle of the run. The behaviour is pre-existing. It
touches coupled core (`okt-client/src/graph.rs` with `okt-client/static/app.css`), and camera or
selection state in `okt-client/src/state.rs`, a high-blast-radius path.

## Outcome / vision

Three scales and a camera that does the work. The space metaphor shapes structure and motion only;
the visual language stays okt's ("I'm not looking for us to lean too much into the space analogy in
terms of visual design language").

- **Galaxy,** the outermost scale: each region is one cluster, its cards a dense cloud. Between
  regions, one bundled edge per linked pair, weighted by link count. Labels: region names only.
- **System:** one region fills the pane in 3D. Subfolders are sub-clusters, folder depth is distance
  from the region's core, and card size carries importance. Other regions stay visible at the rim,
  in their true direction, as labelled clusters with the bundles that reach them: the ways out.
- **Focus:** the selection and its links, labelled, with the camera turned to separate them.

**The camera** is spherical: azimuth θ, elevation ψ and distance. `z` solves the angle, not only
the zoom. Take the focus set's 3D positions (the selection and its links, or the search matches),
compute their covariance, and look along the smallest principal axis, so the plane they spread in
most faces the screen. Then set the distance to fit the set to about 70% of the pane. That is a
3×3 eigen-solve per press.

**Walking.** `j`/`k` step through the focused card's connections in on-screen angular order, like a
clock hand, highlighting the edge and its target. A target in another region shows as a portal: the
edge runs out to that region's cluster at the rim, labelled with the target's path. `<space>` warps:
an eased flight of about 600 ms that rises toward galaxy scale mid-flight and descends on the
target, so the trip itself shows where you went. Each warp goes on okt's back history.

### Visibility: where to be brave

Today everything is drawn and the inactive part is faded. Instead, decide visibility by graph
distance from the focus, and stop drawing most of the graph.

| Tier | Cards | Drawn as |
|---|---|---|
| 0 | the focus | full colour, label, halo |
| 1 | its direct links | full colour, labelled within the label budget |
| 2 | two hops away | dim points, no labels; an edge only on a path to the focus |
| 3+ | the rest of the focused region | faint points, no edges |
| — | every other region | **not drawn card by card:** one cluster glyph (centroid, a soft halo sized by card count, the region's name) and the bundle to it |

1. **Edges:** only those touching the focus set, plus the bundles between regions. Edges inside a
   region that don't touch the focus are hidden. A "show all" toggle keeps today's full picture for
   when the whole is the point.
2. **Depth cues:** perspective size, and alpha and saturation falling with depth. No blur: it costs
   a lot on a canvas and adds little.
3. **A label budget** of about 24, filled by tier and then importance, with collision culling in
   screen space. Labels sit flat on an overlay, never in perspective.
4. **Size carries importance:** link count, or the log of line count. Index cards and folders read
   larger.
5. **Stable positions.** The same KB gives the same layout, and a new card moves only its own
   folder's neighbourhood. Spatial memory is what makes a galaxy navigable, and a layout that
   reshuffles on every change throws it away.
6. **Motion, not jumps.** Every change of focus eases. Under `prefers-reduced-motion` the camera
   cuts instead (`app.css:513` already honours it elsewhere).
7. **A compass** in a corner: the galaxy's orientation and where the camera is in it.

## Open questions for the GOAL

1. **Search sets that span regions** (the pearc28 example). (a) `z` frames the matches where they
   are and the camera finds the angle that separates them, with the layout untouched; (b) a
   temporary "gather" pulls the matches into one constellation and releases them on exit. Lean (a):
   it keeps rule 5, and (b) can follow.
2. **Renderer.** (a) Canvas 2D with software projection and depth sorting: no new dependency, and
   probably enough for about 1,000 points; (b) WebGL2 through `web-sys`: points and edges in a few
   draw calls, real depth, glow, with a 2D canvas overlay for labels. Lean (b), settled in TECH by a
   frame-time benchmark on the live KB on mac-m4.
3. **Layout.** Hierarchical (regions on a Fibonacci sphere, subfolders recursively inside each
   region's cap, depth as radius) or a 3D force layout seeded by the hierarchy. Lean: hierarchical
   with a light, deterministic relaxation.
4. **What a region is.** The top-level folder, as today, or something richer later, such as
   project-centric clusters. Lean: folders for this cycle.
5. **Keys.** What orbits, and what returns from a warp, without clobbering `keys.rs`.
6. **The 2D radial layout.** Does it survive as a mode? Lean: no; "show all" covers the spectacle.

## Sketch of the acceptance criteria

- **R1** — WHEN the graph view opens with nothing selected, the graph SHALL show galaxy scale: one
  cluster per region and one bundle per linked pair of regions.
- **R2** — WHEN a card is selected, the camera SHALL ease to focus scale on it, looking along the
  focus set's smallest principal axis.
- **R3** — WHEN `z` is pressed, the graph SHALL re-solve orientation and distance for the current
  focus set: the selection and its links, or the search matches.
- **R4** — WHEN `j` or `k` is pressed in the graph, the selection SHALL step to the next or previous
  connection of the focused card in on-screen angular order, highlighting the edge and its target.
- **R5** — WHEN `<space>` is pressed on a highlighted connection, the camera SHALL fly to its target,
  AND back SHALL return to the previous focus.
- **R6** — The graph SHALL draw cards outside the focused region only as region clusters, and SHALL
  draw only edges that touch the focus set or join two regions, unless "show all" is on.
- **R7** — The layout SHALL be deterministic, AND adding one card SHALL NOT move any card outside its
  folder.
- **R8** — Orbit and warp SHALL hold 60 fps on the live KB on mac-m4. WHERE reduced motion is set,
  camera moves SHALL cut instead of ease.
- **R9** — `tests/ui/keys.mjs` SHALL pass, extended to cover `z`, `j`/`k` and `<space>` in the graph.

## Notes

- Proposed non-goals: no change to `/api/index` (`index.rs` `snapshot()` is high-blast-radius)
  unless the GOAL argues for one; no space-themed styling; no physics simulation at runtime.
- Related: pane arrangements, shipped 2026-11-09 (`spec/pane-arrangements/`). The graph lives in
  panes of any shape, each its own size container, so the camera fits its pane, not the window.
- Found by: Geoffrey, chat 2026-11-08 (story 3, KB `workspace/open-knowledge-terminal/stories.md`).
- **First attempt archived, 2026-11-08.** A free 3D camera over galaxy, system and focus scales,
  built through two review cycles, was stopped by Geoffrey after a preview: it lost the per-region
  glyphs and colours, read worse than the radial graph, and its proportions were off. Branch
  `archive/graph-galaxy`; lessons and reusable parts in its `spec/graph-galaxy/ARCHIVED.md`. A
  restart begins with concept art from the real index (galaxy, system, focus, a mid-warp frame) in
  the first graph's glyphs and colours, and visual R-IDs graded against the approved frames.
