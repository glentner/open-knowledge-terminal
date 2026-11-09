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

The current run, in order. Each entry is one feature cycle. Re-ordered 2026-11-08: the graph
rebuild is on ice (below), and the small cycles go first.

### 2. Compact History

History rows are two lines tall. Make them one line under the Log's day rules.

**Seed:** [`issues/history-log-rows.md`](issues/history-log-rows.md)

### 3. Pane arrangements

The view pane and the inspector are fixed left and right. Make panes placeable, swappable,
stackable and collapsible, with saved arrangements, so a third view (the dream inbox, an agent chat)
has somewhere to go.

**Seed:** [`issues/pane-arrangements.md`](issues/pane-arrangements.md)

### On ice: the graph as a navigable space

The radial graph draws everything at once, so it no longer helps anyone navigate. A first attempt
(a free 3D camera over galaxy, system and focus scales) was archived as a failed experiment on
2026-11-08: it lost the per-region glyphs and colours and read worse than the radial graph. The
branch is kept as `archive/graph-galaxy`, with the lessons in its `spec/graph-galaxy/ARCHIVED.md`.
A restart begins with concept art, with Geoffrey at every step.

**Seed:** [`issues/graph-galaxy.md`](issues/graph-galaxy.md)

# Part III — Polish

Small follow-ups found while the views run was being reviewed.

### 5. Count cells sized in characters

The History and Log counts cells are sized in pixels for the default font, so a wider `--font` cuts
their counts. Size them, and the thresholds derived from them, in `ch`.

**Seed:** [`issues/font-relative-count-cells.md`](issues/font-relative-count-cells.md)

---

## Settled questions

None yet.
