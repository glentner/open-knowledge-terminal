# Research digest — drag-to-dock

Three briefs, fanned out in parallel on 2026-11-09 (appetite big), plus the planner's own
measurements. Each decision below is the one PLAN.md builds on; where briefs differ, the resolution
is stated.

- [`01-pointer-events.md`](01-pointer-events.md): pointer, click, capture, key and blur semantics in
  headless Chromium under real Playwright input (a throwaway lab, deleted).
- [`02-after-drop-path.md`](02-after-drop-path.md): `nav::operate`, focus, the key view, view state,
  ids, persistence, the hash, the click and key paths, other pointer listeners (code reading).
- [`03-test-harness.md`](03-test-harness.md): what the gate and the probe reuse from
  pane-arrangements, timings, R11's grading, the pitfalls that cycle hit.

## Measured by the planner (env.sh on 6587, today's tree at `6f175fe`)

- The pure tiling (`arrange.rs` test helper `rects`: a split's children take `f·E` and `(1−f)·E`, a
  collapsed child 24 px) matches every preset's drawn pane rects at 1600 and 900 px to **0.000 px**.
  So one pure layout can predict the overlay and decide the floors, and the browser grades it.
- Every shown view root, and the Graph canvas, fills its pane body exactly (0 px) in every preset
  and for each view key; no root scrolls sideways. The probe's `fit` check holds on today's page.
- Costs: first load to ready about 130–150 ms; a reload to ready about 90 ms; a fresh context about
  160 ms; a 12-move Playwright drag about 230 ms. `#panes` is 890 px tall today and 918 px after
  preset-bar; nothing may hard-code it.

## Decisions

1. **One pure function decides everything about a drop** (02 risk 1). `arrange::docked` builds the
   result on a clone of the tree, or returns `None` for an invalid zone. The overlay is the rect of
   the dropped view's pane in `layout(result)`; validity is `docked(..).is_some()`; the release
   applies the same result through `nav::operate`. The prediction is the result by construction,
   and refusal is total by construction (the live tree is replaced only by a whole valid result).
2. **The overlay shows the rect the view will occupy in the result**, which is the target's rect
   (centre) or its half (edge) whenever the drop neither closes a source pane next to the target nor
   expands a collapsed target. When it does, R2's two sentences disagree; PLAN follows R2's "SHALL
   predict the result" sentence. A deviation row and a foreman question (PLAN § 3, § 5).
3. **The floors and the accordion are read on the result** (R7 "would leave a pane it creates
   narrower than …"), not on the target's current rect (Q12's 400/240 px reading). They agree unless
   a source pane closing beside the target grows it. Same row.
4. **Pointer handling** (01): states Idle → Pending → Dragging, plus Cancelled, which swallows the
   rest of the press. Capture on the tab at the 4 px threshold; never `preventDefault` on
   `pointerdown` (it keeps a focused input focused and redirects keys), never `draggable`.
   Coordinates are read as f64 through `Reflect` (web-sys `client_x()` is an integer).
5. **Every drag ends with its click swallowed** (01 rows 1–2, 02 § 7): a one-shot capture-phase
   `click` listener on `window`, armed in `pointerup` (and on a cancel while the button is down),
   eating only `detail > 0` clicks, removed by `setTimeout(0)` or on use. A below-threshold release
   arms nothing, so its click is today's click.
6. **Cancels**: a capture-phase `keydown` on `window` (02 § 8: `#go` has its own keydown, so a check
   inside `keys::on_key` can't make Esc do nothing else). Modifier-only keys are ignored (the
   leader's precedent); Esc cancels and is stopped; any other key cancels and goes on. Window
   `blur`, `pointercancel` and `lostpointercapture` (on `document`, where Chromium delivers it after
   a removal, 01 row 5) cancel. A tree that changed under the drag (an async render placing a view)
   cancels at the release instead of applying a stale target.
7. **§ Invariants 1 in the handlers** (02 § 1): the drag state is a `thread_local` taken out
   (`take()`/`replace`) before any call that can re-enter: `set_pointer_capture`,
   `panes::focus`, `nav::operate`, a toast, showing or hiding the overlay. The `operate` closure gets
   only moved-in values (view, pane id, zone, `#panes`'s width and height).
8. **No `state.rs` change, no load-path change** (02 § 5): `sanitise` keeps every valid result as it
   is, up to renumbering the ids. A new pane takes `max id + 1` (02 § 4), never `len + 1`, and no
   drop renumbers, so a centre drop on the `w v` neighbour is equal to `move_view`, ids included.
9. **R11 is graded strictly** (03 § 4): `tests/ui/keys.mjs` and `tests/ui/panes.mjs` byte-identical
   to the merge base with main after preset-bar lands, each exiting 0 with no FAIL or skip line.
   drag-to-dock has no reason to touch them. `history.mjs` is not run (R11 doesn't name it, and its
   fixture reads the live KB).
10. **The gate is split into parts, each one env.sh run** (03 § 3): the full gate is past the
    10-minute cap. Its logs are kept under the main checkout's `.local/drag-to-dock/`. The probe takes
    a group filter (`DOCK_GROUPS` or argv; a prefix selects every group it begins), and the gate a
    `--part` and `--groups`.
11. **Touch**: `touch-action: none` on `.ptab` is one CSS declaration (01 row 9); the plan adds it,
    untested, as the GOAL allows.

## Contradictions resolved

- 01 suggests hit-testing with `elementFromPoint`; 02 suggests `pane_at`. Neither: the target is
  found from the pure layout of the current tree at the pointer's position relative to `#panes`
  (`arrange::aim`). That is what the overlay is computed from too, so DOM and model can't disagree
  about which pane is under the pointer without the probe seeing it (measured 0 px apart today).
- 01 says no `releasePointerCapture` is needed on a drop (the browser releases on `pointerup`); 02
  worries about `lostpointercapture` from the rebuild. Both hold: the state is taken out before
  `operate`, so the asynchronous `lostpointercapture` that follows the rebuild finds Idle and does
  nothing.
