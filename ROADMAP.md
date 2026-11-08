# Open Knowledge Terminal — Roadmap

`okt` is the KB's terminal, written in Rust. This document records the work still intended.

It is a **forward-looking index, not an implementation plan.** Each entry states a problem and the
intention behind solving it; the *how* belongs to the plan step of a factory cycle. As-built
architecture and the load-bearing invariants live in [`AGENTS.md`](AGENTS.md), which stays ground
truth.

Each entry's **Seed** points at an [`issues/{slug}.md`](issues/) file that carries the evidence in
enough detail that a future session does not have to re-derive it. Those are **candidates, not
contracts**: the feature step promotes one into `spec/{slug}/GOAL.md`, and that promotion is where
appetite, non-goals and the graded R-IDs get negotiated. The `status:` field on each issue is the
guard:
- `unshaped`: a raw deferral;
- `shaped`: negotiated, not yet accepted into a cycle;
- `adopted:{slug}`: promoted, and retired from this index once the cycle lands on `main`.

Two further values close a deferral **without** shipping, `declined` and `accepted-behaviour`.
Those are listed under *Settled questions* below rather than as cycles.

---

# Part I — Tests

### 1. End-to-end regression tests

Nothing checks `/api/index`, `/api/file`, `/raw/`, the watcher or the rendered panes end to end, so
a regression there reaches the installed okt unseen. Build that coverage against okt's own recorded
output.

**Seed:** [`issues/regression-suite.md`](issues/regression-suite.md)

---

# Part II — Views

The current run, in order. Each entry is one feature cycle.

### 2. The graph as a navigable 3D space

The radial graph draws everything at once, so it no longer helps anyone navigate. Rebuild it as
galaxy, system and focus scales, with a camera that solves the viewing angle and visibility decided
by graph distance from the focus.

**Seed:** [`issues/graph-galaxy.md`](issues/graph-galaxy.md)

### 3. Pane arrangements

The view pane and the inspector are fixed left and right. Make panes placeable, swappable,
stackable and collapsible, with saved arrangements, so a third view (the dream inbox, an agent chat)
has somewhere to go.

**Seed:** [`issues/pane-arrangements.md`](issues/pane-arrangements.md)

### 4. Files columns by priority

As the pane narrows, Name is crushed first. Keep Name whole up to a cap, and drop the other columns
in a set order instead.

**Seed:** [`issues/files-column-priority.md`](issues/files-column-priority.md)

### 5. Compact History

History rows are two lines tall. Make them one line under the Log's day rules.

**Seed:** [`issues/history-log-rows.md`](issues/history-log-rows.md)

### 6. Zoom toward the pointer in the 3D graph

The 3D graph zooms about the camera's target, so zooming into something off-centre means selecting
it or orbiting first. Zoom toward what is under the pointer without losing the stable orbit centre.

**Seed:** [`issues/graph-zoom-to-cursor.md`](issues/graph-zoom-to-cursor.md)

---

## Settled questions

None yet.
