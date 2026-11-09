# REVIEW — The graph as a navigable 3D space (galaxy, system, focus)

> Adversarial QA by `code-review`, run in an isolated/clean context (a subagent; where the runtime
> has none, `code-review`'s sequential fallback). The correctness pass grades the branch diff against
> [`GOAL.md`](GOAL.md) + the repo's `AGENTS.md` § Invariants **only** — it does not see
> `PLAN.md`/`TECH.md` (avoids grading-its-own-homework / plan-sycophancy). Every finding cites an
> **executed** command, not an assertion.

- **Reviewed commit:** 199aec28910167734b8bd275fb2e11121fb8c5cf  ·  **Base:** main  ·  **Date:** 2026-11-08
- **Verdict:** changes-requested
- **Cycle:** 1 of ≤3 — mirrors `review.cycle` in `TECH.md` (escalate to human on non-convergence)

Mode: one blind reviewer in a fresh subagent (xhigh), not the debate variant. Inputs: GOAL.md and
the rubric, handed over as one bundle file outside `spec/`; the spec-excluded diff and log; the
runnable worktree; `AGENTS.md`. Contract drift: GOAL.md changed after the shape commit in
`6902ef9` (R3 fit, R10 precondition), a foreman ruling under decision
2026-11-08-okt-dream-inbox-autonomous at the PLAN/TECH sign-off; the foreman confirmed it as the
contract graded. Evidence: the worktree's git-ignored `.local/review1/`.

## Verification run

- `cargo test -p okt-server -p okt-pygments -p okt-client` → okt-client 54, okt-pygments 9,
  okt-server 13 passed (1 ignored, pre-existing opt-in) (`cargo-test.log`)
- `tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"'` → 83/83 ok, no page errors (`keys-run1.log`)
- `tests/env.sh sh -c 'node "$OKT_ROOT/tests/perf/profile.mjs" --graph'`, headed, ANGLE Metal
  (Apple M4 Max), refresh 6.9 ms → orbit p95 8.2 ms (820 frames), orbit_all p95 8.0 ms (817),
  warp p95 8.6 ms (962, 6 trips) (`perf-run1.log`)
- Reviewer probes in the throwaway environment: `probe1.mjs`–`probe3.mjs` (`probe*.log`, screenshots)
- Negative control: `restore(st)` disabled at `graph.rs:1666` → `[ eases back to the previous focus`
  FAILED (`keys-mutant-restore.log`); source and `okt-client/dist` restored and verified by `cmp`

## Requirement → evidence matrix

| R-ID | Implemented by | Verified how | Status |
|------|----------------|--------------|--------|
| R1 | graph.rs, graph/vis.rs, gl.rs | keys.mjs galaxy checks; native `bundle_weight` | ✅ |
| R2 | graph.rs, graph/cam.rs, overlay, labels | keys.mjs system/rim/zoom-cross; probe2 Q1 | ❌ M3 (1–2 rim clusters unnamed) |
| R3 | graph.rs `card_view`, cam.rs `solve_view` | axis/extent check computed independently | ✅ |
| R4 | keys.rs, graph.rs `frame_set`/`frame_selection` | keys.mjs `z` rows; probe2 Q4 | ❌ M4 (re-select keeps match frame) |
| R5 | graph.rs walk, vis.rs `angular_order`, insp.rs | clockwise check; portal check; probe1 P4 | ❌ M2 (no mark past the 40-chip cap) |
| R6 | cam.rs `warp`, nav.rs, graph.rs `restore` | flight rows; P5, P6; mutant-validated | ✅ |
| R7 | keys.rs orbit block, README, chrome.rs | orbit rows; probe1 P1 | ❌ M1 (PageUp/PageDown regress inspector scroll) |
| R8 | graph/vis.rs `tiers`, graph.rs | tier/cluster/show-all checks from the index | ✅ |
| R9 | graph/labels.rs, overlay.rs | cap and no-overlap at rest, show all, drag; Q2 | ✅ |
| R10 | graph/space.rs | native space tests; shuffle and add-card probes | ✅ |
| R11 | gl.rs, cam.rs, profile.mjs `--graph` | headed perf above; reduced-motion rows | ✅ |
| R12 | tests/ui/keys.mjs | 83/83 | ✅ |

Unmapped changes: `theme.rs` `hex_alpha` removed (dead after the canvas renderer);
`actions.rs` `graph::recolor()` on accent change (supports GL colours); the dropped renderer cues
in L2; seeds `issues/graph-zoom-to-cursor.md`, `walk-mark-chip-cap.md`, `perf-profile-points.md`
(ROADMAP 6–8; deferral records, Problem text checked accurate).

## Findings

### [HIGH/CONFIRMED] H1 · Spec R-IDs in source code
- **Where:** `okt-client/src/graph/space.rs:744,766,786,800,810,820,827,893` (test fns `r10_*`)
- **Failure scenario:** § Conventions "No spec R#/P# ids in source code"; the names print in cargo output.
- **Evidence:** `grep -rnE '\br[0-9]{1,2}_' okt-client/src` lists exactly these; none on `origin/main`.
- **Touches:** § Conventions (HIGH).

### [MEDIUM/CONFIRMED] M1 · PageUp/PageDown no longer scroll the inspector in the graph view
- **Where:** `okt-client/src/keys.rs:441-461` (guard `elsewhere`, `:451`); hollow gate `tests/ui/keys.mjs:343`
- **Failure scenario:** click into inspector text (nothing in `#ibody` takes focus; active element
  stays BODY), press PageDown → camera tilts and the key is prevented; on `origin/main` the inspector scrolled.
- **Evidence:** probe1 P1: Files view `#ibody` 0 → 737; graph view scrollTop 0, ψ −16.2 → −26.2,
  `data-move` 2 → 3. The gate sets `tabIndex = -1` on `#ibody`, a state the app never produces.
- **Touches:** R7 ("No other existing key SHALL change meaning"), the D1 ruling (native inspector scrolling unchanged).

### [MEDIUM/CONFIRMED] M2 · Most walk targets on a hub get no inspector mark
- **Where:** `okt-client/src/insp.rs:596` (`take(40)`), `mark_chip` `:532`; gate avoids it at `tests/ui/keys.mjs:60`
- **Failure scenario:** walking `contact/index.md` (147 connections): 96 steps mark no row.
- **Evidence:** probe1 P4.
- **Touches:** R5 ("The inspector's connection list SHALL mark the row of the walk's current target").

### [MEDIUM/CONFIRMED] M3 · Rim clusters drawn without their names
- **Where:** `okt-client/src/graph/labels.rs:9,234` (a region name that fits nowhere is dropped)
- **Failure scenario:** system or focus scale: 1–2 of 21 rim regions unlabelled (e.g. `skill/` selected: activity, note).
- **Evidence:** probe2 Q1 and `q1-*.png`; the gate (`keys.mjs:528`) counts rim glyphs, never names.
- **Touches:** R2 ("every other region at the rim … as a labelled cluster").

### [MEDIUM/CONFIRMED] M4 · Re-selecting the current selection from Results keeps the match frame
- **Where:** `okt-client/src/graph.rs:1647-1651` (`matches` cleared only when `sel_id` changes)
- **Failure scenario:** X selected → FIND → `z` → Results → click X's row → `g`: still the match set.
- **Evidence:** probe2 Q4: `data-focus-n` 258, `data-drawn` 258 after re-selecting `contact/index.md`.
- **Touches:** R4 ("Selecting a match from Results SHALL make that card the focus").

### [MEDIUM/CONFIRMED] M5 · AGENTS.md map out of date
- **Where:** `AGENTS.md:26-28` omits `graph/vis.rs` and `graph/labels.rs`; § Build and test omits `profile.mjs --graph`.
- **Evidence:** grep.
- **Touches:** repo docs (the map is ground truth per ROADMAP's preamble).

### [LOW/CONFIRMED] L1 · Docs say `[` "flies back"; the code eases straight back
- **Where:** `README.md:83`, `okt-client/src/chrome.rs:488`; comment `keys.rs:546` (`z` from Results does push via `set_view`, as on main)
- **Evidence:** probe2 Q3: distance 290 → 290, peak 290; history 10 → 11.
- **Touches:** R6/R7 docs.

### [LOW/CONFIRMED] L2 · The graph drops the dirty-file dot and the collapsed-folder count
- **Where:** `origin/main` `graph.rs:721,742,805` had both; HEAD has neither.
- **Evidence:** reading both trees.
- **Touches:** no R-ID; a regression of existing graph information.

## Triage (foreman, 2026-11-08, under decision 2026-11-08-okt-dream-inbox-autonomous)

All seven are CONFIRMED and route to `code okt build` (remediation); no PLAUSIBLE findings.
- H1 → the class: no spec id in source in any case form (`r10_`, `R10`, `p7`); the static scan
  that let lowercase through is widened to match.
- M1 → the property: an orbit key never takes a key the browser would otherwise use to scroll the
  inspector. Decide by where the user last interacted (pointerdown inside `#insp` hands the keys
  back), not by `document.activeElement`. The gate reproduces the real state: a click into the
  inspector's text, with no `tabIndex` injected.
- M2 → R5 is fixed, not deferred: when the walk target is past the chip cap, its row is shown and
  marked anyway. `issues/walk-mark-chip-cap.md` and ROADMAP 7 are retired or narrowed to what remains.
- M3 → the property: at system and focus scale every rim cluster carries its name, with R9's
  no-overlap still holding; a check grades the names, not the glyph count.
- M4 → the property: any selection made from Results, the current card included, ends the match frame.
- M5, L1 → docs corrected.
- L2 → fixed in the GL renderer: the dirty dot and the collapsed-folder count return.

## Human-gate triggers

- None. No CONFIRMED finding is in a high-blast-radius path; the reviewer verified the `state.rs`
  change (`showAll` persists, an old `labels` key is inert) and found § Invariants 1 clean.

## Optional completeness sub-pass (separate reviewer; may see TECH.md)

- Not run.
