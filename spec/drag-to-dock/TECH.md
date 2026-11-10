---
slug: drag-to-dock
title: Drag a view's tab to dock it in another pane
kind: feature
appetite: big
status: done
branch: feat/drag-to-dock
base: main
current_phase: done
last_updated: '2026-11-10'
phases:
- id: P1
  name: The drop model, pure
  status: done
  satisfies:
  - R3
  - R4
  - R5
  - R7
  - R8
  depends_on: []
  parallel: false
  hammerable: false
  hill: uphill
  verify: sh spec/drag-to-dock/verify/gate.sh P1
- id: P2
  name: Drag, overlay, drop and cancel
  status: done
  satisfies:
  - R1
  - R2
  - R3
  - R4
  - R5
  - R6
  - R7
  - R11
  depends_on:
  - P1
  parallel: false
  hammerable: false
  hill: uphill
  attempts: 1
  verify: sh spec/drag-to-dock/verify/gate.sh P2
- id: P3
  name: Focus, the key view, state and persistence after a drop
  status: done
  satisfies:
  - R8
  - R9
  - R10
  depends_on:
  - P2
  parallel: false
  hammerable: false
  hill: uphill
  attempts: 1
  verify: sh spec/drag-to-dock/remediation/gate.sh P3
- id: P4
  name: The suite in the repo, docs and the ledger
  status: done
  satisfies:
  - R11
  - R12
  depends_on:
  - P3
  parallel: false
  hammerable: false
  hill: uphill
  verify: sh spec/drag-to-dock/remediation/gate.sh P4
review:
  last_reviewed_commit: '3720403'
  verdict: approved
  blocked_reason: ''
  cycle: 3
---

# TECH.md — Drag a view's tab to dock it in another pane

The **context engine and finite-state machine** for building this feature. The YAML
frontmatter above is the resume ground-truth (read it from the cycle worktree with
`uv run --with pyyaml python "$KB/bin/factory/next_phase.py" spec/drag-to-dock/TECH.md`); the per-phase
checklists below are the work. `code-build` executes the next actionable phase, runs its
`verify:` command, updates state via
`uv run --with pyyaml python "$KB/bin/factory/set_phase.py" …`, and makes one atomic code+state commit.

- **Vision / requirements (locked):** [`GOAL.md`](GOAL.md) — R-IDs are the contract.
- **Authoritative design:** [`PLAN.md`](PLAN.md).
- **Backing research:** [`research/00-digest.md`](research/00-digest.md) + briefs.

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

- **Built on preset-bar: rebase first.** P1 starts by rebasing `feat/drag-to-dock` onto `main`
  once preset-bar has landed there (`git fetch` is the foreman's; rebase onto the local `main` the
  foreman fast-forwarded). preset-bar removes the `nav.fkeys` row, `#presets`, and the preset
  marking loop in `panes::apply` (`for b in dom::qa("#presets [data-preset]")`). It moves
  `#crumbs` into the header, so `#panes` is 918 px tall at 1000 px. If preset-bar hasn't landed
  when P1 starts, P1 may build on the current base, because it is pure. Its gate doesn't need
  preset-bar, but the rebase is then P2's first step. P2's static part is red until it is done. The
  plan commit's hash changes with the rebase; the gate finds it by subject.
- **Running the gate.** Sandbox off. Export before every `verify:`:
  ```
  export OKT_TEST_PORT=6587 WASM_BINDGEN=/tmp/claude-501/okt-tools/bin/wasm-bindgen PLAYWRIGHT_BROWSERS_PATH=/tmp/claude-501/ms-playwright PLAYWRIGHT_MODULE=/Users/geoffrey/Software/github.com/glentner/open-knowledge-terminal/tests/node_modules/playwright/index.mjs TMPDIR=/tmp/claude-501
  ```
  Port 6587 is this cycle's; never 6580. Never `make install`. Run the full gate through the
  harness's background runner (about 40 min from P2, § Gate timings) and read its exit
  status. Each part's full log stays in `$REPO/.local/drag-to-dock/drag-to-dock-P<n>-<part>.log`,
  and the summary in `…/drag-to-dock-P<n>.log`, where `$REPO` is the main checkout. Quote failures
  from there.
- **Targeted runs.** `gate.sh P<n> --part dock-a --groups "sweep-three-1600"` reruns one part
  narrowed to probe groups (prefixes allowed, e.g. `sweep`). It prints "not a phase verdict". Use
  it for mutants and for iterating; the phase is green only on a full `gate.sh P<n>`.
- **Stage by path before `verify:`.** The static part reads staged state and fails on unstaged or
  untracked files. Stage each file by name; never `git add -A` or `git add .`.
- **No spec ids** (`R1`, `P2`, `Q14`, `r10_` in any case) in any source, test name, label or file
  name. The static part scans added lines under `okt-client/` and `tests/` for
  `(^|[^A-Za-z0-9])[RrPpQq][0-9]{1,3}([^A-Za-z0-9]|$)`. Pane geometry invites `r1`, `p0` and `q1`;
  name locals `a`/`b`, `lo`/`hi`, `first`/`second`.
- **Never trust what the code meant to draw.** The probe grades from `getBoundingClientRect` and
  `checkVisibility`; `data-*` attributes only find elements. No client hook reports a drop, a zone
  or an overlay rect: the overlay is the element `#dock`, measured.
- **Borrow rule** (AGENTS.md § Invariants 1) for every new handler (PLAN § 2.4). Take the drag
  state out of its `thread_local` first. Then call `set_pointer_capture`, `panes::focus`,
  `nav::operate`, a toast, show or hide `#dock`, or add or remove a listener. Then store the next
  state. The `operate` closure captures only values.
- **Coupled core** (`app.css` with `graph.rs`; `state.rs`): P2 touches `app.css`, so phases are
  `parallel: false`. No graph paint rule changes, so no commit pairs `app.css` with `graph.rs`.
  `state.rs` is not changed at all; the static part fails if it is.
- **Frozen files.** `spec/drag-to-dock/verify/` is the planner's gate: `gate.sh`, `static.sh`,
  `ledger.sh`, `dock.mjs`, `dock-oracle.mjs`, `dock-oracle-test.mjs`, `checks.txt`, `required.txt`.
  The static part fails while it differs from the plan commit. Build never edits it to make a phase
  pass. From P3 the gate is `spec/drag-to-dock/remediation/gate.sh`, with its `static.sh` and
  `dock.mjs`, which supersede `verify/`'s and are frozen with it (PLAN § 6). A real defect in it is a stop-and-report to the foreman, and the amendment lands as a new
  `[feature] Plan drag-to-dock: …` commit.
- **Mutants** (PLAN § 6 lists each phase's decisions). Plant each in the working tree with a backup
  copy outside `$WT`. Run only the part and groups that should go red. Confirm red, restore, and
  `cmp`. Record each mutant and its killing label in the phase's commit body. A survivor is a
  STOP: either the gate or the design is missing a case. Report it.

### Gate timings

Measured on the plan's dry runs (2026-11-09, mac, warm, today's page with no drag feature; logs
`$REPO/.local/drag-to-dock/plan-dryrun-today*.log`):
- env.sh build, clone and server: about 40 s after a source change, 15–20 s when nothing changed.
- `dock.mjs` with `sweep-classic-1600 sweep-three-900 click cancel-1600` (30 + 60 drops, 48 tabs,
  78 cancels, the first draft): 9 min 26 s with setup, about 1.5–1.8 s per drop or cancel.
- The revised probe (second dry run): `sweep-stacked-900` 163 s for 36 drops; `sources` 112 s;
  `keyview` 206 s; `state` 58 s. On today's page every valid drop waits out two 2.5 s save timeouts
  that a working drop never reaches, so these are upper bounds.
- The gate on the plan commit: `gate.sh P1` 36 s, red at "required tests 0/8"; `gate.sh P2
  --static` red at preset-bar and README § Keys; `gate.sh P2 --part dock-a --groups
  sweep-classic-1600` 102 s, red at the three P2 labels with today's page (the P3 labels noted).
- `panes.mjs` about 7 min and `keys.mjs` about 30 s (research 03).

Measured on P2's gate (2026-11-10, mac, `gate.sh P2` on the P2 tree; summary
`$REPO/.local/drag-to-dock/drag-to-dock-P2.log`). A valid drop now saves at once, so the dry runs'
two 2.5 s save timeouts per drop are gone:
- `static` 0 s, `unit` 164 s, `oracle` 0 s.
- `suites` 535 s (8 min 55 s; `keys.mjs` and `panes.mjs`, each with its env.sh build). Under the
  9-minute line by 5 s, so not split; it is the part to split first (one suite per part) if it
  grows.
- `dock-a` 289 s: the 1600 sweeps 50, 50, 50, 102 s and `sources` 34 s.
- `dock-b` 257 s: the 900 sweeps 51, 50, 50, 103 s.
- `dock-c` 252 s: `cancel-1600` 124 s, `cancel-900` 125 s.
- `dock-d` 382 s at P2 (`click` 379 s). From P3 the superseding gate leaves it `click` alone:
  382 s on the P3 tree.
- `dock-e` 127 s at P2 (`refuse` 125 s). From P3 the superseding gate adds `state` and `keyview`:
  357 s on the P3 tree (`refuse` 125 s, `keyview` 177 s, `state` 52 s).
- The full gate: 2 026 s, about 34 min. Targeted runs: one sweep group about 70 s with the
  build, `cancel-1600` about 130 s, `click` about 6.5 min.

Measured on P3's gate (2026-11-10, mac, `remediation/gate.sh P3` on `7fd392a`; summary
`$REPO/.local/drag-to-dock/drag-to-dock-P3.log`), with `keyview` moved to `dock-e`:
- `static` 1 s, `unit` 160 s, `oracle` 1 s.
- `suites` 523 s (8 min 43 s), under the 9-minute line; not split.
- `dock-a` 287 s (50, 50, 50, 101 s; `sources` 34 s); `dock-b` 253 s (50, 50, 49, 101 s);
  `dock-c` 250 s (124, 124 s).
- `dock-d` 381 s (`click` 378 s).
- `dock-e` 357 s (`refuse` 125 s, `keyview` 177 s, `state` 52 s).
- The full gate: 2 213 s, about 37 min. Targeted: `--part dock-e --groups keyview` 192 s.

---

## Phase P1 — The drop model, pure
**Satisfies:** R3, R4, R5, R7, R8 (the model; the browser grades them again in P2) · **Depends on:** —
**Goal:** `arrange.rs` can say, for any tree, view, target pane, zone and `#panes` size, whether a
drop is valid and exactly what tree and overlay rect it gives. The tests prove it exhaustively over
small trees and agree with the planner's hand-worked cases.

- [x] Rebase onto `main` if preset-bar has landed (§ Conventions); otherwise note in the commit
      body that P2 rebases.
- [x] Promote the tests module's `rects`/`layout` to `pub struct Rect` and `pub fn layout(root, w, h)
      -> Vec<(PaneId, Rect)>` (PLAN § 2.2). Move the existing tests onto it; they stay green.
- [x] `pub enum Zone`, `pub fn zone_at`, `pub fn aim` with PLAN § 2.2's geometry: title bar and
      collapsed panes are the centre, quarter bands, the nearest edge, ties left, right, top, bottom.
- [x] `pub fn docked`, `pub fn dock`, `pub fn preview` as PLAN § 2.3 orders them: invalid outright;
      take out; join or split (`f` 0.5, the new pane on its side, id `max + 1`); lift an emptied
      source; validate the whole result (no empty pane, each view once, no two collapsed panes in a
      split, no collapsed root, both halves of an edge split at the floors on the result's layout).
      `move_view` and the other operations are unchanged.
- [x] The eight native tests in `verify/required.txt`, with the domain and assertions of PLAN § 2.6.
      Each sweep asserts its own case count (at least 100 000 drops).
- [x] P1 mutants (PLAN § 6): each turns `--part unit` red. Record them in the commit body.
- **Verify:** `sh spec/drag-to-dock/verify/gate.sh P1` (static, unit, oracle; about 1 min).
- **Touches:** `okt-client/src/arrange.rs`.

**Amendments at build (2026-11-09):**
- **Order.** preset-bar had not landed (`main` at `50c9533`). By the foreman's ruling at the PLAN
  gate, P1 is pure and was built on the plan's base without the rebase. The rebase onto `main` is
  P2's first step, as § Conventions already says for this case. P1 touches no `app.css`,
  `index.html` or `panes.rs`.
- **The sweep domain (PLAN § 2.6).** The full product PLAN names (about 125 700 arrangements and
  58 million drops per sweep) took 24 minutes under `cargo test`'s debug profile. The domain is now:
  every geometry of one to three panes (each shape, `Row`/`Col` and `f` ∈ {0.5, 0.3} per split, each
  collapse set the accordion allows) with every dealing (one or two views a pane, each shown tab);
  and every geometry of four panes, all 3 648 of them, with one dealing each, taken in turn, so every
  dealing meets about 110 four-pane geometries. That is 8 943 arrangements and 3 643 560 drops per
  sweep, × 6 views × every pane × 5 zones × the four sizes. The panes' ids run 1, 3, 5, 7 in reading
  order, as they stand after a pane closes, so a new pane's id is held to `max + 1` and never to the
  count. `layout_tiles_the_root` runs over the 3 853 geometries × 4 sizes. The 7 px grid of
  `aim_finds_the_pane_and_zone_under_a_point` covers every geometry of up to three panes at all
  four sizes, and the four-pane ones at 430 × 250. That is the cost of a 7 px grid over 3 853
  layouts at 1600 px.
- **`aim`** checks the point is inside the root first, then walks down the splits. A pane's right
  edge can sum to a hair over the root's width (`0.3 · 900 + …`), so a lookup over `layout`'s rects
  found a pane at `x = w`. "`None` outside `#panes`" is PLAN § 2.2's rule, so this is a fix to the
  build, not to the plan.
- **Gate time.** `gate.sh P1` takes about 165 s: the unit part runs the profile's `cargo test`, then
  the eight by name, each about 80 s. That replaces "about 1 min".
- **Mutants.** All 13 of PLAN § 6's P1 mutants were planted, and 12 turned `--part unit` red. "A
  collapsed target's edges allowed" survived, as an equivalent mutant. A collapsed pane keeps its
  24 px in the split an edge would make, under both floors, so the floor check refuses the drop
  anyway, and the property holds with or without the explicit check. A variant that allows the edge
  and expands the pane as it splits was killed (`dock_sweep_matches_the_written_rules`,
  `dock_matches_the_hand_worked_cases`). The explicit check stays as PLAN § 2.3 orders it.
- **Warnings.** The wasm build warns that the new items are unused until P2's handlers call them.

## Phase P2 — Drag, overlay, drop and cancel
**Satisfies:** R1, R2, R3–R5 and R7 in the browser, R6, R11 · **Depends on:** P1
**Goal:** a real pointer drag of any tab shows the overlay over valid zones only, drops exactly
there, and every way of not dropping leaves everything as it was. A click is still a click, and
every key and existing suite is unchanged.

- [x] If not yet done, rebase onto `main` with preset-bar (§ Conventions).
- [x] `index.html`: `<div class="dock" id="dock" hidden></div>` at the end of `<body>`.
      `app.css`: `.dock` fixed, `pointer-events: none`, above the sashes and under `#help`, a
      translucent accent fill with a border, `box-sizing: border-box`; `.ptitle { user-select: none }`;
      `.ptab { touch-action: none }`.
- [x] `panes.rs`: the `TAB` state machine of PLAN § 2.4 (Idle, Pending, Dragging, Cancelled): the
      4 px threshold, capture on the tab, `aim` and `preview` on the snapshot tree from `#panes`'s
      rect, the overlay. On `pointerup` the drop goes through
      `nav::operate(move |n, _| arrange::dock(…))`, guarded by "the tree is unchanged since the
      drag began". Cancels on `pointercancel` and on `lostpointercapture` (on `document`). The
      one-shot click swallower (capture-phase on `window`, `detail > 0`, removed by
      `setTimeout(0)` or on use). Button 0 and the primary pointer only.
- [x] `keys.rs`: the capture-phase `keydown` on `window` (modifier alone: nothing; Esc: cancel and
      stop; any other key: cancel and go on). `blur` cancels beside the leader's.
- [x] § Invariants 1, handler by handler: no `TAB`, `DRAG`, `cfg()` or `s()` borrow across a
      re-entrant call (PLAN § 2.4's list). Name each handler's order in the commit body.
- [x] README § Keys, the PANES item: one sentence. Drag a tab onto another pane to add it there, or
      onto a pane's edge to split it. An outline shows where it lands, and Esc cancels. Same commit
      as the drag (same-commit rule).
- [x] P2 mutants (PLAN § 6), each with the narrowest `--part`/`--groups` that kills it.
- [x] Replace § Gate timings' estimates with this phase's measured part times.
- **Verify:** `sh spec/drag-to-dock/verify/gate.sh P2` (all parts; P3's labels in the sweeps are
  noted, not graded).
- **Touches:** `okt-client/src/panes.rs`, `okt-client/src/keys.rs`, `okt-client/static/index.html`,
  `okt-client/static/app.css`, `README.md`.

**Amendments at build (2026-11-10):**
- **The rebase.** preset-bar landed and `main` was released as 0.4.1 (`6097aa9`).
  `feat/drag-to-dock` was rebased onto it with no conflict: git merged main's Notes lines in
  `issues/drag-to-dock.md` with the branch's `status:` line, and main's removal of
  `Preset::name`/`parse` (and their test line) with P1's `arrange.rs` hunks. Every rebased commit
  is re-signed. `gate.sh P1` is green on the rebased tree (static, unit 165 s with required tests
  8/8, oracle).
- **Lost capture, watched each frame.** Chromium fires `lostpointercapture` for a
  `releasePointerCapture` (and for a removed tab) only with the next pointer event, while
  `hasPointerCapture` reads false at once (measured: release at 153 ms, the event at 711 ms after
  a move). A drag that lost its capture kept its outline until the pointer moved, and the probe's
  lost-capture cancel went red (attempt 1). While a drag is under way, a `requestAnimationFrame`
  watch reads `hasPointerCapture` on the tab and cancels when it is false. The `lostpointercapture`
  listener on `document` stays as PLAN § 2.4 has it. The watch only reads state and calls the
  cancel, and stops when the drag ends (a drag counter ends a stale frame); the outline's position
  is still set synchronously in `pointermove`.
- **The drag state** has no `pane` or `aim` field: the press has already focused the pane, and the
  release is aimed as a move is (PLAN § 2.4, "process the up point as a move first"), so a stored
  aim is never read. `Cancelled` carries no pointer id.
- **Mutants.** PLAN § 6's eleven, and a twelfth for the watch, each killed by its own part and
  group; listed in the commit body.

## Phase P3 — Focus, the key view, state and persistence after a drop
**Satisfies:** R8 (through `operate`, graded in the browser), R9, R10 · **Depends on:** P2
**Goal:** after every drop, focus is on the dropped view's pane, and the view the keys act on is
drawn, on screen or off. Moved views keep their scroll, the selection and the Inspector's tab. The
drop is saved, survives a reload, replaces the hash and pushes no history. The probe exits 0.

- [x] Run the P3 groups (`--part dock-e --groups keyview`, `--part dock-e --groups state`) and the
      sweeps' focus, fit and persist labels. Fix each route found where PLAN § 2.5 says it lives.
      Fix it in `panes.rs` (the key view is decided in `apply`, as pane-arrangements left it)
      before `nav.rs`, and never in `state.rs`. Fix the property, not the instance: a fix names
      the class of routes it closes.
- [x] If nothing needs fixing, the phase is the evidence: record in the commit body which labels
      went green with no code change.
- [x] P3 mutants (PLAN § 6).
- **Verify:** `sh spec/drag-to-dock/remediation/gate.sh P3` (every label, and the probe exits 0).
- **Touches:** `okt-client/src/panes.rs` and, only if needed, `okt-client/src/nav.rs`.

**Stopped at build, attempt 1 (2026-11-10):**
- **Done.** `#[cfg(test)]` on `Zone::ALL` in `arrange.rs` (the P2 gate ruling); the wasm build
  now has no warning. No route in `panes.rs` or `nav.rs` needed a fix: `keyview` is green as built
  (18 drops, 10 with the key view off screen, 172 s), and so is `state`'s back-and-forward label
  (6 drops). P2's full gate already printed `ok` for the sweeps' focus, fit and persist labels, and
  every dock part exited 0.
- **Red, outside the phase's work: the probe's `state` precondition.** "A moved view keeps its
  scroll…" is red on its four Files and Inspector cases, each with `landed: true`, nothing
  misdrawn and `before.top: 0`. Measured on the test server, before any drop: `#tree` (the KB
  root's rows) has `scrollHeight` 859 = `clientHeight` 859 at classic and 835 = 835 at three, and
  `#ibody` (README.md's History, 11 commits) has 781 = 781 and 757 = 757. Neither can scroll, so
  the wheel leaves `scrollTop` at 0 and `before.top > 0` can't hold whatever the page does. The
  Log's `#ltable` (32 309 px) scrolls, and its two cases pass. The plan's dry run shows the same
  `before.top: 0`, hidden there because nothing landed. `verify/` is frozen, so this goes back to
  the foreman as a plan amendment.
- **Mutants** (PLAN § 6), each planted from a backup, confirmed landed, restored with `cp` and
  `touch`, and checked with `cmp`:
  - no `save_settings` in the drop path: killed by `sweep-stacked-1600` ("each drop gives the
    defined tree, drawn and saved");
  - focus left on the source pane (`dock`'s `focus: None`): killed by `sweep-stacked-1600`
    ("after a drop the dropped view's pane is focused…");
  - no `draw_key_view` after `operate` (removed from `panes::apply`): **survives `keyview`**. Every
    `keyview` route draws its key view on screen before the drop, and the moved root keeps what it
    drew. The gate does kill it, at `suites` (four `panes.mjs` labels, each a Log never drawn:
    collapsed at load, or under the floor at boot), in 533 s.
- **Times, by group alone:** `keyview` 172 s (part run 175 s), `state` 45 s (part run 47 s). With
  P2's `click` 379 s, `dock-d` comes to about 555 s, over the 9-minute line; `dock-e` with
  `refuse` 125 s about 172 s. Not yet measured as full parts.

**Plan amendment (2026-11-10, the foreman's P3 stop):**
- **The gate is superseded, not edited.** P3's and P4's `verify:` run
  `spec/drag-to-dock/remediation/gate.sh`. Its `dock.mjs` changes `state` and `keyview` only; its
  `static.sh` compares the P4 copy with that probe and freezes the three files with `verify/`,
  which is unchanged. PLAN § 6 says what each change does.
- **Dry runs on the P3 tree** (`b584fe9`, the superseding gate):
  - `state`: the precondition held in all six cases, each scrolled to `before.top` 240. Files
    opened `contact/` (2 606 px of room at classic, 2 630 at three), the Inspector stepped on to
    `AGENTS.md` (571 and 595 px), and the Log had 31 743 and 31 767 px. All three labels are green:
    each moved view kept `top` 240, its selection and the History tab.
  - `keyview`, with `draw_key_view` removed from the end of `panes::apply` (planted from a backup,
    confirmed landed, restored with `cp` and `touch`, checked with `cmp`): red, on the new route
    alone. The Log arrived never drawn and the drop landed, but its keys stepped nothing. Restored,
    green: 19 drops, 11 off screen.
  - `--part dock-d` 382 s and `--part dock-e` 357 s, both green and under 9 minutes.

**Resumed and green (2026-11-10, attempt 2, on the superseding gate):**
- No product code changed for P3 beyond `Zone::ALL`'s `#[cfg(test)]` (attempt 1). Every P3 label
  went green as P2 built it: the eight sweeps' and `sources`' and `refuse`'s focus, fit and
  persist labels; `keyview` (19 drops, 11 off screen, the never-drawn Log route among them); and
  `state`'s three (the scroller scrolled in 6 of 6, each moved view kept its scroll, selection and
  History tab, back and forward never undid the arrangement). Every probe part exited 0.
- The `draw_key_view` mutant, re-run against `remediation/gate.sh P3 --part dock-e --groups
  keyview` (planted from a backup, confirmed landed, restored with `cp` and `touch`, `cmp` and
  `git diff` clean): red on the new route alone ("classic 420: the Log, never drawn, onto the
  Inspector", its keys stepping nothing), green restored, 192 s each. With attempt 1's two
  `sweep-stacked-1600` kills, all three P3 mutants die in the groups PLAN § 6 names.

## Phase P4 — The suite in the repo, docs and the ledger
**Satisfies:** R12 (with R11 regraded) · **Depends on:** P3
**Goal:** the browser suite R12 names lives in the repo as `tests/ui/dock.mjs`, identical to the
frozen probe, which has graded every phase since P2. README describes it, and the ledger check
passes.

- [x] Copy the superseding probe `spec/drag-to-dock/remediation/dock.mjs` to `tests/ui/dock.mjs`,
      and `spec/drag-to-dock/verify/dock-oracle.mjs` to `tests/ui/dock-oracle.mjs`, byte for byte
      (the static part compares them).
- [x] README § Tests: a run line (`tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/dock.mjs"'`) and a
      paragraph saying what `dock.mjs` grades and that `DOCK_GROUPS` narrows it. § Layout's
      `tests/ui/` row names the drag.
- [x] The ledger (`verify/ledger.sh`): walk P1–P3's bodies and commit bodies for the words it
      matches. Each such item has an `issues/{slug}.md` from the ISSUE template plus its
      `ROADMAP.md` entry. The script checks TECH, PLAN and REVIEW prose.
- **Verify:** `sh spec/drag-to-dock/remediation/gate.sh P4` (everything; the static part checks
  the copies, README § Tests and the ledger).
- **Touches:** `tests/ui/dock.mjs`, `tests/ui/dock-oracle.mjs`, `README.md`, and `issues/` with
  `ROADMAP.md` when a seed is filed.

**Built (2026-11-10):**
- **The copies.** `tests/ui/dock.mjs` is `remediation/dock.mjs` byte for byte, its superseding
  header included (the foreman's ruling), and `tests/ui/dock-oracle.mjs` is `verify/dock-oracle.mjs`.
  Seen red once: a staged byte added to the copy turned the static part red ("differs from
  spec/drag-to-dock/remediation/dock.mjs"), and a § Tests with the name changed turned it red
  ("README § Tests does not name dock.mjs"). Both were restored from backups, re-staged and
  checked with `cmp`.
- **README.** § Tests has the run line and a paragraph on what `dock.mjs` grades, that
  `DOCK_GROUPS` (or group names as arguments) narrows it, and that a prefix selects every group
  it begins. § Layout's `tests/ui/` row names dragging a tab to dock it.
- **The ledger.** Its words were walked through P1–P3's TECH and PLAN prose and the bodies of the
  13 branch commits. The one hit in TECH is § Conventions' line naming the ledger check, which the
  script's negation removes. The commit bodies hold none of its words, and there is no REVIEW.md
  yet. GOAL's two hits (the earlier cycle's non-goal it quotes, and Q7's scope ruling) are locked
  scope decisions outside the script's files, not work this cycle set aside. No seed was needed,
  and nothing was reworded. The script's self-test passes.
- **Times** on P4's gate (`remediation/gate.sh P4` on `b60d630` with the P4 files staged; summary
  `$REPO/.local/drag-to-dock/drag-to-dock-P4.log`): `static` 0 s, `unit` 158 s, `oracle` 0 s,
  `suites` 524 s, `dock-a` 288 s, `dock-b` 257 s, `dock-c` 253 s, `dock-d` 381 s, `dock-e` 356 s;
  2 217 s in all, about 37 min.

**Remediation, review cycle 1 (2026-11-10):**
- [x] F1: `AGENTS.md` describes every module, suite and on-disk artifact the branch added or
      moved. Routed as that property: each item of `git diff --stat origin/main...HEAD -- .
      ':(exclude)spec/'` and each new pub item and event handler was checked against `AGENTS.md`.
      Build and test now names `tests/ui/dock.mjs` and `tests/ui/dock-oracle.mjs`: what they
      grade, that a run needs `tests/env.sh` for `OKT` and `OKT_KB` though it writes nothing to
      the clone, about half an hour in full, and `DOCK_GROUPS`. The Map's `arrange.rs` describes its
      layout and the drop model (`aim`, `zone_at`, `docked`, `dock`, `preview`, `Rect`, `Zone`);
      `panes.rs` names the tab drag's pointer states, the `#dock` outline (standing for `index.html`'s element and
      `.dock` in `app.css`) and the click swallower; `keys.rs`'s capture-phase `keydown` on `window` and
      the window's `blur` are named as cancelling a drag. § Invariants is unchanged: nothing in it
      became wrong.
- **Gate, by the foreman's ruling.** Only `AGENTS.md` and `spec/` change, and no gate part reads
  `AGENTS.md`, so P4 is graded by `remediation/gate.sh P4 --static` and `verify/ledger.sh` on this
  tree. Every other part's inputs are byte-identical to `677899d`, whose full `gate.sh P4` was
  green: `git diff --stat 677899d HEAD -- . ':(exclude)spec/'` names only `AGENTS.md`.

**Remediation, review cycle 2 (2026-11-10):**
- [x] F2: every claim `AGENTS.md` and `README.md` make about what `tests/ui/dock.mjs` and
      `tests/ui/dock-oracle.mjs` grade matches what the code runs. Routed as that property: each
      claim was checked against `dock.mjs` (`sources()`, `AIMS`, `WIDTHS`, `PRESETS`, the group
      list, the `DOCK_GROUPS` and argument parsing, `OKT` and `OKT_KB`) and against the case counts
      of three runs: P4's (`$REPO/.local/drag-to-dock/drag-to-dock-P4-dock-*.log`), review cycle
      2's (`$WT/.local/review2/dock-*.log`), and `sweep-classic-900` with `sources` on port 6587
      (36 and 28 cases). Now written as the code runs: the sweep drags each pane's shown tab and
      the last tab it doesn't show onto six points of every pane, aimed at its five zones and its
      title bar (36 drops a preset at each width, 72 for `three`); both files name the `sources`
      group (every view out of a collapsed strip, and views onto one); README's fit check names
      every shown view, and its cancels name what they hold as it was (the panes, saved settings,
      hash and history); a full run is about 25 minutes (both runs' group times sum to 1 520 s),
      where both said half an hour. Every other claim was true as written. `tests/ui/`, `verify/`
      and `remediation/` are untouched.
- **Gate, by the foreman's ruling.** Only `AGENTS.md`, `README.md` and `spec/` change. The gate
  reads `AGENTS.md` nowhere, and `README.md` only in the static part's § Keys and § Tests checks,
  which run here. So P4 is graded by `remediation/gate.sh P4 --static` and `verify/ledger.sh` on
  this tree; every other part's inputs are byte-identical to `677899d`, whose full `gate.sh P4`
  was green: `git diff --stat bf4eddf HEAD -- . ':(exclude)spec/'` names only `AGENTS.md` and
  `README.md`.

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
