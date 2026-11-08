---
status: unshaped
kind: feature
appetite: big
---

# Pane arrangements: place, swap, stack and collapse views

## Problem

Geoffrey, 2026-11-08: horizontal space runs out on smaller screens, and the two primary panes can be
resized but not moved. The view pane (Graph, Files, Log, Table, Results: "GFLTR") is always on the
left and the inspector (Overview, Content, History: "OCH") always on the right.

Evidence at `8d2f001`:
- `.panes` is `grid-template-columns: minmax(0, 1fr) 5px var(--insp-w, 46%)`
  (`okt-client/static/app.css:152`). The splitter resizes; nothing swaps.
- The top-and-bottom arrangement he saw once is `@media (max-width: 900px)` (`app.css:552`):
  automatic on a narrow window, never a choice.
- Every future view he expects needs a pane the fixed grid can't host: the dream-protocol card view
  for fast, system-one triage, an agent chat, other per-workflow views.

## Why it was deferred

Not deferred: Geoffrey's request, the second cycle of the run, after `graph-galaxy`. The behaviour
is pre-existing. Layout state belongs in settings, in `okt-client/src/state.rs`, a high-blast-radius
path, so the plan runs research.

## Outcome / vision

A general arrangement system: a tree of row and column splits whose leaves are panes, each pane
holding a set of views as tabs. Any pane can be collapsed to its title bar, expanded, resized,
swapped with a neighbour or moved. Arrangements are named and saved in settings, and a key cycles
them. His example must be expressible: the graph on top, collapsed; the inspector on the left; a
third view on the right.

That has an implication worth naming: GFLTR stops being one fixed tab strip. Each view becomes
placeable, so Files and Graph can be open at once in different panes.

## Open questions for the GOAL

1. **Interaction.** Presets and keys first and drag-to-dock later, or both now. Lean: presets and
   keys first.
2. **The third pane today.** With only the existing views, what fills it? Lean: any existing view,
   so the system is real and tested before the dream view arrives.
3. **The 900 px rule.** Keep it as the default for narrow windows, or retire it once arrangements are
   a choice. Lean: retire it in favour of a narrow preset.
4. **Pane size.** Each pane reports its own size, so views fit their pane rather than the window.
   `files-column-priority` and `graph-galaxy` both depend on it.

## Sketch of the acceptance criteria

- **R1** — WHEN the user swaps panes, the view pane and the inspector SHALL exchange places.
- **R2** — The user SHALL be able to set any split to side by side or stacked.
- **R3** — WHEN a pane is collapsed, it SHALL shrink to its title bar AND its neighbours SHALL take
  the space.
- **R4** — The arrangement system SHALL support at least three panes, each able to host any view.
- **R5** — The arrangement SHALL persist across reloads, AND named presets SHALL be selectable by key.
- **R6** — Every existing key SHALL keep its meaning in every arrangement, AND `tests/ui/keys.mjs`
  SHALL pass.

## Notes

- Proposed non-goals: the dream-inbox view and the agent chat. This cycle unblocks them; it doesn't
  build them.
- Related: `issues/graph-galaxy.md`, `issues/files-column-priority.md`.
- Found by: Geoffrey, chat 2026-11-08 (story 1, KB `workspace/open-knowledge-terminal/stories.md`).
