---
slug: preset-bar
title: 'No preset bar: the crumbs move into the header'
kind: fix
appetite: small
status: done
branch: fix/preset-bar
base: main
current_phase: done
last_updated: '2026-11-09'
phases:
- id: P1
  name: The bar gone, the crumbs in the header, the presets graded by their arrangement
  status: done
  satisfies:
  - R1
  - R2
  - R3
  - R4
  - R5
  - R6
  - R7
  depends_on: []
  parallel: false
  hammerable: false
  hill: uphill
  attempts: 1
  verify: sh spec/preset-bar/verify/gate.sh
review:
  last_reviewed_commit: 09878c0
  verdict: approved
  blocked_reason: ''
  cycle: 4
---

# TECH.md — No preset bar: the crumbs move into the header

The **context engine and finite-state machine** for building this fix. The YAML
frontmatter above is the resume ground-truth (read it from the cycle worktree with
`uv run --with pyyaml python "$KB/bin/factory/next_phase.py" spec/preset-bar/TECH.md`); the
per-phase checklist below is the work. `code-build` executes the next actionable phase, runs its
`verify:` command, updates state via
`uv run --with pyyaml python "$KB/bin/factory/set_phase.py" …`, and makes one atomic code+state commit.

- **Vision / requirements (locked):** [`GOAL.md`](GOAL.md) — R-IDs are the contract.
- **Authoritative design:** [`PLAN.md`](PLAN.md).
- **Backing research:** none (lean path); the planner's measurements are in PLAN § 4.

## Frontmatter field reference

- `status` (top): `planned | in_progress | blocked | in_review | done` (`done` is stamped by
  `code-publish` after confirmation, just before landing — the terminal state of the retained record)
- `appetite`: `small | big` — caps phase count and build-iteration budget (circuit breaker).
- `branch`, `base`: the cycle's branch (prefix per GOAL `kind`, from the profile's `branches`) and the
  profile's `base`.
- phase `status`: `pending | in_progress | done | blocked`
- `satisfies`: GOAL R-IDs this phase delivers (traceability anchor for `code-review`).
- `depends_on`: phase ids that must be `done` first (a phase is actionable only when met).
- `parallel`: `true` only for genuinely independent, non-coupled work. Any phase touching the
  profile's `coupled core` is always `false`.
- `hammerable`: `false` marks a correctness/security phase that scope-hammering must **never** cut.
- `hill`: `uphill` (still figuring it out) → `crest` (unknowns resolved) → `downhill` (just
  executing). A phase stuck `uphill` across builds is a raised hand → escalate to the human.
- `attempts`: durable failed-verify counter (absent = 0), bumped by `set_phase.py --phase P<n>
  --record-attempt` on every red verify gate; `next_phase.py` warns at ≥3.
- `verify`: the exact command that proves the phase. A gate carrying `$( )` or quoting the calling
  shell would mangle is set with `set_phase.py --verify-file PATH`, not `--verify`.
- `review.cycle`: completed review passes, auto-incremented by every `set_phase.py --verdict …`.

## Conventions (apply to every phase)

- Commit conventions, code style, and load-bearing invariants come from the repo's `AGENTS.md` — it
  is the constitution. Walk the sections of its § Invariants that this change touches; that is the
  curated footgun checklist.
- One phase per `code-build` invocation by default; one atomic commit containing **both** the code and
  the `TECH.md` state change. The branch commit subject is code-build's (`$KB/skill/code-build.md` →
  Step 6), not restated here; branch commits are squashed away at `code-publish` per the profile's
  `landing`. Trailers follow the profile's `branch commits`.
- The last phase carries the standing last-phase items: the deferral ledger (`code-plan` Step 6.8),
  and every item the previous cycle's last phase carried for a project-level obligation. P1 is the
  last phase: it carries the ledger, and pane-arrangements' P5 says its predecessors carried the
  ledger alone.
- A PR body (when the profile's `landing` opens one) ends with an attribution naming the actual
  harness, model, and variant.
- A CLI/feature change obeys the profile's `same-commit rules` (§ Conventions in `AGENTS.md`): the
  help text, docs, or generated files it names change **in the same commit**.

### Running the gate

The gate runs with the sandbox off, exporting `OKT_TEST_PORT=6586` (never 6580),
`WASM_BINDGEN`, `PLAYWRIGHT_BROWSERS_PATH`, `PLAYWRIGHT_MODULE` and `TMPDIR`. Its log stays at
`$REPO/.local/preset-bar/gate.log` (`gate-static.log`, `gate-probe.log`, `gate-self-test.log` for
the other modes), with one line per run appended to `runs.txt` beside it; `$REPO` is the main
checkout and `.local/` is git-excluded there. Modes: none (steps 1–3), `--static` (step 1, the
pre-push part), `--probe` (step 3 with `page.mjs` alone, for a mutation of the page),
`--self-test STEPS` with `STATIC_BASE` (plan and review only; step `m` runs the page mutants).

### Gate timings (measured on the prototype dry run, 2026-11-09)

The prototype is `.worktrees/preset-bar-prototype`: the GOAL commit `fac7e93`, a local plan commit
carrying this `spec/preset-bar/`, and PLAN § 2's design staged on top. Every figure below is from
the run named beside it, on the M-series Mac this cycle runs on.

| Part | Measured | Run |
|---|---|---|
| step 1, `static.sh` | under 3 s | every run |
| step 2, `cargo test` | about 10 s warm | `--self-test 123m`, dry run 2 |
| step 3, env.sh build + clone + server | about 45 s cold, 20 s warm | first full run |
| step 3, `keys.mjs` | 99 s | `--self-test 123m`, dry run 2 |
| step 3, `history.mjs` | 33 s | same |
| step 3, `panes.mjs` | 423 s | same |
| step 3, `page.mjs` | 22 s (the sweep: 1204 widths × 2 pages in 7–11 s) | same |
| the full gate (steps 1–3) | 606 s red, about 10 min green | first full run (red on `panes.mjs:156`'s 890, PLAN § 2.4); dry run 2 |
| `--part keys` / `--part panes` | about 2.5 min / 8 min, each with the build | derived from the rows above |
| self-test step `m`, 23 mutants | 306 s, with step 1 and a warm build; the clean page green, all 23 killed | `--self-test 1m`, dry run 3 |

The full gate sits at the 10-minute command cap: build runs it in the background, and uses the
parts for a re-run that needs only one half.

---

## Phase P1 — The bar gone, the crumbs in the header, the presets graded by their arrangement
**Satisfies:** R1, R2, R3, R4, R5, R6, R7 · **Depends on:** —
**Goal:** the header sits on the panes with nothing between; no preset button anywhere; the crumbs
live in the header between GO and the chips, taking only the room GO leaves; the keys, `p` and the
`w` leader work and are listed as before; `panes.mjs` grades a re-applied preset after a drag in
place of the bar's mark and click.

- [x] `okt-client/static/index.html`: delete `nav.fkeys` (the four `.pre` buttons and `#presets`);
  put `<div class="crumbs" id="crumbs"></div>` in `header.top` after `form#goform`, before
  `div.chips` (PLAN § 2.1).
- [x] `okt-client/static/app.css`, exactly as PLAN § 2.1 says:
  - drop the `28px` track from both `#app` row rules;
  - delete the `/* function keys */` section;
  - `.go` gets `flex: 0 1 900px`;
  - the `.crumbs` rules move into the top bar section with `flex: 1 1 0; min-width: 0;
    margin-left: -10px` and the 6 px `::before` spacer, and a comment saying why the margin is
    there;
  - the `@media (max-width: 900px)` rule stays as it is.
- [x] `okt-client/src/panes.rs`: delete the `.on` loop over `#presets [data-preset]` and its
  `arrange::which` call.
- [x] `okt-client/src/keys.rs`: delete the `[data-preset]` click branch.
- [x] `okt-client/src/arrange.rs`: delete `Preset::name`, `Preset::parse`, and the test's
  `Preset::parse(p.name())` assertion. Keep `which`: `cycle` uses it (PLAN § 2.2).
- [x] `tests/ui/panes.mjs` (PLAN § 2.3):
  - the PRESETS header lines name no window bar;
  - delete `marked`, `strip` and `none`;
  - replace the window bar's check with `each preset key applied again after a drag restores its
    rects`, label verbatim: per preset at 1600 px, key, drag the first visible sash 120 px across
    its line, assert the drag moved it off the preset, key again, grade the rects;
  - the first-load check's `near(s.H, 890, 1)` becomes `918`, with a comment deriving it (1000
    less the 38 px header, 22 px ticker and 22 px status bar);
  - remove no other line: the gate allows exactly `verify/panes-removed.txt`'s, and no change to
    `keys.mjs` or `history.mjs`. A further removal needs a plan amendment.
- [x] `issues/drag-to-dock.md`: under line 46, append `- 2026-11-09: the preset bar is gone
  (preset-bar); drag-to-dock is the mouse path to an arrangement still to come.` Leave the line
  above as it was (code-build's append-only rule).
- [x] **Fail-once mutation** (code-build Step 4), as PLAN § 6 specifies:
  - in `keys.rs`'s preset key branch, ignore a key naming the preset the last key press applied
    (a `thread_local`);
  - confirm the mutant landed (the client rebuilt, the key branch changed);
  - the gate must go red with `FAIL each preset key applied again after a drag restores its
    rects`;
  - revert to the phase's own state, not to HEAD, and note it in the commit body.
- [x] **Deferral ledger** (standing last-phase item, code-plan Step 6.8):
  - walk this phase's body and commit body, prose and list items (not headings), for the phrases
    `verify/ledger.sh` reads;
  - each must name an `issues/` seed with a `ROADMAP.md` entry.

  No deferrals are planned. Anything build leaves undone becomes a seed from
  `$KB/skill/_templates/ISSUE.md` plus a ROADMAP entry, staged by path.

  Amendment (build P1, 2026-11-09): the foreman's PLAN gate ruling on Q4 sends the KB name shown
  twice to Geoffrey, and the ledger read it as a deferral with no seed. Build seeded it as
  issues/crumbs-kb-name-twice.md with ROADMAP entry 8, and appended the seed's name to that
  ruling in PLAN.md. Its text is otherwise unchanged.
- [x] Stage by path before the gate (its static step reads the index).
- **Remediation, review cycle 1 (2026-11-09).** Reopened for the review's F2 (seed prose), with
  the triage's F1 note and O1 seed; docs only, no code change.
  - [x] F2: issues/crumbs-kb-name-twice.md says the repeat predates this cycle (main showed it in
    its own row, under the brand) and that this cycle moved it into the header. The seed and its
    ROADMAP entry 8 are this branch's own text, not on main, so both are corrected in place.
  - [x] F1 note in issues/crumbs-kb-name-twice.md: a long KB display name widens the brand and
    leaves the crumbs no room at 1600 px, with app.css lines 110, 113, 114 and 131 cited and the
    review's measurements.
  - [x] O1: issues/offbar-placement.md from the ISSUE template (unshaped, fix, small) with ROADMAP
    entry 9 in Part III.
  - [x] Gate: the parts that read these files, gate.sh --static (the static checks with the
    ledger) and ledger.sh --self-test. The browser steps read none of them and are not re-run.
    First run (2026-11-09): the ledger self-test passes, and every static check but the ledger
    passes (kept 163). The ledger is red on three prose units of REVIEW.md alone (lines 44, 66 and
    70: the F2 finding's Where line and two triage items). They quote the seed's section title or
    name the seed only inside code spans or as "the crumbs seed above", and ledger.sh strips code
    spans. The same three units are red on 1f13e4e, before this run's edits. REVIEW.md is the
    review's and the foreman's record, so build leaves it as written: stopped for the foreman,
    attempt recorded.
    Second run (2026-11-09), after the foreman reworded those three REVIEW.md units to name
    issues/crumbs-kb-name-twice.md in plain text (4cdbf00): gate.sh --static green (kept 163,
    ledger ok, static ok) and the ledger self-test green.
- **Remediation, review cycle 2 (2026-11-09).** Reopened for the review's G1 (seed prose), with
  the triage's two crumbs notes; docs only, no code change.
  - [x] G1: issues/offbar-placement.md corrected in place (new on this branch). At 1196 px and
    wider this cycle raised the title-bar overlap from 6.7 to 18 of 24 px, the top 6 px visible;
    below that the banner wraps to two lines (45.34 px, capped at half the window by left: 50%)
    and main already covered 23.3 px. "Outright" is gone, the sketched R1 allows for a 45 px
    banner, and ROADMAP entry 9 carries the same qualifier.
  - [x] Crumbs notes in issues/crumbs-kb-name-twice.md: with ~/KB the box opens at about 1339 px,
    the name paints from 1350 px and is whole from 1377 px; under the 74-character path GO shrinks
    to 678.1 px at 1600 px, the brand 618.7 px.
  - [x] Gate: gate.sh --static (the static checks with the ledger) and ledger.sh --self-test.
    Green on the first run (kept 163, ledger ok, static ok; self-test both ok). The browser steps
    read none of these files and are not re-run.
- **Verify:** `sh spec/preset-bar/verify/gate.sh`, about 10 minutes, so run it in the background.
  It runs, in order:
  - the static checks and the ledger;
  - the unit tests;
  - one `tests/env.sh` run of `keys.mjs`, `history.mjs`, `panes.mjs` and `verify/page.mjs`;
  - checks that the client build printed no warning;
  - the kept and named checks.
- **Touches:** `okt-client/static/index.html`, `okt-client/static/app.css`,
  `okt-client/src/panes.rs`, `okt-client/src/keys.rs`, `okt-client/src/arrange.rs`,
  `tests/ui/panes.mjs`, `issues/drag-to-dock.md`. README, `AGENTS.md` and `chrome.rs` need no edit
  (PLAN § 2.3); if build finds otherwise, the same commit changes them.

---

## How `code-build` drives this

1. `next_phase.py` prints the next actionable phase (statuses are authoritative; the
   `current_phase` pointer is reconciled against them).
2. Pre-flight: the cycle worktree is clean, on `branch`, and `base` is reachable.
3. Execute every `[ ]` in the phase (consult `PLAN.md` for detail).
4. Run the phase's `verify:` command — never advance on a checkbox alone.
5. Amend this file freely if reality diverges (regenerate frontmatter with `set_phase.py`; note the
   amendment in the commit body). STOP and escalate only on a **`GOAL.md` contradiction**.
6. Mark the phase `done`, advance `current_phase`, `--touch`; one `[{category}]` commit; stop and report.
