# GOAL — Drag a view's tab to dock it in another pane

> **Origin spec.** The *what* and *why* — the locked contract `code-review` grades against.
> The *how* lives in [`PLAN.md`](PLAN.md) and [`TECH.md`](TECH.md) (written by `code-plan`).
> Keep this at the right altitude: solved and bounded, but not over-specified — leave design
> freedom for the plan. Edit requirements here; do **not** silently drift them during build.

- **slug:** drag-to-dock
- **kind:** feature
- **appetite:** big

## Problem

Pane arrangements (0.4.0, `e5b94ec`) arranges views by presets and keys only. Moving a view to
another pane, or splitting a pane to give a view its own, takes the `w` leader, and the leader only
reaches a *neighbour*: `w v` / `w V` move the focused pane's shown view one pane over
(`arrange::move_view`, `okt-client/src/arrange.rs:775`), so a view three panes away takes several
steps, and no operation at all splits a pane to open a new one beside it. The mouse can resize
splits (the sashes, `okt-client/src/panes.rs`) and pick a tab (a click on `.ptab`, handled in
`okt-client/src/keys.rs:137`), but it can't move a view.

`spec/pane-arrangements/GOAL.md` named this as the follow-up (Non-goals: "Drag-to-dock, and any
mouse drag of a view between panes. A follow-up candidate."; Q1, 2026-11-08). Geoffrey, 2026-11-09:
"The visual fix and the drag-to-dock can be their own seed." The sibling cycle `preset-bar` removes
the always-on preset row, so after it lands a drag becomes the main mouse path for arranging.

What is there today, at `origin/main` `50c9533` (pre-existing, not a regression):

- The arrangement is a binary tree of row and column splits whose leaves are panes holding views as
  tabs (`arrange::Node`, `Pane`); every key operation is a pure function on that tree returning an
  `Outcome` (changed, focus, toast), run by `nav::operate` (`okt-client/src/nav.rs:156`), which
  saves the settings, redraws (`panes::apply`), renders views that came on screen and replaces the
  hash without a history entry.
- A press anywhere in a pane focuses it (`panes::on_down`); the press on a tab is followed by the
  click that shows that tab's view.
- The operations that exist: `place`, `apply_preset`, `collapse_toggle`, `focus_step`, `swap`,
  `flip`, `close`, `move_view` (to the neighbour only). None moves a view to an arbitrary pane and
  none splits a pane.

## Outcome / vision

Geoffrey presses a view's tab in a pane's title bar and drags it. As the pointer crosses the panes,
an overlay shows where the view will land: the whole pane when over its centre (the view joins that
pane as a tab), or the half of the pane on that side when near one of its four edges (the pane
splits there and the view gets the new half). Where a drop isn't allowed, no overlay shows. He
releases, and the arrangement is exactly one the keys could have reached: saved, restored on
reload, with focus on the view he moved. Esc, or letting go somewhere invalid, leaves everything as
it was. A click on a tab still just shows that tab, and every key does what it did.

**Terms** (as in `spec/pane-arrangements/GOAL.md`, plus):
- *Drag source*: a view's tab (`.ptab`) in a pane's title bar, the Inspector's included.
- *Target pane*: the pane under the pointer.
- *Zone*: one of a target pane's five landing zones: its centre, or its left, right, top or bottom
  edge. PLAN states the zones' geometry.
- *Valid zone*: a zone a release would change the arrangement through, under R4–R7.

## Acceptance criteria (the contract)

**Starting and showing a drag**

- **R1** — WHEN the user presses a pointer on any pane's tab and moves it, held, at least the
  movement threshold PLAN states (a few px), a drag of that tab's view SHALL begin; WHEN the pointer
  is released having moved less than the threshold, it SHALL act exactly as a click on the tab does
  today and nothing SHALL move. Every tab SHALL be a drag source: each of the six views, shown or
  not, in an expanded or collapsed pane (a collapsed side-by-side strip's turned tabs included), and
  the Inspector's under the same rules as every other view, as the `w` operations treat it. A drag
  SHALL NOT also act as a click on its tab, pan the Graph, resize a split or select text.
- **R2** — WHILE a drag is under way and the pointer is over a valid zone, exactly one overlay SHALL
  show the region the view will occupy if released there: the target pane's whole rect for its
  centre, the half of the target pane on that side for an edge. WHILE the pointer is over no valid
  zone (outside every pane, on an invalid zone), no overlay SHALL show. The overlay SHALL predict the
  result: after a release on a valid zone, the dropped view's pane rect SHALL equal the overlay's
  last rect within 2 px.

**Dropping**

- **R3** — WHEN a drag is released over the centre of a pane other than the view's own, the view
  SHALL leave its pane and join that pane as its last tab and shown tab, expanding the pane if it
  was collapsed. The source pane SHALL keep its other views; if the dragged view was its shown tab,
  it SHALL show the view after it (the first, after the last), as `w v` does. A pane's title bar
  counts as its centre.
- **R4** — WHEN a drag is released over an edge zone of a pane, that pane SHALL split on that side:
  a new pane holding only the dragged view SHALL take that side, the target pane SHALL keep its
  other views on the other side, and the new split SHALL share the target pane's former extent
  half and half. The target may be the view's own pane, when it holds other views.
- **R5** — WHEN a drop takes the last view out of its pane, that pane SHALL close as moving the last
  view out with the keys closes it today (pane-arrangements R14): its sibling takes its space. The
  arrangement SHALL never hold an empty pane, a view in two panes, or a view in no pane that was in
  one before the drag.
- **R6** — WHEN a drag is released over its own pane's centre or title bar, outside every pane, or
  over an invalid zone; WHEN the user presses Esc during a drag; or WHEN the drag loses its pointer
  (`pointercancel`, lost capture, the window losing focus), the drag SHALL end with the overlay gone
  and the arrangement, every pane's tabs and shown tab, the saved settings and the hash unchanged
  from the moment the drag began. Esc SHALL do nothing else.
- **R7** — IF a drop would leave a pane it creates narrower than 200 px in a side-by-side split or
  shorter than 120 px in a stacked one at the current window size (the pane-arrangements floors,
  `arrange::MIN_W`, `MIN_H`), or would break another arrangement rule (R5; the accordion, never two
  collapsed panes in one split nor a collapsed root), THEN that zone SHALL be invalid: no overlay
  SHALL show over it and a release there SHALL change nothing. No drop SHALL ever be applied
  part-way.

**The result**

- **R8** — Each drop SHALL be an operation on the existing arrangement tree, run through the same
  path as the `w` operations (one short settings borrow, then the focus, the drawing, the save, the
  render of each view that came on screen, the hash); there SHALL be no second layout model. A
  centre drop onto the pane `w v` or `w V` would reach from the source pane, of the source pane's
  shown view, SHALL produce exactly the tree that key produces.
- **R9** — WHEN a drop changes the arrangement, okt SHALL save it in the KB's settings (localStorage
  `okt:<kb>`) as a key-made change is saved, and a reload at the same window size SHALL restore the
  same tree, views, shown tabs, collapsed states and each pane's rect within 1 px
  (pane-arrangements R7). The hash SHALL be replaced as after the equivalent `w` operation, naming
  the focused pane's view, and no history entry SHALL be pushed: back and forward SHALL behave
  exactly as they would after that `w` operation, restoring views and selection, never undoing the
  arrangement.
- **R10** — After a drop, focus SHALL be on the pane holding the dropped view and that pane's title
  bar marked (for the Inspector, its pane takes focus and the view keys act on the last focused
  non-Inspector view, pane-arrangements Q20). The view the keys act on SHALL be drawn wherever the
  drop leaves it (the pane-arrangements key-view property). Each view that comes on screen SHALL
  render and fit its pane (pane-arrangements R15, the Graph canvas filling its pane), and each moved
  view SHALL keep its state (scroll, selection, Inspector tab) as a `w v` move keeps it.
- **R11** — No key SHALL change meaning: `tests/ui/keys.mjs` and `tests/ui/panes.mjs` SHALL pass with
  their existing assertions keeping their expected outcomes (selectors may change where the DOM
  moved, as `preset-bar` leaves it).

**What Geoffrey sees**

- **R12** — A browser suite in the throwaway environment SHALL drive real pointer drags (Playwright
  mouse down, move, up), not synthetic DOM events or calls into the client, and grade from bounding
  rects and the saved arrangement in `okt:<kb>`:
  - for each shipped preset at 1600×1000 and 900×1000, and for every pane in it and every one of
    that pane's five zones, a view dragged there from another pane: the overlay shown or not as R7
    says, and after release each pane's rect, the views and shown tab each pane holds, the overlay's
    prediction (R2), focus (R10) and the saved arrangement after a reload (R9);
  - a drag of the Inspector's tab, and a drop that empties its source pane (R5);
  - cancel by Esc, by a release outside every pane and on the view's own pane's centre, each leaving
    the rects, tabs, saved settings and hash unchanged (R6);
  - a refusal: an edge zone of a pane too narrow to split shows no overlay and its release changes
    nothing (R7);
  - click-not-drag: a press and release that moves less than the threshold shows the tab and moves
    nothing (R1).

  Presets are applied by their keys; the suite SHALL NOT depend on the preset buttons, which
  `preset-bar` removes.

## Non-goals (no-gos)

- Floating or undocked windows, and dragging a view out of the browser window or between windows.
- Touch gestures. The pointer events the drag uses do carry touch and pen input, but touch does not
  work for free: the tabs don't set `touch-action: none` (only the sashes do,
  `okt-client/static/app.css:172`), so a touch drag on a tab is taken by the browser as a scroll.
  The plan may make touch work if it costs nothing; it is not required and not tested.
- Changes to drag-resize (the sashes) and to the floors it applies.
- Reordering tabs within a pane by drag, and dropping into a specific tab position.
- Dragging a whole pane (its title bar's background) rather than one view's tab.
- An undo for arrangement changes; back and forward don't carry the arrangement (R9).
- Any new key, and any change to the presets, to `/api/index` or to the server.
- The preset bar and the crumbs' place in the header: `preset-bar`'s scope.

## Clarifications

Foreman rulings, under Geoffrey's word in session (2026-11-09: "Run both cycles now with 0.4.1 and
0.4.2 releases."):

- **Q1:** Drag source. — **A:** A view's tab in a pane title bar, by pointer events, with a short
  movement threshold so a click still just selects the tab. The Inspector's tab drags too, under the
  same rules the `w` operations apply to it (R1). (2026-11-09, foreman ruling under Geoffrey's word)
  - *Applied to the code:* the `w` operations give the Inspector no rule of its own
    (`arrange::move_view`, `close`, `swap` treat every view alike); what is particular to it is
    focus, pane-arrangements Q18 and Q20. R1 drags it as any view and R10 carries Q20.
- **Q2:** Drop targets. — **A:** A pane's centre joins that pane as a tab; each of its four edges
  splits that pane on that side. A visible overlay shows the landing zone while dragging. Dropping on
  the view's own pane centre, or anywhere outside a valid target, does nothing. Esc cancels
  mid-drag (R2–R4, R6). (2026-11-09, foreman ruling under Geoffrey's word)
- **Q3:** Emptying and refusal. — **A:** A drag that would empty a pane removes that pane, as moving
  the last view out does with the keys today, and a drop that would break the narrow floor or
  another arrangement rule is refused visibly (no target shown), never applied half-way (R5, R7).
  (2026-11-09, foreman ruling under Geoffrey's word)
  - *Applied to the code:* the floors are the drag floors, 200 px across and 120 px down
    (`arrange::MIN_W`, `MIN_H`). Removing a source pane and joining a pane only grow or keep the
    panes they touch, except that joining a collapsed pane expands it, as `w v` into it does today
    (`move_view` expands its target); so only an edge split, which makes two panes out of one, can
    break the floor (R7).
- **Q4:** Persistence and history. — **A:** The result is saved and restored exactly like a key-made
  arrangement; back/forward and the hash behave as after the equivalent `w` operation (R9).
  (2026-11-09, foreman ruling under Geoffrey's word)
  - *Applied to the code:* `nav::operate` ends in `nav(false)`, a `replaceState`; no `w` operation
    pushes history, and pane-arrangements has no undo. So a drop pushes no entry and back doesn't
    undo it.
- **Q5:** Keys and focus. — **A:** Keyboard behaviour is unchanged; focus after a drop goes to the
  dropped view's pane, and the key view is drawn (R10, R11). (2026-11-09, foreman ruling under
  Geoffrey's word)
- **Q6:** The browser suite. — **A:** It drives real pointer drags (Playwright mouse down, move, up)
  for each target kind in every preset at 1600×1000 and 900×1000 and grades the result from bounding
  rects and the saved arrangement, plus cancel, refusal and click-not-drag (R12). (2026-11-09,
  foreman ruling under Geoffrey's word)
  - *Shaper's addition:* the lesson of pane-arrangements' five review rounds is to grade a property
    across its whole domain, so R12 covers every pane and every zone in each preset, and R2 makes
    the overlay a prediction the suite checks after each drop.
- **Q7:** Out of scope. — **A:** Floating windows, dragging between browser windows, touch gestures
  (say whether pointer events make touch work for free, without requiring it), and drag-resize
  changes. (2026-11-09, foreman ruling under Geoffrey's word)
- **Q8:** "Every drop through the existing arrangement operations, not a second model." —
  *Applied to the code:* the operations that exist can't express a drop as they stand: `move_view`
  reaches only the neighbour, and nothing splits a pane. R8 therefore reads the ruling as one model:
  the plan adds the drop's operations to `arrange.rs` as pure functions on the same `Node` tree,
  built from its helpers (the tree walk, `lift`, `accordion`, the floors) and returning the same
  `Outcome`, run by `nav::operate`; and where a drop and a key coincide (a centre drop on the `w v`
  neighbour), they give the same tree.
  **A:** Confirmed: one model, the drop operations added to `arrange.rs` as pure functions on the same tree, run by `nav::operate`. (resolved 2026-11-09, foreman ruling at the GOAL gate)
- **Q9:** The sibling cycle. — **A:** `preset-bar` lands first, removing the `nav.fkeys` row with the
  preset buttons and moving `#crumbs` into the header. This GOAL is written for the code that
  leaves: the drag source is the tabs in each pane's title bar, which `preset-bar` doesn't touch,
  and nothing here depends on the preset row (R11, R12). (2026-11-09, foreman ruling under
  Geoffrey's word)

Ruled at the GOAL gate (2026-11-09):

- **Q10:** Ruling Q2 makes the own pane's centre
  a no-op but is silent on its edges. *Recommendation:* when the own pane holds other views, its
  edges are valid and split it, giving the dragged view the new half (the common way to open two
  tabs side by side); when it holds only the dragged view, none of its zones is valid.
  **A:** As recommended: the own pane's edges are valid and split it when it holds other views; with only the dragged view, none of its zones is valid. (resolved 2026-11-09, foreman ruling at the GOAL gate under Geoffrey's word)
- **Q11:** *Recommendation:* a target pane's
  title bar is part of its centre zone, and a join appends the view as the last tab, as `w v` does;
  no insertion by drop position (Non-goals).
  **A:** As recommended: the title bar is part of the centre zone, and a join appends the tab last. (resolved 2026-11-09, foreman ruling at the GOAL gate under Geoffrey's word)
- **Q12:** *Recommendation:*
  the new split shares the target pane's former extent half and half, so an edge split is valid only
  where the target pane is at least 400 px wide (left, right) or 240 px tall (top, bottom); and a
  collapsed pane (a 24 px bar or strip) offers only its centre, which joins and expands it.
  **A:** As recommended: half and half; edges valid only at ≥ 400 px wide (left/right) or ≥ 240 px tall (top/bottom); a collapsed pane offers only its centre. (resolved 2026-11-09, foreman ruling at the GOAL gate under Geoffrey's word)
- **Q13:** *Recommendation:* follow the pane
  leader's precedent (`keys.rs:360`): Esc cancels the drag and does nothing else; any other key
  cancels the drag and is then handled as usual, so no key can change the arrangement under a live
  drag.
  **A:** As recommended: Esc cancels and does nothing else; any other key cancels the drag and is then handled as usual. (resolved 2026-11-09, foreman ruling at the GOAL gate under Geoffrey's word)
- **Q14:** A press in a pane focuses it today
  (pane-arrangements R8, `panes::on_down`), before any drag can begin. *Recommendation:* a cancelled
  drag or no-op drop leaves focus where that press put it (the source pane), and the pressed tab is
  not shown, since no click happened; "unchanged" in R6 covers the arrangement, the tabs, the saved
  settings and the hash, not focus.
  **A:** As recommended: focus stays where the press put it (the source pane); the pressed tab is not shown. R6's "unchanged" excludes focus. (resolved 2026-11-09, foreman ruling at the GOAL gate under Geoffrey's word)
- **Q15:** The overlay where emptying the source grows the target, or a collapsed target
  expands (PLAN D1). — **A:** The overlay shows the rect the dropped view takes in the resulting
  arrangement, which is what R2 grades within 2 px. Q12's "half and half" is of the target's extent
  in the result. (resolved 2026-11-09, foreman ruling at the PLAN gate)
- **Q16:** Which size the floors read (PLAN D2). — **A:** R7's floors and Q12's 400 px and 240 px
  are read on the result's layout, as R7 words it. They differ from the target's current size only
  when a closing source pane sits beside the target. (resolved 2026-11-09, foreman ruling at the
  PLAN gate)

## Related materials

- Seed: `issues/drag-to-dock.md` (promoted; `status: adopted:drag-to-dock`)
- Prior cycle: `spec/pane-arrangements/` (GOAL R1, R7, R8, R11, R13–R15, Q13, Q17, Q18, Q20; PLAN,
  TECH, REVIEW), landed as `e5b94ec`
- Sibling cycle: `issues/preset-bar.md` (lands first)
- Source: `okt-client/src/arrange.rs` (the tree, `move_view` :775, `close` :744, `lift` :820,
  `min_extent` :842, `MIN_W`/`MIN_H` :10–12); `okt-client/src/panes.rs` (`build`, `update`,
  `apply`, `move_into`/`moveBefore`, `key_view`, `on_down`/`on_move`/`on_up`, `pane_at`);
  `okt-client/src/nav.rs` (`operate` :156, `place` :124, `sync_view`, `nav`);
  `okt-client/src/keys.rs` (the tab click :137, the pane leader :360); `okt-client/src/state.rs`
  (`arr`, `arr_saved`, `okt:<kb>`; high-blast-radius); `okt-client/static/app.css:160–210` (panes,
  sashes, title bars, tabs)
- Tests: `tests/ui/panes.mjs`, `tests/ui/keys.mjs`, `tests/env.sh`
