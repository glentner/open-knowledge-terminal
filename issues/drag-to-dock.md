---
status: unshaped
kind: feature
appetite: big
---

# Drag a view between panes to dock it

## Problem

Pane arrangements (0.4.0) arranges views by presets and keys only. Moving a view to another pane,
splitting a pane, or swapping two takes the `w` leader. The mouse can resize splits and pick tabs,
but it can't move a view.

`spec/pane-arrangements/GOAL.md` named this as a follow-up:
- Non-goals: "Drag-to-dock, and any mouse drag of a view between panes. A follow-up candidate."
- Q1: "Interaction: presets and keys, or drag-to-dock too? — **A:** Presets and keys first.
  Drag-to-dock is a non-goal and a follow-up candidate." (foreman ruling, 2026-11-08)

Geoffrey, 2026-11-09: "The visual fix and the drag-to-dock can be their own seed."

## Why it was deferred

It was out of pane-arrangements' appetite. The arrangement model and its operations
(`okt-client/src/arrange.rs`, `okt-client/src/panes.rs`) had to exist first.

## Outcome / vision

Drag a view's tab by its title onto another pane's edge or centre. A drop target shows where it
will land: split left, right, top or bottom, or join that pane as a tab. The result is the same
arrangement the `w` operations would make, saved the same way.

## Sketch of the acceptance criteria

- A drag of a tab onto a pane's centre moves the view into that pane, and onto an edge splits that
  pane on that side. Both are expressed as the existing `arrange.rs` operations, not a second model.
- A visible drop indicator, and Esc cancels the drag.
- The pane-arrangements rules hold after any drop: the 200 px narrow floor, one view in one pane,
  the Inspector's place, persistence per KB, and the key view drawn.
- A browser suite drives drags and grades the result from bounding rects, as `tests/ui/panes.mjs`
  does for presets.

## Notes

- Floating or undocked windows stay out of scope, as in pane-arrangements.
- Related: the preset bar's removal, shipped 2026-11-09 (`spec/preset-bar/`), which leaves
  drag-to-dock the main mouse path for arranging.
- 2026-11-09: the preset bar is gone (preset-bar); drag-to-dock is the mouse path to an arrangement
  still to come.
