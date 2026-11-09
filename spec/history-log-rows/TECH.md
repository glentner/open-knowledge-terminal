---
slug: history-log-rows
title: 'History: one line per commit, under the Log''s day rules'
kind: feature
appetite: small
status: done
branch: feat/history-log-rows
base: main
current_phase: done
last_updated: '2026-11-09'
phases:
- id: P1
  name: History rows as one line under the Log's day rules
  status: done
  satisfies:
  - R1
  - R2
  - R3
  - R4
  - R5
  - R6
  - R7
  - R8
  depends_on: []
  parallel: false
  hammerable: false
  hill: uphill
  verify: OKT_TEST_PORT=6583 sh spec/history-log-rows/verify/gate.sh
review:
  last_reviewed_commit: ac8178e056928f1e96ce6ae36ca0e757d69dc98e
  verdict: approved
  blocked_reason: ''
  cycle: 3
---

# TECH.md — History: one line per commit, under the Log's day rules

The **context engine and finite-state machine** for building this feature. The YAML
frontmatter above is the resume ground-truth (read it from the cycle worktree with
`uv run --with pyyaml python "$KB/bin/factory/next_phase.py" spec/history-log-rows/TECH.md`); the
per-phase checklist below is the work. `code-build` executes the next actionable phase, runs its
`verify:` command, updates state via
`uv run --with pyyaml python "$KB/bin/factory/set_phase.py" …`, and makes one atomic code+state commit.

- **Vision / requirements (locked):** [`GOAL.md`](GOAL.md) — R-IDs are the contract.
- **Authoritative design:** [`PLAN.md`](PLAN.md).
- **Backing research:** none (lean path).

## Frontmatter field reference

- `status` (top): `planned | in_progress | blocked | in_review | done` (`done` is stamped by
  `code-publish` after confirmation, just before landing — the terminal state of the retained record)
- `appetite`: `small | big` — caps phase count and build-iteration budget (circuit breaker).
- `branch`, `base`: the cycle's branch (prefix per GOAL `kind`, from the profile's `branches`) and the
  profile's `base`.
- phase `status`: `pending | in_progress | done | blocked`
- `satisfies`: GOAL R-IDs this phase delivers (traceability anchor for `code-review`).
- `depends_on`: phase ids that must be `done` first (a phase is actionable only when met).
- `parallel`: `true` only for genuinely independent, non-coupled work (docs, tests, isolated leaf
  modules). Any phase touching the profile's `coupled core` is always `false`.
- `hammerable`: `false` marks a correctness/security phase that scope-hammering must **never** cut.
- `hill`: `uphill` (still figuring it out) → `crest` (unknowns resolved) → `downhill` (just
  executing). A phase stuck `uphill` across builds is a raised hand → escalate to the human.
- `attempts`: durable failed-verify counter (absent = 0), bumped by `set_phase.py --phase P<n>
  --record-attempt` on every red verify gate; `next_phase.py` warns at ≥3 — the circuit breaker
  runs on this file, not on session memory.
- `verify`: the exact command that proves the phase. A gate carrying `$( )` or quoting the calling
  shell would mangle is set with `set_phase.py --verify-file PATH`, not `--verify`.
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
  and every item the previous cycle's last phase carried for a project-level obligation. This repo's
  previous cycle left no such item in the tree (its `spec/` is gone and `ROADMAP.md` names none), so
  P1 carries the ledger alone.
- A PR body (when the profile's `landing` opens one) ends with an attribution naming the actual
  harness, model, and variant.
- A CLI/feature change obeys the profile's `same-commit rules` (§ Conventions in `AGENTS.md`): the
  help text, docs, or generated files it names change **in the same commit**.
- **This cycle's port is 6583.** Every `tests/env.sh` run sets `OKT_TEST_PORT=6583`; never 6580, and
  6581 belongs to a parallel cycle. The gate refuses 6580 and requires the variable.
- **No spec ids** (`R1`, `P1`, in any case) in any source, test, test label or function name.
- **Stage by path.** Never `git add -A`; name each file.

---

## Phase P1 — History rows as one line under the Log's day rules
**Satisfies:** R1–R8 · **Depends on:** —
**Goal:** every History listing (a file's, a folder's, Repository history) shows one 21px line per
commit (hash, `HH:MM`, status, subject, actions) under the Log's day rules, pinned while scrolling,
inside the inspector at every width (counts hidden where it is too narrow for them);
the Overview's "Last commit" row and the Log render exactly as on `main`.

- [x] `okt-client/src/insp.rs`, per PLAN § 2: add `hrows(commits, n)` (a `.dayrule` with
      `js::date_string(t) · {run length}` before each run of one `fmt::day_key`), call it from both
      branches of `history_view` (Repository history: the first 300), and in `hrow` set the class to
      `hrow hl` and the `.dt` cell to `fmt::time(c.t)`. Leave the Overview's "Last commit" markup,
      `fmt::dt`, `views/log.rs` and every handler untouched. No wrapper element around the rows.
- [x] `okt-client/static/app.css`: add the `.hrow.hl` rule and the `@container (width < 312px)`
      query from PLAN § 2 directly after the `.hrow` block, with their comments. Edit no existing
      rule (`.hrow*`, `.dayrule`, `.crow` and `#ibody` stay byte-for-byte). The query is R3's
      narrow-width clause (GOAL Clarifications, 2026-11-08): no part of a row past the inspector's
      right edge at any width, with the subject yielding first and then the counts hiding.
- [x] `tests/ui/history.mjs`: the repo's History suite, made from the History checks of
      `spec/history-log-rows/verify/history-rows.mjs` (its `--baseline` mode may be left out).
      Header comment in the style of `keys.mjs`; no spec ids anywhere in it.
- [x] Same commit: README § Layout (`tests/ui/` row names the History suite) and § Tests (a
      `tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/history.mjs"'` line); AGENTS.md § Build and
      test names `tests/ui/history.mjs` beside `keys.mjs`.
- [x] Deferral ledger (code-plan Step 6.8, standing last-phase item): walk this phase's body, the
      build's notes and any review notes for deferral wording; each deferral found has an
      `issues/{slug}.md` seed (from the factory's `ISSUE.md`) and a `ROADMAP.md` entry, staged by path.
      This plan files none.
- [x] Stage by path (`git add okt-client/src/insp.rs okt-client/static/app.css tests/ui/history.mjs
      README.md AGENTS.md spec/history-log-rows/TECH.md`), then run the verify.
- [x] Review cycle 1, F1 (PLAN § 2 amendment, 2026-11-08): `hrow` wraps the counts in `.ad` (hover
      text = the counts); `.hrow.hl` rows are their own size containers with a fixed 110px actions
      cell (the Log's 96px counts box, then a slot for `@` on every row); the threshold is
      re-derived from the fixed parts, `@container (width < 261.5px)` on the row's content box
      (287.5px of `#ibody`), so the counts hide only once the subject is at 0, with or without a
      drawn scrollbar.
- [x] Review cycle 1, F2 (PLAN § 6 amendment): `tests/ui/history.mjs` sweeps the band 4px at a time
      with the app's scrollbars drawn, in a 901 and a 1280px window, file and Repository history,
      grading every row at every step (nothing past the inspector's edge or the row's own box;
      counts hidden only where the subject has no room). Threshold 261.5→200 and →301.5, an `auto`
      actions cell and an `#ibody` container each turn it red. The frozen `verify/` is unchanged.
- [x] Review cycle 2, N1 (GOAL R3 Clarification "How wide is the counts cell…?", 2026-11-08; PLAN
      § 2 and § 6 amendments, 2026-11-09): `.hrow.hl .ad` holds 14 characters (105px, measured),
      `.rt` 118.5px, threshold re-derived to `@container (width < 270px)` (296px of `#ibody`).
      `tests/ui/history.mjs` commits a "+12,345 −6,789" fixture into `OKT_KB` only (refusing
      `~/KB`) and checks it whole at 46% and 75% in Repository history and its file's History;
      every geometry grade and band step checks shown counts of up to 14 characters whole; the band
      steps 2px. The old 96px cell, `933a94f`'s stylesheet and the threshold ±3.5px each turn it
      red. AGENTS.md § Build and test notes the suite runs only under `tests/env.sh`. The frozen
      `verify/` is unchanged: 198 < 296 < 319 still brackets the threshold.
- **Verify:** `OKT_TEST_PORT=6583 sh spec/history-log-rows/verify/gate.sh` — the profile's unit
  tests; then the base commit's client built into `target/history-log-rows-base/`; then in one
  throwaway environment `keys.mjs`, `tests/ui/history.mjs`, the frozen `history-rows.mjs` and its
  `--baseline` comparison (PLAN § 6). Measured on the prototype (mac, the final gate): 83 s warm
  for the whole gate; a first run adds about 40 s to build the base client. For the fail-once
  mutation, `gate.sh browser` runs the browser half alone (about 80 s). Mutants worth killing here
  are in PLAN § 6; dropping the container query must turn the 901px checks red.
- **Touches:** `okt-client/src/insp.rs`, `okt-client/static/app.css`, `tests/ui/history.mjs`,
  `README.md`, `AGENTS.md`, `spec/history-log-rows/TECH.md` (review cycle 1: also `PLAN.md`).

---

## How `code-build` drives this

1. `next_phase.py` prints the next actionable phase (statuses are authoritative; the
   `current_phase` pointer is reconciled against them).
2. Pre-flight: the cycle worktree is clean, on `branch`, and `base` is reachable.
3. Execute every `[ ]` in the phase (consult `PLAN.md` for detail).
4. Run the phase's `verify:` command — never advance on a checkbox alone.
5. Amend this file freely if reality diverges (regenerate frontmatter with `set_phase.py`; note the
   amendment in the commit body). STOP and escalate only on a **`GOAL.md` contradiction**.
6. Mark the phase `done`, advance `current_phase`, `--touch`; one `[feature]` commit; stop and report.
