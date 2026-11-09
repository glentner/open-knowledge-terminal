# PLAN — History: one line per commit, under the Log's day rules

> **Status:** Draft for review · **Last updated:** 2026-11-08
> **Authoritative technical design.** The *how*. Vision/contract is [`GOAL.md`](GOAL.md);
> the phased executable roadmap is [`TECH.md`](TECH.md). Lean path (`appetite: small`): no
> `research/`. Every design element traces to a GOAL R-ID.

## 1. Summary

History keeps its one row builder, `hrow`, but its rows get a modifier class, `hl`, and show the
time (`fmt::time`) where they showed the date and time. A new `hrows` puts the Log's own
`.dayrule` markup before each run of one day's rows, and every History listing goes through it.
One added stylesheet rule, scoped to `.hrow.hl`, makes the row one 21px line, and one container
query on `#ibody` hides the counts where the inspector is too narrow for them (the GOAL's
narrow-inspector clarification). The Overview's
"Last commit" row writes its own markup with the plain `.hrow` class, so no existing rule changes
and it renders as before. One phase: two source files, a browser suite, two doc lines.

## 2. Design

**`okt-client/src/insp.rs`** (all changes are below `// ---- history`; nothing above it moves):

- `history_view`: both listings stop looping over `hrow` and call `hrows` once. Repository
  history hands it the first 300 commits (`&data.commits[..len.min(300)]`); a file or folder hands
  it all of `path_history`. The header line, the sparkline, OPEN THE FULL LOG, the empty and
  untracked messages and the surface message are emitted exactly as today, before (or after, for
  OPEN THE FULL LOG) the rows.
- New `fn hrows(commits: &[Commit], n: Option<&Node>) -> String`. It walks the commits in the order
  given, in runs of consecutive commits with the same `fmt::day_key(c.t)`. Before each run it emits
  the Log's rule markup, `<div class="dayrule">{esc(js::date_string(t))} · {run length}</div>`
  (`t` of the run's first commit; the same text `views/log.rs` writes), then `hrow` for each commit
  of the run. A run's length is the N of R2: the rows under that rule. It is synchronous string
  building, with no `S` access.
- `hrow`: the row's class becomes `hrow hl`; the `.dt` cell holds `fmt::time(c.t)` (local 24-hour
  `HH:MM`, the Log's time) instead of `fmt::dt(c.t)`. The cell order (`.h .dt .st .sj .rt`), the
  `data-commit`, the `title` on `.sj`, the renamed-path note (`.px`) and the `@` button
  (`data-at`, `data-atpath`, shown on a file's rows unless the status is `D`) are unchanged.
  `fmt::dt` stays: the Overview's "Last commit" row still uses it.
- The Overview's "Last commit" markup (`insp.rs:775`) is not touched.

**`okt-client/static/app.css`**: a rule and a container query added directly after the `.hrow`
block, nothing edited:

```css
/* History's rows: one line, the Log's row height (the Overview's Last commit keeps the plain .hrow) */
.hrow.hl { grid-template-columns: 64px max-content 18px minmax(0, 1fr) auto; height: 21px; padding: 0 12px; align-items: center; white-space: nowrap; }
/* #ibody narrower than a History row with no subject and counts of 16 characters (+999,999 −99,999):
   the counts go; hash, time, status and @ stay */
@container (width < 312px) { .hrow.hl .plus, .hrow.hl .minus { display: none; } }
```

**Amended 2026-11-08 (review cycle 1, F1; the CSS block above and the bullets below are the
cycle's first build).** The `auto` actions track and the 312px threshold, sized for 16-character
counts, hid real counts (5–9 characters) while the subject still had room, and a query on `#ibody`
reads its width with its 10px scrollbar in it (measured: counts shown at a 281px `#ibody` content
width against a 287.5px threshold). Now:
- `hrow` wraps the counts in `<span class="ad" title="+a −d">` inside `.rt`; the cells stay
  `h, dt, st, sj, rt`.
- `.hrow.hl` takes `max-content` for `.rt` and `container-type: inline-size`: each row is its own
  query container, so the query reads the width its cells share, scrollbar or not.
- `.hrow.hl .rt` is a fixed 110px flex cell, `gap: 6px`: the counts' box `.ad` at the Log's 96px
  (`overflow: hidden; text-overflow: ellipsis; text-align: right`), then a slot for `@` (7.5px, its
  6px margin dropped for the gap) on every row, `@` or not, so every row's fixed parts are equal.
  96px holds 12 characters; 1862 of the KB's 1865 commits have counts of 11 or fewer, and the 3 of
  13–14 end in "…" with the full text on hover.
- The threshold, in the row's content box: 64 hash + 37.5 time + 18 status + four 8px gaps + 110
  actions = 261.5px (287.5px of `#ibody`, with 24px padding and the 2px border). `@container (width
  < 261.5px)` sets `.rt` to `width: auto` and hides `.ad`, so the counts hide exactly where the
  subject has reached 0. A row then needs 185px of `#ibody` with `@`, 177.5px without: inside the
  901px window's narrowest with its scrollbar drawn (188px).
- `tests/ui/history.mjs` sweeps that band with the scrollbars drawn (§ 6 amendment).

**Amended 2026-11-09 (review cycle 2, N1; GOAL Clarification "How wide is the counts cell…?", 2026-11-08).**
The 96px box cut the KB's three commits with counts of 13–14 characters at every width. The
counts cell now holds 14 characters; the per-row container, the shared `@` slot, the ellipsis and
the `title` are unchanged, and so are the Log and the Overview.
- `.hrow.hl .ad` is 105px: 14 characters at the History font's 7.5px advance (12.5px monospace),
  measured in the browser as 105.00px for "+12,345 −6,789", the `−` included.
- `.hrow.hl .rt` is 105 counts + 6 gap + 7.5 `@` slot = 118.5px.
- The threshold, re-derived as before, in the row's content box: 64 hash + 37.5 time + 18 status +
  four 8px gaps (32) + 118.5 actions = 270px, so `@container (width < 270px)`. In `#ibody` that is
  270 + 24 padding + 2 border = 296px, scrollbar excluded (each row is its own container).
- Below it nothing changes: `.rt` goes to `width: auto` and `.ad` hides, so a row needs 185px of
  `#ibody` with `@`, 177.5px without.
- The frozen probe's widths still bracket it: 198px (the 901px window's narrowest, counts hidden)
  < 296px < 319px (22% of a 1450px window, counts shown).
- Counts of 15 characters or more are cut with "…" and keep their full text in `title`.

**Narrow inspector** (R3's narrow-width clause, GOAL Clarifications, 2026-11-08). `#ibody` is
already `container-type: inline-size` (`app.css:448`), so the query follows the pane's content
width, scrollbar excluded, not the window. The subject's `minmax(0, 1fr)` track yields first. Below
312px the counts hide. The figure comes from the tracks, at the body font's 7.5px advance (12.5px
monospace).
- **Fixed parts:** 12 + 12 padding, a 2px border, the 64px hash, 37.5px time, 18px status and four
  8px gaps make 177.5px. The `@` button adds 13.5px (a 6px margin and one character).
- **Check against the prototype:** at the 1600px window's narrowest, `#ibody` was 352px wide. That
  holds 191px of fixed parts, a 116px subject and 45px of counts ("+26 −0", six characters with the
  space), with `.rt` measured at 59px.
- **The threshold:** with no subject and counts of 16 characters ("+999,999 −99,999", 120px), a row
  needs 177.5 + 120 + 13.5 = 311px, so the counts hide below 312px. The widest counts in the KB's
  last 2000 commits are 14 characters ("+21,899 −7,071").
- **Without counts:** the leading space before the hidden spans collapses at the start of the line,
  so a row needs 191px. That fits the 198px of the split's narrowest in a 901px window, with 7px
  left for the subject.

The 88px date column becomes `max-content` for `HH:MM`, as `.crow` sizes its time. It drops
`.crow`'s `minmax(0, …)`: a track allowed to shrink to 0 painted the time over the status at a 198px
inspector before the counts could hide. Now the counts hide first, so at the split's widths the two
render the same (§ 6). `height: 21px`
and `white-space: nowrap` are `.crow`'s. `.hrow .sj` already truncates with an ellipsis, and
`.hrow .rt` already holds the counts and `@` on one line. `.dayrule` and `.crow` are reused
unchanged.

**Sticky rules.** The rules and rows are direct children of `#ibody`, with no wrapper, the way the
Log's sit in `#ltable`. `.dayrule` is `position: sticky; top: 0`, so in the `#ibody` scrollport each
rule pins at the top until the next day's rule slides over it (later siblings paint above at the same
`z-index`), as in the Log. `#ihead` and `#itabs` are siblings of `#ibody`
(`index.html:81–87`), so a pinned rule cannot cover them. No wrapper may be introduced: a wrapper
with its own overflow breaks the pin, and one per day changes how rules hand over.

**`tests/ui/history.mjs`** (new): the repo's History suite, the History checks of this cycle's
frozen gate (`spec/history-log-rows/verify/history-rows.mjs`, without its `--baseline` mode),
run like `keys.mjs`: `tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/history.mjs"'`. No spec ids in
it, in any case. README (§ Layout `tests/ui/` row, § Tests block) and AGENTS.md § Build and test
name it in the same commit.

**Not touched:** `views/log.rs`, `fmt.rs`, `js.rs`, `keys.rs`, `state.rs`, `index.html`, the
server. `state.rs` is not needed, so Trigger 2 does not fire.

### Requirement → design map

| R-ID | Design element(s) that satisfy it |
|------|-----------------------------------|
| R1   | `hrows` emits a `.dayrule` before each run of one local day (`fmt::day_key`, the Log's grouping), in the order `history_view` already uses; same class as the Log's. |
| R2   | The rule text is `js::date_string(t) · {run length}`; the run is exactly the rows under that rule. |
| R3   | `hrow` cells in order `.h` (short hash), `.dt` (`fmt::time`), `.st`, `.sj` (+ `.px`), `.rt` (counts, `@`); no date anywhere in the row. Narrow-width clause: the subject's `minmax(0, 1fr)` yields first, then `@container (width < 312px)` on `#ibody` hides `.plus`/`.minus`; hash, time, status and `@` keep fixed or content-sized tracks. |
| R4   | `.hrow.hl`: `height: 21px`, `white-space: nowrap`, `max-content` time column; `.hrow .sj` ellipsis and `title`, unchanged. |
| R5   | Rules and rows are siblings in `#ibody`; `.dayrule` is sticky at `top: 0` of that scrollport, below the header and tabs, which are outside it. |
| R6   | `data-commit`, `data-at`/`data-atpath`, `data-view="log"`, the `h` key, header and sparkline are untouched and emitted before `hrows`. |
| R7   | The Overview row's markup and every existing rule are untouched; the new rule matches only `.hrow.hl`, which only History emits. `views/log.rs` is untouched. Graded rendered, against the base build (§ 6). |
| R8   | The phase gate (§ 6), with `keys.mjs` and the profile's unit tests. |

## 3. Invariant gate (AGENTS.md constitution check)

Walked before the design (from the GOAL) and again against it.

- **High-blast-radius paths** — none touched: no `http.rs`, `launch.rs`, `index.rs` or `state.rs`.
  Trigger 2 does not fire.
- **§ 1 Client re-entrancy** — `hrows` and `hrow` are pure string builders called after
  `history_view`'s existing `await`s; no new handler, no borrow of `S`, settings or the graph state
  is held across an `await` or a re-entrant call.
- **§ 2 Pygments data** — not touched.
- **§ Conventions** — `app.css` changes alone: the "change with `graph.rs`" rule covers the graph
  rules the stylesheet specifies for `graph.rs`, and the added rule is not one of them. No spec ids
  in source or tests. The new browser suite is named in README and AGENTS.md in the same commit.
  Subjects per code-build.
- **Coupled core** — `app.css` is in it, so the phase is `parallel: false`.

### Deviation justifications

| Deviation | Why needed | Simpler alternative rejected because |
|-----------|-----------|--------------------------------------|
| A day whose commits are not consecutive in History's order gets a rule before each run, and each rule's N counts its own run. Read against R1 ("a day rule before the first row of each day, keeping today's row order, so every row sits under the rule for its own day") and R2 ("N is the number of rows History shows under that rule"). | History keeps git's order (R1), and a commit's local day can repeat after a run of another day (rebased or back-dated commits). Only a rule per run keeps every row under its own day's rule and lets N count the rows under it. On the KB today every day is one run, so the two readings print the same. | One rule per day with the day's total: rows of the later run would sit under another day's rule, against R1, and N would not count the rows under the rule, against R2. Re-sorting by time breaks "keeping today's row order". |
| *(Settled.)* Row 2 proposed letting the counts and `@` spill past the inspector's right edge below a ~236px inspector. The sign-off did not accept it. The GOAL now carries the rule: R3's narrow-width clause and the Clarification "What does a History row do in an inspector too narrow for all of it?" (2026-11-08). The design meets it with the container query in § 2. | — | — |

## 4. Rabbit holes (resolved)

No `research/` (lean path). Settled by reading the code and dry-running the gate on a prototype
(`.worktrees/history-log-rows-prototype`, `origin/main` plus the design above):

- **Does a 21px row fit?** Yes. Body line-height is 1.45 × 12.5px ≈ 18.1px, inside 21px; every
  History row measured 21px at a 736px, 352px and 198px inspector and with the panes stacked.
- **Does the rule pin inside the inspector?** Yes: `#ibody` is the scrollport, `position: relative`,
  and History has no other sticky element. The prototype pinned the topmost day's rule at six scroll
  positions with the tabs unmoved.
- **How to grade R7 on what renders, against the base?** The fast server serves the client from
  disk and the assets are four files. The gate builds the base commit's client into
  `target/history-log-rows-base/` and loads the page twice from one server and one KB: as served,
  and with `app.css`, `main.js`, `okt.js` and `okt_bg.wasm` answered from the base dist. It compares
  the Overview row's outer HTML, box, cell boxes and computed style at three widths, and the Log's
  whole `#v-log` HTML, the first 80 row boxes and the rule and row styles. A probe proves the base
  assets were served: the base build's History time cell still carries a date.
- **Adversarial review.** Trigger 3 does not strictly fire (the deliverable is not a gate), but the
  R8 grade is a rendered measurement and easy to make hollow, so one fresh reviewer attacked the
  drafted gate, as the foreman asked. It confirmed that the no-wrap check belongs on the row and its
  cells, not on `scrollWidth <= clientWidth` of the subject: an ellipsis-cut subject always has
  `scrollWidth > clientWidth`, so that check would fail every cut subject and contradict "some
  subject is cut". Folded:
  - *HIGH, overlap and visual reorder.* A time column that can shrink (`minmax(0, max-content)`)
    paints `HH:MM` over the hash without wrapping or spilling, and CSS `order` can show the cells
    out of R3's order. Now, at every width, the cells' boxes must run left to right in markup order
    without overlapping, and every cell except the subject must show all its text.
  - *HIGH, what `--baseline` can't see.* Only the client assets are swapped, so `index.html`,
    `main.js` and the server are this tree's on both sides. `gate.sh` now fails if they, or
    `views/log.rs`, differ from the base.
  - *MEDIUM.* The style comparisons used hand-picked properties; now every computed property of the
    Log's rule, row and cells and of the Overview row and its cells is compared, plus its hover
    background. A date could hide in the subject cell; the subject must now be exactly the subject
    plus the renamed-path note. Fixed waits are now waits on the state each check needs. In the
    901px case the spill allowance no longer drops vertical containment (since superseded: the
    sign-off ruled out spill entirely, § 3 row 2).
  - *LOW.* "One line" no longer depends on the cell's own line-height (every line box starts within
    half an em of the first). The OPEN THE FULL LOG check is required, not skipped, and the KB must
    have more than 300 commits. Every rule must have rows. The Repository header must show the
    total. The routed `app.css` must be the base file. The base tree goes in a directory per commit
    from `git archive -o`, and the base is the merge-base with `origin/main`. Repository history is
    graded at the narrowest too. Pins are taken from the first two-thirds of the rows.
  - *Not taken:* a preference for a renamed file. The KB rarely has one, and `.px` is unchanged
    code.

  Each fix was tested against a mutant on the prototype (§ 6).

## 5. Risks & open questions

- **Counts wider than 16 characters.** A commit of a million or more changed lines in a pane just
  over 312px could still push its counts past the edge. The KB's widest are 14 characters. Raising
  the threshold hides counts in more panes; 312px keeps them at the 1600px window's narrowest
  (352px) and at 22% of a 1450px window (319px), and the gate grades both on Repository history.
- **The 7.5px advance.** The threshold is in px and assumes the 12.5px monospace body font. A
  `--font` with a wider advance moves the fit point; a theme that changes `--fs` would need the
  threshold rederived.
- **The KB changes under the gate.** The gate picks its file from the clone each run (a recent file
  with commits on two days, two on one day, all within the Log's first 1500 rows). A KB with no such
  file fails the gate loudly ("a file with commits on two days…"), never passes it empty.
- **Midnight.** The `--baseline` comparison renders the Log twice; the Log's chart extends to today,
  so a run that straddles local midnight can differ. Rerun.

## 6. Verification strategy

One gate, `spec/history-log-rows/verify/gate.sh`, run from the worktree root with
`OKT_TEST_PORT=6583` (never 6580; a parallel cycle holds 6581). Modes: `unit`, `browser`, or both.

1. `unit`: the profile's `cargo test -p okt-server -p okt-pygments -p okt-client`.
2. `browser`: fails if `index.html`, `main.js`, `okt-server/` or `views/log.rs` differ from the
   base (`git merge-base HEAD origin/main`, override `OKT_BASELINE`); builds the base commit's
   client into `target/history-log-rows-base/<commit>/`; then in one `tests/env.sh`: `keys.mjs`,
   the repo's `tests/ui/history.mjs`, the frozen `history-rows.mjs`, and `history-rows.mjs
   --baseline <base dist>`.

What `history-rows.mjs` grades, all on the rendered page:

- **Content** (file, folder, repository listings): rows in the API's order; a rule first; every row
  under the rule of its own local day (`toDateString`, the function behind `js::date_string`); each
  rule's N equals the rows under it; no two consecutive rules for one day; each row's cells are
  exactly `h, dt, st, sj, rt` with the short hash, the `HH:MM` the Log shows for that commit, the
  status, the subject (and its `title`), the counts, and `@` exactly on a file's non-`D` rows; no
  month name or year outside the subject. The rule's computed style equals the Log's rule's, and
  each rule's date text equals the Log's rule for the same commit.
- **Geometry** (every row, measured with `getBoundingClientRect`): height equal to the Log's
  `.crow` (which must itself be 21px) at the default width (46%), with the split dragged to its
  narrowest (22%, checked against the panes' width) in a 1600px window (352px), the same in a 901px
  window (198px), and with the panes stacked (860px). Repository history is graded at the default,
  at the narrowest of 1600px, 1450px (319px, just above the counts' threshold, against the KB's real
  counts) and 901px. "One line" is graded per cell, not by the row's box: every line box of a cell's
  content starts within half an em of the first, every cell sits vertically inside the row, and the
  row's `scrollHeight` does not exceed its `clientHeight`, so a fixed height that clips a wrapped
  cell fails. At every width the cells run left to right in markup order without overlap, and every
  cell but the subject shows all its text. At every width nothing spills (narrow-inspector
  clarification): the row's `scrollWidth` ≤ `clientWidth`, and every cell, and every shown element
  in it, lies inside `#ibody`'s rect. In the 901px window the counts must be hidden and `@` shown
  on every file row that has one. At every other graded width the counts must be shown. The subject's computed `white-space`, `text-overflow` and `overflow` are `nowrap`,
  `ellipsis`, `hidden`; at the narrowest, some subject must actually be cut (its `scrollWidth` >
  `clientWidth`, which is why the no-wrap check is on the row and cells, not the subject's widths).
- **Pinning:** in Repository history, six rows that are not the first of their day are scrolled to
  just below the top; the element at the top of `#ibody` must be a rule, flush with `#ibody`'s top,
  below `#itabs`, and the rule of the row beneath it; the tabs must not move.
- **Clicks**, each from a fresh tab: a row's click opens that commit (`c=` in the hash, `COPY SHA`
  for that hash); `@` travels (`rev=`, the travel bar) without opening the commit; `h` opens the
  History tab with its rules; OPEN THE FULL LOG shows the Log.
- **`--baseline`**: the Overview row (outer HTML, box, cell boxes, every computed property of the row
  and its cells, hover background) at three widths, and the Log (`#v-log` HTML, the first 80 boxes,
  every computed property of its rule, row and cells), this build against the base build (§ 4). It
  first proves the swap: the routed `app.css` is the base file, and the base History time cell
  still carries a date.

**Amended 2026-11-08 (review cycle 1, F2).** `tests/ui/history.mjs` adds a band sweep in a second
Chromium that draws the app's scrollbars (`ignoreDefaultArgs: ['--hide-scrollbars']`): one drag
holds the split from its narrowest and moves it 4px at a time until every row has shown its counts
for 8 steps, in a 901px and a 1280px window, on the file and on Repository history. Every row at
every step: nothing shown past `#ibody`'s content edge (left of the scrollbar), no cell past the
row's right padding, and counts hidden only where the subject's width, less what the counts would
add back to `.rt` (measured per row at the default split), is not above 0. It also asserts the band
starts with every row's counts hidden and that the scrollbars are drawn (10px). A counts box that
is not `nowrap`/`ellipsis`/`hidden`, or whose `title` differs from its text, fails at every graded
width. Negative controls: threshold 261.5→200px (edge and own-box clauses), 261.5→301.5px (room
clause), `.rt` `width: auto` (room clause, Repository history) and the `#ibody` container at
287.5px (own-box clause, scrollbars drawn) all go red. The frozen `verify/` is unchanged: its
checks hold for the new layout.

**Amended 2026-11-09 (review cycle 2, N1).** `tests/ui/history.mjs` first commits a large-counts
fixture into `OKT_KB`, the clone `tests/env.sh` serves (it refuses an unset `OKT_KB`, `~/KB`, or a
path or git directory inside it): `okt-tests/large-counts.md`, 6,789 lines, then all of them
replaced by 12,345 others, so the second commit reads "+12,345 −6,789" (14 characters). It waits
until the server's log leads with it. Two commits on one day, so the file under test is never this
one; later suites in the same environment see the fixture in Repository history.
- The fixture's counts show whole (text, `title`, both spans shown, `scrollWidth ≤ clientWidth`, text
  width ≤ box) in Repository history and in its file's History, at the default split (46%) and the
  widest (75%).
- Every geometry grade and every band step also checks that shown counts of up to 14 characters
  are whole, on every row.
- The band is swept 2px at a time (shown for 16 steps), so a fault 3px wide is never stepped over.
- Negative controls, each restored and checked with `cmp`: `.ad` back to 96px (9 checks red: the
  fixture at 46% and 75% in both listings, and the whole-counts check at three widths and in two
  sweeps); the whole stylesheet of `933a94f` (the same 9); threshold 270→266.5px (own-box clause,
  all four sweeps); threshold 270→273.5px (room clause, all four sweeps).

**Dry runs on the prototype**, each a mutant of the passing design. The gate fails every one,
on the check named:

| Mutant | Fails on |
|---|---|
| History as on `main` | rules, cells, header-before-rule, geometry |
| Date kept, row clipped to 21px with `overflow: hidden` | cells (`HH:MM`), time vs the Log, one line |
| Subject wrapping, clipped | one line, ellipsis |
| Overview padding changed through the shared `.hrow` | `--baseline` overview ×3 |
| N = all rows | rule counts |
| `@` dropped | cells |
| The Log's time changed | `--baseline` log |
| History's rule made `static` | rule style, pinning |
| Rows in a scrolling wrapper | pinning |
| Time track `minmax(0, max-content)` (overlap; reviewer finding 1) | 901px window: time cell not whole (0px wide, text over the status) |
| A date appended inside the subject cell (reviewer finding 4) | cells, in all three listings |
| `index.html` adds an inline `.hrow` style (reviewer finding 2) | `gate.sh` scope check, before any build |
| `.dayrule` `z-index` raised for everyone (reviewer finding 3) | `--baseline` log (`rule.z-index`) |
| No container query: the counts never hide (narrow-inspector ruling) | 901px window, file and repository: row `scrollWidth` 222 > `clientWidth` 196, cells outside the inspector, counts shown |

The overlap mutant (`minmax(0, max-content)` time track) failed the gate before the narrow-inspector
ruling. Since the ruling the counts hide below 312px, and a row without them needs 191px, under the
split's 198px minimum, so no track is squeezed at any width the split allows. The mutant now renders
the same as the design and passes: it is equivalent at those widths, not a hole. The cell
order, overlap and whole-cell checks stay to guard any other way the time could cover its
neighbours.

The first eight mutants ran on the gate before the review was folded in. The overlap, subject-date,
`index.html`, `z-index`, Overview-padding, wrapped-subject and non-sticky mutants ran again on the
final gate, and so did a clean run. The prototype passes the final gate whole (unit tests, `keys.mjs`,
the History suite, the History checks and `--baseline`, 160 History checks), in 83 s warm, after
the narrow-inspector ruling; the clean run, the container-query, Overview-padding and overlap mutants
ran on that final gate. Every check
waits on the state it reads (rows rendered, the Log settled, the commit or travel bar shown), with a
30 s ceiling, not on fixed timers.

---

*Backing research: none (lean path).*
