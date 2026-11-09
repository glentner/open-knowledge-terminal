# REVIEW — Pane arrangements: place, swap, stack and collapse views

> Adversarial QA by `code-review`, run in an isolated context (a subagent). The correctness pass
> grades the branch diff against [`GOAL.md`](GOAL.md) + the repo's `AGENTS.md` § Invariants only;
> it does not see `PLAN.md`/`TECH.md`. Every finding cites an executed command.

- **Reviewed commit:** dc19f4f  ·  **Base:** main (8bbc64b)  ·  **Date:** 2026-11-09
- **Verdict:** changes-requested
- **Cycle:** 1 of ≤3

Mode: one blind reviewer (fresh subagent, xhigh). Evidence: `$REPO/.local/pane-arrangements/review1/`
(main checkout, git-ignored; `NOTES.md` indexes it).

Contract drift: GOAL changed after shaping in `6c45ff0` (first-load preset, pane floor) and `54a9b77`
(inspector keys keep focus). Both are foreman rulings at the PLAN/TECH sign-off, and the foreman
confirmed them as the contract graded.

## Verification run

- `cargo test …`: okt-client 29/29, okt-server, okt-pygments green.
- In env.sh on 6584: keys.mjs 87 ok, history.mjs 105 ok, panes.mjs 53 ok.
- Probes p1–p12 (all presets at 1600 and 900, migration, persistence, pane operations, floors,
  fitting, and re-entrancy with `moveBefore` and with the `appendChild` fallback).
- state.rs scenarios: 60/60, including 19 corrupt `arrangement` variants.
- Negative control: removing the pagehide listener left panes.mjs green (F3). Source and dist were
  restored and their hashes verified.

## Requirement → evidence matrix

| R-ID | Status |
|---|---|
| R1–R7, R10–R14, R16, R17 | ✅ (R7: see F3 coverage) |
| R8, R9 | ✅ with F1 caveat; F4 (Q6 conflict, below) |
| R15 | ❌ F2: Inspector at 200–272 px |

Unmapped changes: the clickable preset buttons in the window bar (current preset marked); a help
line reworded; dead `.fk` CSS; the Graph tooltip clamp (serves R15).

## Findings

### [MEDIUM/CONFIRMED] F1 · A Table tab rendering on its own takes focus, rewrites the hash and pushes history
- **Where:** `nav.rs:233-238` → `views/table.rs:103-104`, whose `open` always ends with
  `nav::set_view("table", false)`. It is reached from `boot_view`, `bodies_resized` and `operate`'s
  newly visible renders.
- **Failure scenario:** in "three", press `` ` ``, `t`, `g`, select a card, then reload. Focus lands on
  the Table pane, the hash reads `#v=table…`, and `j` no longer walks the Graph. A `#v=graph`
  bookmark and a window growing past the 200 px floor have the same effect (`history.length` 3→4).
- **Evidence:** `p11-three-table.log`, `p8-table-focus.log`.
- **Touches:** R8 (focus moves only by a click or a key), R9 and Q6 (the hash's view picks focus).

### [MEDIUM/CONFIRMED] F2 · The Inspector doesn't fit a 200 px pane
- **Where:** the Overview's "Last commit" row, a plain `.hrow` (`app.css:462`, 261 px), and the
  Content toolbar's BLAME button. `#ibody` scrolls sideways at every width from 200 to 272 px.
- **Gate gap:** `tests/ui/panes.mjs:716-731` exempts `#ibody` from the sideways check.
- **Evidence:** `p10-insp-shot.log`, `p5-insp200.log`, `insp-200-*.png`.
- **Touches:** R15 ("Views SHALL fit down to a pane 200 px wide").

### [LOW/CONFIRMED] F3 · Nothing in the suite exercises the pagehide flush
- **Where:** `keys.rs:717`. Removing the listener left panes.mjs at 53/53, because its reload checks
  wait past the 200 ms debounce.
- **Evidence:** `panes-mutant-noflush.log`. The reviewer's probe caught it
  (`p9-flush-mutant.log` vs `p9-flush-restored.log`).
- **Touches:** R7.

### [MEDIUM/CONFIRMED, from the reviewer's "seen, not graded"] F4 · After a click in the Inspector, `j`/`k`/arrows stop walking the Graph
- **Failure scenario:** click inspector text, then `j`: nothing happens. `ArrowDown` scrolls
  `#ibody` instead of walking. On origin/main both walk the Graph.
- **Evidence:** the reviewer's p3 notes, read against the origin/main key handling.
- **Touches:** Q6 (no existing key changes meaning) against R8 read literally (a click focuses a pane).

## Triage (foreman, 2026-11-09, under decision 2026-11-08-okt-dream-inbox-autonomous)

No CONFIRMED finding is in a high-blast-radius path: state.rs was graded clean at 60/60, and § 1 at
10/10 drives. There is no human gate. Routed to `code okt build` (remediation):

- **F1, the property:** a view rendering because it came on screen never moves focus, rewrites the
  hash or pushes history. Only a user action does. `table::open` from a render path must not call
  `set_view`. Guard: the three reproductions above, as checks.
- **F2, the property:** every view, the Inspector included (Overview, Content, History), fits a
  200 px pane with no sideways scroll in `#ibody`.
  - The Overview's "Last commit" row: give it a narrow-pane rule scoped to the Overview, so it
    doesn't touch History's `.hrow.hl`. That row's look at today's widths stays as on main.
  - The Content toolbar wraps or shrinks.
  - Guard: drop the `#ibody` exemption from `spill()` and grade every Inspector tab at 200, 240
    and 272 px.
- **F3:** a check that applies a preset and reloads at once (inside the debounce) and finds it kept.
  The no-flush mutant must go red.
- **F4, GOAL clarification Q19 (dated):** the Inspector's pane never takes key focus, whether by a
  click (R8) or by a key (Q18). A click in it leaves focus, and the view keys, on the last focused
  non-Inspector pane. The one exception is when the Inspector is the only pane. Its title bar is
  never marked focused, and Inspector scroll stays on `J`/`K`, as on main. Guard: click inspector
  text, then `j` walks the Graph and `ArrowDown` walks, both as on main.

## Human-gate triggers

- None.

### Triage amendment (foreman, 2026-11-09): F4 under Q20

The remediation builder stopped on F4 (`5b10458` carries F1–F3). Q19 kept the Inspector's pane
from ever taking focus, which would stop every pane operation (R10, R12–R14) from reaching it. It
would also have turned six frozen probe rows red that rightly expect a click to focus a pane. Q19 is
withdrawn, and Q20 replaces it: the Inspector's pane takes focus as R8 says, and view-level keys
pressed while it is focused act on the last focused non-Inspector pane's view.

F4's guard: click Inspector text in classic and in three, then check that `j` and `ArrowDown` walk
the Graph as on main, while the Inspector's title bar is marked focused and `w d` collapses the
Inspector's pane.

## Review cycle 2 — changes-requested (2026-11-09)

- **Reviewed commit:** 453ce7a.
- **Mode:** scoped to verifying F1–F4, plus a full R1–R17 regression pass; one blind reviewer (xhigh).
- **Evidence:** `$REPO/.local/pane-arrangements/review2/`.
- **Verification run:**
  - `cargo test` 51 passed; `keys.mjs` 87 ok, `history.mjs` all passed, `panes.mjs` 59 ok.
  - Probes for each finding plus siblings (f1, f1b, f2, f3, q20, q20b, q20c, q20d).
  - Six mutants, each restored and its dist hash verified.
  - Six random 260-step storms for § 1, run with `moveBefore` and with the `appendChild` fallback: no errors.
- **F1: fixed.** The reproduction and 9 sibling cases pass, and the mutant fails exactly the 3 guards.
- **F3: fixed.** The guard is red without the flush, and 16 more flush cases pass.
- **F4 (under Q20): fixed** for the cases Q20 names: every view-key family, all three ways of focusing the Inspector, and the five pane operations on its pane.
- **F2: partly fixed.** See N2.
- **R1–R7, R9–R14, R16, R17:** pass. Every key keeps its meaning; `state.rs` is clean.

### [MEDIUM/CONFIRMED] N1 · After `w x` swaps the focused Inspector pane, view keys fall back to the wrong view
- **Where:** `panes.rs:86-88,318-333` and `arrange.rs:693-718`. `KEYS_PANE` records a slot id, and swap keeps ids while exchanging contents.
- **Failure scenario:** in three, focus Files, click the Inspector, `w x`. `j`/`k` then walk the collapsed, invisible Graph, while the hash still says `v=files`.
- **Evidence:** `q20b.log`.
- **Touches:** R8 and Q20.

### [MEDIUM/CONFIRMED] N2 · The Inspector still scrolls sideways in two states at the 200 px floor
- **Where:**
  - **Blame gutter:** `.src.blame` (`app.css:524,533`). Under 268 px the 210 px gutter plus the line number squeeze the code column to 12 px (scrollWidth 268).
  - **Time-travel bar:** `app.css:514-520`. At 200–204 px its LIVE button is cut off (207 vs 200).
- **Gate gap:** the suite's blame step uses rendered markdown, which draws no gutter.
- **Evidence:** `f2.log`, `f2diag*.log`, `f2-python-content-blame-at-250.png`, `f2-travel-on-hub-at-200.png`.
- **Touches:** R15.

### [LOW/CONFIRMED] N3 · The Q20 guard can't see a missing last-pane record
- **Failure scenario:** a mutant that never records `KEYS_PANE` survives, because in both of the suite's cases the last-focused pane is also the first main pane.
- **Evidence:** `q20d-mutant-M4b.log`.

### Triage (foreman, 2026-11-09, under decision 2026-11-08-okt-dream-inbox-autonomous)

This is converging: the cycle-1 findings are fixed, and the new ones are narrow. Routing the last
remediation the bound allows. A CONFIRMED finding in review cycle 3 stops the cycle for Geoffrey.

- **N1 + N3 (the property):** view keys pressed while the Inspector's pane is focused act on the
  view that last held key focus, tracked by **view, not pane slot**, wherever that view now lives.
  The fallback is `S.view`, as on main.
  - Guards: the reviewer's three-swap case, and the row arrangement in `q20b`.
  - Add a case where the last-focused pane is not the first main pane, so that a never-record
    mutant goes red.
- **N2 (the property):** every Inspector state fits a 200 px pane with no sideways scroll,
  source-with-blame and time travel included.
  - Blame: a narrow-pane rule for the gutter (under its container width, shrink or abbreviate it)
    that keeps the code column readable. A check grades source view with blame on a code file, and
    on markdown after `s` then `b`, at 200, 240 and 268 px, for no sideways scroll and a code column
    of at least 120 px.
  - The travel bar fits at 200 px.
  - A mutant removing each rule must go red.

## Human-gate triggers (cycle 2)

- None.

## Review cycle 3 — changes-requested, stopped for Geoffrey (2026-11-09)

- **Reviewed commit:** bebe6af. **Mode:** scoped to verifying N1–N3, plus a full R1–R17 regression pass; one blind reviewer (xhigh). Evidence: `$REPO/.local/pane-arrangements/review3/`.
- **Verification run:**
  - `cargo test`: 29 + 9 + 13 passed.
  - `keys.mjs` 87 ok, `history.mjs` 105 ok, `panes.mjs` 62 ok.
  - 162 sibling runs for N1 across presets, row arrangements, focus paths and pane operations.
  - N2 swept at 200–440 px for blame (three code files, three cards) and the travel bar.
  - Mutants M1–M6: each guard bites, and each restore was touched and rebuilt.
  - `state.rs` scenarios 23/23; about 1,900 rapid interleaved steps with no panic.
- **N1, N2, N3:** fixed. R1–R17 hold, except F1 against R8 and R9.

### [MEDIUM/CONFIRMED] F1 · With the Log off screen, `j`/`k` stay on the first commit
- **Where:**
  - `okt-client/src/insp.rs:1979`: `open_commit` marks the `.on` row only when `panes::visible("log")`. On main the test was `s().view == "log"`.
  - `views/log.rs:335-347`: `log::key` finds its position from that row.
  - `panes.rs:315-321`: `key_view` routes the keys to the Log while it is off screen.
- **Failure scenario:** in three, press `l`, click the Inspector, then `w v` or `w q`. Classic with `l`, Inspector, `w q` fails the same way, as does `l`, `w d` in three or classic.
  - Expected: `j j j k` steps rows 0, 1, 2, 1, as it does with the Log on screen.
  - Actual: it opens commit 43799d6 (row 0) four times.
- **Evidence:** `r3-log.log`.
- **Touches:** R8, R9 and Q20. No high-blast-radius path is involved.

### Observation, out of Scope (pre-existing)
The Pygments literal token class `l` matches `.src .l`, the source-row rule. With blame on, a YAML literal span becomes a grid with the 210 px gutter, and `#ibody` scrolls sideways at 360–440 px (`_common.yaml`: 466 > 440). The pane itself doesn't overflow. The CSS is the same on main (`r3-yaml2b.log`). Its home is a seed, to be filed with Geoffrey's decision on F1.

### Triage (foreman, 2026-11-09, under decision 2026-11-08-okt-dream-inbox-autonomous)

This is a CONFIRMED finding at review cycle 3, the bound in `skill/code-review.md`. The foreman committed in cycle 2's triage that such a finding stops the cycle for Geoffrey. **Stopped: no landing, no 0.4.0.**

The foreman's recommendation, for Geoffrey to decide:
- **(a)** A fourth remediation by override. F1 is narrow:
  - the fix is to test "the Log is the key view" instead of "the Log is visible" in `open_commit`, or to rebase `log::key` on the selected commit rather than the `.on` row;
  - a guard covering the four scenarios;
  - then a scoped review 4.
- **(b)** Land 0.4.0 with F1 seeded, by override. The rubric gives a CONFIRMED finding no deferral route, so this takes Geoffrey's typed override.

## Human-gate triggers (cycle 3)

- None. The stop is the review bound, not a high-blast-radius path.

### Geoffrey's decision (board answer `2026/11-09-pane-arrangements-a`, 2026-11-09)

**(a), by Geoffrey's override of the review bound:** a fourth remediation for F1, then one blind
review 4 limited to that fix. If it comes back clean, publish and release 0.4.0; if not, stop and
go back to him.

Routing for the remediation (the property, not the instance):
- **F1:** the position `j`/`k` step from in the Log is the commit the Log has selected, whether
  or not the Log's pane is on screen (collapsed, or its slot showing the Inspector). Keys routed
  to an off-screen Log step through its commits exactly as they do on screen, and when the Log
  comes back on screen its marked row is the last commit stepped to.
  - Guards: the four scenarios in cycle 3's F1 (three: `l`, click the Inspector, `w v`; three:
    the same with `w q`; classic: `l`, Inspector, `w q`; three and classic: `l`, `w d`). In each,
    `j j j k` opens rows 0, 1, 2, 1, and bringing the Log back shows row 1 marked.
  - A mutant restoring the on-screen test in `open_commit` must go red.
  - Sweep the class: every view whose key navigation reads its position from a marker that is set
    only when the view is on screen.
- **The YAML `.l` observation** gets its seed (`issues/` plus a ROADMAP entry), as cycle 3
  promised. It is pre-existing and outside this cycle.

## Review cycle 4 (by Geoffrey's override) — changes-requested, stopped for Geoffrey (2026-11-09)

- **Reviewed commit:** 9bccbbd. **Mode:** scoped to the F1 fix (`b5d3e7a..9bccbbd`), plus a regression pass; one blind reviewer (xhigh). Evidence: `$REPO/.local/pane-arrangements/review4/`.
- **Verification run:**
  - `cargo test`: 29 + 9 + 13 passed, 1 ignored.
  - `panes.mjs` 65 ok (the three new checks included), `keys.mjs` 87 ok, `history.mjs` 105 ok. No page errors.
  - Probe `r4-log.mjs`:
    - the Log in all four presets × {none, click `w v`, click `w q`, `w d`, click `w V`, click `w x`}: 24/24 open rows 0,1,2,1, and the Log shown again marks row 1;
    - focus stepping onto a Log collapsed at load: 2/2;
    - live commits off screen: 4/4;
    - Files on seven more routes: 7/7.
  - Mutants M1, M2, M4 and M5 are killed by the new checks. M3 (no `show_key_view` in `operate`) survives the suite. Every mutant was restored, `cmp`-equal, and rebuilt; the tree is clean.
- **F1:** fixed on every route cycle 3 named, plus the siblings the builder found.
- **R3, R8, R9, R15, R1–R17 regression, § 1, § Conventions:** pass. No high-blast-radius path is touched.
- **Seed `issues/source-row-token-class.md`:** accurate. Its evidence path under `.local/` exists only on this clone.

### [MEDIUM/CONFIRMED] G1 · A Log off screen below the 200 px floor is never drawn, so `j`/`k` do nothing
- **Where:**
  - `panes.rs:289-306`: `visible` counts a pane under `MIN_W` as off screen.
  - `nav.rs:226-236`: `show_key_view` runs from `focus`, `operate` and `live`. It doesn't run after `place()` (`set_view` at `nav.rs:104-118`, tab clicks, back/forward), and it doesn't run at boot (`nav.rs:268`).
  - `log::key` returns false with no rows.
- **Failure scenario (runtime):** the five-view pane is saved at 250 px of a 1000 px window. Shrink the window to 760 px, so that pane is 190 px and its body is hidden. Press `l`, then `j j j k`: nothing opens. At full width the same keys open rows 0,1,2,1.
- **Failure scenario (boot):** the same arrangement loaded at 380 px with `#v=log` gives 0 rows, and nothing opens.
- **Evidence:** `mut-control.log` (`nrows0: 0`, `opened: ["","","",""]`); `r4-log.log` sections `narrow` and `narrowl`.
- **Not a regression of 9bccbbd.** The route behaved this way before the fix. It isn't on main, which has no narrow floor.

### [LOW/CONFIRMED] G2 · No suite check guards `operate`'s call to `show_key_view` (`nav.rs:187`)
- Mutant M3 passes the suite. With M3 applied, `w w` onto a Log pane collapsed at load gives `[-1,-1,-1,-1]` (`mut-M3.log`).

### Triage (foreman, 2026-11-09)
Geoffrey's override allowed one review 4, and said to stop and come back if it isn't clean. It isn't: G1 is CONFIRMED. **Stopped: no landing, no 0.4.0.** The question is on the board.

## Human-gate triggers (cycle 4)

- None. The stop is Geoffrey's condition on the override, not a high-blast-radius path.

### Geoffrey's decision (in session, 2026-11-09): (a)

A fifth remediation, then a scoped review 5. If review 5 is clean, or finds only new edge routes of
G1's kind (each one seeded), publish and release 0.4.0. Anything else stops again.

Routing (the property): **the view that keys go to is always drawn.** Whenever a view becomes the
key view, by any path (`place()`/`set_view`, tab clicks, back/forward, focus, pane operations,
live reload, and boot), and its pane is off screen (collapsed, its slot showing the Inspector, or
under the narrow floor), it is drawn, so its keys step as they do on screen. Decide this in one
place that every path passes through, not as a call added per path.
- Guards: G1's runtime scenario (a window shrunk so the pane falls under the floor, then `l`,
  `j j j k`) and its boot scenario (`#v=log` loaded under the floor). G2: a case that kills
  mutant M3, the draw in `operate`.
- Every new guard is mutant-checked, and the run log proves the new labels printed `ok`.

## Review cycle 5 (by Geoffrey's decision (a)) — approved under that decision, H1 seeded (2026-11-09)

- **Reviewed commit:** 00b8783. **Mode:** scoped to the G1/G2 fix (`1f97122..00b8783`), plus a regression pass; one blind reviewer (xhigh). Evidence: `$REPO/.local/pane-arrangements/review5/`.
- **Verification run:**
  - `cargo test`: 29 + 9 + 13 passed, 1 ignored.
  - `keys.mjs` 87/0, `history.mjs` 105/0, `panes.mjs` 68/0 (the three new checks included, no skip lines).
  - Probes `probe5*.mjs`: the Log off screen in all four presets 24/24; Files off screen in all presets the same as on screen; live commits under the floor drawn.
  - Mutants: M1 (no draw in `apply`) red on 4 checks; M3 (live only when visible) red on 1 check; M2 (no latch) passes the suite (see C1). Each was restored, `cmp`-equal, and checked with a control run; the tree is clean.
- **G1, G2:** fixed on every route named. R8, R9, Q20, R11, R15, R3 and the earlier F1 routes hold. No regression. § 1 and § Conventions are clean, and no high-blast-radius path is touched.

### [MEDIUM/CONFIRMED] H1 · An off-screen Log under the floor, shown again, keeps an outdated row mark
- **Where:**
  - `nav.rs:112-115`: `set_view` draws only if visible.
  - `panes.rs:143-149`: the latch draws only when the off-screen key view changes.
  - `log::key` (`log.rs:330-349`) steps from `.on`.
- **Failure scenario:** the Log's pane is under the 200 px floor. After `l`, `j j j`, `[` moves the Inspector to row 1, but the mark stays on row 2, so `j` opens row 3 (on screen it opens row 2). After Esc, then `l` or the LOG tab, `j` opens row 2 (on screen, row 0).
- **Evidence:** `probe-base.log` (P1, P2), reproduced again in `control-probe.log`.
- **Classification:** a new edge route of G1's kind: an off-screen key view stepping differently from on screen. Only the narrow floor reaches it. It is not a regression of main.

### Coverage observation C1 (LOW)
Nothing in the suite guards against repeated full Log redraws during a sash drag with the Log off screen. Without the latch, a 30-move drag costs 30 full renders, against 0 at HEAD. HEAD behaves correctly.

### Out of scope (noted)
`live.rs:85-87` re-renders the Log on every commit even when no pane holds it: one render per commit, harmless.

### Triage (foreman, 2026-11-09, under Geoffrey's decision (a))
Review 5 found only a new edge route of G1's kind (H1) and a coverage gap (C1). Geoffrey's (a) said: seed such routes and release 0.4.0. So H1 and C1 are seeded (an `issues/` file and a ROADMAP entry each), and the verdict is **approved under that decision**. On to publish and 0.4.0.

## Human-gate triggers (cycle 5)

- None.
