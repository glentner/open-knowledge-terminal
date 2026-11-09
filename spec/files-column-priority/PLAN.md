# PLAN — Files table: columns give way by priority, Name first

> **Status:** Draft for review · **Last updated:** 2026-11-08
> **Authoritative technical design.** The *how*. Vision/contract is [`GOAL.md`](GOAL.md);
> the phased executable roadmap is [`TECH.md`](TECH.md). Lean path (`appetite: small`): no
> `research/`; the design was dry-run on a prototype instead. Every design element traces to a
> GOAL R-ID.

## 1. Summary

Pure CSS, keyed to the pane through a container query. The Files rows get one wrapper element,
`.tcols`, that is the query container, and its `font-size` carries the Name column's width at its
cap, `calc(60ch + 35px)`. Every threshold is then written `calc(1em + <the columns beside Name>)`,
so the 60 appears once and moves every threshold (R2). Five `@container` blocks, widest first,
each drop one column and its track (R3). The Name cell gains a `title` with the full path (R8);
its end-cut with "…" is already in the stylesheet and is kept (R7). The window-keyed 900px rule
stops touching `.trow` (R6). Two phases: the columns, then the Name cell.

## 2. Design

### The query container and the one cap value (`okt-client/static/app.css`, `views/tree.rs`)

`render()` in `okt-client/src/views/tree.rs` wraps the header and the rows in
`<div class="tcols">…</div>` inside `#tree`. The wrapper, not `#tree`, is the container, because
the container carries an unusual font-size and `#tree` is the scroller: a scroller's font sets
the wheel's line step in some browsers. The wrapper's inline size is `#tree`'s inner width, so
the query reads the pane (R6), never the window.

```css
@property --name-w { syntax: '<length>'; inherits: true; initial-value: 0px; }
.tcols { container: tcols / inline-size; font-size: calc(60ch + 35px); --name-w: 1em; }
.tcols > .trow:not(.thead) { font-size: var(--fs); }
```

- `60ch` is the cap, the one tunable value (R2). In `font-size`, `ch` is the parent's, so it is
  one character of the rows' font (`#tree` inherits the body's `--mono` at `--fs`).
- `35px` is the top-level prefix before the Name text: the zero-width indentation span, the
  10px twisty and the 10px glyph, with the cell's three 5px gaps (`.trow .nm { gap: 5px }`). So
  `1em` on `.tcols` is the Name width at its cap, measured at the top level; a nested row's
  indentation comes out of it (Clarifications, ruling (a)).
- Relative units in a container query's condition resolve against the container, so `1em` in
  each `@container tcols (width < calc(1em + …))` is that Name width.
- `--name-w` is registered so it computes to pixels on `.tcols` and rows inherit the number, not
  the `1em` token; the header keeps its own 10.5px through `.thead`, which the `:not(.thead)`
  leaves alone.
- The rows' font-size reset is the only cost of the trick: nothing else inside `.tcols` sizes by
  `em` (the indentation, twisty, glyph, gaps and tracks are all in px). Anything but a `.trow`
  placed in `.tcols` (an empty-state line, say) would render at about 485px, so the CSS comment
  says only rows go in it.

### The column bands

The base `.trow` rule stays today's template except for Name's floor and the flex factors:

```css
.trow { grid-template-columns: minmax(var(--name-w), 10fr) minmax(0, 9fr) 92px 56px 64px 60px 22px; … }
```

`10fr`/`9fr` is today's 1 : 0.9 split. The factors are scaled up because once Name sits at its
floor, a lone `0.9fr` Title would take only 90% of the leftover (a flex sum under 1), leaving an
empty strip at the row's end; the prototype's gate caught exactly that. On a pane wide enough for
the 1 : 0.9 split to give Name its cap, nothing changes (R1, checked at a 1540px pane).

Then five blocks, widest first, so a narrower one also matches every wider one and their
`display: none` rules accumulate while the last matching template wins:

| Block (inner width `<`) | Template | Hides |
|---|---|---|
| `calc(1em + 496px)` | `minmax(0, 1fr) 92px 56px 64px 60px 22px` | `.ti`; `.nm.wide` back to `grid-column: auto` |
| `calc(1em + 366px)` | `minmax(0, 1fr) 92px 64px 60px 22px` | `.lines` |
| `calc(1em + 300px)` | `minmax(0, 1fr) 92px 60px 22px` | `.size` |
| `calc(1em + 226px)` | `minmax(0, 1fr) 92px 22px` | `.age` |
| `calc(1em + 156px)` | `minmax(0, 1fr) 22px` | `.ty` |

The offsets are what the shown columns beside Name take: 22 for the row's 2px border and 20px
padding, 32 for the status column and its gap, then each column with its 10px gap (Type 102,
Changed 70, Size 74, Lines 66), and Title 130 (a 120px floor plus its gap). In each band the Name
track is `1fr` of what the fixed tracks leave, which the band's own threshold makes at least
`1em` (R4); below the last threshold Name takes the whole row beside the status column. Header
cells carry the same classes, so each header hides with its cells (R3). A hidden track is removed
from the template, not zeroed, so it leaves no gap (R3).

`render()` adds `lines` and `size` to the two `.num` cells (header, folder and file rows), so the
rules name the column, not a child position. `.lrow .ln` is the Results list's own class; the new
names avoid it.

With the rows' font at 7.5px per character on this host (the `--mono` stack at 12.5px under
headless Chromium; Berkeley Mono is installed here), `1em` is 485px and the thresholds on the table's inner width are 641 (Type),
711 (Changed), 785 (Size), 851 (Lines) and 981 (Title). Measured run: the plan's prototype dry run
of `verify/columns.mjs`, which prints them. So at 760px (R9 medium) Type and Changed show, Size,
Lines and Title are hidden. The figures move with the font; the formula is the contract TECH.md
states.

### The window rule (`app.css` `@media (max-width: 900px)`)

Its three `.trow` lines go (the Files template, the Title and Lines hide, the `.nm.wide` reset).
The stacked layout then uses the same container blocks, so a 760px Files pane shows the same
columns stacked as beside the inspector (R6). The rest of the media block (stacking the panes,
hiding the splitter) is untouched (GOAL non-goal).

### The Name cell (`views/tree.rs`)

`render()` gives the `.nm` span `title="{full KB path}"` (`esc(&r.id)`) on folder and file rows
(R8). The hover text then covers the whole cell (indentation, twisty, glyph, text), and the
status cell keeps its own `title="uncommitted"` (R5). The end-cut already holds: `.nm .t` has
`overflow: hidden; text-overflow: ellipsis` under the row's `white-space: nowrap`, and the
indentation span, `.tw` and `.gl` are `flex: none` (R7). The phase keeps those rules and grades
them.

### Selection after the wrapper

`render()` finds the selected row by index in `#tree`'s children; it now indexes the wrapper's
children (`tree.first_element_child()`), with the header still at 0. Every other lookup is a
descendant query (`#tree .trow…`, `q_in(&tree, ".trow.on")`) and needs no change, nor do
`tests/ui/keys.mjs:37` and `tests/perf/profile.mjs:101`. The sticky header stays inside the
wrapper, which spans every row, so it still pins to the top of `#tree`.

### Requirement → design map

| R-ID | Design element(s) that satisfy it |
|------|-----------------------------------|
| R1 | Base `.trow` template, today's columns and 1 : 0.9 split (`10fr`/`9fr`) with Name floored at `--name-w`; `.nm.wide` spans Title while Title shows |
| R2 | `.tcols { font-size: calc(60ch + 35px) }`, the only place the cap is written; every threshold is `calc(1em + …)` and `--name-w` is `1em` |
| R3 | Five `@container tcols` blocks, widest first, Title → Lines → Size → Changed → Type; each removes the track and hides header and cells by class |
| R4 | Each band's threshold leaves the `1fr` Name track at least `1em`; Title's band floors Name at `--name-w`; only below the last threshold is Name under the cap |
| R5 | `22px` status track in every template; `.st` never hidden |
| R6 | Container on `.tcols` (pane width); `.trow` lines removed from the 900px window rule |
| R7 | Existing `.nm .t` ellipsis and `flex: none` prefix, kept and graded |
| R8 | `title` on `.nm` with the row's id, folder and file rows |
| R9 | The thresholds above; graded by the gate at 1200, 760 and 480px set with the splitter |
| R10 | Rules scoped to `.tcols`/`.trow`; profile `verify` with the new `keys.mjs` section; the other views smoke-checked |

## 3. Invariant gate (AGENTS.md constitution check)

Walked before the design and again against it. The change touches `okt-client/static/app.css`
and `okt-client/src/views/tree.rs`, plus `tests/ui/keys.mjs`.

- **High-blast-radius paths** — none touched (`state.rs`, `http.rs`, `launch.rs`, `index.rs`).
  Trigger 2 does not fire. `tree.rs` reads `state::` as today and adds no new access.
- **§ 1 Client re-entrancy** — `render()` gains a wrapper and an attribute in its HTML string and
  one `first_element_child()` call; no new borrow of `S`, settings or the graph, no new handler
  or `await`.
- **§ 2 Pygments data** — not touched.
- **§ Conventions, `app.css` with `graph.rs`** — the graph rules (`.n`, `.lab`, `.ebase`,
  `.ehot`) are untouched, so `graph.rs` has nothing to follow. `app.css` is still coupled core,
  so every phase touching it is `parallel: false`.
- **§ Conventions, no spec ids in source** — no `R1`/`P1`-style ids in CSS, Rust, or test labels
  (TECH § Conventions repeats it for the test section).
- **§ Conventions, point to a rule** — the threshold offsets restate the column widths; the CSS
  comment says so and names them as one same-commit obligation (change a width, change its
  offsets). README does not describe the Files columns, so no doc change is owed.

### Deviation justifications

| Deviation | Why needed | Simpler alternative rejected because |
|-----------|-----------|--------------------------------------|
| The cap rides in the query container's `font-size` (`.tcols`), with a reset on the rows and a registered `--name-w` | R2 wants one value that moves every threshold. A container query's condition can't read `var()`, but it resolves `em` against the container | Literal `calc(60ch + …)` in each query writes the cap six times (fails R2). A Rust-generated `<style>` from a `const` moves the column layout out of `app.css` and adds a runtime stylesheet for a static rule. A `ResizeObserver` in `tree.rs` restates every column width in Rust and adds a resize path. |
| A wrapper element `.tcols` inside `#tree` | The container must carry the font trick; `#tree` is the scroller | Putting it on `#tree` gives the scroller a ~485px font, which some browsers use as the wheel's line step |
| `10fr`/`9fr` in place of `1fr`/`.9fr` | Same ratio; with Name at its floor a lone `.9fr` takes 90% of the leftover and leaves an empty strip (fails R3 "no gap") | Keeping `.9fr` was the prototype's first try and its gate went red on the empty strip |

Each row read against the R-IDs: row 1 meets R2 as worded (one value in one place; the gate
changes it at runtime and every threshold moves); the 35px prefix is the twisty and glyph
ruling (a) puts inside the cap, not a second tunable. Row 2 keeps R6's definition: the wrapper's
inline size is `#tree`'s inner width, and R9 grades `#tree`'s width. Row 3 keeps R1's "as
today": the gate checks the 1 : 0.9 split at a 1540px pane against today's code, where it also
passes. No row bends an R-ID.

## 4. Rabbit holes (resolved)

- **Does Chromium resolve `em` in a container query against the container?** → Yes. The
  prototype's thresholds land within 1px of `60 × 7.5 + 35 + offset`, and moving the cap by 20
  characters moves all five by 150px (dry run of `verify/columns.mjs`).
- **A flex sum under 1 once Name hits its floor** → found by the gate's packing check on the
  first prototype (an empty strip at the row's end at 1200px); fixed by `10fr`/`9fr`.
- **The splitter's pane widths** → the splitter clamps the inspector to 22–75%, so in a 1600px
  window the Files pane spans about 395–1243px; 1200, 760 and 480 are all reachable by dragging,
  to within the 0.1% rounding (≤2px).
- **Scrollbars** → headless Chromium on this host draws overlay scrollbars, so the table's inner
  width equals the pane width; the thresholds are stated on the inner width so a classic
  scrollbar moves them by its own width and no more.
- **Adversarial review** → one fresh reviewer, one pass, five mutants on the prototype (scaled
  to `appetite: small`). It found the design sound and the gate's packing and header checks
  exact, and six holes, all folded into `verify/columns.mjs` and re-run:
  - *Stacked windows only at 760 and 480* (MEDIUM): leaving the old Lines hide in the 900px
    rule stayed green, because both widths sit below the Lines threshold. Now every stacked
    window from 900 to 380px, 4px a step, is graded and matched to the side-by-side sweep at the
    same inner width; the mutant goes red at 900px.
  - *The folder's 60-character name never checked* (MEDIUM): a wider folder face cut it at
    1200px and stayed green. The not-cut test now covers the folder row everywhere it covers the
    file; the mutant goes red in four checks.
  - *The cap scan blind to nested rules* (MEDIUM): wrapping the cap rule in `@supports` failed a
    good product and skipped the move test; a second `60ch` inside a template passed the
    "written once" check. The scan now walks nested rules, a source check counts the cap's
    literal across `app.css` and `okt-client/src/**/*.rs` (exactly one), and a missing cap is
    reported as such. The `@supports` variant now passes.
  - *The moved-cap sweep ungraded* (LOW-MEDIUM): it now runs the full grade with the smaller cap
    (all but the 60-character not-cut clause).
  - *"The list view renders" could not fail* (LOW): `#list` is static. It now runs a search
    (`#v=list&q=plain`, a word the title-less fixture holds) and needs a visible result row.
  - *Hover probed only at the text and glyph* (LOW): it now also probes the twisty, the cell's
    far end (for the title-less row, inside the span over Title) and a nested row's indentation.
  - A `visibility: hidden` Title band (control mutant) went red in 14 checks.

## 5. Risks & open questions

- **Font.** The thresholds scale with the font's `ch`. In a font whose bold is wider than its
  regular, a 60-character folder name could need a few px more than the cap; the gate checks the
  60-character folder name is whole at every width where a droppable column shows (a widened
  folder face turns it red). Berkeley Mono, Menlo and JetBrains Mono keep one advance across
  weights.
- **Coupling.** The gate's threshold check carries the offsets (156, 226, 300, 366, 496). A change
  to a column width or Title's 120px floor changes `app.css`, TECH.md's stated thresholds and the
  `keys.mjs` table together.
- **Browsers.** `@property`, container queries with `calc()` and `em` in the condition are in
  current Chromium, Safari (16.4+) and Firefox (128+). The gate runs Chromium only.
- **The other views (R10).** No baseline screenshot exists; they are guarded by scoping every new
  rule to `.tcols`/`.trow`/`@container tcols`, the existing `keys.mjs` checks, and the new
  section's smoke checks that Log, Table and Results render.

## 6. Verification strategy

One gate, drafted at [`verify/columns.mjs`](verify/columns.mjs) and dry-run on the prototype
(all pass there, in about 15 s once the build is cached; 23 checks failed on today's code
before the review added more). Build folds it into `tests/ui/keys.mjs` as a
`FILES columns` section, so the profile's `verify` runs it. It grades what renders, never the
stylesheet alone:

- **Fixtures** written into the throwaway clone (`$OKT_KB`; it refuses the real KB): a titled file
  and a folder with 60-character Name texts, a title-less file, and a titled file inside the
  folder (a nested row). They are untracked, so they also carry the uncommitted marker.
- **A sweep** of the pane from 1300 to 380px and back, 2px a step, through `--insp-w`. At every
  step each cell is clearly shown (`display`, a box inside the row and the pane) or hidden;
  every column agrees with its header and sits under it; shown cells pack with exactly one gap
  and no empty track; the status column shows; the table never scrolls sideways; while any
  droppable column shows, Name is at least the measured cap and the 60-character name is not cut
  (its text range fits its box, `scrollWidth <= clientWidth`). The shown set is always a suffix
  of Title, Lines, Size, Changed, Type; widening gives the same set at every width (reverse
  order); each column gives way once, at TECH's stated threshold within 2px.
- **The cap** found at runtime as the one `ch` length among the rules, nested ones included,
  that apply to the Files table, and its literal found exactly once across `app.css` and the
  client's Rust; changing it by 20 characters through the CSSOM keeps every sweep width clean and
  moves every threshold by `20 × ch` within 2.5px, and restoring it restores them.
- **The splitter**, dragged in a 1600px window to 1200, 760 and 480px: R9's facts at each, with
  760's Size and Changed as the thresholds say, and at 480 the long name cut with "…" after at
  least 40 characters, the folder and nested rows' indentation, twisty and glyph whole.
- **Stacked windows** from 900 down to 380px, 4px a step: each graded as above and showing the
  same columns as a side-by-side pane of the same inner width; at 760 and 480 the same sets as
  the splitter gave (R6).
- **Hover**: the element under the pointer at the Name text, the glyph, the twisty, the cell's
  far end and a nested row's indentation carries the row's full path as its `title`, for a file, a title-less file, a folder and a nested file, at 1200
  and 480px; the uncommitted marker and its "uncommitted" text show.
- **As today**: a 1540px pane splits Name and Title 1 : 0.9; the header stays pinned while the
  table scrolls; Log and Table render rows, and a search fills Results; no page errors.

Phase dry runs on the prototype: a P1-shaped tree (no `title` on `.nm`) passes the gate without
its hover block, and `keys.mjs`; the full prototype passes the whole gate, `keys.mjs` and the
`cargo test` crates once.

Each phase's `verify:` is the profile's: `cargo test -p okt-server -p okt-pygments -p okt-client`
and `tests/ui/keys.mjs` in `tests/env.sh`.
