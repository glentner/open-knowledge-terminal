---
status: unshaped
kind: fix
appetite: small
---

# Make the legacy perf profile safe to run and fit for the 3D graph

> **Candidate, not a contract.** This file records deferred work in enough detail that a future
> session does not have to re-derive it. It is **not** graded by `code-review` and must never be
> copied into `spec/{slug}/GOAL.md` verbatim — `code okt feature` promotes it, and that is where
> appetite, non-goals and the R-IDs get negotiated with a human. The `status:` field above is the
> guard.

## Problem

`tests/perf/profile.mjs` has two modes. `--graph` (added by `graph-galaxy`) targets `$OKT` only and
refuses 6580. The legacy mode, `node tests/perf/profile.mjs <url> <label> [width height dpr]`, has
three defects, all on `main` before that cycle:

- **It defaults to the live server.** With no URL argument it targets `http://127.0.0.1:6580/`
  (`const TARGET = process.argv[2] || 'http://127.0.0.1:6580/'`, line 146 on the branch, line 7 on
  `main`), the developer's own running okt, and drives clicks, key presses and view switches
  against it. The same fallback sits in `tests/ui/keys.mjs` (`const OKT = process.argv[2] ||
  process.env.OKT || 'http://127.0.0.1:6580/'`, line 37 on the branch, line 11 on `main`): run
  outside `tests/env.sh`, with neither an argument nor `OKT` set, the key suite walks the live KB.
  It does not write to it, but its LAYOUT probe is guarded only by an `OKT_KB` path check.
- **`points.json` is read unconditionally.** The `click_select` and `zoom_selected` scenarios click
  fixed screen points from `tests/perf/points.json` (`JSON.parse(fs.readFileSync(…points.json))`,
  line 206 on the branch, line 67 on `main`), so a missing or empty file throws before the first
  scenario. The `graph-galaxy` harness research (`spec/graph-galaxy/research/06-harness.md`) found
  the file 0 bytes in its working copy; at the time of filing it holds six points (61 bytes) at
  `HEAD`, so the throw needs the file emptied or missing, but nothing regenerates it.
- **Its scenarios are written for the retired 2D graph.** The six points are screen positions of
  file nodes in the old radial layout's fit view at 1920×1080; in the 3D graph they land on
  whatever is there. `pan_drag` measures a pan the camera no longer does (a drag orbits), and
  `compare.py` compares runs that no longer mean the same thing.

## Why it was deferred

Pre-existing, and outside `graph-galaxy`'s scope: that cycle needed a frame-time verdict, which it
added as the separate `--graph` mode, and left the legacy mode untouched (PLAN § 5, research 06).
The legacy mode is not part of any gate.

## Outcome / vision

The legacy profile runs only against a server it is pointed at, never 6580 by default, and its
scenarios exercise the 3D graph (orbit, zoom, select, walk) with no screen coordinates baked in.
`keys.mjs` likewise refuses to run with no target, or refuses 6580.

## Sketch of the acceptance criteria

- **R1** — WHEN `profile.mjs` or `keys.mjs` is run with no target argument and no `OKT`, it SHALL
  exit non-zero without contacting any server.
- **R2** — The legacy profile's scenarios SHALL select cards by id from `/api/index`, not by fixed
  screen points, and SHALL run without `points.json`.

## Notes

- Related: `tests/perf/profile.mjs`, `tests/perf/points.json`, `tests/perf/compare.py`,
  `tests/ui/keys.mjs`; `spec/graph-galaxy/research/06-harness.md`; `spec/graph-galaxy/PLAN.md` § 5.
- Found by: `graph-galaxy` (the plan named the profile defects, filed in P7; the `keys.mjs`
  fallback was found in P1).
