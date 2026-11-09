---
slug: files-column-priority
title: 'Files table: columns give way by priority, Name first'
kind: feature
appetite: small
status: done
branch: feat/files-column-priority
base: main
current_phase: done
last_updated: '2026-11-08'
phases:
- id: P1
  name: Columns give way by pane width, Name holds its cap
  status: done
  satisfies:
  - R1
  - R2
  - R3
  - R4
  - R5
  - R6
  - R7
  - R9
  depends_on: []
  parallel: false
  hammerable: false
  hill: uphill
  verify: cargo test -p okt-server -p okt-pygments -p okt-client && tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"'
- id: P2
  name: Name cell hover shows the full path; other views unchanged
  status: done
  satisfies:
  - R8
  - R10
  depends_on:
  - P1
  parallel: false
  hammerable: false
  hill: uphill
  verify: cargo test -p okt-server -p okt-pygments -p okt-client && tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"'
review:
  last_reviewed_commit: 54f9c24e1bb1b46a47b620dff579711835a05758
  verdict: approved
  blocked_reason: ''
  cycle: 2
---

# TECH.md — Files table: columns give way by priority, Name first

The **context engine and finite-state machine** for building this feature. The YAML
frontmatter above is the resume ground-truth (read it from the cycle worktree with
`uv run --with pyyaml python "$KB/bin/factory/next_phase.py" spec/files-column-priority/TECH.md`);
the per-phase checklists below are the work. `code-build` executes the next actionable phase, runs
its `verify:` command, updates state via
`uv run --with pyyaml python "$KB/bin/factory/set_phase.py" …`, and makes one atomic code+state
commit.

- **Vision / requirements (locked):** [`GOAL.md`](GOAL.md) — R-IDs are the contract.
- **Authoritative design:** [`PLAN.md`](PLAN.md).
- **Drafted gate:** [`verify/columns.mjs`](verify/columns.mjs), dry-run on the plan's prototype.
  Build folds its checks into `tests/ui/keys.mjs`; the copy in `keys.mjs` is the gate.

## Frontmatter field reference

- `status` (top): `planned | in_progress | blocked | in_review | done` (`done` is stamped by
  `code-publish` after confirmation, just before landing — the terminal state of the retained record)
- `appetite`: `small | big` — caps phase count and build-iteration budget (circuit breaker).
- `branch`, `base`: the cycle's branch (prefix per GOAL `kind`, from the profile's `branches`) and the
  profile's `base`.
- phase `status`: `pending | in_progress | done | blocked`
- `satisfies`: GOAL R-IDs this phase delivers (traceability anchor for `code-review`).
  **Artifact-deliverable R-IDs.** Some requirements are met by a *committed document* — a research
  audit, a protocol doc, an assessment — not by CLI-observable behaviour. Attach such an R-ID to the
  phase that produces and commits the artifact, and give that phase a content `verify:`
  (`test -f …`, a `grep -q` for the sections that must exist) rather than forcing a CLI drive.
  **Say so in the phase body**, because it changes who can grade it: `code-review` blinds its reviewer
  to all of `spec/`, so an R-ID whose evidence lives there is graded by the *orchestrator*, not the
  blind reviewer. Naming it in `TECH.md` is what lets the review skill route it correctly instead of
  reporting an unverifiable requirement.
- `depends_on`: phase ids that must be `done` first (a phase is actionable only when met).
- `parallel`: `true` only for genuinely independent, non-coupled work (docs, tests, isolated leaf
  modules). Any phase touching the profile's `coupled core` is always `false`.
- `hammerable`: `false` marks a correctness/security phase that scope-hammering must **never** cut.
- `hill`: `uphill` (still figuring it out) → `crest` (unknowns resolved) → `downhill` (just
  executing). A phase stuck `uphill` across builds is a raised hand → escalate to the human.
- `attempts`: durable failed-verify counter (absent = 0), bumped by `set_phase.py --phase P<n>
  --record-attempt` on every red verify gate; `next_phase.py` warns at ≥3 — the circuit breaker
  runs on this file, not on session memory.
- `verify`: the exact command that proves the phase (prefer driving the real CLI, not just tests —
  wrap CLI drives in the profile's `throwaway environment` helper, `<helper> sh -c "…"`, so they hit
  a disposable instance, never the developer's real data). A gate carrying `$( )` or quoting the
  calling shell would mangle is set with `set_phase.py --verify-file PATH`, not `--verify`.
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
  and every item the previous cycle's last phase carried for a project-level obligation (mpsearch's
  `## Times` quote, taken as `code-build` Step 4 says). A plan that drops one says why in `PLAN.md`.
- A PR body (when the profile's `landing` opens one) ends with an attribution naming the actual
  harness, model, and variant.
- A CLI/feature change obeys the profile's `same-commit rules` (§ Conventions in `AGENTS.md`): the
  help text, docs, or generated files it names change **in the same commit**.

Cycle-specific:

- **No spec ids** (`R1`, `P2`, `r10_`, any case) in CSS, Rust, or `keys.mjs` labels and names.
- **Stage by path** (`git add okt-client/static/app.css okt-client/src/views/tree.rs
  tests/ui/keys.mjs spec/files-column-priority/TECH.md`), never `git add -A`.
- **Running the gate.** `tests/env.sh` builds and serves on 6581 (never 6580); run it with the
  sandbox off and with `WASM_BINDGEN`, `PLAYWRIGHT_BROWSERS_PATH`, `PLAYWRIGHT_MODULE` and `TMPDIR`
  exported as the foreman's brief gives them. Never `make install`.
- **The `keys.mjs` section.** It goes last in `keys.mjs` (after FIND), on a fresh page in the same
  browser, so the fixtures it writes into `$OKT_KB` can't move the earlier checks' picks. Without
  `OKT_KB` (`keys.mjs` run against a real server) it prints one `skip` line and checks nothing;
  under `tests/env.sh` `OKT_KB` is always set. It refuses an `OKT_KB` that resolves to `~/KB`.
  Footprint: the one Chromium `keys.mjs` already launches; about 15 s more.

---

## Phase P1 — Columns give way by pane width, Name holds its cap
**Satisfies:** R1, R2, R3, R4, R5, R6, R7, R9 · **Depends on:** —
**Goal:** the Files table drops Title, Lines, Size, Changed and Type in that order as its pane
narrows, by the pane's width alone, with Name never under its cap while any of them shows, and the
gate that grades it on what renders.

- [x] `okt-client/src/views/tree.rs` `render()`: wrap the header and rows in
      `<div class="tcols">…</div>`; add `lines` and `size` to the Lines and Size cells (header,
      folder and file rows: `class="num lines"`, `class="num size"`); find the selected row
      among the wrapper's children (`tree.first_element_child()`), header still at index 0.
- [x] `okt-client/static/app.css`, files tree block: `@property --name-w`; `.tcols` as the
      `tcols` inline-size container with `font-size: calc(60ch + 35px)` and `--name-w: 1em`;
      `.tcols > .trow:not(.thead) { font-size: var(--fs) }`; the base `.trow` template
      `minmax(var(--name-w), 10fr) minmax(0, 9fr) 92px 56px 64px 60px 22px`; the five
      `@container tcols (width < calc(1em + N))` blocks of PLAN § 2, widest first. A comment
      above them says what `60ch` and `35px` are, that only `.trow`s go in `.tcols`, and that the
      offsets restate the column widths (change a width, change its offsets, TECH's thresholds
      and the `keys.mjs` table in the same commit). `60ch` is written nowhere else in the client,
      comments included.
- [x] `app.css` `@media (max-width: 900px)`: remove its three `.trow` lines; leave the rest.
- [x] Keep `.trow .nm .t`'s `overflow: hidden; text-overflow: ellipsis` and the `flex: none` on
      the indentation span, `.tw` and `.gl` (R7 rests on them).
- [x] `tests/ui/keys.mjs`: add the `FILES columns` section from `verify/columns.mjs`, every check
      except the hover block (P2 adds it): fixtures and their wait, the in-page reader, the
      sweep and its order, reverse and threshold checks, the cap check (one `ch` length, 60,
      in the source once, the moved-cap sweep graded, moves every threshold by 20 characters,
      restores), the splitter at 1200/760/480 with R9's facts, the stacked windows 900 → 380px,
      the uncommitted marker on the fixture, the 1540px split, the sticky header. Update the
      file's header comment to name the section.
- [x] The thresholds this phase states (R9's "thresholds TECH.md states"): a column shows while
      the table's inner width (`#tree.clientWidth`) is at least `60ch + 35px` plus its offset:
      Type 156, Changed 226, Size 300, Lines 366, Title 496 (px). On the plan's dry-run host
      (`ch` = 7.5px) that is 641, 711, 785, 851 and 981px, so a 760px pane shows Type and
      Changed and hides Size, Lines and Title. The gate checks every threshold against this
      formula within 2px.
- *Amendment (build):* the `app.css` comment names the threshold table in `tests/ui/keys.mjs` as
      the same-commit partner of the offsets, not TECH's thresholds: this file becomes a record
      when the spec merges, and the comment must point at what stays enforced.
- **Verify:** the profile's `verify`: `cargo test -p okt-server -p okt-pygments -p okt-client`,
  then `tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"'`.
- **Touches:** `okt-client/src/views/tree.rs`, `okt-client/static/app.css`, `tests/ui/keys.mjs`.

## Phase P2 — Name cell hover shows the full path; other views unchanged
**Satisfies:** R8, R10 · **Depends on:** P1
**Goal:** every Name cell, folder or file, cut or not, carries the row's full KB path as hover
text, and the rest of the client renders as before.

- [x] `okt-client/src/views/tree.rs` `render()`: `title="{esc(&r.id)}"` on the `.nm` span of
      folder rows and file rows. Nothing else carries a `title` inside `.nm`.
- [x] `tests/ui/keys.mjs` `FILES columns`: add the hover block from `verify/columns.mjs` (the
      element under the pointer at the Name text and at the glyph resolves to the row's full
      path at the text, glyph, twisty, the cell's far end and a nested indentation, for the
      titled, title-less, folder and nested fixtures, at 1200 and 480px) and the other-views
      smoke checks (Log and Table rows, a Results search with a visible row; no page errors).
- [x] R10: the full profile `verify` passes; every rule this cycle added to `app.css` is scoped
      to `.tcols`, `.trow` or `@container tcols` (read the cycle's diff of `app.css`).
- [x] Deferral ledger: walk both phases' bodies and commit bodies, prose and list items (not
      Markdown headings, and not this item), for anything left for later. Each one needs an
      `issues/{slug}.md` seed (from the factory's `ISSUE.md` template, `status: unshaped`) and a
      `ROADMAP.md` entry pointing at it. None is planned; an unrecorded one fails the phase.
- [x] No standing project-level last-phase items carry over: this is the repo's first factory
      cycle to reach a last phase.
- *Build notes:* the cycle's `app.css` diff adds only rules under `.tcols`, `.trow` and
      `@container tcols`, plus the `@property --name-w` registration, which is global by nature
      and set only on `.tcols`. The hover checks label each row by the tail of its path, since the
      folder and the nested file share their first 40 characters. Deferral ledger walked: nothing
      was left for later, so no seed.
- *Amendment (review cycle 1 remediation, 2026-11-08):* P2 reopened for REVIEW.md F1 and F2.
  - [x] F1: `AGENTS.md` § Build and test and `README.md` § Tests name what `keys.mjs` now covers:
        the Files columns section, its fixtures written only into `$OKT_KB` and its skip line
        outside `tests/env.sh`. `AGENTS.md` § Conventions gains two same-commit rules: the Files
        column widths, the `@container tcols` offsets and the `keys.mjs` threshold table (pointing
        at the `app.css` comment above `.tcols`, no numbers restated); and the other views' row
        tracks with their pinned rows in `keys.mjs` (`VIEWS`). Each of the three other-view
        sections in `app.css` gains a one-line comment pointing at the pin (comments only, no rule
        changes).
  - [x] F2: the other-views check grades layout, not presence. Log (`.crow`), Results (`.lrow`)
        and Paths (`.lsimple`, opened by `RECENT`) grade every row against a pinned track table
        (fixed widths, gap, border plus padding at the start, padding at the end; Log's height),
        and Table grades every row's cells under their headers, the table spanning the pane and
        each cell's text inset; at 1200 and 480px panes, with each list view's title asserted so
        the right list is graded. The class: a check that passes when the view merely shows.
        Negative controls (working-tree mutants of `app.css`, restored and `cmp`-confirmed):
        `.crow` 64px→30px reds the Log check; `.lrow` 48px→30px reds Results; `.lsimple` gap
        14→4px with `.tgrid td` padding 10→4px reds Paths and Table.
  - The one profile `verify` run on the final tree stands for the gate.
- **Verify:** the profile's `verify`, as P1.
- **Touches:** `okt-client/src/views/tree.rs`, `tests/ui/keys.mjs`; `issues/`, `ROADMAP.md`
  only if a deferral appears. Remediation: `AGENTS.md`, `README.md`, `okt-client/static/app.css`
  (comments), `tests/ui/keys.mjs`.

---

## How `code-build` drives this

1. `next_phase.py` prints the next actionable phase (statuses are authoritative; the
   `current_phase` pointer is reconciled against them).
2. Pre-flight: the cycle worktree is clean, on `branch`, and `base` is reachable.
3. Execute every `[ ]` in the phase (consult `PLAN.md` / `research/` for detail).
4. Run the phase's `verify:` command — never advance on a checkbox alone.
5. Amend this file freely if reality diverges (regenerate frontmatter with `set_phase.py`; note the
   amendment in the commit body). STOP and escalate only on a **`GOAL.md` contradiction**.
6. Mark the phase `done`, advance `current_phase`, `--touch`; one `[{category}]` commit; stop and report.
