# GOAL — Pane arrangements: place, swap, stack and collapse views

> **Origin spec.** The *what* and *why* — the locked contract `code-review` grades against.
> The *how* lives in [`PLAN.md`](PLAN.md) and [`TECH.md`](TECH.md) (written by `code-plan`).
> Keep this at the right altitude: solved and bounded, but not over-specified — leave design
> freedom for the plan. Edit requirements here; do **not** silently drift them during build.

- **slug:** pane-arrangements
- **kind:** feature
- **appetite:** big

## Problem

Geoffrey, 2026-11-08 (story 1, verbatim in part):

> generally the OKT is great to navigate by finding items but horizontal space is a challenge on
> smaller screens. I think we should explore having greater flexibility on the placement of the two
> primary views. The Graph-Files-Log-Table-Results (GFLTR) view that is generally on the left, and
> the Overview-Content-History (OCH) view that is generally on the right. We can resize them, but we
> can't switch them. I did see in one instance where it automatically switched to top-bottom
> orientation, it would be good to have that as a deliberate choice. Doing this work also opens the
> door to the future effort of adding new interfaces, like custom views depending on workflow or the
> agent chat interface. […] we can't assume such a side screen that we have just 3 columns. I want
> to be able to have stacked pane arrangements and collabability; for example, the graph on top but
> collapsed, and the OCH moved to the left and the 3rd view shown on the right. If we build a
> generalized window/view arrangement system, we can start to use that for understanding.

What is there today, at `edcbed4` (pre-existing behaviour, not a regression):

- The layout is one fixed grid: `.panes` is `grid-template-columns: minmax(0, 1fr) 5px
  var(--insp-w, 46%)` (`okt-client/static/app.css:152`). The DOM is fixed too: `#view` (the five
  GFLTR panes, one shown at a time), `#split`, then `#insp` (`okt-client/static/index.html`).
- The splitter only resizes. It writes `Settings.insp`, a percentage clamped to 22–75 and saved in
  localStorage `okt:<kb>` (`okt-client/src/keys.rs:646–689`, `okt-client/src/state.rs:85`). Nothing
  swaps, stacks, collapses or moves.
- The top-and-bottom arrangement Geoffrey saw is `@media (max-width: 900px)` (`app.css:552–561`):
  automatic on a narrow window, never a choice.
- One view is shown at a time. `S.view` holds it, `set_view` hides the other four
  (`okt-client/src/nav.rs:88`), and the GFLTR buttons (`.fkeys`) are a single tab strip. Files and
  the Graph can never be seen together.
- Every future view he expects (the dream-protocol card view for fast, system-one triage, an agent
  chat, other per-workflow views) needs a pane the fixed grid can't host.

## Outcome / vision

A general arrangement system. The area between the command bar and the commit ticker is a tree of
side-by-side and stacked splits whose leaves are panes. Each pane has a title bar and holds one or
more views as tabs. Any view can go in any pane, so Files and the Graph can be open at once. There
is still one selection, and every view follows it. Panes collapse to their title bar, expand,
resize, swap, flip orientation, close, and pass views to each other, all from the keyboard. Four
named presets ship, Geoffrey's example among them; a key picks or cycles them; and the arrangement
he leaves is the one he finds on reload. Each pane is its own size container, so a view fits its
pane rather than the window. The 900 px rule stops being automatic: stacking is a deliberate
choice. This cycle builds the frame that the dream-inbox view and the agent chat will later occupy.

**Terms.**
- *View*: one of Graph, Files, Log, Table, Results, and the Inspector (Overview, Content and History
  together, as one view).
- *Arrangement*: the tree of splits and panes, with each split's orientation and sizes, each pane's
  views and shown tab, and each pane's collapsed state.
- *Focused pane*: the one pane that view-level keys act on (R8).
- *Preset*: a named arrangement okt ships (R4).

## Acceptance criteria (the contract)

**The arrangement**

- **R1** — The client SHALL lay out the area between the command bar and the ticker as an
  arrangement: a tree of splits, each side-by-side or stacked, whose leaves are panes. Each pane
  SHALL show a title bar and hold one or more views as tabs, one shown at a time, and clicking a tab
  SHALL show that view. Any view SHALL be placeable in any pane; a view is held by at most one pane.
- **R2** — WHILE two or more views are shown in different panes, each SHALL render and respond at
  the same time (for example the Graph and Files side by side, both drawn and both scrollable or
  navigable).
- **R3** — WHEN a card is selected in any view, the Inspector SHALL show it wherever the Inspector
  is, and every shown view SHALL follow the selection as it does today (graph highlight, Files row,
  Log and Results marks).

**Presets, default, persistence**

- **R4** — okt SHALL ship at least these presets, each holding at least the views listed:
  - **classic**: side by side; left pane Graph, Files, Log, Table, Results; right pane Inspector.
  - **swapped**: side by side; left pane Inspector; right pane Graph, Files, Log, Table, Results.
  - **stacked**: stacked; top pane Graph, Files, Log, Table, Results; bottom pane Inspector.
  - **three**: stacked; on top, across the full width, a pane holding the Graph, collapsed to its
    title bar; below it, side by side, the Inspector on the left and, on the right, a pane holding
    Files, Log, Table and Results as tabs, with Files shown.

  Initial split sizes are stated in PLAN.md; classic and swapped start at today's default (the
  Inspector 46% of the width; classic takes a splitter width saved before this cycle instead, R7),
  stacked at today's narrow layout (two equal halves).
- **R5** — okt SHALL bind one key per shipped preset that applies it, and one key that cycles the
  presets in a fixed order, all unmodified keys proven free in `keys.rs`; the help overlay (`?`) and
  README § Keys SHALL list them.
- **R6** — WHEN okt loads with no arrangement saved for this KB, it SHALL start in stacked if the
  window is narrower than 900 px and in classic otherwise. WHEN an arrangement is saved, okt SHALL
  restore it at every window width, and resizing the window SHALL never change which panes exist,
  their orientation, their views or their collapsed states. The automatic stacking in the
  `@media (max-width: 900px)` block SHALL be gone.
- **R7** — WHEN the user changes the arrangement (a preset, a split drag, collapse or expand, swap,
  orientation, close, moving a view, a tab), okt SHALL save it in the KB's settings (localStorage
  `okt:<kb>`, as today), and a reload at the same window size SHALL restore the same tree, the same
  views and shown tab per pane, the same collapsed states, and each pane's rect within 1 px.
  WHEN okt loads with no arrangement saved but a splitter width saved from before this cycle
  (`insp`), and R6 starts it in classic, classic SHALL start with the Inspector at that width, so the
  first load after the upgrade looks as before: each window shows what it showed before this cycle
  (stacked under 900 px, classic at the saved width otherwise) (Q16).

**Focus and keys**

- **R8** — Exactly one pane SHALL be focused at a time, marked on its title bar; clicking in a pane
  SHALL focus it. Keys that act on a view (the Graph walk `j`/`k`/Enter/space, the Files and Log
  keys) SHALL act on the focused pane's shown view.
- **R9** — In every shipped preset, `g`, `f`, `l` and `t` SHALL show their view in whichever pane
  holds it: as that pane's shown tab, expanding the pane if collapsed, and focusing it. IF no pane
  holds the view, THEN okt SHALL add it to the focused pane and show it there. `o`, `c` and `h`
  SHALL set the Inspector's tab and show the Inspector: as its pane's shown tab, expanding the pane
  if collapsed, or, IF no pane holds it, adding it to the focused pane. They SHALL NOT move focus,
  so the focused pane's view keys keep acting as before (`c` then `j` still walks the Graph when the
  Graph's pane is focused) (Q18). `/` and `:` SHALL still focus the command
  line, and a search that opens Results SHALL place it by the same rule. No other existing key
  SHALL change meaning, and `tests/ui/keys.mjs` SHALL pass with its existing key assertions keeping
  their expected outcomes (selectors may change where the DOM moved).

**Pane operations** (each by key on the focused pane; mouse drag of views is a non-goal)

- **R10** — WHEN the user collapses a pane in a stacked split, it SHALL shrink to its title bar;
  WHEN the user collapses a pane in a side-by-side split, it SHALL shrink to a narrow vertical strip
  with its title turned on its side. In both, its neighbours SHALL take the space, and WHEN the user
  expands it, it SHALL return to the size it had before.
- **R11** — WHEN the user drags any split, the panes on either side SHALL resize with the pointer,
  down to a floor: each side at least 200 px wide in a side-by-side split, or half the split when the
  split is narrower than 400 px, and at least 120 px tall in a stacked split, where the drag stops;
  and the new sizes SHALL be saved (R7) (Q17).
- **R12** — WHEN the user swaps the focused pane with a neighbour, the two SHALL exchange places,
  each keeping its views. WHEN the user flips a split's orientation, it SHALL change between side by
  side and stacked, keeping its panes and their order.
- **R13** — WHEN the user closes the focused pane, its views SHALL move to a neighbouring pane and
  its neighbours SHALL take the space. IF it is the only pane, THEN okt SHALL refuse and leave the
  arrangement unchanged.
- **R14** — WHEN the user moves the focused pane's shown view to a neighbouring pane, the view SHALL
  appear there as its shown tab and leave the source pane. WHEN the moved view was the source
  pane's last, the source pane SHALL close exactly as R13 closes it. IF the focused pane is the only
  pane, THEN moving its only view SHALL do nothing and say so.

**Fitting the pane**

- **R15** — Each pane SHALL be a size container, and its views SHALL fit the pane rather than the
  window: the Files columns respond to the Files pane's width; the Graph canvas fills its pane after
  every change of the pane's size (split drag, collapse or expand of a neighbour, preset change,
  window resize); the Inspector and the Log fit their pane's width. Views SHALL fit down to a pane
  200 px wide, and no view's content SHALL overflow its pane horizontally (`scrollWidth ≤
  clientWidth` on the pane). WHILE a pane is narrower than 200 px (only a shrinking window can make
  one, R11), it SHALL hide its body behind its title bar rather than overflow (Q17).
- **R16** — The Graph legend SHALL hide by the Graph pane's own width, not the window's. The brand
  word, the KB name and the crumbs SHALL keep hiding on a window narrower than 900 px (they sit in
  window chrome). No window-scoped rule for the Files rows SHALL remain.

**What Geoffrey sees**

- **R17** — A browser suite in the throwaway environment SHALL, for each shipped preset at 1600×1000
  and at 900×1000, grade from bounding rects: each pane's position and size against the preset's
  definition (within 2 px of the sizes PLAN.md states), the views each pane holds, and every
  collapsed pane at its title bar's height. For **three** at both sizes it SHALL also check that the
  Graph's title bar spans the top, the Inspector is on the left showing the selected card with its
  O/C/H tabs, and Files is on the right showing its filter field and at least 10 rows with their
  Name visible, with no horizontal overflow in either.

## Non-goals (no-gos)

- The dream-inbox view and the agent chat. This cycle unblocks them; it doesn't build them.
- Drag-to-dock, and any mouse drag of a view between panes. A follow-up candidate.
- Saved layouts beyond the named presets plus the current arrangement: no user-named presets, no
  per-workflow layouts.
- Floating or undocked windows.
- The same view in two panes at once.
- Any change to `/api/index` or to the server.
- What the views themselves show: the Files columns' priorities are `files-column-priority`, the
  History rows are `history-log-rows`, the graph's drawing is `graph-galaxy` (on ice). This cycle
  only makes each view fit its pane.

## Clarifications

All rulings below: 2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous.

- **Q1:** Interaction: presets and keys, or drag-to-dock too? — **A:** Presets and keys first.
  Drag-to-dock is a non-goal and a follow-up candidate. (resolved 2026-11-08, foreman ruling under
  decision 2026-11-08-okt-dream-inbox-autonomous)
- **Q2:** What does a pane hold? — **A:** An arrangement is a tree of row and column splits whose
  leaves are panes; each pane holds one or more views as tabs. The views are Graph, Files, Log,
  Table, Results and the Inspector (Overview, Content, History together, as one view). Any view can
  go in any pane, and Files and Graph can be open at once in different panes. The selection stays
  one: selecting a card anywhere updates the Inspector wherever it is, and every view follows the
  selection as today. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* ruling 6 speaks of "whichever pane holds it", so a view sits in at most
    one pane (R1); the same view twice is a non-goal.
- **Q3:** Geoffrey's example and the presets. — **A:** His example must be expressible and ship as a
  preset: the Graph on top, collapsed to its title bar; the Inspector on the left; a third view on
  the right (Files). Ship at least classic (view pane left, Inspector right), swapped (Inspector
  left, view pane right), stacked (view pane on top, Inspector below) and three (his example).
  (resolved 2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* "today's" sizes are the code's: the Inspector defaults to 46% of the
    width (`state.rs:128`), and the narrow layout stacks two equal rows (`app.css:553`). R4 starts
    classic, swapped and stacked there.
- **Q4:** The 900 px rule. — **A:** It stops being automatic CSS; stacked is the deliberate choice.
  Only when no arrangement has been chosen yet (first run, default) and the window is narrower than
  900 px does okt start in stacked. A chosen arrangement is honoured at every window width. The
  shaper rules on the block's other narrow-window rules. (resolved 2026-11-08, foreman ruling under
  decision 2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* ruling 6 saves the current arrangement with its sizes on every change,
    and a saved arrangement must survive reload at any width, so "chosen" means "saved": any change
    the user makes to the arrangement (R7) counts as choosing it (R6).
  - *Applied to the code:* "narrower than 900 px" is `< 900`. Today's `max-width: 900px` also
    matches at exactly 900, so the boundary moves by 1 px; R6 follows the ruling's words.
  - *Shaper's disposition of the block's other rules (`app.css:552–561`)*, by where the element
    lives:
    - `.panes`, `.split`, `.insp` (the stacked grid, the hidden splitter, the Inspector's top
      border): removed; the stacked preset replaces them (R6).
    - `.glegend { display: none }`: moves to a container query on the Graph's pane, since the legend
      sits inside the Graph view (R16).
    - `.word, .kbname, .crumbs { display: none }`: stays window-scoped. The brand and KB name are
      in the header, the crumbs in the window-level view bar; none of them is in a pane. If the plan
      moves the crumbs into a pane, their rule moves with them to that pane's container query.
    - `.trow` (three rules): `files-column-priority` replaces them with container queries on the
      Files table, and it lands first; this cycle leaves no window-scoped `.trow` rule (R16).
- **Q5:** Pane operations. — **A:** Collapse to the title bar and expand (neighbours take the
  space); resize by dragging any split; swap with a neighbour; set any split side by side or
  stacked; close a pane (its views move to a neighbour; the last pane can't close). Move a view
  between panes by key; mouse drag is a non-goal. (resolved 2026-11-08, foreman ruling under
  decision 2026-11-08-okt-dream-inbox-autonomous)
- **Q6:** Persistence and keys. — **A:** The current arrangement, with its sizes and collapsed
  states, persists across reloads in settings (`state.rs`, keyed per KB as today). Presets are
  selectable by key, and a key cycles them; the plan chooses keys proven free in `keys.rs`. No
  existing key changes meaning in any arrangement: `g`/`f`/`l`/`t`/`/` still show their view, in
  whichever pane holds it, expanding it if collapsed and creating it in the focused pane if no pane
  holds it. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* `/` does not show Results today; it focuses the command line, as `:`
    does (`keys.rs:315`), and Results opens from a search. Keeping its meaning, `/` still focuses
    the command line, and the Results view a search opens is placed by the ruling's rule (R9).
  - *Applied to the code:* `o`, `c` and `h` set the Inspector's tab, which is always visible today.
    Under "no key changes meaning" they show the Inspector by the same rule as the view keys (R9).
  - *Applied to the code:* keys like `j`/`k`/Enter act on `S.view`, the one view shown today
    (`keys.rs:320–431`). For `f` then `j` to keep walking Files when the Graph is also on screen, a
    view key focuses the pane it shows, and view-level keys act on the focused pane (R8).
  - *Applied to the code:* the view keys push a history entry and the URL hash carries the view
    (`nav.rs:128–173`); back and forward keep restoring the shown view and selection, by the same
    placement rule.
- **Q7:** Pane size. — **A:** Each pane is a size container, so views fit their pane, not the
  window: the Files columns (container queries), the Graph canvas (fits its pane on every resize),
  the Inspector. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* the Log is width-dependent too: it re-renders after a split drag and a
    window resize (`keys.rs:681`, `:694`). R15 includes it.
- **Q8:** Visual check. — **A:** An R-ID states what Geoffrey sees, graded on the rendered page by
  bounding rects: for each shipped preset at 1600×1000 and 900×1000, each pane's position and size,
  which views are in it, and collapsed panes at title-bar height; plus his example: the Graph's title
  bar on top, the Inspector left, Files right, both usable. (resolved 2026-11-08, foreman ruling
  under decision 2026-11-08-okt-dream-inbox-autonomous)
- **Q9:** Non-goals. — **A:** The dream-inbox view and the agent chat; drag-to-dock; per-workflow
  saved layouts beyond named presets plus the current arrangement; floating or undocked windows;
  any change to `/api/index`. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
- **Q10:** Size. — **A:** One cycle, no pilot split, even past about 12 R-IDs. (resolved
  2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous)

Answered after shaping:

- **Q11:** In "three", where do Log, Table and Results live? Ruling 3 places only the Graph, the
  Inspector and Files. — **A:** As tabs in the right pane beside Files, with Files shown (R4).
  (resolved 2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous)
- **Q12:** What does collapse do to a pane in a side-by-side split? A title bar runs across the top,
  so a side-by-side neighbour can't take the space below it. — **A:** It shrinks to a narrow
  vertical strip with its title turned on its side, and the neighbour takes the width; in a stacked
  split it shrinks to its title bar (R10). (resolved 2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous)
- **Q13:** Moving the last view out of a pane, which ruling 2 says can't stay empty. — **A:** The
  move closes that pane, exactly as closing it would. The last pane overall can't be emptied: moving
  its only view does nothing and says so (R14). (resolved 2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous)
- **Q14:** Existing users have `insp` saved in `okt:<kb>` (`state.rs:85`) but no arrangement. —
  **A:** Classic starts at that saved width, so the first load after the upgrade looks as before
  (R7). (resolved 2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous)
- **Q15:** The shaper's additions (the focused pane marked on its title bar, R8; new keys listed in
  the help overlay and README, R5), the shaper's disposition of the rest of the 900 px block (Q4),
  and 17 R-IDs as one cycle. — **A:** Accepted. (resolved 2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous)

Answered at the PLAN/TECH sign-off:

- **Q16:** With no arrangement saved, a window under 900 px and an `insp` key present, R6 starts in
  stacked while R7 starts classic at `insp`. The case is the common one: `Settings::to_json` writes
  `insp` on every save of any setting, so nearly every existing user has it. Which wins? — **A:** The
  window width picks the starting preset (R6: stacked under 900 px, classic otherwise). A pre-cycle
  `insp` width sizes classic only when classic is the preset that starts (R7). This reproduces
  exactly what each window showed before this cycle. (resolved 2026-11-08, foreman ruling under
  decision 2026-11-08-okt-dream-inbox-autonomous)
- **Q17:** R11 lets panes follow the pointer without bound, while R15 forbids overflow at any size,
  and content can't fit a pane a few pixels wide (today's splitter clamps at 22–75%). — **A:** A drag
  stops each side at 200 px wide, or at half the split when the split is under 400 px; the vertical
  floor is 120 px. Views fit down to 200 px wide. A pane below 200 px wide (only a shrinking window
  can cause one) hides its body behind its title bar rather than overflowing. R11 carries the bound
  and R15 the floor. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
- **Q18:** As PLAN first read R8 and R9 together, `c` focused the Inspector's pane, so a following
  `j` had no Graph walk to drive, though today `c` then `j` walks the Graph. Is that change
  acceptable? — **A:** No. Q6 binds: no existing key changes meaning in any arrangement. `o`, `c`
  and `h` switch the Inspector's tab and show it, expanding its pane if collapsed, or adding the
  Inspector to the focused pane by R9 if no pane holds it, but they do not move focus. `g`, `f`, `l`
  and `t` keep showing and focusing their view as R9 says. R9 is reworded to match. (resolved
  2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous)

## Related materials

- Seed: `issues/pane-arrangements.md` (promoted; `status: adopted:pane-arrangements`)
- Story 1, Geoffrey, 2026-11-08: KB `workspace/open-knowledge-terminal/stories.md`
- Source: `okt-client/static/index.html` (the pane DOM), `okt-client/static/app.css:151–157`
  (`.panes`, `.view`, `.vpane`, `.split`, `.insp`), `:448` (`#ibody` container precedent),
  `:552–561` (the 900 px block); `okt-client/src/nav.rs:77–173` (`set_tab`, `set_view`, the hash);
  `okt-client/src/keys.rs:288–580` (key dispatch), `:646–689` (the splitter);
  `okt-client/src/state.rs:79–250` (Settings, `okt:<kb>`, `save_settings`; high-blast-radius)
- Related cycles: `files-column-priority` (container queries on the Files table; lands first),
  `history-log-rows` (lands first), `issues/graph-galaxy.md` (on ice)
- Tests: `tests/ui/keys.mjs`, `tests/env.sh`
- **Q19:** Does a click in the Inspector's pane take key focus (R8), when Q6 says no existing key
  changes meaning and on main `j`/`k`/arrows walk the Graph whatever was clicked? — **A:** No. The
  Inspector's pane never takes key focus, by a click or by a key (extends Q18). A click in it leaves
  focus, and the view keys, on the last focused non-Inspector pane. The one exception is when the
  Inspector is the only pane. Its title bar is never marked focused, and scrolling the Inspector
  stays on `J`/`K` as on main. R8's "a click focuses a pane" reads with this exception. (2026-11-09,
  foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous, at review cycle 1, F4.)
- **Q20 (supersedes Q19):** What keeps `j`/`k`/arrows walking the Graph after a click in the
  Inspector, without stopping pane operations from reaching the Inspector's pane? — **A:** The
  Inspector's pane takes focus as R8 says: a click focuses it, `w w` steps to it, it is marked on its
  title bar, and every pane operation (R10, R12–R14) applies to it. Its view has no view-level keys of
  its own, so a view-level key pressed while it is focused (`j`/`k`, the arrows, Enter, space, `z`,
  `n`/`N`, and any key the focused view would otherwise handle) acts on the view of the last
  focused non-Inspector pane. On main those keys act on the one view pane, so Q6 holds. Q18 stands
  (`o`/`c`/`h` never move focus). Q19 is withdrawn: under it no pane operation could reach the
  Inspector's pane, which R10 and R12–R14 forbid. (2026-11-09, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous, review cycle 1, F4.)

