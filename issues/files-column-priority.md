---
status: unshaped
kind: feature
appetite: small
---

# Files table: columns negotiate width by priority, Name first

## Problem

Geoffrey, 2026-11-08: on a wide screen the Files table is right, but as space shrinks every column
compresses and the most important field, Name (the path), becomes unreadable.

Evidence at `8d2f001`: `.trow` (`okt-client/static/app.css:223`) is
`minmax(0, 1fr) minmax(0, .9fr) 92px 56px 64px 60px 22px`. Name and Title split the flexible width
almost evenly while the fixed columns keep theirs, so as the pane narrows Name is crushed first,
and Title, the least useful column in his words, keeps nearly as much.

## Why it was deferred

Not deferred: Geoffrey's request, the third cycle of the run. The behaviour is pre-existing.

## Outcome / vision

Name keeps its full width up to a cap and is truncated only beyond it. As the pane narrows, whole
columns drop out, header and cells together, in his order: Title, then Lines, then Size, then
Changed, then Type.

## Open questions for the GOAL

1. **The cap.** Lean: about 60 characters, as one tunable value.
2. **Where to cut.** At the end, or keep the file name and cut the folders
   (`…/pearc28/venues.md`). Lean: keep the file name.
3. **What to measure.** The pane, not the window: CSS container queries on the table, or a
   `ResizeObserver`. Lean: container queries, which are pure CSS and stay right once panes are
   rearrangeable.

## Sketch of the acceptance criteria

- **R1** — WHILE the pane is wide enough for every column, the Files table SHALL show them all, with
  Name untruncated up to the cap.
- **R2** — WHEN the pane narrows, columns SHALL drop out in the order Title, Lines, Size, Changed,
  Type, before Name truncates below its cap.
- **R3** — WHEN a path is longer than the cap, Name SHALL truncate with "…" AND the full path SHALL
  stay available on hover.
- **R4** — The drop order SHALL follow the pane's width, not the window's.

## Notes

- Proposed non-goal: the other tables (the Log's `.crow`), unless the GOAL widens it.
- Related: `issues/pane-arrangements.md`.
- Found by: Geoffrey, chat 2026-11-08 (story 2, KB `workspace/open-knowledge-terminal/stories.md`).
