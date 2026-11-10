# REVIEW — Drag a view's tab to dock it in another pane

> Adversarial QA by `code-review`, run in an isolated context (a fresh subagent). The correctness
> pass grades the branch diff against [`GOAL.md`](GOAL.md) and `AGENTS.md` § Invariants only. It
> does not see `PLAN.md` or `TECH.md`. Every finding cites an executed command.

- **Reviewed commit:** 677899d  ·  **Base:** 6097aa9  ·  **Date:** 2026-11-10
- **Verdict:** changes-requested
- **Cycle:** 1 of ≤3

Evidence: `$WT/.local/review1/`.

## Verification run

- `cargo test -p okt-client`: 37 passed.
- `keys.mjs`: 88 ok, 0 FAIL, 0 skip. `panes.mjs`: 68 ok, 0 FAIL, 0 skip. Both files are unchanged
  on the branch.
- `tests/ui/dock.mjs` in full, in five runs split by `DOCK_GROUPS`: every group ok.
- A probe of the reviewer's own, `walk.mjs`: 236 random drags (seeds 7 and 11, at 1600 and 900
  px), with random `w d`/`w r` keys mixed in, over trees the suite never starts from. 168 valid
  with 0 bad, and 68 refused with 0 bad.
- Port 6588, through `tests/env.sh`. No build-state mutation. The tree was clean on hand-back.

## Requirement → evidence matrix

| R-ID | Verified how | Status |
|------|--------------|--------|
| R1 | dock `click` (48 tabs × 3 labels), `sources`, mid-drag selection checks in every sweep case and the walk | ✅ |
| R2 | dock "prediction" in 8 sweeps plus `sources`/`refuse`, comparing the overlay with the landed pane's rendered rect; walk 168/0 | ✅ |
| R3 | dock "result" (stored tree equals the oracle, tab order included, plus drawn rects); native sweep and hand-worked tests | ✅ |
| R4 | as R3, plus native `dock_preview_is_the_rect_the_view_lands_in` (equal halves) | ✅ |
| R5 | sweeps that empty a pane, the `refuse` accordion case, native `refused_dock_changes_nothing` | ✅ |
| R6 | dock `cancel-1600` and `cancel-900`: 7 kinds × 13 drags; Esc leaves an open commit open | ✅ |
| R7 | dock `refuse` (69 valid, 31 refused), native sweeps (>100k drops), walk 68 refused / 0 bad | ✅ |
| R8 | `nav::operate(arrange::dock)` read in code; native `centre_dock_on_the_neighbour_equals_the_move_keys` over the whole domain | ✅ |
| R9 | dock "persist" and "focus" (history length unchanged), `state` back/forward; native save/load | ✅ |
| R10 | dock "focus" and "fit"; `keyview` (19 drops, 11 off screen); `state` 6 of 6 with the precondition met | ✅ |
| R11 | keys.mjs 88/0, panes.mjs 68/0, both files unchanged | ✅ |
| R12 | `tests/ui/dock.mjs`: real Playwright mouse input; a label passes only when it held in every case, and a group with no checks fails | ✅ |

Unmapped changes: none that matter. `touch-action: none` on `.ptab` (the Non-goals allow it if it
costs nothing), the README text, the seed's status line, and `arrange::layout` made public for
`preview`.

The reviewer found `dock.mjs` neither hollow nor aggregated. It noted three points, each covered
elsewhere:
- The DOM check compares tabs as sets; the stored-tree check keeps their order.
- The blur and lost-capture cancels are triggered by dispatch, but the drag itself is real input.
- A `state` case with no valid target would drop silently, but the back/forward count catches it.

## Findings

### [MEDIUM/CONFIRMED] F1 · AGENTS.md doesn't describe what this diff added
- **Where:** `AGENTS.md:15-27` (Build and test) and `AGENTS.md:36-39` (Map).
- **Failure scenario:**
  - Build and test lists every browser suite with what it covers, but not `tests/ui/dock.mjs` or its
    oracle `tests/ui/dock-oracle.mjs`.
  - The Map says `panes.rs` draws the arrangement "(title bars, the view roots moved between panes,
    the sash drag)". It now also holds the tab-drag pointer states, the `#dock` overlay and the
    click swallowing.
  - `keys.rs`'s capture-phase window keydown that cancels a drag isn't mentioned.
- **Evidence:** `grep -n "dock\|drag" AGENTS.md` matches only line 22 ("dragging a split") and
  line 39 ("the sash drag"). Re-run by the orchestrator on 677899d: same two lines.
- **Touches invariant / requirement:** operating-manual drift (rubric Scope 5). It is MEDIUM: not
  § Invariants, not a file-path-keyed gate list, and the profile's `verify` names only `keys.mjs`.
- **Route (the property, not the instance):** every module, suite and on-disk artifact this diff
  introduced or moved is described in `AGENTS.md`, checked by grep over the diff's added files and
  new handlers.

## Human-gate triggers

- None. F1 is in `AGENTS.md`, which is not on § Invariants' high-blast-radius list, and violates no
  *[human gate]* section. The diff touches no high-blast-radius path.

## Optional completeness sub-pass (separate reviewer; may see TECH.md)

- Not run.

## Review cycle 2 — changes-requested (2026-11-10)

- **Reviewed commit:** bf4eddf  ·  **Base:** 6097aa9  ·  **Mode:** a fresh blind pass over the full
  spec-excluded diff
- Evidence: `$WT/.local/review2/`.

### Verification run

- `cargo test -p okt-client`: 37 passed. `okt-server` and `okt-pygments` pass.
- `keys.mjs` 87 ok, `panes.mjs` 68 ok, both all passed and unchanged on the branch.
- `tests/ui/dock.mjs` in full: all 15 groups, every label ok, `dock: all passed`, exit 0.
- Two of the reviewer's own probes:
  - `probe-walk` (seeds 7 and 987654): 311 drags over trees no preset makes, at six viewports from
    420 to 1600 px wide, with sash drags mixed in. Outline validity, outline rect, landed rect and
    aimed pane all matched the oracle. 0 bad.
  - `probe-edge`: a release on a sash, release off screen in four directions, a resize mid-drag, a
    preset key mid-drag (cancels, then applies), and a drag across the Graph (no pan).
- Every suite label carries its case count and a minimum, so none passes on an empty domain.
- § Invariants 1 checked by reading: every `TAB` state is taken out before capture calls and
  `nav::operate`. The `cfg()` borrows are statement-scoped.
- Tree clean, no build-state mutation, port 6588 only.

### Requirement → evidence matrix

R1–R12 ✅, as in cycle 1, now with the cycle 2 probes above: R1 (no pan, no selection over 311
drags), R2 (outline ⇔ validity, landed = outline, at non-preset trees and six viewports), R6 (off
screen and a key mid-drag), R7 (walks at 420 and 700 px).

### Findings

#### [MEDIUM/CONFIRMED] F2 · AGENTS.md and README overstate what the dock sweep covers
- **Where:** `AGENTS.md:27` and `README.md:84`. Both say the sweep runs "over every pane, zone and
  source tab".
- **Failure scenario:** `tests/ui/dock.mjs:362-369`, `sources()`, drags each pane's shown tab and
  its last unshown tab only. So `sweep-classic-1600` runs 36 cases (3 sources × 2 panes × 6 aim
  points), not 72, and Files, Log and Table are never dropped in a classic or swapped sweep. Anyone
  reading AGENTS.md next would think every tab × zone is graded.
- **Evidence:** `dock-A.log`, "sweep-classic-1600: each case starts from the preset its key
  applies (36 cases)". Re-checked by the orchestrator on bf4eddf: same count, and the same phrase
  at both lines.
- **Touches invariant / requirement:** operating-manual drift (rubric Scope 5), MEDIUM. R12 itself
  is met: it asks for "a view dragged there from another pane".
- **Route (the property):** every claim AGENTS.md and README make about what `dock.mjs` grades
  (which sources, panes, zones, widths and counts) matches what the code runs, checked against
  the code and a run's case counts.

Observation (not a finding): `tests/ui/dock-oracle.mjs`'s header names `dock-oracle-test.mjs`,
which isn't in `tests/ui/`. It's a byte-for-byte copy of the frozen oracle, so its comment is
recorded here and left as is.

### Human-gate triggers

- None. F2 is in `AGENTS.md` and `README.md`, neither on the high-blast-radius list.

## Review cycle 3 — approved (2026-11-10)

- **Reviewed commit:** 3720403  ·  **Base:** 6097aa9  ·  **Mode:** a fresh blind pass over the full
  spec-excluded diff
- Evidence: `$WT/.local/review3/`.

### Verification run

- `cargo test -p okt-client`: 37 passed. The dock tests assert their sweep counts (at least 100k
  each), so none can pass empty.
- `keys.mjs` 87 ok and `panes.mjs` 68 ok, both all passed with no skips. Neither file changes on
  the branch.
- `tests/ui/dock.mjs`: all 15 groups, every check ok, no page errors, about 1,515 s. That matches
  the "about 25 minutes" in AGENTS.md and README, and the counts match README's 36 or 72 drops per
  preset per width.
- The reviewer's own probes:
  - A: a pointer grid at sizes the suite doesn't use (1200×800 and 700×640), every preset and
    source, about 45k points. The overlay showed exactly where the oracle says a drop is valid,
    within 1 px, and never outside `#panes`.
  - B: 42 drops at band boundaries (±1.5 px) and on the sash all matched the oracle.
  - C: releases outside the viewport on all four sides changed nothing.
  - D: drags across the Graph canvas left its pixels unchanged and selected nothing.
- Tree clean, no build-state mutation, port 6588 only, every throwaway server stopped.

### Requirement → evidence matrix

R1–R12 ✅. AGENTS.md and README were checked against the code and the run output: the `#dock`
overlay, the capture-phase `keydown`, `blur`, the groups, the case counts, `DOCK_GROUPS` and the
run time. Neither has drifted. F1 and F2 are resolved.

### Findings

None. Candidates the reviewer tried and dropped:
- § Invariants 1;
- the click swallower;
- a Graph pan or text selection during a drag;
- the accordion repair against `docked`'s refusal;
- two speculative edge paths (a pointerup that never arrives, back/forward pressed mid-drag). The
  per-frame capture watch and the tree-unchanged check at release cover both.

Observation (as in cycle 2): `tests/ui/dock-oracle.mjs`'s header names `dock-oracle-test.mjs`,
which lives only under `spec/drag-to-dock/verify/`. Any drift in the oracle still fails `dock.mjs`
against the client, so the gate is not hollow.

### Human-gate triggers

- None in any cycle. No CONFIRMED finding touched a high-blast-radius path or a *[human gate]*
  section.
