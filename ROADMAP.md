# Open Knowledge Terminal — Roadmap

`okt` is the KB's terminal, rebuilt in Rust from `bin/terminal.py`, which retired from the KB on
2026-10-14. This document records the work still intended.

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

### 1. Regression tests for what the parity suites covered

The parity suites retired with the reference. Nothing now checks `/api/index`, `/api/file`,
`/raw/`, the watcher or the rendered panes end to end, so a regression there reaches the installed
okt unseen. Rebuild that coverage against okt's own recorded output.

**Seed:** [`issues/regression-suite.md`](issues/regression-suite.md)

---

## Settled questions

- **A code fence in a tight list item renders with an extra newline** (`declined`, 2026-10-15). It
  was a defect only against the reference; the newline doesn't render.
  [`issues/tight-list-fence-newline.md`](issues/tight-list-fence-newline.md)
