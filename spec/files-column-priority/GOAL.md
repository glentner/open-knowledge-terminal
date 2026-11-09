# GOAL — Files table: columns give way by priority, Name first

> **Origin spec.** The *what* and *why* — the locked contract `code-review` grades against.
> The *how* lives in [`PLAN.md`](PLAN.md) and [`TECH.md`](TECH.md) (written by `code-plan`).
> Keep this at the right altitude: solved and bounded, but not over-specified — leave design
> freedom for the plan. Edit requirements here; do **not** silently drift them during build.

- **slug:** files-column-priority
- **kind:** feature
- **appetite:** small

## Problem

Geoffrey, 2026-11-08: "The Files viewer is great the way it is when we have a wide screen. The other
columns like Title, Type, Lines, Size, Changed are great (Title is less helpful). But as we lose
horizontal space all these columns get compressed and we can no longer see the most important
field, the Name." He wants each supplemental column to have a priority, so that the columns
negotiate the width when space is tight. Name comes first and is never truncated below some limit.
Truncating a very long name with "…" is fine. As the viewer narrows, Title drops first, then Lines,
then Size, then Changed, then Type.

Evidence, confirmed at `edcbed4` (the seed cited `8d2f001`; the anchors have not moved). This is
**pre-existing** behaviour, not a regression:

- `.trow` (`okt-client/static/app.css:223`) is
  `minmax(0, 1fr) minmax(0, .9fr) 92px 56px 64px 60px 22px`: Name, Title, Type, Lines, Size,
  Changed, and the git-status marker. Name and Title split the flexible width almost evenly while
  the five fixed columns keep 294px between them. As the pane narrows, Name is crushed first, and
  Title, the column Geoffrey values least, keeps nearly as much as Name.
- The only responsive rule is keyed to the **window**, not the pane: `@media (max-width: 900px)`
  (`app.css:552–561`) stacks the panes and hides Title and Lines (`.trow .ti`,
  `.trow .num:nth-of-type(4)`). A narrow Files pane in a wide window gets no relief.
- The Name cell (`okt-client/src/views/tree.rs:200`, `:218`) is a tree cell: indentation of 14px
  per depth, a twisty, a glyph, then the node's **basename** (`index.rs:820`, `basename(p)`), with
  a trailing "/" on folders. It carries no hover text. A file with no title already lets Name span
  the Title column (`.nm.wide`, `app.css:232`).
- `.trow` is used only by the Files tree (`views/tree.rs`). The browser suites select it as
  `#tree .trow` (`tests/ui/keys.mjs:37`, `tests/perf/profile.mjs:101`). The Table view renders a
  `<table>` (`views/table.rs`), Results and Paths render `.lrow` and `.lsimple` (`views/list.rs`),
  and the Log renders `.crow`.

## Outcome / vision

On a wide pane the Files table looks as it does today. As the pane narrows, Name holds a width that
fits a 60-character name, and the supplemental columns give way one at a time, each column with its
header, in Geoffrey's order: Title, Lines, Size, Changed, Type. Once all five are gone, Name shares
the row only with the narrow git-status marker. Only then does a long name get cut, at its end with
"…", and the full path is always one hover away. The pane's own width decides all of this, so the
table behaves the same whether the window is narrow or the split is simply dragged.

## Acceptance criteria (the contract)

Terms. *Pane width* is the inline width of the Files table's own container (`#tree`), as the
browser reports it. The *droppable columns* are Title, Lines, Size, Changed and Type. The *status
column* is the narrow git-status marker (`.st`). The *cap* is R2's value. *Name text* is what the
Name cell writes after its indentation, twisty and glyph: the basename, plus "/" for a folder.

- **R1** — WHILE the pane is wide enough for every column with Name at its cap, the Files table SHALL
  show Name, Title, Type, Lines, Size, Changed and the status column, as today, including a
  title-less file's Name spanning into the Title column.
- **R2** — The cap SHALL be 60 characters of Name text, defined as one tunable value in exactly one
  place in the source. Changing that one value SHALL move every width threshold of R3 and R4 with
  it. The cap fixes one Name-column width, measured at the top level: room for 60 characters plus
  the twisty and glyph. A nested row's indentation comes out of that width.
- **R3** — WHEN the pane narrows, the Files table SHALL hide the droppable columns in the order
  Title, Lines, Size, Changed, Type, never hiding a column while one earlier in that order is still
  shown. Each column's header SHALL hide with its cells, and a hidden column SHALL leave no empty
  track or gap. WHEN the pane widens again, the columns SHALL return in the reverse order.
- **R4** — WHILE any droppable column is shown, the Name column SHALL be at least as wide as the cap.
  Name SHALL narrow below the cap only after all five droppable columns are hidden.
- **R5** — The Files table SHALL show the status column at every pane width, with its uncommitted
  marker and hover text as today.
- **R6** — The columns shown SHALL depend on the pane width, not the window width. WHEN the split is
  dragged and the window width does not change, the columns SHALL follow the new pane width. A Files
  pane of a given width SHALL show the same columns in a 1600px window, with the panes side by side,
  as in a window narrow enough to stack the panes.
- **R7** — IF the Name text does not fit the width the Name cell has, THEN the cell SHALL cut the
  text at its end with "…" and keep the row's indentation, twisty and glyph whole. Name text that
  fits SHALL never show "…".
- **R8** — WHEN the pointer rests on a row's Name cell, the Files table SHALL show that row's full
  KB path as hover text, for file and folder rows alike, truncated or not.
- **R9** — In the throwaway environment, at these three pane widths, the Files table SHALL show the
  following. The widths come from today's column sizes in Clarifications.
  - **Wide, 1200px:** every column of R1; a top-level Name text of 60 characters shows whole.
  - **Medium, 760px:** Title and Lines are hidden; Type and the status column are shown; Size and
    Changed are shown or hidden by the thresholds `TECH.md` states, consistent with R3; a top-level
    Name text of 60 characters shows whole.
  - **Narrow, 480px:** all five droppable columns are hidden; only Name and the status column show;
    Name fills the rest of the row and shows at least the first 40 characters of a top-level Name
    text before any "…".
- **R10** — The profile's `verify` SHALL pass, `tests/ui/keys.mjs` included. The Table, Results,
  Paths and Log views SHALL render as before this cycle.

## Non-goals (no-gos)

- The Log's `.crow` rows, the Table view's `<table>`, and the Results and Paths lists. None of them
  uses `.trow`, so no other view gains this behaviour in this cycle.
- Showing a path in the Name cell. The tree keeps showing basenames under their folders, as on a
  wide screen today.
- Cutting the middle of a name or shortening folders inside the cell (see Clarifications, ruling 2).
- A user setting for the cap, per-column show/hide toggles, column resizing, or changes to sorting.
- Changing what any column holds, or the folder rows' own summaries (file count, types, lines).
- The window-level layout: when the panes stack, and the split itself. Rearranging panes is
  `issues/pane-arrangements.md`.

## Clarifications

- **Q:** How long may Name get before it is truncated? — **A:** 60 characters shown untruncated, as
  one tunable value named in one place. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
- **Q:** Where is an over-long name cut? — **A:** Keep the file (or folder) name and cut the leading
  folders, as `…/pearc28/venues.md`, keeping as many trailing segments as fit. If the name alone
  exceeds the space, cut it at the end with "…". The full path is always available on hover.
  (resolved 2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* the Name cell never contains leading folders, because it shows a basename
    at its tree depth (`tree.rs:200`, `:218`), and Geoffrey calls the wide-screen view right as it
    is. So the ruling's second sentence is the one that applies (R7), and the hover carries the full
    path (R8).
- **Q:** What width governs the drop order? — **A:** The table's own width (its pane), not the
  window. The lean is CSS container queries; the plan decides the mechanism. R6 grades pane width
  against window width. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
- **Q:** What order do columns drop in, and what never drops? — **A:** Title, then Lines, then Size,
  then Changed, then Type, each header with its cells. Name truncates below its cap only after every
  droppable column is gone. The git-status column always stays: it isn't in Geoffrey's list, and it
  carries the dirty signal. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
- **Q:** Which views are in scope? — **A:** The Files table. Other views that reuse `.trow` must not
  regress, and may gain the behaviour if they share the Files columns. The Log's `.crow` is a
  non-goal. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* at `edcbed4` no other view uses `.trow` (Problem), so only the Files table
    changes, and R10 guards the rest.
- **Q:** How is the visual result graded? — **A:** An R-ID states what shows at three named pane
  widths (wide, medium, narrow): which columns, and that Name is readable. The widths come from the
  current column sizes. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
  - *The widths (R9):* the body font is `--mono` at 12.5px, about 7.5px per character, so 60
    characters take about 450px. The twisty, glyph and their gaps add about 30px at the top level,
    so Name at its cap is about 480px. The status column (22px), the row's padding (20px) and its
    border (2px) are always present. Adding a column adds its width plus a 10px gap: Type 102,
    Changed 70, Size 74, Lines 66. That gives these approximate thresholds: about 535px for Name and
    status alone, then about 635 (Type), 705 (Changed), 780 (Size), 845 (Lines), with Title above
    that. 1200px leaves Title about 350px. 480px is below every threshold. At 760px, Title and Lines
    are gone and Type is shown with about 120px to spare either way. R9 does not pin Size and Changed
    at 760px, because the exact font metrics and the scrollbar can move those thresholds by a few
    tens of pixels.
- **Q:** Name is a tree cell, so every level of depth costs 14px of indentation before the name.
  Does the cap hold at every depth, or fix one Name-column width? — **A:** One width, measured at
  the top level. The Name column reserves room for 60 characters plus the twisty and glyph, and a
  nested row's indentation comes out of that width, so a 60-character name at depth 3 shows about 54
  characters. This keeps the pure pane-width mechanism (ruling 3), and the thresholds stay stable:
  if the cap held at every depth, columns would come and go as deeper rows scroll into view. The
  shaper's application of ruling 2 to the basename cell (end-cut, full path on hover) is accepted
  as written. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)

## Related materials

- Seed: `issues/files-column-priority.md` (promoted; `status: adopted:files-column-priority`)
- ROADMAP: Part II, entry 2, "Files columns by priority"
- Geoffrey's words: KB `workspace/open-knowledge-terminal/stories.md`, story 2 (2026-11-08)
- Source: `okt-client/static/app.css:223–240` (`.trow`, `.thead`), `:552–561` (the 900px window
  rule); `okt-client/src/views/tree.rs:177–230` (`render()`, the Files rows and header); existing
  container query precedent `#ibody { container-type: inline-size; }` (`app.css:448`)
- Profile: `app.css` is coupled core with `okt-client/src/graph.rs` (AGENTS.md § Factory)
- Related seed: `issues/pane-arrangements.md`
