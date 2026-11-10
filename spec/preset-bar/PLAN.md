# PLAN — No preset bar: the crumbs move into the header

> **Status:** Draft for review · **Last updated:** 2026-11-09
> **Authoritative technical design.** The *how*. Vision/contract is [`GOAL.md`](GOAL.md);
> the phased executable roadmap is [`TECH.md`](TECH.md). No `research/`: the lean path (`kind: fix`,
> `appetite: small`); the planner's own reads and measurements are in § 4. Every design element
> traces to a GOAL R-ID.

## 1. Summary

Delete the `nav.fkeys` row and its grid track, move `#crumbs` into the header between the GO form
and the chips, and give the crumbs only the room GO leaves once it reaches its 900 px cap, with a
zero footprint when there is none, so neither GO nor the chips ever move for them. Drop the two
pieces of client code that served the buttons (the `.on` mark in `panes.rs`, the click in
`keys.rs`) and the two `Preset` methods only they used. In `tests/ui/panes.mjs`, replace the window
bar's check with one grading each preset key re-applied after a drag. One phase: the change is a few
dozen lines, and every requirement is graded on the same rendered page.

## 2. Design

### 2.1 The page (`okt-client/static/index.html`, `app.css`)

- `index.html`: remove `<nav class="fkeys">` (lines 37–45) with the four `.pre` buttons and
  `#presets`; put `<div class="crumbs" id="crumbs"></div>` inside `header.top`, after
  `form#goform` and before `div.chips`.
- `#app`'s two `grid-template-rows` rules (`app.css:104-105`) lose their `28px` track:
  `38px minmax(0, 1fr) var(--ticker-h) 22px`, and `… 0 22px` under `.noticker`. `main#panes` is the
  second row and starts at the header's bottom edge (38 px), 28 px taller than in 0.4.0.
- The `/* function keys */` section (`app.css:139-155`: `.fkeys`, `.presets`, `.pre`, `.pre kbd`,
  `.pre:hover`, `.pre.on` and the old `.crumbs` rules) is deleted. The crumbs' rules move into the
  top bar section, with a comment saying why the margin is there:
  - `.go`: `flex: 1` becomes `flex: 0 1 900px`; `min-width: 120px` and `max-width: 900px` stay.
  - `.crumbs { flex: 1 1 0; min-width: 0; margin-left: -10px; display: flex; align-items: center;
    gap: 4px; color: var(--dim); font-size: 11.5px; overflow: hidden; white-space: nowrap; }`
  - `.crumbs::before { content: ""; flex: none; width: 6px; }`, and the existing
    `.crumbs button`, `.crumbs button:hover` and `.crumbs .sep` rules unchanged.
- The `@media (max-width: 900px)` rule hiding `.word, .kbname, .crumbs` is unchanged (R5).

**Why the flex values hold R3 and R4 (the rule, rederived).** Let *H* be the header's width,
*B* the brand's, *C* the chips', and *g* = 10 px the header's gap, with 8 px padding each side.
- GO's base size is 900 px and it alone can shrink (brand and chips are `flex: none`; the crumbs'
  base is 0 with `min-width: 0`, so they have nothing to give). When the row is short, GO absorbs
  all of it, down to 120 px.
- The crumbs alone can grow (GO's grow is 0). When the row is long, they take the whole surplus.
- Their outer footprint is *g* − 10 px + *w* = *w*: the negative margin takes back the gap before
  them. So GO's width is min(900, max(120, *H* − 16 − *B* − *C* − 2*g*)), exactly 0.4.0's, and the
  crumbs' width is max(0, *H* − 16 − *B* − *C* − 2*g* − 900).
- Nothing in either formula reads the crumbs' content, so GO and the chips can't move when the
  selection changes (R4), and the crumbs' box always sits between GO and the chips (R3).
- The `::before` spacer and the 4 px gap put the first crumb 10 px after GO, where the gap would
  have put it. Clipped by `overflow: hidden`, the trail never paints over the chips.
- Measured on the prototype with the KB name the live okt shows (`~/KB`): *B* ≈ 136 px and
  *C* ≈ 267 px, so GO reaches its cap at *H* ≈ 16 + 136 + 20 + 900 + 267 = 1339 px, and the crumbs
  get ≈ 261 px at 1600 px. That is the GOAL-gate ruling's "about 1350 px and wider". Below it they
  are a zero-width box: hidden by having no room, not by a rule (deviation D3).

### 2.2 The client (`okt-client/src`)

- `panes.rs:125-129`: delete the loop that marks `#presets [data-preset]` with `.on`, and the
  `arrange::which` call that fed it. `cfg` stays imported (used at 84, 393 and 474).
- `keys.rs:128-135`: delete the click branch for `[data-preset]`. The `Preset` import stays for
  the keys (`Preset::from_key`, 551).
- `arrange.rs`: delete `Preset::name` and `Preset::parse`, and the test assertion
  `assert_eq!(Preset::parse(p.name()), Some(p));` in `presets_hold_every_view_once`. With the
  button gone nothing outside the tests calls them, and the wasm build warns that both are never
  used (seen on the prototype; the build is warning-free without them). The presets' trees,
  sizes, keys, cycle order and saving are untouched (a GOAL non-goal); `Preset::key`,
  `from_key`, `next`, `tree` and `ALL` stay.
- **`arrange::which` stays**, unchanged and `pub`. `panes.rs` no longer calls it, but `cycle`
  does (`which(n, c).map(Preset::next)`): it is how `p` finds the preset the arrangement equals.
  The native tests keep grading it. Removing it would mean inlining it into `cycle` for no gain.
- `nav::render_crumbs` is unchanged: it writes `#crumbs` by id, wherever the element sits.
  `chrome.rs`'s help rows already list every key (R6); unchanged.
- § Invariants 1: the change only removes code. The deleted `panes.rs` loop held a `cfg()` borrow
  only for the length of one expression; nothing new borrows `S`, settings or the graph state.

### 2.3 The suite (`tests/ui/panes.mjs`) and prose

- The PRESETS header lines (19–21) say "by key and by the cycle key, and each key again after a
  drag", with no window bar.
- Delete `marked` (725), the `strip` collection (728, 738) and `none` (758), and replace the
  window bar's check (779–786) with the check **"each preset key applied again after a drag
  restores its rects"**, label verbatim, since the gate requires it by name. On the 1600 px page
  already open there, for each of classic, swapped, stacked and three:
  1. press the preset's key;
  2. drag the first visible sash 120 px across its own line;
  3. assert that the drag moved the arrangement off the preset (`grade` non-empty), so a no-op
     drag can't pass;
  4. press the same key again and assert `grade` is empty.

  Every other preset check (each key, the cycle, from-none, the reloads, the resize, the help
  rows, the grid) stays as it is.
- The first-load check (`panes.mjs:156`) pins the root's height at 890 px: 1000 less the 38 px
  header, the 28 px row, the 22 px ticker and the 22 px status bar. It becomes **918**, with a
  comment deriving it. The prototype's first full gate run went red on exactly this line
  (`"H":918`), so the literal stays a literal: it is the durable suite's own guard that the row
  doesn't come back.
- `verify/panes-removed.txt` lists every line P1 removes from `panes.mjs`: the three header lines,
  `marked`, the `strip` and `none` lines, the old check's eight lines, and the 890 line. The gate
  refuses any other removal (§ 4.1, H1).
- No check reads a preset button afterwards (R7). README § Tests and `AGENTS.md` § Build and test
  already say "the presets", never a bar, so they need no change. A tracked-prose grep over
  README, `AGENTS.md`, `tests/` and `okt-client/` finds "window bar" only in the header lines above
  and `fkeys` only in the deleted markup and CSS (2026-11-09).
- `issues/drag-to-dock.md:46` says "If the preset bar goes, drag-to-dock becomes the main mouse
  path". It is a landed seed, so P1 appends a dated line under it, `- 2026-11-09: the preset bar is
  gone (preset-bar); …`, per code-build's append-only rule, and leaves the text above as it was.
  `issues/preset-bar.md` and ROADMAP § 7 are the seed of this cycle; retiring them is
  `code okt roadmap`'s job after the landing, never this cycle's.

### 2.4 Checks that hard-coded the panes' top or height

One: `tests/ui/panes.mjs:156`, `near(s.H, 890, 1)` in the first-load check, fixed in § 2.3. A
first search by literal (`38`, `66`, `28` and nearby sums) missed it, and the prototype's first
full gate run found it.

`spec/pane-arrangements/verify/rects.mjs:299` carries the same 890. It is a landed record, which
the GOAL leaves as it is, and no gate of this cycle runs it, so it stays. Run against a page from
this cycle, it would fail on that line alone.

Everything else was searched again in `tests/ui/*.mjs` and `spec/*/verify/` (2026-11-09):
- every pane rect in `panes.mjs` is taken relative to `#panes`'s own rect (`snap`, `R.top`, `H2`);
- `keys.mjs` and `history.mjs` compare elements with each other (`insp` against the Files pane,
  `#ibody` and `#itabs`);
- `spec/pane-arrangements/verify/rects.mjs` (a frozen record) measures from `#panes` too, and its
  crumbs check (733, hidden at 899 px) still holds with the crumbs in the header.

Apart from 890, the only literal heights are the 1000 px viewports and the 24 px title bar, and
neither moves.

### Requirement → design map

| R-ID | Design element(s) that satisfy it |
|------|-----------------------------------|
| R1   | § 2.1: the row and its 28 px track deleted; `#panes` is the grid's second row, so its panes start at the header's bottom at every preset and width |
| R2   | § 2.1: the four `.pre` buttons and `#presets` deleted; § 2.2: nothing creates or marks one |
| R3   | § 2.1: `#crumbs` inside `header.top` between GO and the chips; zero footprint, clipped; the header's grid row stays 38 px and `flex-wrap` stays off |
| R4   | § 2.1: GO's width independent of the crumbs (base 900, sole shrinker); the crumbs take only the surplus, ≈ 261 px at 1600 px |
| R5   | § 2.1: the existing ≤ 900 px rule already hides `.crumbs` |
| R6   | § 2.2: the keys, `p` and the `w` leader untouched (`keys.rs` 549–557, `nav::preset`, `cycle_preset`); help rows and README § Keys unchanged |
| R7   | § 2.3: the window bar's check replaced by the re-applied-after-drag check; the header lines rewritten; no prose names a bar |

## 3. Invariant gate (AGENTS.md constitution check)

Walked before the design (from the GOAL) and again against this draft.

- **§ High-blast-radius paths.** None touched: no `http.rs`, `launch.rs`, `index.rs` or
  `state.rs` edit. Research trigger 2 does not fire.
- **§ 1 Client re-entrancy.** Touched only by deletion (`keys.rs`'s click branch, `panes.rs`'s
  mark loop). No new borrow of `S`, settings or graph state; no new handler or `await`.
- **§ 2 Pygments data.** Not touched.
- **§ Conventions.**
  - `app.css` with `graph.rs`: the change touches no graph rule (only the top bar and the deleted
    row), so `graph.rs` has nothing to follow. The Graph canvas refits to its taller pane through
    the existing `ResizeObserver`; `panes.mjs`'s fitting checks grade it.
  - `.trow`, `@container tcols` and `keys.mjs`'s threshold table: not touched.
  - The view row tracks and `keys.mjs`'s `VIEWS`: not touched.
  - No spec ids in source or test names: the gate scans the added lines.
  - Commit subjects: the profile's `[fix] Build preset-bar P1`.
- **Profile `coupled core`.** `app.css` is in it, so P1 is `parallel: false`.

### Deviation justifications

No invariant is bent. The rows below are readings of R-IDs the design carries, and one piece of
added complexity. Each was read against the R-ID's wording; none changes what an R-ID SHALL do,
but D1–D3 are for the foreman to confirm at sign-off.

| Deviation | Why needed | Simpler alternative rejected because |
|-----------|-----------|--------------------------------------|
| **D1 (R4, last sentence).** The gate grades "a card selected in a 1600 px window shows its crumbs" with `#kbname` set to `~/KB`, the name the live okt shows for the default KB, on both sweep pages. Every R3/R4/R5 width check of the sweep runs with that name too. | The throwaway clone's name is its temp path, about 283 px wide (measured). With it, the crumbs get 5.8 px at 1600 px: a non-zero width, but only the 6 px spacer and no crumb. R4 holds literally there, and that reading would also pass a page where no crumb ever shows. | Grading with the clone's path (passes R4's words while showing nothing), or moving the clone to a short path (`tests/env.sh` is the profile's helper, out of this cycle's scope; a short `$TMPDIR` still gives about 170 px). The residual is recorded in § 5: a user whose KB name is long sees fewer crumbs at 1600 px, as the GOAL-gate ruling accepts. |
| **D2 (R4, "do not shrink for them").** The gate also grades a counterfactual: at every width, GO's and each chip's width with the crumbs taken out of layout (`display: none`, set in-page, then restored) equal their widths with the crumbs in, within 1 px. The design meets it with `margin-left: -10px` and a 6 px `::before` spacer. This is the one piece of added complexity. | Deep-against-empty alone can't see a cost the crumbs impose whatever they hold. A plain flex item with base 0 keeps a 10 px gap, so GO is 10 px narrower than 0.4.0's wherever it is under its cap (901 to about 1339 px). That is GO shrinking for the crumbs, against the GOAL-gate ruling "GO keeps its full flexible width". | The plain gap: one rule less, but GO loses 10 px for an element that is usually empty there. The gate's `gap10` mutant is exactly that alternative, and must go red. |
| **D3 (R3, "SHALL render inside the header's box … from 1600 px down to 901").** Read with the GOAL-gate ruling ("hide below about 1350 px"): at every width from 901 to 1600 px the crumbs keep a box (`getClientRects()` non-empty) inside the header, after GO and before the chips. Below about 1339 px that box is zero wide, hidden by having no room. The gate refuses a `display: none` crumbs anywhere above 900 px. | Taken alone, R3's words want the crumbs rendered across the whole range, and the ruling lets them hide below about 1350. A zero-width box in the slot satisfies both. | Accepting "no box" as satisfying R3 would let a crumbs hidden at every width pass R3 vacuously (only R4's 1600 px clause would catch it); the `gone` mutant tests that. |

Figures above are measured on the prototype (2026-11-09) or derived from § 2.1's formula: 283 px
and 5.8 px from the run with the clone's name, 261 px from the formula with *B* = 136 and
*C* = 267, which the `~/KB` run confirms.

## 4. Rabbit holes (resolved)

The lean path (no fan-out). Each item below was settled by the planner's own reads and a prototype
(`.worktrees/preset-bar-prototype`, detached at the GOAL commit `fac7e93`, the design applied).

- **How much room the crumbs get.** The flex arithmetic in § 2.1, measured. The surprise was the
  throwaway environment: its KB name is a temp path that eats about 280 px of the header (D1).
- **What the removal leaves dead.** The wasm build warned that `Preset::name` and `Preset::parse`
  are never used. Removed (§ 2.2), the build is clean, and the gate holds it so ("the client
  build printed no warning"). `arrange::which` is still live through `cycle`.
- **Checks that pin the panes' top or height.** One, `panes.mjs:156` (§ 2.4). Only the dry run
  found it.
- **Hit-testing at the header's edge.** `elementFromPoint` at a fractional row 0.5 px above the
  header's bottom returned the pane's title bar (Chrome rounds the point). The gate samples whole
  rows instead (1 px above; ten rows from 1 to 23 px below), and grades the 0.5 px tolerance on
  the rects.
- **The help's two-key rows.** `w w  w W` is one `<kbd>`, and `innerText` collapses its double
  space. The gate reads `textContent`.
- **Adversarial design review (trigger 3: the deliverable includes test and gate code).** One
  fresh reviewer ran against the GOAL, this design and the drafted `verify/` scripts; its findings
  and what they changed are in § 4.1.

### 4.1 Adversarial review

One fresh reviewer read the GOAL, this design, the drafted `verify/` scripts and the prototype's
diff (2026-11-09). It confirmed the flex arithmetic and the R-ID domains. Its findings were
3 HIGH, 5 MEDIUM and 6 LOW, all about the gate; each is folded in as follows.

- **H1. A check could be loosened under its old label.** `kept.mjs` sees only that a label still
  prints `ok`. Fixed in `static.sh`:
  - `keys.mjs` and `history.mjs` must be unchanged since the plan commit;
  - every line removed from `panes.mjs` must be listed in `verify/panes-removed.txt`, no more often
    than it is listed there.
- **H2. The chips could move with the selection while keeping their widths.** One example is
  crumbs that absorb the deficit and are content-sized above GO's cap. The sweep now compares GO's
  and each chip's left edge on both pages too (`shift` mutant).
- **H3. An icon-only chip titled with a key passed R2.** In the header, any clickable outside GO,
  the crumbs and the five chips now counts as a preset button (`iconchip` mutant).
- **M1. KB-data regions skipped the whole R2 test, the crumbs among them.** There, only the name
  test is skipped now, so `data-preset` and preset-key kbds still count (`datapreset` mutant).
  `static.sh` also refuses `data-preset` or `id="presets"` anywhere under `okt-client/`.
- **M2. The crumbs' visibility was graded only at 1600 px.** At every width where GO leaves room
  of 40 px or more, the deep card's crumbs must now be visible, fill the room but for the gap
  before the chips, and hit-test at their first segment (`nogrow` mutant). The room is the
  header's content box less GO and the chips. The first draft measured the gap between GO and the
  chips instead; the dry run showed `nogrow` slipping through it, because without growth the
  surplus sits after the chips.
- **M3. R1 had blind spots.** The band check used to skip all of `#panes` and the header,
  pseudo-elements went unseen, and the hit rows sampled only 1 and 12 px. Now:
  - the band skips only panes, sashes and their ancestors;
  - out-of-flow `::before`/`::after` on the header, `#app` and `#panes` are flagged;
  - ten hit rows run from 1 to 23 px.

  Mutants: `pseudo`, and `ghost` (an invisible strip only the hit rows see).
- **M4. Mutants to prove each new decision bites:** `ghost`, `nogrow`, `datapreset` and `shift`
  were added, 23 mutants in all.
- **The self-test's own bugs, found by its first run (dry run 2).** Two of them hid 15 mutants:
  - the JS mutants ran through `eval`, which okt's CSP refuses. They now ride in the init
    script's own text;
  - three expected labels lacked their `from 1600 to 901 px` prefix.

  Dry run 3 killed all 23, each on its expected line, with the clean page green (TECH § Gate
  timings).
- **M5. The plan commit could be matched by a body line.** It is now found by its subject alone
  (`static.sh`, `ledger.sh`).
- **L1.** A `$TMPDIR` ending in `/` hid Chromium from the leftover check; the gate strips the
  slash. The self-test dry run uses such a `$TMPDIR` on purpose.
- **L2.** A build failure after a busy-port retry was misreported as the environment; the line was
  dropped (`envrun` handles the busy port).
- **L3. A live event could redraw one sweep page's status.** A width that differs is measured
  again once, 600 ms later, with the KB name written again. `#offbar`, fixed 44 px from the top,
  would now cover the panes if the event stream dropped, and the band check would go red; that
  is a real visual regression, left as a red.
- **L4.** Dead `.fkeys`/`.presets`/`.pre` rules are refused by `static.sh`.
- **L5. R5 can't fail against this design.** At 900 px and below the crumbs have no room anyway.
  The `narrow` mutant grades a crumbs given room there; no change.
- **L6, sampling.** R2 is now scanned every 10 px of the sweep, not every 50.
- **L6, after landing.** On `main` after the landing, the gate goes red by construction: the merge
  base is HEAD, and the replaced label is gone. The gate is the branch's, and is never run on
  `main`.
- **For the foreman, not the gate.** With no selection, `render_crumbs` writes the KB's display
  name into the crumbs (`nav.rs:396-397`). At about 1339 px and wider the header then shows the KB
  name twice: in the brand, and again right after GO. The crumbs' content is a non-goal, so the
  design leaves it; question Q4.

## 5. Risks & open questions

- **A long KB name leaves the crumbs little room.** With a KB whose displayed name is long (a deep
  path outside `~`), GO's cap is reached later and the crumbs show less, or nothing, at 1600 px.
  This is the GOAL-gate ruling's trade ("only the space GO leaves"). The Inspector still shows the
  path.
- **R2's scan skips KB-data regions:** the Files tree, Log, Table, Results, the Inspector's head and
  body, the crumbs, the ticker, the legend and the tip. A file or commit may well be called
  "three", so scanning them would flag data. A preset button planted inside one of them would
  pass. Nothing in the design puts chrome there.
- **The R6 and R7 behaviour checks live in `panes.mjs`, which build edits.** The gate requires the
  new check by name and every kept check by label (`kept.mjs`, `checks.txt`). Build's fail-once
  mutation proves the new check bites (TECH P1).
- Questions for the sign-off, none of which blocks the design:
  - **D1–D3** above;
  - **Q4:** the KB name shown twice with no selection at about 1339 px and wider (§ 4.1);
  - **Q5:** the build is held to exactly § 2.3's removals from `panes.mjs`, and to no change of
    `keys.mjs` or `history.mjs` (H1). If build needs another removal, the gate goes red, and the
    fix is a plan amendment (a new `[fix] Plan preset-bar:` commit), not a gate edit.

## 6. Verification strategy

One gate, `spec/preset-bar/verify/gate.sh`, frozen with this commit. Its run log is kept at
`$REPO/.local/preset-bar/gate[-<mode>].log` (`$REPO` the main checkout, `.local/` git-excluded)
with a line per run in `runs.txt` beside it.

1. **`static.sh`**:
   - committed state, and the gate frozen at the plan commit;
   - no spec ids in the added lines;
   - README § Keys names all 14 keys (R6);
   - `panes.mjs` reads no `#presets`, `data-preset` or `.pre`, and its PRESETS block names no
     window bar or button (R7);
   - no tracked prose in README, `AGENTS.md`, `tests/` or `okt-client/` says "window bar",
     "preset button(s)", "preset bar" or `fkeys` (R7);
   - every existing check label of `keys.mjs`, `history.mjs` and `panes.mjs` is still present,
     the replaced one gone (`kept.mjs`);
   - `keys.mjs` and `history.mjs` unchanged, and only `panes-removed.txt`'s lines removed from
     `panes.mjs`;
   - nothing of the bar left under `okt-client/` (`data-preset`, `id="presets"`, the `.fkeys`,
     `.presets` and `.pre` rules);
   - the deferral ledger (`ledger.sh`).
2. **Unit tests:** the profile's `cargo test` of the three crates.
3. **One `tests/env.sh` run:**
   - `keys.mjs`, `history.mjs` and `panes.mjs` each exit 0;
   - `page.mjs` (below) exits 0;
   - the client build printed no warning;
   - no leftover process;
   - every kept label printed `ok`, the replaced one absent;
   - every `checks.txt` label printed `ok`, the new re-applied-after-drag check among them.

**`page.mjs`** grades the rendered page across each R-ID's whole domain:
- **stack** (R1, R2):
  - at 1600 and 900 px, every preset by its key, with its pane shapes checked, then a dragged and
    a collapsed arrangement;
  - the panes' and `#panes`' top within 0.5 px of the header's bottom;
  - hit rows across the whole width every 2 px, 1 px above and ten rows below;
  - no visible element but the panes, the sashes and their ancestors in the 24 px band, and no
    out-of-flow pseudo-element on the header, `#app` or `#panes`;
  - no preset button.
- **sweep** (R3, R4, R5, R2), on an empty selection and the deepest card (8 crumb links) side by
  side, at 3000, 2400 and 1800 px and then at every width from 1600 down to 400 px:
  - the header one 38 px row;
  - the crumbs' box in the slot from 1600 to 901 px, neither painting nor hit-tested over GO or a
    chip;
  - GO's and each chip's width and left edge equal on both pages, and their widths equal with the
    crumbs out of layout (D2);
  - the deep card's crumbs visible and filling any room GO leaves of 40 px or more;
  - the crumbs, brand word and KB name hidden at 900 px and below;
  - the deep card's crumbs visible and hit-tested at 1600 px (D1);
  - no preset button every 10 px.
- **help** (R6): every key's row in `?`.

**Self-test** (`gate.sh --self-test m`, plan and review only). 23 page mutants, each injected
before load, each must turn `page.mjs` red on its expected line:
- the row back, holding a preset button;
- a 20 px strip over the panes, a 2 px strip under the right half only, an invisible strip taking
  the title bars' clicks, and a bar hung under the header by `::after`;
- a 1 px gap under the header, and a 46 px header;
- a THREE chip, a chip holding only the backtick key, an icon-only chip titled with a key, a
  pointer span naming a preset, a `data-preset` button in the Inspector's head;
- a swallowed key;
- crumbs sized by their content, crumbs moving the chips, crumbs never growing, crumbs keeping
  their gap (D2), crumbs after the chips, crumbs never shown, crumbs unclipped, a wrapped header,
  crumbs shown at 900 px;
- a help row removed.

**Build's fail-once mutation** (code-build Step 4; specified here, run by P1). It targets the
phase's own new check:
- In `keys.rs`'s preset key branch, ignore a key that names the preset the last key press
  applied, through a `thread_local` holding it.
- `panes.mjs` must print `FAIL each preset key applied again after a drag restores its rects`.
- Revert, and record it in the commit body.

The full run takes about 10 minutes (TECH § Gate timings), so `--part keys` and `--part panes`
split its step 3 for a re-run under the 10-minute command cap. The phase's verify is still the
full run. The cheaper `--probe` mode (page.mjs alone) serves a mutation of the page, such as dropping the
crumbs' negative margin, which the `gap10` mutant predicts red.

---

*Backing research: none (lean path); the prototype dry runs are recorded in TECH.md § Gate timings.*

## PLAN gate (foreman, 2026-11-09, under Geoffrey's word in session)

Signed off. Rulings:
- **D1, D2, D3:** accepted as read.
- **Q4 (the KB name shown twice when nothing is selected):** accepted for this cycle. What the
  crumbs show is a GOAL non-goal; it goes to Geoffrey as a follow-up if he dislikes it.
  Seeded by build P1 as [issues/crumbs-kb-name-twice.md](../../issues/crumbs-kb-name-twice.md)
  (the deferral ledger).
- **Q5 (panes.mjs removals limited to § 2.3; keys.mjs and history.mjs frozen):** accepted. Any
  other removal needs a plan amendment, never a gate edit.
- **Removing `Preset::name` and `Preset::parse`:** accepted.
- **R2's name test skipped inside KB-data regions:** accepted as a recorded residual.
- **`#offbar` over the panes' top while the event stream is down:** accepted as a known visual
  effect of the removed row, outside R1's steady-state grading. It goes to Geoffrey with the cycle's
  report.
- **`.worktrees/preset-bar-prototype`:** not removed with `--force`. Geoffrey decides at landing.
