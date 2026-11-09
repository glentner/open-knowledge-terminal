---
status: unshaped
kind: fix
appetite: small
---

# An off-screen Log, shown again, steps from an outdated row mark

## Problem

When the Log's pane is under the 200 px narrow floor, the Log is off screen but still the key view,
and `j`/`k` step through rows it drew while off screen. Showing it again does not bring its row mark
up to date, so the next `j` or `k` steps from the wrong row. On screen the same keys step from the
right one.

Evidence at `pane-arrangements`' landing on `main`:
- `okt-client/src/nav.rs:112-115`: `set_view` places the view, then draws it only
  `if panes::visible(v.name())`. An off-screen Log is not redrawn when it is shown again by its key
  (`l`), its LOG tab, or the hash (back and forward).
- `okt-client/src/panes.rs:143-149`: `draw_key_view` draws an off-screen key view only when it
  differs from the last one latched in `KEY_OFF` (`panes.rs:52-53`). Showing the Log again while it
  is already the latched off-screen key view draws nothing.
- `okt-client/src/views/log.rs:330-349`: `log::key` steps from whichever row carries `.on`, so a
  mark left behind by an earlier step decides where the next step lands.

Reproduction (pane-arrangements review cycle 5, H1; window 760 px wide, so the Log's pane is 190 px,
against 1000 px wide and a 250 px pane on screen):
1. `l`, then `j j j`: rows 0, 1, 2 open, on screen and off.
2. `[` (back): the Inspector shows row 1 in both cases. On screen the mark is on row 1 and `j` opens
   row 2. Off screen the mark stays on row 2, and `j` opens row 3.
3. From a fresh load, `l`, `j j`, Esc, then `l` or a click on the LOG tab: on screen the mark is
   cleared and `j` opens row 0. Off screen the mark stays on row 1, and `j` opens row 2.

The probe's run logs (`probe-base.log`, `control-probe.log`) are under `.local/pane-arrangements/
review5/` in the clone that ran the review; `.local/` is git-excluded, so the steps above are the
record.

## Why it was deferred

Found by review cycle 5 of `pane-arrangements`, which ran under Geoffrey's decision (a): seed any
new edge route of the "off-screen key view steps differently from on screen" kind and release
0.4.0. It is a new route of that kind, reached only under the narrow floor. It was introduced with
the arrangement system, not a regression of the previous `main`, where the Log was never off screen
while it held the keys.

## Outcome / vision

The view that keys go to steps the same way whether its pane is on screen or not: whenever an
off-screen key view is shown again, by any path, its row mark matches what the same path leaves on
screen.

## Sketch of the acceptance criteria

- **R1** — WHEN the Log is the key view under the narrow floor AND the history moves back or
  forward, `j` and `k` SHALL step from the row the Inspector shows, as on screen.
- **R2** — WHEN the Log is shown again under the narrow floor by `l` or its LOG tab, its row mark
  SHALL match what the same action leaves on screen.
- **R3** — `tests/ui/panes.mjs` SHALL carry both scenarios, each checked against a mutant that
  restores the outdated mark.

## Notes

- The fix belongs in the one place every route passes through (`panes::apply` → `draw_key_view`),
  as G1's did, not in a call added per route.
- Related: `spec/pane-arrangements/REVIEW.md` § Review cycle 5 (H1); `issues/sash-drag-redraw-guard.md`
  (the latch this must keep working).
- Found by: `pane-arrangements` review cycle 5 (H1, MEDIUM/CONFIRMED).
