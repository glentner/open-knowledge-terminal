# Research 03 — the layout engine

Read at `edcbed4`: `okt-client/static/app.css` (`#app`, `.panes`, `.view`, `.vpane`, `.split`, `.insp`,
`#ibody`, the `@media (max-width: 900px)` block), `static/index.html`, the splitter in `keys.rs`
(`dom::on_id("split", "pointerdown", …)`), `dom.rs`, `graph.rs` (`wire`, `resize`), `views/log.rs`.
Measured with a throwaway `file://` probe in Playwright Chromium 158 at 1600×1000 and 900×1000.

## 1. Rendering the tree — recommend nested flex, binary splits

Each split is a flex container (`flex-direction: row` side by side, `column` stacked) holding
`child · gutter · child`; every child has `min-width: 0; min-height: 0`. Expanded children carry
`flex: <weight> 1 0px` with **weight = fraction × 1000** (never the raw fraction: flex-grow values
summing below 1 leave free space unfilled). A collapsed leaf is `flex: 0 0 24px`; a gutter is
`flex: 0 0 5px` (today's `.split`: 5px with 1px borders).

**Size formulas** (binary split with first-child fraction `f`, extent `E` along its axis, gutter `G = 5`,
collapsed size `C = 24`):

- both expanded: first = `(E − G)·f`, second = `(E − G)·(1 − f)`;
- one child a collapsed leaf: collapsed = `C`, the other = `E − G − C`;
- the cross axis is the full extent of the split; nested splits recurse on their own rect.
- Root: `E = W` (window width) for a row, `H` for a column, where today
  `H = window height − 110` (`#app` rows 38 + 28 + 22 + 22; `#app.noticker` drops the ticker's 22). If the
  plan removes the `.fkeys` row, `H` gains 28; state it in PLAN.

Measured (getBoundingClientRect, f = 0.54 classic, 0.5 elsewhere, H = 890):

| preset | 1600 | 900 |
|---|---|---|
| classic | left 0..861.2969, Inspector at x=866.2969, width 733.7031 | 483.2969 / 411.7031 |
| three | Graph 1600×24 at y=0; row at y=29, h=861; Inspector and Files 797.5 each, Files at x=802.5 | 900×24; 447.5 each |
| stacked | 1600×442.5, second at y=447.5 | 900×442.5 |

Rounding: Chrome lays out in 1/64-px units, so `getBoundingClientRect` returns values like 733.7031 for
733.7, always within 0.02 px of the formula. `offsetWidth`/`clientWidth` are rounded integers (734). The
same tree at the same window size lays out the same way every time, so R7's 1 px and R17's 2 px hold with
room to spare. Tests should compare rects to the formula, not to rounded integers.

**Today's grid differs.** `minmax(0,1fr) 5px var(--insp-w)` takes the percentage of the whole width:
measured 859 / 736 at 1600. Under the formula, f = 0.46 gives 861.3 / 733.7, so the Inspector is
2.3 px narrower (0.46 × 5). See Questions.

Rejected:
- **Nested grids:** the same arithmetic (fr totals below 1 have the same trap), but every collapse or
  drag means rewriting the parent's template string instead of one child's style.
- **Absolute positioning computed in Rust:** gives exact integer pixels, but needs a ResizeObserver
  and a layout pass on every window resize. Views would either still have to be re-parented, or sit
  outside their panes, which breaks "the pane is the container" (R15).

**Binary, not n-ary.** Every preset is a binary tree; three is
`column[Graph (collapsed), row[Inspector, Files…]]`. With binary splits, close means the parent split is
replaced by the sibling subtree, so the neighbours take the space for free; swap means exchanging the
two children; flip means toggling the orientation. Each split stores exactly one `f`.

**DOM building.** The client mostly uses `set_html` plus lookups by id, and the views are static
elements in `index.html` (`#v-graph`, `#v-files`, …, `#insp`). Keep the views' identity: build the
split and pane skeleton with `set_html`, then *move* each view root into its pane body. The probe
measured the move:
- `appendChild` re-parent: resets `.tree`/`.ltable` `scrollTop` to 0, drops focus, and **fires
  `blur` + `focusout` synchronously inside the call**. That is the AGENTS.md Invariant 1 hazard: hold no
  borrow of `S`, settings or graph state across it. The canvas bitmap survives.
- `Element.moveBefore` (present in Chromium 158) keeps `scrollTop` and focus and fires no events.
  It throws `HierarchyRequestError` into a detached parent. Moving through a `hidden` parking element
  loses `scrollTop`.
- So: insert the new skeleton beside the old one, `moveBefore` each view into it, then remove the old
  skeleton. Feature-detect `moveBefore` (web-sys 0.3.106 has no binding, so use a small
  `#[wasm_bindgen]` extern with `js_name = moveBefore, catch`). The fallback is `appendChild`, saving
  and restoring `scrollTop` with no borrow held.
- Re-parent only on structural changes (preset, close, move view, swap). Drag, collapse and flip
  change styles or classes only.

## 2. Collapse

Today's metrics: `.vhead` 35 px (6 + 22 px input + 6 + 1 border), `.itabs` 31 px, `.fkeys` 28 px
(`.fk` 27), `.fk kbd` 18 px tall. **Title bar: 24 px**, border-box including its 1 px bottom border;
it holds an 18 px kbd chip at 11.5 px text. A collapsed pane in a column is exactly 24 px tall, with its
body `display: none`.

Side-by-side collapse: a **24 px strip** (so `C` is one constant), with the title bar `flex: 1` and
`writing-mode: vertical-rl` (top-to-bottom; measured 24 × 76 for "FILES · LOG"). `sideways-lr`
(bottom-to-top) is supported in Chromium 158 if preferred. Avoid `transform: rotate`: it doesn't
change the layout box. The collapsed look follows the parent's orientation, so a flip turns a strip
into a title bar automatically.

**Every child collapsed.** Two collapsed leaves in one split leave nothing to grow, and the space
goes unfilled. Recommend the **accordion rule**: collapsing a pane whose sibling is a collapsed leaf
expands that sibling, so the neighbour takes the space and R10 holds as written. That keeps the
invariant "no split has two collapsed-leaf children", and with it every split subtree has an
expanded leaf. Collapsing the only pane is refused (nothing can take the space). Alternative: refuse
the collapse with a toast. See Questions. Expand restores exactly, because the stored `f` is never
touched by collapse.

## 3. Drag-resize (R11)

Today's splitter: a window `pointermove`/`pointerup` added per drag, both `Closure`s `.forget()`-ed (it
leaks two closures per drag despite the comment), percentage clamped to 22–75 and rounded to 0.1, the CSS
var set live, then `save_settings()` and a log re-render on pointerup. It does not use pointer capture.

Recommend one delegated set of listeners, wired once on the arrangement root:
- `pointerdown` on `.gutter` calls `set_pointer_capture(pointer_id)` and records in a thread-local the
  split's id, orientation and rect (read once).
- `pointermove` sets `f = (client − rect.start) / (E − G)`, clamped, and writes the two children's
  `flex`.
- `pointerup` / `lostpointercapture` saves and notifies the views. No borrow is held across
  `save_settings` or the render.

Clamp in px at drag time (e.g. each side ≥ 120 px → `f`), and store only `f`. Don't put a CSS
min-width on panes: it would break the formulas. A gutter next to a collapsed leaf is inert (no drag,
default cursor) but keeps its 5 px, so the formula stays constant.

Cost on drag: two style writes and a re-layout of a dozen boxes. The graph canvas redraws through its
ResizeObserver each frame, the same as today.

## 4. Neighbour, swap, flip

Define **neighbour = the focused pane's sibling in its parent split**.
- **Swap:** exchange the parent's two children. When the sibling is a subtree, the pane trades places
  with the whole subtree; in three, swapping the Graph puts it at the bottom.
- **Flip:** toggle the parent split's orientation.
- **Close:** the parent is replaced by the sibling. The views go to the sibling's leaf nearest the
  shared edge: descend, taking the near child when the split has the parent's orientation and the
  first child otherwise.
- **Move view:** goes to that same leaf.
- The root-only pane has no neighbour: refuse and say so (R13/R14).

Directional neighbours (h/j/k/l by geometry) are richer but not required. See Questions.

## 5. Container queries

Put `container: pane / size` on the **pane element** (title bar plus body), not on the body. Then
title-bar tab rules and view rules can both query it, and nothing queries itself. `size` is safe here
because every pane's extents come from flex: the probe's size container in a flex column got its full
500×300, and children at `height: 100%` resolved. The risk with `size` is only an auto-height
ancestor, and there is none. `inline-size` suffices for every query the GOAL needs; use `size` if
R15's "size container" is graded literally.

- Always use **named** queries (`@container pane (…)`). `#ibody` stays its own unnamed inline-size
  container, and `.md > table`'s `100cqw` still resolves to it (nearest container).
- Files: files-column-priority should name its container (`#tree`, e.g. `container-name: files`) so the
  pane container never intercepts its rules.
- Graph: `.glegend` is a descendant of the pane, so it can query `pane`. **Don't use 900 as the
  threshold**: classic's Graph pane is 861 px at 1600 and shows the legend today.

**Graph and Log hooks.** The Graph's ResizeObserver on `#v-graph` fires on size changes only. A
pane that **moves without resizing** leaves `g().host_rect` stale, and pointer hit-testing
(`graph.rs` ~947, the tip at ~2079) goes wrong. Classic → swapped at 1600 is exactly this case: the
Graph pane keeps 861.3 px and moves 738 px right. So call the graph's `resize()` (make it `pub`) after
every arrangement apply. The Log reads `#lchart.clientWidth` and re-renders only on split pointerup
or window resize, when `S.view == "log"`. Replace that with a debounced ResizeObserver on `#lchart`.

## 6. Overflow (R15)

Measured: `overflow: hidden` **and** `overflow: clip` both still report `scrollWidth` 400 >
`clientWidth` 200. Clipping does not pass the check; content must really shrink. Overflow inside a
nested `overflow: auto` (`.tree`, `.ltable`, `.tgrid`, `.list`, `.ibody`) does not count. An absolute
descendant past the edge does count (350 > 200).

Widths below which today's views overflow (measured, realistic meta text):

| view | overflows below | culprit |
|---|---|---|
| Table | 464 px | `.vhead`: vt 40, tfold, input min 80, tmeta 99 nowrap, COLUMNS 66, CSV 37, gaps |
| Log | 264 px | `.vhead` (`lmeta`) |
| Inspector | 288 px | `.itabs` buttons, nowrap (98 / 90 / 90) |
| Files | 220 px | `.vhead` |

Graph and Results did not overflow down to 120 px.

**Three at 900** gives 447.5 px panes, so the Table overflows there today. Fixes:
- `.vhead > .vmeta, .tfold`: `min-width: 0; overflow: hidden; text-overflow: ellipsis;
  white-space: nowrap`.
- `.vhead input`: `min-width: 0` (or ~60 px).
- `.mini`: `flex: none`.
- `.itabs button`: `min-width: 0` with an ellipsised label, or hide the label under
  `@container pane (width < ~300px)`.
- `.gtip`: `max-width` 360 against the JS flip at `x + 300 > w`; cap it at 300 and clamp x ≥ 0.
- `.lchart` (svg 100%) and `.gtools` are safe.
- New title bar: 5 tabs at `.fk` metrics are about 395 px, so tab labels need shrinking (kbd only)
  under a pane container query.

## 7. The 900 px block (`app.css` `@media (max-width: 900px)`), per Q4

- `.panes {…rows…}`, `.split {display:none}`, `.insp {border-top}`: **delete**. The stacked preset
  replaces them.
- `.glegend {display:none}`: becomes `@container pane (width < N)` (threshold per §5).
- `.word, .kbname, .crumbs {display:none}`: **stays** window-scoped, as `@media (width < 900px)` to
  match Q4's `< 900`. If the crumbs move into a pane, their rule moves to that pane's query.
- `.trow {…}`, `.trow .ti, .trow .num:nth-of-type(4)`, `.trow .nm.wide`: already removed by
  files-column-priority (lands first). Confirm none remain (R16).

## Questions for the planner

1. **Legacy `insp` and the 46% default.** Today's grid takes the percentage of `W`; the formula takes
   it of `W − 5`, so the Inspector is about 2.3 px narrower. Either accept that and state it in PLAN
   (R17 grades PLAN's sizes), or convert once at migration, `f = p·W/(W − 5)` at the first load's
   width, so R7's "looks as before" holds exactly.
2. **All children collapsed.** Accordion (keeps R10 as written) or refusal (narrows R10)?
3. **Neighbour.** Sibling-based, so a swap can exchange a pane with a subtree. Is that acceptable
   for R12–R14, or does the GOAL want spatial adjacency?
4. **Container type.** `size` vs `inline-size` for R15's "size container".
5. **Legend threshold.** The pane width below which `.glegend` hides.
