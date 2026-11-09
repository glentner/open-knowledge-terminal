---
slug: pane-arrangements
title: 'Pane arrangements: place, swap, stack and collapse views'
kind: feature
appetite: big
status: done
branch: feat/pane-arrangements
base: main
current_phase: done
last_updated: '2026-11-09'
phases:
- id: P1
  name: The arrangement frame, default and focus
  status: done
  satisfies:
  - R1
  - R3
  - R6
  - R7
  - R8
  - R9
  - R11
  depends_on: []
  parallel: false
  hammerable: false
  hill: uphill
  attempts: 1
  verify: mkdir -p .local/pane-arrangements && sh spec/pane-arrangements/verify/gate.sh P5 2>&1 | tee .local/pane-arrangements/gate-P5.log; sh spec/pane-arrangements/remediation/F1-key-view-drawn.sh .local/pane-arrangements/gate-P5.log
- id: P2
  name: Presets, collapse and their keys
  status: done
  satisfies:
  - R4
  - R5
  - R7
  - R10
  - R17
  depends_on:
  - P1
  parallel: false
  hammerable: false
  hill: uphill
  verify: sh spec/pane-arrangements/verify/gate.sh P2
- id: P3
  name: Swap, flip, close and move
  status: done
  satisfies:
  - R1
  - R7
  - R12
  - R13
  - R14
  depends_on:
  - P2
  parallel: false
  hammerable: false
  hill: uphill
  verify: sh spec/pane-arrangements/verify/gate.sh P3
- id: P4
  name: Each view fits its pane
  status: done
  satisfies:
  - R2
  - R15
  - R16
  depends_on:
  - P3
  parallel: false
  hammerable: false
  hill: uphill
  attempts: 1
  verify: sh spec/pane-arrangements/verify/gate.sh P5
- id: P5
  name: What Geoffrey sees, graded in full, and the ledger
  status: done
  satisfies:
  - R17
  depends_on:
  - P4
  parallel: false
  hammerable: false
  hill: uphill
  verify: sh spec/pane-arrangements/verify/gate.sh P5
review:
  last_reviewed_commit: 00b8783
  verdict: approved
  blocked_reason: ''
  cycle: 5
---

# TECH.md — Pane arrangements: place, swap, stack and collapse views

The **context engine and finite-state machine** for building this feature. The YAML
frontmatter above is the resume ground-truth (read it from the cycle worktree with
`uv run --with pyyaml python "$KB/bin/factory/next_phase.py" spec/pane-arrangements/TECH.md`); the
per-phase checklists below are the work. `code-build` executes the next actionable phase, runs its
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

- **Running the gate.** Sandbox off. Export before every `verify:`:
  ```
  export OKT_TEST_PORT=6584 WASM_BINDGEN=/tmp/claude-501/okt-tools/bin/wasm-bindgen PLAYWRIGHT_BROWSERS_PATH=/tmp/claude-501/ms-playwright PLAYWRIGHT_MODULE=/Users/geoffrey/Software/github.com/glentner/open-knowledge-terminal/tests/node_modules/playwright/index.mjs TMPDIR=/tmp/claude-501
  ```
  Port 6584 is this cycle's; never 6580. Never `make install`.
- **Stage by path before `verify:`.** The gate reads staged state and fails on unstaged or untracked
  files under `okt-client/`, `tests/`, `README.md` and `AGENTS.md`. Stage each file by name; never
  `git add -A` or `git add .`.
- **No spec ids** (`R1`, `P2`, `r10_` in any case) in any source, test name, label or file name. The
  static check scans added lines with `(^|[^A-Za-z0-9])[RrPp][0-9]{1,3}([^A-Za-z0-9]|$)`. Pane
  geometry is where `r1`, `p0`, `p1` would creep in, so name locals `a`/`b`, `lo`/`hi`, `first`/`second`.
- **Never trust what the code meant to draw.** `tests/ui/panes.mjs` grades from
  `getBoundingClientRect` and `checkVisibility`; `data-*` attributes find elements, and each is
  cross-checked against geometry. No client hook reports "drawn" or "laid out".
- **History-API checks open a fresh page**, because Chromium caps `history.length` at 50.
- **Borrow rule** (AGENTS.md § Invariants 1) for every new path. Run the pure `arrange` operation
  inside a short `cfg_mut()` block, then drop it. Only then move DOM, `save_settings()`, render,
  `nav`, and call `dom::focus` last. Treat a view-root move like `focus()`, because `appendChild`
  fires `blur` synchronously. Bind `s()` reads with `let` before an `if let` or `match`.
- **Coupled core** (`app.css` with `graph.rs`; `state.rs`): every phase touches `app.css` or
  `state.rs`, so every phase has `parallel: false`. No graph paint rule changes in this cycle; no
  commit has to pair `app.css` with `graph.rs` for that reason.
- **Frozen files.** `spec/pane-arrangements/verify/` is the planner's gate: `gate.sh`, `static.sh`,
  `ledger.sh`, `kept.mjs`, `rects.mjs`, `checks.txt`, `required.txt`. `static.sh` fails while it
  differs from the plan commit. Build never edits it to make a phase pass. A real defect in it is a
  stop-and-report to the foreman, and the amendment lands as a new
  `[feature] Plan pane-arrangements: …` commit.
- **graph.rs locals.** `graph.rs` has geometry locals named `r0`, `p1`, `p2` and `p92`. Don't reflow
  those functions; if one must change, rename its locals, because the spec-id scan reads added
  lines.

### Gate timings (measured on the prototype dry run, 2026-11-08)

The prototype is `.worktrees/pane-arrangements-prototype`: `origin/main` plus a local, unsigned
dry-run commit of this plan.

| Part | Measured |
|---|---|
| step 1, `static.sh` | under 2 s |
| step 2, `cargo test` | 4 s warm (about 14 s cold) |
| step 3, env.sh build + clone + server | about 15–20 s warm (73 s cold) |
| step 3, `keys.mjs` | 30 s |
| step 3, `rects.mjs` P1 groups on today's page | 64 s; every group fails at its 15 s ready timeout, because there are no panes yet |
| gate P1, steps 1–2 | 4 s, red at "required tests 0/9" (build writes them) |
| gate P1, step 3 with stub suites | 93 s, red at the probe |

What the dry runs covered, and where each stopped:
- **Static.** P1 passes on the dry-run tree. P2 is red on README § Keys and P4 on the predecessor's
  `.trow` rules, both expected before build and the rebase. Mutants were caught: an `r10_name`
  local, a `cell_p1`, an unstaged file, and an edited `checks.txt` (the frozen check).
- **The required-test machinery.** Exercised with two real test names (2/2 passes) and one missing
  name (1/2 red).
- **kept.mjs.** Passes on a real `keys.mjs` log (26 of 26), and catches a check turned to FAIL.
- **The ledger.** Its self-test passes, and it passes on the committed PLAN and TECH.
- **Where the dry runs stop.** No phase's gate can run green before build: `rects.mjs` and the
  checks.txt labels need the panes DOM, and `history.mjs` comes with the rebase. The probe's
  arithmetic and selectors rest on the adversarial review, which rederived § 2.4's numbers and the
  selectors, and on its first run in P1. **Build's first P1 gate run records the real step-3
  time here.** The estimate is 2–4 minutes per phase, and P4 and P5 run every group.
- **P1, measured (2026-11-09).** Gate P1 in full: about 3 min 13 s warm (setup about 15 s; steps 1–2
  under 10 s; step 3, env.sh with `keys.mjs`, `history.mjs`, `panes.mjs` and the probe's four P1
  groups, about 3 min).

---

## Phase P1 — The arrangement frame, default and focus
**Satisfies:** R1, R3, R6, R7, R8, R9, R11 · **Depends on:** —
**Goal:** today's two panes, rebuilt as an arrangement. Classic starts at 900 px or wider and
stacked below, or classic at a pre-cycle `insp` width. Each pane has a title bar with its tabs. One
pane is focused, and the view keys place and focus. The split drags down to its floors and survives
a reload. Every existing suite stays green.

- [x] **Rebase first.**
  - Confirm that `main` carries both predecessor cycles: `git log --oneline main` shows the
    files-column-priority and history-log-rows landings, and `git show main:tests/ui/history.mjs`
    succeeds.
  - Then, sandbox off: `<agit> rebase --gpg-sign main`, where `<agit>` is the session's agit
    wrapper.
  - On any conflict: `<agit> rebase --abort`, stop, and report the conflicting paths. Resolve
    nothing by guesswork.
  - After the rebase, `git log --format='%G? %GF' main..HEAD` shows `G …EFD5DC8662D71CB9` for every
    commit.
  - `okt-client/static/app.css` has no `.trow` rule under `@media`. If one remains, stop and report:
    files-column-priority promised to remove them.
  - Read the landed `keys.mjs` FILES columns section and `tests/ui/history.mjs`. Where they differ
    from their PLANs, follow the landed code.
- [x] `okt-client/src/arrange.rs` (new, pure: std and serde only) (PLAN § 2.1):
  - the types and their JSON;
  - the sanitiser;
  - `default_for`;
  - `place`;
  - `drag` with the floors, counted per pane through nested splits (PLAN § 2.1, D5);
  - native tests named in `verify/required.txt` for P1;
  - register it in `lib.rs`.
- [x] `okt-client/src/state.rs` (high blast radius) (PLAN § 2.2):
  - `Settings.arr`, `arr_saved`, a pure `Settings::from_map`, the `arrangement` arm in `load`;
  - `to_json` writes the arrangement only when `arr_saved`;
  - `insp` frozen but kept;
  - `S.focus`;
  - the boot rule (PLAN § 2.2):
    - a saved arrangement is restored untouched, and the hash's `v` only picks focus;
    - with nothing saved, `v` is placed in memory without saving;
    - `popstate` places and saves;
    - every arrangement change replaces the hash;
  - the `pagehide` flush of the save debounce;
  - native test `state::tests::arrangement_is_written_only_once_chosen`.
- [x] `okt-client/src/panes.rs` (new) (PLAN § 2.3):
  - `render`, building the skeleton beside the old one, then moving view roots with `moveBefore`
    (a `#[wasm_bindgen]` extern, `catch`), falling back to `appendChild` with `scrollTop` restored,
    and parking views no pane holds in `#parking`;
  - the title bars and tabs, the tabs shrinking to fit (label ellipsised, kbd kept);
  - each split child's inline `flex`, never an inherited CSS variable;
  - `visible(view)` and `focused_view()`;
  - the delegated `pointerdown` that focuses a pane and starts a `.sash` drag (pointer capture;
    `--f` updated on move, saved on up);
  - `graph::fit_pane()` (the renamed `graph::resize`, now `pub`) called after every apply, with no
    borrow held.
- [x] `okt-client/static/index.html`:
  - `#view` and `#split` go;
  - the six view roots move into `#parking` inside `#panes`;
  - the `.fkeys` view buttons go, and the crumbs stay.
- [x] `okt-client/static/app.css`:
  - `.psplit`, `.pane`, `.ptitle`, `.ptab`, `.pbody` and `.sash`;
  - the focused title bar drawn differently (accent top edge, lighter background);
  - `.vpane` and `#insp` fill `.pbody`;
  - the 900 px block loses `.panes`, `.split` and `.insp` and keeps `.word, .kbname, .crumbs`;
    `.glegend` stays there until P4.
- [x] `okt-client/src/nav.rs`, `keys.rs`, `go.rs`, `live.rs`, `views/tree.rs`, `views/list.rs`,
  `views/table.rs` and `insp.rs` (PLAN § 2.6–2.7):
  - **Placement.** `set_view` becomes placement (`arrange::place`), then render-on-show, then
    history. `apply_hash` always places and focuses the hash's `v`. `list::show` and `table::open`
    place.
  - **Inspector keys.** `o c h` set the tab and show the Inspector (expand its pane, or add it to
    the focused pane when none holds it) **without moving focus** (GOAL R9, Q18). `c` then `j`
    still walks the Graph. `set_tab`'s other callers reveal the Inspector the same way. The `[data-view]` click delegate routes `insp` to
    the Inspector's placement.
  - **Key dispatch.** `keys::on_key` reads `focused_view()` in place of `s().view` for the Files,
    Log and Graph-walk branches, Escape's walk stop, `u` and `z`.
  - **Visibility.** Every `s().view == "…"` render gate becomes `panes::visible("…")`.
  - **The old splitter.** Its handlers in `keys::wire` go. The window resize keeps the ticker and
    re-renders the Log when it is visible; P4 replaces that with the Log's own observer.
- [x] **Ports**, keeping every existing check's label text and expected outcome (PLAN § 2.8;
  `verify/kept.mjs` checks both):
  - `tests/ui/keys.mjs`: `state().view` reads the shown view in the focused pane. The FILES columns
    section drives the new `.sash` where it drove `#split` and `--insp-w`, its sweep as one held
    drag moved 2 px a step;
  - its stacked windows come from a fresh context loaded under 900 px;
  - `tests/ui/history.mjs`: the same, its drags going to the same widths.
- [x] `tests/ui/panes.mjs` (new; grades rects; prints every P1 label in `verify/checks.txt`).
  README (§ Layout `tests/ui/` row, the § Tests block) and AGENTS.md (§ Build and test, and the
  § Map module list naming `arrange` and `panes`) name it in the same commit.
- **Build notes (P1, 2026-11-09).**
  - The split's 1 px rule is drawn as a `::before` overlay on the second child, not a border: a
    border takes its pixel out of the flex free space, so classic's left pane came out at
    0.54 × 1599, not 0.54 × 1600. The overlay keeps PLAN § 2.3's exact `f·E` tiling.
  - A press and release on a sash with no movement focuses the pane beneath the pointer: the sash
    overlays 2.5 px of each pane, and R8 says a click in a pane focuses it (the probe's
    `clickTitle` lands 2 px inside the left pane's right edge).
  - `tests/ui/keys.mjs`: the guard `FILES columns: OKT_KB is not the real KB` was called only on its
    failure path, so it never printed `ok`, and `verify/kept.mjs` (every label printed as passed)
    could not pass on it, on `main` either. It now prints its pass; same label, condition and
    outcome. The 1540 px Name/Title check runs in an 1800 px window, since the Inspector's 200 px
    floor caps the Files pane at 1400 px in a 1600 px one.
  - First gate run red on that guard alone (attempt recorded); the rerun from the start was green.

- **Remediation, review cycle 1 (2026-11-09).** Reopened for the review's F1 and F3 (R8, R9, R7)
  and F4 (GOAL Q19, then Q20). Its `verify:` is retuned to `gate.sh P5`, the latest done phase's
  gate; P4's is too, so one full gate run on the combined tree stands in for both. F1–F3 share
  `tests/ui/panes.mjs` with P4's F2, so they land in one commit, `P5: F1-F3`, routed here and under
  P4; F4 lands on its own, `P1: F4`.
  - [x] F1, the property: a view that comes on screen by itself only draws; focus, the hash and the
    history move only by the user's action. `views::table::open` is now the user's open: it calls
    the new `views::table::load`, then `set_view`. `load` loads the folder's table and shows
    nothing. `nav::show`, the render-on-show used by boot, `bodies_resized` and `operate`, loads
    with it and renders when the Table is still on screen. `apply_hash` also loads with it, since
    its own `set_view` follows. The sweep covered every render path `nav::show` reaches:
    `tree::render`, `log::ensure` and `render`, `list::render`, `graph::schedule`, `insp::render`.
    None moves focus, rewrites the hash or pushes history; the Table's was the only instance.
    `state.rs` is untouched.
  - [x] F1 guards (`tests/ui/panes.mjs`), each checking focus, the hash and `history.length`
    against the page's start (an init script), and that the Table was drawn: three with its Table
    tab shown, then a reload; a saved arrangement with the Table in its own pane, loaded from a
    `#v=graph` bookmark; the Table's pane under 200 px at 900 px, then the window grown to 1600 px.
  - [x] F3: `tests/ui/panes.mjs` applies stacked and three and reloads at once. The check asserts
    the store is still without the arrangement as the reload starts, so only the `pagehide` flush
    can keep the preset; the preset is kept.
  - [x] F4, under GOAL Q20, which supersedes Q19: the stop on the frozen probe's rows no longer
    applies, and they pass as they are. The Inspector's pane takes focus as R8 says, and every pane operation
    reaches it; its view has no view keys of its own. `panes::key_view` is the view the view keys
    act on: the focused pane's, or, while that shows the Inspector, the view of the last focused
    pane showing another (tracked in `panes::apply` as `KEYS_PANE`), else the first main view
    shown. `keys::on_key` dispatches on it in place of `focused_view`. `key_view`'s settings
    borrow ends before it returns, so none is held across the dispatch. `o`/`c`/`h` still never move
    focus (Q18). Guard (`tests/ui/panes.mjs`): in classic and three, a click on the Inspector's text
    marks its title bar, then `j` steps the Graph walk, Escape stops it, `ArrowDown` walks, and
    `w d` collapses the Inspector's pane to its strip.

- **Remediation, review cycle 2 (2026-11-09).** Reopened for the review's N1 and N3 (R8, under GOAL
  Q20). Its `verify:` stays `gate.sh P5`, and P4 is reopened with it for N2: one full gate run on
  the combined tree stands in for both. N1–N3 share `tests/ui/panes.mjs`, so they land in one
  commit, `P5: N1-N3`, routed here (N1, N3) and under P4 (N2).
  - [x] N1 and N3, the property: while the Inspector's pane is focused, the view keys act on the
    view that last held key focus, tracked by view, not by pane slot, wherever that view now lives.
    The slot record (`KEYS_PANE`) and its "first main view shown" fallback are gone:
    `panes::key_view` returns the focused pane's view, or, while that shows the Inspector,
    `S.view`, as on main. `S.view` already is the by-view record: `nav::sync_view`, which every
    change of focus or arrangement runs (`place`, `operate`, `panes::focus`), sets it to the focused
    pane's view when that is a main view and leaves it while the Inspector is shown. A second,
    separate tracker would be written at exactly the same points, so its never-record mutant would
    be equivalent and could not go red; the record that matters, and the one the controls mutate, is
    `sync_view`'s write. The borrow ends inside `key_view`, before the dispatch. `state.rs` is
    untouched.
  - [x] Guard (`tests/ui/panes.mjs`): Files is focused by its title bar after the Graph's, and is
    never the first pane holding a main view; the Inspector's text is clicked, an operation runs on
    its pane, and `j j k` must give the same trace (rows, selection, hash, no Graph walk) as a
    reference run with Files focused. Cases: three with no operation, three with `w x` (the
    review's case), Graph | Files | Inspector with `w x` (`q20b`'s row), and Graph | Inspector beside
    Files with `w v` and with `w q`, where the Inspector's neighbour is the Graph's pane.
  - [x] Negative controls, each alone in the working tree under `tests/env.sh`, restored and
    checked with `cmp`: the slot-id mutant (`panes.rs` as reviewed, `KEYS_PANE`) and the
    never-record mutant (`sync_view` no longer writes `S.view`) each turn the guard red. The
    slot-id mutant fails only this guard; the never-record mutant fails it and then breaks the
    suite at a later hash check, since `S.view` also writes the hash's `v`.
  - [x] Gate P5 green on the combined tree (one run for P1 and P4), the three new checks among its
    `ok` lines.

- **Remediation, review cycle 3 (2026-11-09), by Geoffrey's override of the review bound** (board
  answer `2026/11-09-pane-arrangements-a`). Reopened for the review's F1 (R8, R9, under GOAL Q20);
  P1 alone. Its `verify:` stays `gate.sh P5`. One commit, `P1: F1`.
  - [x] F1, the property: the position the Log's `j`/`k` step from is the commit it has selected,
    whether or not its pane is on screen. `insp::open_commit` now marks the Log's row for the commit
    unconditionally. It had marked it only while the Log was on screen (on main, only while the Log
    was the view), so keys routed to an off-screen Log kept stepping from row 0. The mark is the
    one record of the position: `log::key` reads it, and `log::render` writes it from `S.commit`
    as before, so a Log shown again marks the last commit stepped to.
  - [x] The class sweep, by the predicate "a view whose keys step from a marker that is written only
    while the view is on screen". The views with key navigation are Files, the Log and the Graph.
    The Graph's walk steps from its own state (`graph::WALK` and `S.sel`), not a marker. Files'
    keys step from the `.trow.on` row, which `tree::render` marks from `S.sel`. The Files keys
    render after each step, but `nav::select` and the live reload redraw the tree only while it is
    on screen, so a node chosen in the Inspector while Files' pane was collapsed, or its slot showed
    the Inspector, left the mark on the old row, and `j` stepped from there. That is a second
    instance, fixed: `tree::key` draws the tree first when Files is off screen, so it steps from
    the selection as on screen.
  - [x] Two siblings of the property, found while driving. A Log restored at load in a collapsed
    pane, then focused by its title, had never been drawn, so its keys found no rows and did
    nothing. And a commit arriving while the Log was the key view off screen wasn't drawn, so `j`
    opened the old first row. `nav::show_key_view` draws the Log when the keys act on it off
    screen; `panes::focus` and `nav::operate` run it once the panes are applied, and the live
    reload runs it when the Log isn't on screen.
  - [x] Guards (`tests/ui/panes.mjs`). The Log: the review's four scenarios (three: `l`, a click in
    the Inspector, `w v`; the same with `w q`; classic: `l`, the Inspector, `w q`; three and classic:
    `l`, `w d`), plus the collapsed-at-load Log focused by its title, and the Log on screen as the
    reference. In each, `j j j k` opens rows 0, 1, 2 and 1, and `l` brings the Log back with row 1
    alone marked. Files: classic with `w d`, and three with `w d` and with a click in the Inspector
    then `w v`. A node chip clicked in the Inspector, then `j k`, must give the same selections as a
    reference run with Files on screen, and not the step from the old row. The live reload: in three
    with the Log's pane collapsed, an empty commit in the throwaway clone is drawn off screen, `j`
    opens it, and `l` shows it marked.
  - [x] Negative controls, each alone in the working tree under `tests/env.sh`, restored and checked
    with `cmp`. The on-screen test restored in `open_commit` turns the Log check red in all six
    off-screen cases (`0,0,0,0`), and only that check. Without the draw in `tree::key`, the Files
    check goes red in all three cases (`j` steps from the old row), and only that check. With
    `show_key_view` inert, the Log check goes red in the collapsed-at-load case alone (no rows).
    Without the live reload's call, the commit check alone goes red (`j` opens the old first row).
  - [x] Gate P5 green on the final tree: `cargo test` 29 + 9 + 13 passed, required 23/23;
    `keys.mjs` 87 ok, `history.mjs` 105 ok, `panes.mjs` 65 ok (the three new checks among them),
    the probe 152 ok; `kept.mjs` 100 checks; no skip lines.
  - Seeded, not fixed here: the review's pre-existing observation that Pygments' literal token
    class `l` matches the source-row rule `.src .l`, so YAML scalars scroll the blame view sideways
    (seed issues/source-row-token-class.md, ROADMAP Part III). `state.rs` is untouched.

- **Remediation, review cycle 4 (2026-11-09), by Geoffrey's decision (a)** (REVIEW.md, "Geoffrey's
  decision (in session, 2026-11-09): (a)"). Reopened for G1 and G2 (R8, R9, R15, under GOAL Q20);
  P1 alone, one commit, `P1: G1-G2`. Its `verify:` now runs the frozen gate `gate.sh P5` teed to
  `.local/pane-arrangements/gate-P5.log`, then the class checker
  `spec/pane-arrangements/remediation/F1-key-view-drawn.sh` on that log: the frozen `checks.txt`
  can't name this remediation's guards, so the checker proves from the same run that the gate ended
  green and each new label printed exactly one `ok` line and no `FAIL` line. It is the cycle's first
  checker, so `F1`; its header names review cycle 4, G1 and G2.
  - [x] The property: the view the keys go to is always drawn. It is decided in one place,
    `panes::apply`, which every change of focus or arrangement draws through: `place` (a view's key,
    its tab, the hash on back and forward and at boot), `panes::focus` (a click), `nav::operate`
    (pane operations and presets) and the sash drag. At its end, `draw_key_view` draws the key view
    (`panes::key_view`) when it is off screen by `panes::visible` (collapsed, its slot showing the
    Inspector, or under the 200 px floor), once as it gets there: a thread-local records the view
    last drawn off screen, so a sash drag, which applies on every move, doesn't redraw the Log's rows
    each time. `nav::show_key_view` and its three calls (`panes::focus`, `nav::operate`, `live`) are
    gone. The live reload now renders the Log after reloading its history wherever the Log is, since
    its rows are what its keys step through; that is a change of data, not of the key view, and it
    takes the place of `live`'s call. `tree::key` keeps drawing Files as its keys run: that covers
    a selection moved elsewhere while Files is off screen, which no apply sees.
  - [x] A window resize after the draw: shrinking the key view's pane under the floor runs no apply,
    and needs none, since the rows drawn on screen stay in place. The guard drives it (`l` at
    1000 px, then the window shrunk to 760 px).
  - [x] Guards (`tests/ui/panes.mjs`), three labels. G1 runtime: the five views' pane saved at a
    quarter of a 1000 px window; shrunk to 760 px (190 px) then `l`, and `l` then shrunk, against
    the on-screen reference; `j j j k` opens rows 0,1,2,1, and the window grown back shows row 1
    marked. G1 boot: the same arrangement showing the Log, loaded with `#v=log` at 760 px, against
    1000 px. G2: `w w` onto a Log collapsed at load, never drawn, opens rows 0,1,2,1.
  - [x] Negative controls, each alone in the working tree with the full `panes.mjs` under
    `tests/env.sh`, restored with `cp`, touched, checked with `cmp`, and rebuilt by the next run.
    Without the draw in `apply`: all three new checks go red (no rows, `-1,-1,-1,-1`; the shrunk-after
    case still passes, its rows drawn on screen), and so does the existing Log check's collapsed-at-load
    case, focused by its title; 64 ok, 4 failed. The reviewed design (`panes.rs`, `nav.rs`, `live.rs` as
    at 9bccbbd, with the new suite): the runtime and boot checks go red, and the operate check passes,
    since 9bccbbd drew from `operate`; 66 ok, 2 failed. The live reload rendering only while the Log
    is on screen: the commit check alone goes red (the new commit not drawn); 67 ok, 1 failed. The
    checker was seen red on the first mutant's log (no green gate line, each label 0 ok and 1 FAIL).
  - [x] Gate green on the final tree, with the checker green on the same run's log: static, ledger
    and kept (100 checks) ok; `cargo test` 29 + 9 + 13 passed, 1 ignored, required 23/23; `keys.mjs`
    87 ok, `history.mjs` 105 ok, `panes.mjs` 68 ok (the three new checks among them), the probe 152
    ok; no skip lines; the checker 3/3 labels. About 11 minutes with setup.

- **Verify:** `sh spec/pane-arrangements/verify/gate.sh P1`. It runs the static checks, `cargo test`
  with the P1 native tests, and then `keys.mjs`, `history.mjs`, `panes.mjs` and the frozen probe's
  `first-load migration keys drag` groups in one env.sh run on port 6584. It ends with the
  leftover check, `kept.mjs`, and the P1 labels.
- **Touches:** `okt-client/src/{arrange,panes,state,nav,keys,go,live,insp,graph,lib}.rs`,
  `okt-client/src/views/{tree,list,table,log}.rs`, `okt-client/static/{index.html,app.css}`,
  `tests/ui/{keys,history,panes}.mjs`, `README.md`, `AGENTS.md`.

## Phase P2 — Presets, collapse and their keys
**Satisfies:** R4, R5, R7, R10, R17 · **Depends on:** P1
**Goal:** the four presets at PLAN § 2.4's sizes, applied by `;` `'` `\` and backtick and cycled by
`p`. A pane collapses to its title bar, or to a strip side by side, and expands back, through the
`w` leader's `w d`. The rect grid holds at 1600 and 900.

- [x] `arrange`:
  - `classic(c)`, `swapped`, `stacked` and `three`, with `f` values exactly as PLAN § 2.4 states;
  - the cycle order;
  - "which preset the tree equals";
  - `collapse_toggle` with the accordion rule, refusing the only pane, focus staying on the pane;
  - the P2 native tests.
- [x] `keys.rs`:
  - the preset keys and `p`;
  - the leader `w`, a `Cell` flag, with `w d` in P2:
    - modifier-only keydowns leave it pending;
    - Escape cancels it;
    - any other key cancels it and is then handled as usual;
    - blur or two seconds clear it;
  - every key unmodified, proven free by the arms of `on_key`.
- [x] The window bar's preset strip (CLASSIC ; · SWAPPED ' · STACKED \ · THREE `): click applies,
  and the current preset is marked.
- [x] `app.css`: a collapsed pane's body hidden. The 24 px title bar in a column split; the 24 px
  strip with `writing-mode: vertical-rl` in a row split. The sash is hidden beside a collapsed leaf.
- [x] `chrome::show_help`: one row per preset key, its `dd` naming the preset; a `p` row reading
  "cycle the presets"; `w` and `w d`. Fix the help's stale "lists" line.
- [x] README § Keys: a PANES bullet naming classic, swapped, stacked and three, `p` written as
  `` `p` ``, `w` and `w d` (R5).
- [x] `tests/ui/panes.mjs`: the P2 labels. The four presets at 1600×1000 and 900×1000 are graded
  from rects against PLAN § 2.4 within 2 px; views held, the shown tab, the collapsed state and a
  reload within 1 px are graded too.
- **Build notes (P2, 2026-11-09).**
  - A split's children are drawn first child, sash, second child (PLAN § 2.3's skeleton), not
    with the sash last as P1 drew it: in document order an outer split's sash now comes before any
    sash nested in its second child, so "the first visible sash" is the outer one, as the probe's
    nested-floor and canvas-refit drags assume. The 1 px rule's overlay moves to `:nth-child(3)`.
  - `arrange::which` compares trees ignoring shown tabs (a tab click doesn't leave classic), with
    shares within 1e-6. Classic by key takes `classic_share(Settings.insp)`: `insp` is frozen at
    46 unless a pre-cycle blob held another value, so no `state.rs` change was needed.
  - A preset is always saved as a choice, even when the tree already equals it (the probe stores
    after `;` on a default classic).
  - `nav::operate` runs every pane operation: the pure function under one `cfg_mut()` borrow, then
    focus, `panes::apply`, the save, a render of each view that came on screen, and `nav(false)`.
  - A preset button blurs itself on click, so a later space or Enter isn't a second click.
  - Gate P2 in full: about 4 min warm (step 3 with the probe's seven groups).

- **Verify:** `sh spec/pane-arrangements/verify/gate.sh P2` (adds the probe's `presets collapse three`
  and the README check).
- **Touches:** `okt-client/src/{arrange,keys,panes,chrome}.rs`, `okt-client/static/{index.html,app.css}`,
  `tests/ui/panes.mjs`, `README.md`.

## Phase P3 — Swap, flip, close and move
**Satisfies:** R1, R7, R12, R13, R14 · **Depends on:** P2
**Goal:** the remaining pane operations on the focused pane, each saved and surviving a reload, with
the refusals the GOAL names.

- [x] `arrange`:
  - `neighbour` as PLAN § 2.5 defines it;
  - `swap` (contents in place) and `flip`;
  - `close` (views to the neighbour, which shows the closed pane's view, expands and takes focus;
    the sibling subtree takes the space; refused for the root pane);
  - `move_view` (to the neighbour as its shown tab; an emptied source closes; refused for the root
    pane);
  - the P3 native tests.
- [x] `keys.rs`: `w w`/`w W` focus the next/previous pane, `w x` swap, `w r` flip, `w q` close,
  `w v`/`w V` move. A refusal toasts and leaves the arrangement unchanged.
- [x] Help PANES block and README § Keys: every `w` operation and the focus rule.
- [x] `tests/ui/panes.mjs`: the P3 labels, including "any view can be moved into any pane": each of
  the six views is moved by `w v` into a pane that didn't hold it.
- **Build notes (P3, 2026-11-09).**
  - PLAN § 2.5's neighbour takes "the sibling after P (before P), else the one on the other side".
    A split has two children, so both directions reach the same sibling subtree, and the descent
    then lands on the same pane: `w v` and `w V` move to the same neighbour, as `w x` and `w q`
    would with either direction. `arrange::neighbour` keeps the direction as PLAN states it; no R-ID
    asks the two keys to differ (R14 says "a neighbouring pane").
  - "P shows its next held view": the view after the moved one, or the first when the moved one
    was last.
  - `w w`/`w W` step the focus through the panes in reading order, round the ends; they change
    nothing saved.
  - Refusals toast under PANES: "The only pane doesn't close", "No other pane to move it to",
    "No other pane to swap with", "The only pane has no split to turn".
  - Gate P3 in full: about 4 min warm (step 3 with the probe's eight groups).

- **Verify:** `sh spec/pane-arrangements/verify/gate.sh P3` (adds the probe's `ops`).
- **Touches:** `okt-client/src/{arrange,keys,panes,chrome}.rs`, `tests/ui/panes.mjs`, `README.md`.

## Phase P4 — Each view fits its pane
**Satisfies:** R2, R15, R16 · **Depends on:** P3
**Goal:** every pane is a named size container, and its views fit it down to 200 px wide. A narrower
pane shows only its title bar. The legend hides by the Graph pane's width, the Log chart and the
Graph canvas follow their pane, and the Graph and Files run side by side.

- [x] `app.css` (PLAN § 2.6):
  - `section.pane { container: pane / size }`;
  - `@container pane (width < 200px) { .pbody { display: none } }`;
  - the `.vhead` shrinkables, with `.vmeta`/`.tfold` dropped under 360 px;
  - `#itabs` and pane tabs showing only the kbd under 300 px (`insp.rs` puts the tab words in a
    span);
  - `.gtip` capped and clamped;
  - the rows of Files, the Log and Results fit a 200 px pane with no sideways scroll; `#tgrid`
    and `#ibody` keep theirs;
  - `.glegend` hides under `@container pane (width < 480px)`, and the 900 px block keeps only
    `.word, .kbname, .crumbs`.
- [x] `views/log.rs`: `render_chart()` and a debounced `ResizeObserver` on `#lchart`. The window
  resize keeps only the ticker.
- [x] `panes::visible` also treats a pane under 200 px as not showing its view. A `ResizeObserver`
  on each `.pbody` re-runs render-on-show when the pane crosses 200 px.
- [x] `tests/ui/panes.mjs`: the P4 labels. The Graph is graded from pixels and
  `elementFromPoint`, at DPR 2, never from a client flag.
- **Build notes (P4, 2026-11-09).**
  - A pane under 200 px keeps every tab in its title bar: the tabs drop their words (under 300 px,
    as PLAN says) and, under 200 px, their padding, with the key hint free to shrink, so a 55 px
    pane in a 440 px window spills nothing. These rules skip a collapsed pane, whose strip keeps
    its words turned on their side (R10).
  - Under 360 px of its pane the Log's rows keep the hash, the time and the subject (`.px`, `.nf`
    and `.ad` go); the full tracks stay pinned in `keys.mjs` at 1200 and 480 px, and its comment
    now names the narrow set (§ Conventions). The view bars' titles, the Table's fold, the
    Results group paths, the Inspector's badges and its action buttons also shrink or wrap.
  - Render-on-show for a pane crossing 200 px: `panes::apply` records the views on screen, and the
    `.pbody` observer renders only those that came on screen since, so an arrangement change
    (which renders its own) isn't rendered twice. The observer is disconnected and re-attached to
    the new bodies whenever the skeleton is rebuilt.
  - The Log chart's observer replaces the sash's `pointerup` redraw; the window `resize` keeps
    only the ticker. The hover tip measures itself, flips to the pointer's other side, and is
    clamped 4 px inside the pane.
  - `graph::fit_pane()` was already called after every apply (P1); nothing new there.
  - First static run red on two test locals named like spec ids (`r0`, `r1`; attempt recorded),
    renamed; the full gate then green. Gate P4 in full: about 4 min warm.

- **Remediation, review cycle 1 (2026-11-09).** Reopened for the review's F2 (R15). Its `verify:`
  is retuned to `gate.sh P5`, run once with P1's (see P1).
  - [x] The Overview's Last commit row is `.hrow.last`. Under 360 px of `#ibody` it takes two lines:
    the hash and the time, then the subject and the counts. That fits a 200 px pane with counts of
    14 characters. From 360 px it is the plain one-line `.hrow`, as on main. History's `.hrow.hl`
    and its rules are untouched.
  - [x] The Content toolbar under 360 px of `#ibody`: a tighter gap and padding, and its segmented
    buttons and the blame legend shrink, cut with an ellipsis, before they would spill.
  - [x] Guards (`tests/ui/panes.mjs`). `spill()` no longer exempts `#ibody`: free are only the Table
    grid and, inside the Inspector, the blocks meant to scroll on their own (a rendered table, a
    code block, a data table). `threeExtras` reads the same rule. A new check grades every
    Inspector tab at 200, 240 and 272 px: Overview, Content, Content with the blame on, and
    History.

- **Remediation, review cycle 2 (2026-11-09).** Reopened for the review's N2 (R15); gated with P1's
  run (see P1).
  - [x] The blame gutter under 400 px of `#ibody`: it shrinks with the container,
    `clamp(12px, 100cqi - 190px, 210px)`, down to its age bar and the date's first letters, with
    tighter padding; the line-number column tightens from 46 to 36 px. The code column keeps about
    150 px from 200 px up (152 at 200, 154 at 240 and 268); from 400 px the gutter is its full 210 px.
  - [x] The time-travel bar under 360 px of `#ibody`: a tighter gap and padding, LIVE without its
    margin, and its buttons free to shrink with an ellipsis. At 200 px every button is inside
    `#ibody`.
  - [x] Guards (`tests/ui/panes.mjs`): source with the blame on, for a code file (the first by name
    with a code extension) and for the hub card after `s` then `b`, at 200, 240 and 268 px: the
    gutter drawn, `#ibody` not scrolling sideways, nothing spilling past the pane, and a code
    column of at least 120 px. The time-travel bar at 200 px: three buttons inside `#ibody`, no
    sideways scroll, no spill.
  - [x] Negative controls, each alone, restored and checked with `cmp`: the blame rule removed
    turns the source check red (gutter 210 px, code column 12 px, `#ibody` 268 > 200), and the
    travel check with it, its card still in source with the blame; the travel rule removed turns
    the travel check alone red (`#ibody` 207 > 200, LIVE past the edge).

- **Verify:** `sh spec/pane-arrangements/verify/gate.sh P4` (adds the probe's `together fit` and the
  static check that no `.trow` rule sits under `@media`).
- **Touches:** `okt-client/static/app.css`, `okt-client/src/{panes,insp,graph}.rs`,
  `okt-client/src/views/log.rs`, `keys.rs`, `tests/ui/panes.mjs`.

## Phase P5 — What Geoffrey sees, graded in full, and the ledger
**Satisfies:** R17 · **Depends on:** P4
**Goal:** `tests/ui/panes.mjs` is the repo's rect suite for every preset at 1600×1000 and 900×1000,
with three's extras. The deferral ledger is clean.

- [x] `tests/ui/panes.mjs`: one check per preset and size grading:
  - each pane's position and size against PLAN § 2.4 within 2 px;
  - the views each pane holds, read from visible tabs inside the title bar;
  - every collapsed pane at its title bar's height.

  For three at both sizes, it also checks:
  - the Graph's title bar spans the top;
  - the Inspector is on the left, showing the selected card with its O/C/H tabs;
  - Files is on the right, showing its filter field and at least 10 rows with their Name visible;
  - no horizontal overflow in either pane.

  It prints the P5 label.
- [x] Run `tests/ui/soak.mjs` once in the throwaway environment (`tests/env.sh sh -c 'node
  "$OKT_ROOT/tests/ui/soak.mjs" "$OKT_KB" "$OKT"'`) and note the result in the commit body.
  It checks that listeners and memory still plateau with the pane `pointerdown` and the
  observers in place.
- [x] **Deferral ledger** (standing last-phase item, code-plan Step 6.8):
  - walk every earlier phase's body and commit body, prose and list items (not headings), for
    the phrases `verify/ledger.sh` reads;
  - each must name an `issues/` seed with a `ROADMAP.md` entry;
  - `verify/ledger.sh` checks TECH, PLAN and REVIEW, and its `--self-test` must pass.

  No deferrals are planned. Anything build leaves undone becomes a seed from
  `$KB/skill/_templates/ISSUE.md` plus a ROADMAP entry, staged by path.
- [x] No other standing project-level last-phase item carries over: the predecessors' last phases
  carried the ledger alone.
- **Build notes (P5, 2026-11-09).**
  - The full grid is one check: each width opens a fresh page, selects the hub, and applies
    swapped, stacked, three and classic by key. Each pane is graded as P2's presets are, plus the
    views read from tabs drawn inside its title bar and every collapsed pane at 24 px. Three's
    extras are P2's three check, now a shared helper (`threeExtras`) both checks call.
  - `tests/ui/soak.mjs`, 400 rounds in the throwaway environment: listeners 66 from round 50 on,
    DOM nodes flat (−78 from round 100 to 400), heap +179 KB, WebAssembly memory flat from
    round 200 (18,048 KB). It plateaus with the pane `pointerdown` and the observers in place.
  - The ledger walk found no deferral in TECH, PLAN or the branch's commit bodies; `ledger.sh` and
    its `--self-test` pass, and no `issues/` seed was written.

- **Verify:** `sh spec/pane-arrangements/verify/gate.sh P5` (every probe group, the static checks
  and the ledger).
- **Touches:** `tests/ui/panes.mjs`, and `issues/`/`ROADMAP.md` only if build leaves something undone.

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
