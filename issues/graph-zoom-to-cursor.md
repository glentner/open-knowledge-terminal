---
status: unshaped
kind: feature
appetite: small
---

# Zoom toward the pointer in the 3D graph

> **Candidate, not a contract.** This file records deferred work in enough detail that a future
> session does not have to re-derive it. It is **not** graded by `code-review` and must never be
> copied into `spec/{slug}/GOAL.md` verbatim — `code okt feature` promotes it, and that is where
> appetite, non-goals and the R-IDs get negotiated with a human. The `status:` field above is the
> guard.

## Problem

The 2D radial graph zoomed about the pointer: the wheel and a pinch kept the point under the
cursor fixed, so you could zoom straight into whatever you were looking at. The 3D graph
(`graph-galaxy`) zooms about the camera's target instead: the wheel, a pinch, `+`/`=` and `-`
change only the camera distance (`okt-client/src/graph.rs`, the `wheel` handler and `zoom_by`;
`okt-client/src/graph/cam.rs`, `wheel_delta`). To zoom into something off-centre you first select
it, or orbit until it is near the centre.

## Why it was deferred

A design choice of `graph-galaxy`, recorded as row D3 of `spec/graph-galaxy/PLAN.md`: orbit and
zoom share one centre, so the next orbit never swings the scene off the pane, and R3's centred
focus (the selection at the pane centre, which the clockwise walk order depends on) stays intact.
Zooming toward the pointer in 3D needs a ray cast into the scene on every wheel event to choose a
new target, which fights that centring. It is a change introduced by that cycle (the 2D behaviour
was pre-existing), deferred because the GOAL only requires "zoom it on wheel or pinch" (R7).

## Outcome / vision

Wheel and pinch zoom toward what is under the pointer, without breaking orbiting about a stable
centre or the selection's place at the centre during a walk.

## Sketch of the acceptance criteria

- **R1** — WHEN the wheel or a pinch zooms the graph with the pointer over a drawn card or cluster,
  the graph SHALL move the camera toward that point, keeping it under the pointer.
- **R2** — WHEN the camera then orbits, it SHALL orbit about a centre that keeps the scene in the
  pane.
- **R3** — WHILE a walk is in progress, zooming SHALL NOT move the selection off the pane centre.

## Notes

- Related: `spec/graph-galaxy/PLAN.md` row D3 and § 2.3 (Input); `issues/pane-arrangements.md`
  (the camera fits its pane, whatever its shape).
- Found by: `graph-galaxy` P2 (the deferral the plan named).
