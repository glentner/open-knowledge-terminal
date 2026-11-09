# Research 01: today's pane DOM, view switching and focus

Code at `edcbed4`. Anchors are `file::symbol` with the line as of that commit.

## 1. The DOM and who touches it

`index.html`: `nav.fkeys` (five `button.fk[data-view]` + `#crumbs`), then `main#panes.panes` > `section#view.view` (five `div.vpane#v-{graph,files,log,table,list}`, `position:absolute; inset:0` inside the `position:relative` `.view`) + `div#split` + `aside#insp` (`#ihead`, `#itabs` with three `button[data-tab]`, `#ibody`).

Lookups by id (all go through the document-global `dom::id`/`dom::q`, so they work wherever the element sits):
- `#v-graph`: `graph::resize` (2128), `graph::wire` (ResizeObserver target, 2172), `graph::set_hint` (appends a lazily created `#ghint`, 1156).
- `#v-*` hidden toggle: **only** `nav::set_view` (97–99). Nothing else shows or hides a view.
- `#panes`, `#split`: `keys::wire` only (`--insp-w` and the splitter drag, 647–689).
- `#ihead`, `#ibody`: `insp::render` (224), `insp::render_commit` (1969), `keys::on_click` (anchors, 69/83, `.lsimple` in `#ibody`, 268), `keys::on_key` `J`/`K` (509) and `press("#ihead …")` (541–569), `list::find_step` (188), `lib::boot` error text (54/68).
- `#itabs button`: `keys::wire` (click listeners, 607), `insp::render` (`.on` and the History count, 252/267).
- `.fk`: `keys::wire` (click listeners, 601), `nav::set_view` (`.on`, 100). `#crumbs`: `nav::render_crumbs` only.
- `#view` and `#insp` themselves: never looked up by code.

## 2. Readers and writers of `S.view` (`state.rs:300`, `&'static str`, default `"graph"`)

Writer: `nav::set_view` only. Callers: `keys::on_key` g/f/l (436–444), `z` (519), `.fk` clicks (604), the `[data-view]` click delegate (95; e.g. the status bar's `data-view="log"` button in `chrome::render_status`), `go::run_named` GRAPH/FILES/LOG (224–227), `tree::key` space (413), `table::open` (104), `list::show` (26), `nav::apply_hash` (214), `lib::boot` (86).

| Reader | Purpose | Multi-view verdict |
|---|---|---|
| `nav::select` 54–68: graph `focus_node`, Files re-render, `#tgrid tr.on` | (a) render on selection | Becomes "is X visible" (shown tab of an expanded pane), per view, not one `if/else` on one value. |
| `tree::render` 262 (scroll the `.on` row into view) | (a) | "Files visible". |
| `insp::open_commit` 1957 (`#ltable .crow.on`) | (a) | "Log visible" (or toggle unconditionally; cheap). |
| `keys::wire` splitter `up` 680, window resize 693 (Log re-render); 60 s interval 701 (Files) | (a) | Replace the Log hooks with a ResizeObserver on the Log's root (covers split drag, neighbour collapse, preset); interval → "Files visible". |
| `live.rs` 85, 96, 101 (Log, Files, Table refresh on change) | (a) | "X visible". Note: a hidden Table is never refetched and `set_view("table")` renders the stale `S.table` (pre-existing). |
| `keys::on_key` 320–328 (`tree::key`, `log::key`), 377 (Escape ends the walk), 403 (graph walk j/k/Enter/space), 518 (`z`) | (b) key dispatch | "The focused pane's shown view" (R8). `z` shows the Graph by the placement rule. |
| `nav::state_hash` 131, 144, 149; `nav::apply_hash` 213, 228–235 | (c) | See § 6. |
| `lib::boot` 96 | (d) dead read | Drop. |

`set_view` itself does three jobs that must be split: **place** (hide the other four, `.fk.on`), **on-shown render** (Files `tree::render`; Log `ensure`+`render`; Table `render` or `open(current_folder)`; Results `list::render`; Graph `schedule`), and **history** (`nav(true)` unless quiet). The on-shown render must run whenever a view becomes visible by any path: tab click, expand, preset, swap, move, back/forward.

## 3. Lazy rendering

- **Graph**: no view gate. `draw` returns early while `gs.w`/`gs.h` ≤ 0 (`graph::draw` 1430); hidden `#v-graph` reports 0×0 to the ResizeObserver, and showing it fires `resize` → `apply_view` → `schedule`. So Graph already redraws itself when shown or resized, including in a collapsed-then-expanded pane.
- **Files, Log, Table, Results**: render only from `set_view`, from their own events, or from the gated hooks above. Hidden views are stale until shown.
- **Inspector**: `insp::render` is never gated; always current.
- **Selection triggers (R3)**: `select` always calls `graph::update`, `render_crumbs`, `insp::render`; Files and Table marks and the graph camera move are gated on `S.view`. The Log's `.crow.on` follows `S.commit`, **not** the card selection; Results rows carry no selection mark at all (the FIND marks are `mark.fm` in `#ibody`, driven by `S.mark`). With R2 every gate in the table above becomes per-view visibility; nothing else is needed for Graph.
- **`graph::host_rect` hazard**: the canvas's client offset is cached in `resize` and used by `user_point` (945) and the tooltip (2079). A ResizeObserver does **not** fire on a position-only move (probe below), so a swap or a neighbour change that keeps the Graph pane's size leaves hit-testing and zoom-at-pointer off by the move. Call `graph::resize` after every arrangement apply, or read the rect at event time.

## 4. Reparent, don't regenerate

Listeners are attached **to the elements** at boot (`dom::on_id`/`dom::on` in `tree::wire`, `log::wire`, `table::wire`, `list::wire`, `graph::wire` on the canvas and `.gtools`, `keys::wire` on `.fk`/`#itabs`), plus two document-level delegates (`keys::on_click`, `keys::on_key`) and window-level pointer listeners in `graph::wire`. Module state assumes the same elements persist: `graph::G` holds the canvas and its 2D context; `tree::render`'s `LAST` cache only toggles `.on` when the HTML is unchanged, so a freshly generated empty `#tree` would stay empty. Ids stay unique because a view lives in at most one pane.

Probe (headless Chromium 156, throwaway page): after `appendChild` of a subtree into another container, listeners fire, the canvas keeps its context and pixels, ids resolve. But **scrollTop resets to 0**, **focus is lost to `body`**, and **`blur` fires synchronously inside `appendChild`** (a re-entrancy point under AGENTS.md § Invariants 1: no `S` borrow across the move). `Element.moveBefore(node, null)` kept scroll (500) and focus with no blur. `hidden` toggling preserved scroll.

Recommendation: generate the pane frames (title bar, tab strip, body with `position:relative` and `container-type`) and **move** the existing `#v-*` and `#insp` roots into pane bodies, using `moveBefore` when present and otherwise `appendChild` with saved-and-restored `scrollTop` of `#tree`, `#ltable`, `#tgrid`, `#list`, `#ibody` and re-focus; then `graph::resize`. Keep `#view`'s role (positioning context) on the pane body.

## 5. Focus today

Key dispatch ignores focus except to bail out on `input`/`textarea`/`select`/contenteditable (Escape blurs them; `keys::on_key` 292–303). `#tree`, `#ltable`, `#tgrid`, `#list` carry `tabindex="0"` but no code focuses them; a click inside one focuses it by browser default, which changes nothing. The canvas and `#ibody` are not focusable, so clicking the Graph or the Inspector leaves focus on `body`. Only `#go` is focused programmatically (`/`, `:`; `go::run_cmd` 713). So "focused pane" must be new client state, set by a delegated `pointerdown` on the arrangement root (closest pane) and by the view keys, not derived from `document.activeElement`.

## 6. Hash and history

`state_hash` carries `v` (always), `s`, `tab` (≠ overview), `c`, `rev`, `t` (only when `v=table`), `q` (only when `v=list`). `nav(push)` is suppressed while `S.restoring`. `apply_hash` opens the Table folder or re-runs FIND when the hash asks, then `set_view(v, quiet)` if `S.view ≠ v`, restores selection, and re-runs the Graph camera and Files render gated on the view. To apply GOAL Q6: `v` becomes the view the last view key showed (the focused pane's shown view); restoring calls the placement routine (show in the holding pane, expand, focus; add to the focused pane if unheld) with history suppressed, then the per-view-visible renders. The arrangement itself is not in the hash.

## 7. `.fkeys` and `#crumbs`

Options for the global strip once panes have their own tabs (R1):
- **A. Remove `.fkeys`; keep a slim window-level bar for `#crumbs`.** No duplicate tabs; crumbs stay window chrome, so the `<900px` hide rule stays as Q4 disposes. Costs one row for crumbs alone.
- **B. Move `#crumbs` into the header** (between the command bar and the chips). Saves the row; the header gets busier on a narrow window, where crumbs hide anyway.
- **C. Keep `.fkeys` as a launcher** running the placement rule, `.on` = visible or focused. Two tab strips that mean different things; spends the row.
- **D. Crumbs into the Inspector's title bar.** Then their hide rule moves to that pane's container query (Q4 sub-bullet).

Pane tabs written as `button[data-view]` without class `fk` are already handled by `keys::on_click`'s delegate (93–97), so no per-button wiring is needed; the boot-time `.fk` loop in `keys::wire` goes with the strip. `#itabs` (O/C/H) stays inside the Inspector view.

## 8. Results

Every path into Results is `list::show` → `set_view("list", false)` (`list.rs` 26): `list::find` (from `go::run` 692, `go::run_cmd` 723/740 for `/q` and unmatched input, `go::run_named` FIND 250), `list::list_of` (BROKEN/DIRTY/…/TYPE, `[data-list]`, `[data-type]`), and the `[data-more]` click in `keys::on_click` 260. `apply_hash` re-runs `find` for `q`. So the placement rule lands in one place for Results (the new show routine called by `list::show`). `table::open` likewise funnels through `set_view("table")`.

## Questions for the planner

1. **Hash `v` in a multi-view arrangement**: is it the focused pane's shown view, or the last view a view key showed? And should `t`/`q` ride whenever Table/Results is visible (changes today's hashes only in multi-view arrangements) or only when it is `v`?
2. **Back/forward and the arrangement**: when restoring `v` expands a collapsed pane or adds a view to the focused pane, is that an arrangement change that R7 saves? Back never restores a previous arrangement.
3. **Focused pane on load (R7)**: saved with the arrangement, or the pane holding the hash's `v`?
4. **`z` and Files' space**: today they switch to the Graph. Under R9's rule they show it in its holding pane; in classic that replaces Files in the same pane, as today. Confirm that focus moves to the Graph pane (so the next `j` walks the graph, as today).
5. **R3 "Log marks"**: the Log marks the open commit, not the selected card; "as it does today" reads as no new Log behaviour. Confirm.
