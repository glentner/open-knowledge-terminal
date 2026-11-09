---
status: unshaped
kind: tests
appetite: small
---

# No suite check guards the off-screen Log against a redraw per sash move

## Problem

Dragging a sash redraws the arrangement on every pointer move: `on_move` ends in `apply()`
(`okt-client/src/panes.rs:425-448`), and `apply` ends in `draw_key_view()`
(`panes.rs:143-149`). When the key view is off screen, `draw_key_view` draws it only when it differs
from the view latched in `KEY_OFF` (`panes.rs:51-53`, "drawn once as it gets there and not again on
every redraw (a sash drag redraws on each move)"). That latch is all that stops a full Log render on
each move of a drag, and nothing in `tests/ui/` checks it.

Evidence, from `pane-arrangements` review cycle 5 (C1): with the Log alone in a collapsed pane
(a seeded arrangement, window 1600 px) and made the key view with `w w`, a 30-move drag of the root
sash rendered the Log's rows 0 times at HEAD and 30 times with the latch removed (mutant M2:
`if let Some(v) = off {` in place of the `last != Some(*v)` filter). The suite (`keys.mjs` 87/0, `history.mjs` 105/0, `panes.mjs` 68/0)
stayed green under M2. The probe and its logs (`probe5.mjs`, `mut-M2-drag.log`) are under
`.local/pane-arrangements/review5/` in the clone that ran the review; `.local/` is git-excluded, so
the numbers above are the record.

HEAD behaves correctly. This is a coverage gap, not a defect: a later change to `draw_key_view` or
to `apply` could drop the latch and no check would notice, at a cost of one full Log render (up to
1500 rows) per pointer move.

## Why it was deferred

Found by review cycle 5 of `pane-arrangements` as a LOW coverage observation, under Geoffrey's
decision (a) to seed such findings and release 0.4.0. Introduced with the arrangement system (the
latch is new there); not pre-existing on the previous `main`.

## Outcome / vision

A check in `tests/ui/panes.mjs` counts the Log's renders during a sash drag with the Log the key
view off screen, and goes red when the latch is removed.

## Sketch of the acceptance criteria

- **R1** — WHEN a sash is dragged through many moves while the Log is the key view off screen
  (collapsed, or under the narrow floor), the Log's rows SHALL NOT be rendered again during the
  drag.
- **R2** — The check SHALL go red under a mutant that removes the `KEY_OFF` filter in
  `draw_key_view`, and its label SHALL print `ok` in the run log at HEAD.
- **R3** — After the drag, `j`/`k` SHALL still step through the Log's rows as before it.

## Notes

- Counting renders: a `MutationObserver` on `#ltable` that counts child-list additions, reset just
  before the drag, as `probe5.mjs` did.
- Pair with `issues/offscreen-log-mark.md`: a fix there that redraws an off-screen Log more often
  must keep this check green.
- Related: `spec/pane-arrangements/REVIEW.md` § Review cycle 5 (C1).
- Found by: `pane-arrangements` review cycle 5 (C1, LOW coverage).
