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

The views run has shipped its cycles: the Files columns on 2026-11-08, Compact History and pane
arrangements on 2026-11-09. The graph rebuild is on ice (below).

### On ice: the graph as a navigable space

The radial graph draws everything at once, so it no longer helps anyone navigate. A first attempt
(a free 3D camera over galaxy, system and focus scales) was archived as a failed experiment on
2026-11-08: it lost the per-region glyphs and colours and read worse than the radial graph. The
branch is kept as `archive/graph-galaxy`, with the lessons in its `spec/graph-galaxy/ARCHIVED.md`.
A restart begins with concept art, with Geoffrey at every step.

**Seed:** [`issues/graph-galaxy.md`](issues/graph-galaxy.md)

# Part III — Polish

Small follow-ups found while the views run was being reviewed.

### 2. Count cells sized in characters

The History and Log counts cells are sized in pixels for the default font, so a wider `--font` cuts
their counts. Size them, and the thresholds derived from them, in `ch`.

**Seed:** [`issues/font-relative-count-cells.md`](issues/font-relative-count-cells.md)

### 3. Literal tokens drawn as source rows

The source-row rule `.src .l` also matches Pygments' literal token class `l`, so with the blame on
a YAML scalar becomes a row grid with its own gutter, and the Inspector scrolls sideways. Scope the
rule to the rows.

**Seed:** [`issues/source-row-token-class.md`](issues/source-row-token-class.md)

### 4. An off-screen Log steps from a stale row mark

Under the 200 px narrow floor the Log is off screen but still takes `j`/`k`. Shown again by back or
forward, `l` or its LOG tab, it keeps an outdated row mark, so the next step lands on the wrong row.
Bring the mark up to date on every route, as on screen.

**Seed:** [`issues/offscreen-log-mark.md`](issues/offscreen-log-mark.md)

### 5. A guard on redraws during a sash drag

Only the `KEY_OFF` latch stops an off-screen Log being fully redrawn on every move of a sash drag,
and no suite check would notice it gone (30 renders for a 30-move drag, against 0). Add the check.

**Seed:** [`issues/sash-drag-redraw-guard.md`](issues/sash-drag-redraw-guard.md)

---

## Settled questions

None yet.
