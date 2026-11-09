---
status: unshaped
kind: fix
appetite: small
---

# Mark the graph walk's target past the inspector's chip cap

> **Candidate, not a contract.** This file records deferred work in enough detail that a future
> session does not have to re-derive it. It is **not** graded by `code-review` and must never be
> copied into `spec/{slug}/GOAL.md` verbatim — `code okt feature` promotes it, and that is where
> appetite, non-goals and the R-IDs get negotiated with a human. The `status:` field above is the
> guard.

## Problem

While `j`/`k` walk the selection's connections in the graph, the inspector marks the chip of the
walk's current target (`insp::mark_chip`, the `.nchip.kb` class), so the graph's angular order and
the inspector's list stay linked. But the Connections section shows at most 40 chips per group
(`okt-client/src/insp.rs`, `connections()`: `items.iter().take(40)`, then a "+N more" button), and
`mark_chip` can only mark a chip that is in the page. A walk target past the 40th chip of its group
gets no mark at all: the hint and the graph show where the walk is, the inspector shows nothing.
Hubs are where this bites, and hubs are where a walk is most useful: the most connected card in the
KB has groups well past 40.

## Why it was deferred

Pre-existing: the cap and the by-id mark were both on `main` before `graph-galaxy`, and a walk in
the inspector's own order hit the same wall past the 40th row of a group. `graph-galaxy` P6 makes it
easier to see, because the angular walk no longer starts at the top of the first group, but fixing
it means changing how the inspector pages its chips, which is outside that cycle's scope (PLAN
§ 2.7, § 5).

## Outcome / vision

Whatever the walk is on, its row in the inspector is marked and scrolled into view: the group that
holds it shows that chip (expanding past the cap, or showing a window around it), and the mark
survives a re-render.

## Sketch of the acceptance criteria

- **R1** — WHEN the graph walk's target is past the first 40 chips of its Connections group, the
  inspector SHALL show that target's chip and mark it.
- **R2** — WHEN the walk moves on, or stops, the inspector SHALL return the group to its usual
  length.

## Notes

- Related: `spec/graph-galaxy/PLAN.md` § 2.7 (Inspector mark) and § 5; `okt-client/src/insp.rs`
  (`connections`, `mark_chip`, `walk_list`); `okt-client/src/graph.rs` (`walk`).
- Found by: `graph-galaxy` (the plan named it; filed in P6).
