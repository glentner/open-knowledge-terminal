# REVIEW — History: compact rows under day rules, like the Log

> Adversarial QA by `code-review`, run in an isolated context (a subagent). The correctness pass
> grades the branch diff against [`GOAL.md`](GOAL.md) + the repo's `AGENTS.md` § Invariants only;
> it does not see `PLAN.md`/`TECH.md`. Every finding cites an executed command.

- **Reviewed commit:** bcbd0a9  ·  **Base:** main  ·  **Date:** 2026-11-08
- **Verdict:** changes-requested
- **Cycle:** 1 of ≤3

Mode: one blind reviewer (fresh subagent, xhigh). Evidence: `.local/review1/`. Contract drift: GOAL
changed after shaping in `8205da3` (the narrow-inspector clause, a foreman ruling at the PLAN/TECH
sign-off); the foreman confirmed it as the contract graded.

## Verification run

- `cargo test …` → 6 / 9 / 13 passed (1 ignored, pre-existing); `keys.mjs` all passed;
  `tests/ui/history.mjs` 93 ok (`verify.log`)
- Probes on 6583 over one fixed KB clone, branch vs an origin/main build (`edcbed4`): rows, rules,
  labels, widths 22–75% in 1600/1440/1280/1100/1000/901 windows and stacked 860–320, three themes,
  with and without 10 px scrollbars
- Negative control: threshold 312→200 px left `history.mjs` green while rows spilled (`mut-threshold200.log`)

## Requirement → evidence matrix

| R-ID | Status | Evidence |
|---|---|---|
| R1 | ✅ | rules per run of one local day, order = API order (618, 130, 150, 300-row listings) |
| R2 | ✅ | label = `toDateString() · run length`, date text vs Log: 0 mismatches |
| R3 | ❌ F1 | cells, order, HH:MM, `@`, no spill anywhere; order clause fails in a band |
| R4 | ✅ | every row 21.00 px in ~90 combinations; ellipsis + title |
| R5 | ✅ | pinned rule at 9 scroll positions, header and tabs untouched |
| R6 | ✅ | row click, `@`, `h`, OPEN THE FULL LOG, header and sparkline lead |
| R7 | ✅ | Log and Overview "Last commit" identical to origin/main build (`cmp-*.json`) |
| R8 | ✅ | verify green; 44 commits on one day, 21 px at default and narrowest |

Unmapped changes: none of substance (docs lines accurate; seed status bookkeeping).

## Findings

### [MEDIUM/CONFIRMED] F1 · Counts hide while the subject still has room (R3 order clause)
- **Where:** `okt-client/static/app.css:370` (`@container (width < 312px)` for every row) with the
  counts' `auto` track (`:367`); 312 px is sized for the worst case (`+999,999 −99,999`), real counts are 5–9 characters.
- **Failure scenario:** 1280 px window, split at its narrowest (`#ibody` 282 px): all 618 rows of
  `memory/inbox.md` hide `+3 −0` (37.5 px) while the subject shows 91 px; Repository history 300/300.
  Seen in windows ~1040–1420 px at the narrowest split, wider at 22–30%.
- **Evidence:** `probe-branch.json` (`sweep`), `shots/hist-1280-22-light.png`, `ovf-scrollbars.log`.

### [LOW/CONFIRMED] F2 · `history.mjs` doesn't grade R3's narrow clause between 198 and 317 px
- **Where:** `tests/ui/history.mjs:371-377` (only 198 px and ~319 px)
- **Evidence:** threshold mutant 312→200 px: suite green, 129 + 3 rows spill by 4–13 px (`mut-threshold200.log`).

## Triage (foreman, 2026-11-08, under decision 2026-11-08-okt-dream-inbox-autonomous)

Both CONFIRMED; routed to `code okt build` (remediation):
- F1 → the property, for every row at every width: the subject yields before the counts hide, and
  nothing spills. Give the counts a fixed track (the Log's `.ad` 96 px track is the precedent; counts
  wider than the track clip inside it with "…" and keep their full text on hover), and re-derive the
  single container-query threshold from fixed parts + that track. Counts then hide only when the
  subject has already reached 0.
- F2 → `history.mjs` sweeps the band in small steps (every 4 px from the narrowest split to past the
  threshold, in at least two window widths, scrollbars as the app draws them) and grades both
  clauses on every row: no part past the inspector's edge, and counts shown whenever the subject
  width is > 0. A threshold mutant (either direction) and an `auto` counts track mutant must go red.

## Human-gate triggers

- None: no high-blast-radius path is touched.

## Review cycle 2 — changes-requested (2026-11-08)

- **Reviewed commit:** 933a94f. **Mode:** scoped to verifying F1 and F2, plus an R1–R8 regression check; one blind reviewer (xhigh). Evidence: `.local/review2/`.
- **Verification run:**
  - `cargo test …`: 6 + 9 + 13 passed.
  - `keys.mjs` 33 ok; `history.mjs` all passed, including four BAND sweeps with 10 px scrollbars drawn.
  - Reviewer probes: windows 901/1000/1100/1280/1440/1600 and stacked 860, swept in 0.1% steps, over five listings plus a renamed file.
  - Mutants, each restored and checked with `cmp`: thresholds 200, 320, 258 and 265, and no row container, all red. The `auto` track is an equivalent mutant: element boxes are identical.
  - R7: identical to an `edcbed4` build (`cmp7-*.json`).
- **F2:** fixed.
- **F1:** fixed when graded on cells.
  - Every listing flips at one width (#ibody ≤ 287 hides the counts, ≥ 288 shows them), with the subject at 0 px whenever the counts are hidden.
  - Graded against the counts' own text width rather than their 96 px cell, short counts still give up to 72 px. Ruling below.
- **R1, R2 and R4–R8:** hold.

### [MEDIUM/CONFIRMED] N1 · Counts longer than 12 characters are cut at every width
- **Where:** `okt-client/static/app.css:373,375` (`.rt` 110 px, `.ad` 96 px with ellipsis), introduced in `933a94f`.
- **Failure scenario:** `workspace/pearc28/index.md`, commit `4d079f7`, reads "+3,145 −1,9…" while its subject fits whole at 75% and at 46%. `2fc03b1` and `57a6ef3` are cut the same way: 3 of 1,870 commits.
- **Evidence:** `f1b.log`, `f2b.log`, `shots/bigcounts-*.png`. The suite has no such commit.

### Triage (foreman, 2026-11-08, under decision 2026-11-08-okt-dream-inbox-autonomous)

- **F1 residual: fixed, on a reading.** The counts are a fixed cell, after the Log's own 96 px `.ad` column, which Geoffrey asked History to match. R3's "the subject yields first" is graded on cells: the counts cell hides only when the subject cell is 0. A dated R3 Clarification records this.
- **N1: routed to build** (remediation 2, the last the bound allows).
  - Widen the counts cell to 14 characters at the History font. That holds every commit in this KB, and any count up to 99,999 lines per side.
  - Re-derive the single threshold from it.
  - Counts wider than 14 characters (100,000 or more lines on a side) cut with "…" and keep the full text in `title`. The same R3 Clarification records this.
  - `history.mjs` gains a large-counts fixture: a commit in the clone with counts of 13–14 characters. It checks them whole at the default width and at 75%. A 96 px cell mutant must go red.
- A CONFIRMED finding in review cycle 3 stops the cycle for Geoffrey.

## Human-gate triggers (cycle 2)

- None.

## Review cycle 3 — approved (2026-11-09)

- **Reviewed commit:** 4c948db. **Mode:** scoped to verifying N1, plus an R1–R8 regression check;
  one blind reviewer (xhigh). Evidence: `.local/review3/`.
- **Verification run:**
  - `cargo test …` 6 + 9 + 13 passed; `keys.mjs` 33/33; `history.mjs` 105/105, including the
    LARGE fixture (`+12,345 −6,789`, 105.00 px text in a 105 px box at 46% and 75%).
  - Reviewer probe: 1600, 1280 and 901 windows, the split dragged 75→22% in 2 px steps (0.25 px
    around the threshold), drawn scrollbars, four listings plus stacked windows 860→210: 198/198
    steps ok.
  - Mutants (restored and checked with `cmp`): threshold 276 red; `.ad` 104.5 px red (caught by
    LARGE).
  - R7 identical to the `edcbed4` build.
- **N1:** fixed.
  - `4d079f7` (13 characters), `2fc03b1` (14, the widest in the KB) and `57a6ef3` show whole
    wherever counts show.
  - The counts hide only when the subject cell is 0 (switch at #ibody 296 px).
  - Nothing goes past the inspector (max overshoot 0.000 px).
  - Counts over 14 characters are cut with the full text in `title`.
- **F1 and F2:** re-checked; still fixed.
- **R1–R8:** hold.

### [LOW/CONFIRMED, out of Scope] F-font · Under a wider `--font`, 14-character counts are cut
- **Where:** `okt-client/static/app.css:375`, where `.hrow.hl .ad` is 105 px (with `.rt` 118.5 px
  and the 270 px threshold).
- **Failure scenario:** with `--font Menlo`, `+21,899 −7,071` measures 105.375 px of text in the
  105 px box and is cut at every width. With SF Mono it measures 108.2 px.
- **Evidence:** `probe-font.log`, `font-menlo.png`, `font-default.png`.
- **Why out of Scope:**
  - The cycle's contract (the R3 Clarification, 2026-11-08) defines the cell as 105 px at the
    History font's 7.5 px advance, and the code matches it exactly.
  - Making it font-relative is hardening no R-ID asks for.
  - The Log's 96 px `.ad` has the same coupling and predates this branch.
- **Home:** `issues/font-relative-count-cells.md` (`status: unshaped`) and ROADMAP Part III, entry 5.
  Both are committed before the verdict.

### Triage (foreman, 2026-11-09, under decision 2026-11-08-okt-dream-inbox-autonomous)

No CONFIRMED finding remains in Scope, and none is PLAUSIBLE. Verdict: approved, at review cycle 3,
the bound.

## Human-gate triggers (cycle 3)

- None, in any cycle.
