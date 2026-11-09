# REVIEW — Files table: columns negotiate width by priority, Name first

> Adversarial QA by `code-review`, run in an isolated context (a subagent). The correctness pass
> grades the branch diff against [`GOAL.md`](GOAL.md) + the repo's `AGENTS.md` § Invariants only;
> it does not see `PLAN.md`/`TECH.md`. Every finding cites an executed command.

- **Reviewed commit:** 5c4aaf1a0762fbcc173aaafd0e005b11d6841ea4  ·  **Base:** main  ·  **Date:** 2026-11-08
- **Verdict:** changes-requested
- **Cycle:** 1 of ≤3

Mode: one blind reviewer (fresh subagent, xhigh); inputs GOAL + rubric bundle, spec-excluded diff,
runnable worktree, AGENTS.md. Evidence: `.local/review1/`. Contract drift: none (GOAL unchanged since
the shape commit `24bb080`).

## Verification run

- `cargo test -p okt-server -p okt-pygments -p okt-client` → 6 / 9 / 13 passed (1 ignored, pre-existing opt-in)
- `tests/env.sh … keys.mjs` → 81 ok, all passed (two clean runs)
- Probes through `tests/env.sh` on 6581 (Chromium): splitter sweeps 1240→400→1240, stacked windows
  (86 widths, ±4 px around each threshold), a cap source mutant 60ch→40ch (every threshold moved
  exactly 150 px), three themes, HEAD vs origin/main geometry for Log, Table, Results and Paths
  (identical at 1600 and 820)
- Not verified: WebKit and Firefox (not installed); the design relies on `em` in `@container`
  conditions and `@property` (Safari 16.4+, Firefox 128+)

## Requirement → evidence matrix

| R-ID | Verified how | Status |
|---|---|---|
| R1 | wide panes and the splitter; Name/Title 1:0.9 at 1540 identical to main; `.nm.wide` row | ✅ |
| R2 | single `60ch`; source mutant moves every threshold 150 px | ✅ |
| R3 | splitter sweep both ways, exact order, header and cells aligned, no gap | ✅ |
| R4 | Name min 485 px while any droppable shows; 60-char names never cut | ✅ |
| R5 | status column at every width; uncommitted fixture | ✅ |
| R6 | stacked vs side-by-side at 86 widths; window size independence; real drags | ✅ |
| R7 | ellipsis iff overflow; prefix whole at depths 0, 1, 3 | ✅ |
| R8 | `title` = id on all rows; `elementFromPoint` over every part of the cell | ✅ |
| R9 | 1200 / 760 / 480 exactly as stated | ✅ |
| R10 | `verify` green; HEAD vs main geometry identical for the other views | ✅ (suite check weak: F2) |

Unmapped changes: the seed's `status: adopted:files-column-priority` (factory bookkeeping, accurate).

## Findings

### [MEDIUM/CONFIRMED] F1 · AGENTS.md and README no longer describe the change
- **Where:** `AGENTS.md:15` and `README.md:61` ("keys.mjs covers the navigation keys"); the
  same-commit rule (column widths ↔ `@container` offsets ↔ keys.mjs `OFF` table) is stated only in
  `app.css:222-234`, not in AGENTS.md § Conventions.
- **Evidence:** grep; the offset mutant shows the rule is real (finding F2's run).

### [LOW/CONFIRMED] F2 · The suite's "other views still render" check can't fail on a layout regression
- **Where:** `tests/ui/keys.mjs`, the other-views block (shown, has elements, first row has a box); Paths not exercised.
- **Evidence:** mutant `.crow` 64px→30px left "ok the log view renders" green (`keys-mutantA.log`).

## Triage (foreman, 2026-11-08, under decision 2026-11-08-okt-dream-inbox-autonomous)

Both CONFIRMED; routed to `code okt build` (remediation):
- F1 → AGENTS.md § Build and test and README name what keys.mjs now gates (the Files columns
  section, its `$OKT_KB` fixtures and skip line); § Conventions gains the same-commit rule for
  column widths, `@container` offsets and the keys.mjs threshold table (pointing to the comment, not
  restating numbers).
- F2 → the other-views check grades layout, not presence: each of Log, Table, Results and Paths
  (RECENT) asserts its rows' column geometry against values derived from its own CSS tracks (or a
  pinned table that changes with them), and a `.crow` track mutant must go red.

Review cycle 2 is scoped to verifying F1 and F2's remediation (foreman's call).

## Human-gate triggers

- None: no high-blast-radius path is touched.

## Review cycle 2 — approved (2026-11-08)

- **Reviewed commit:** 06889b2. **Mode:** scoped to verifying the remediation of F1 and F2 (the
  foreman's call), one blind reviewer (xhigh). Evidence: `.local/review2/`.
- **Verification run:**
  - `cargo test …`: 6 + 9 + 13 passed (1 ignored).
  - `keys.mjs` in env.sh: 86/86. Outside env.sh it prints the skip line and writes no fixtures.
  - 13 `app.css` source mutants through the real gate. Each was restored and confirmed with
    `git diff --quiet`, and `okt-client/dist` matches by `cmp` and `shasum`.
- **F1: fixed.** AGENTS.md and README describe what keys.mjs covers. § Conventions carries both
  same-commit rules, and the pointer comments exist. No spec ids.
- **F2: fixed for its class.** Fixed-track, gap, padding and row-height mutants of Log, Table,
  Results and Paths go red: M1–M5, M7, M10–M12.
- **R1–R10 regression probe:** clean. Drop order, the cap, status, header alignment, hover, and
  stacked vs side-by-side all still hold.

### [LOW/CONFIRMED, out of Scope] F3 · Other-views geometry check doesn't measure content-sized or flexible tracks
- **Where:** `tests/ui/keys.mjs:491-493` (`VIEWS`) and `:512-513` (`gradeView`).
- **Evidence:** four mutants stayed green while changing what renders:
  - M6, `.crow` date track `max-content` → `1fr`;
  - M9, `.lsimple` path flex;
  - M13, `.lrow` vertical padding;
  - M8, `.tgrid td` max-width.
- **Why out of Scope:** R10's product behaviour holds. The reviewer verified it directly, and the
  cycle changed no `.crow`, `.lrow`, `.lsimple` or `.tgrid` rule. What remains is how much a
  regression gate for unchanged views covers, which no R-ID, invariant or correctness defect of
  this cycle asks for.
- **Home:** `issues/regression-suite.md`, the existing Part I seed for end-to-end checks of the
  rendered panes, gains a dated note naming these four mutants. It is committed before the verdict.

### Triage (foreman, 2026-11-08, under decision 2026-11-08-okt-dream-inbox-autonomous)

No CONFIRMED finding remains in Scope, and there are no PLAUSIBLE findings. Verdict: approved.

## Human-gate triggers (cycle 2)

- None.
