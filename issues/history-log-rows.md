---
status: adopted:history-log-rows
kind: feature
appetite: small
---

# History: compact rows under day rules, like the Log

## Problem

Geoffrey, 2026-11-08: the inspector's History tab looks double-spaced and clashes with the Log
view's compact look, which he prefers.

Evidence at `8d2f001`: `history_view` (`okt-client/src/insp.rs:1831`) lays commits out as `.hrow`
(`okt-client/static/app.css:356`), `64px 88px 18px minmax(0, 1fr) auto`. The 88 px date-and-time
column wraps the date over the time, so every commit is two lines tall. The Log already separates
days with a sticky `.dayrule` (`app.css:267`).

## Why it was deferred

Not deferred: Geoffrey's request, the fourth cycle of the run. The behaviour is pre-existing.

## Outcome / vision

History matches the Log's density: one line per commit with `HH:MM` inline after the hash, and days
separated by the Log's day rule, labelled `Weekday Month Day Year · N`, where N is that day's
commit count.

## Open questions for the GOAL

1. Does the label change the Log's own day-rule text too? Lean: one format for both views.

## Sketch of the acceptance criteria

- **R1** — Each History row SHALL be one line: hash, `HH:MM`, status, subject, actions.
- **R2** — WHEN several commits share a day, they SHALL sit under one day rule labelled
  `Weekday Month Day Year · N`.
- **R3** — The History day rule SHALL use the Log's `.dayrule` style.

## Notes

- Found by: Geoffrey, chat 2026-11-08 (story 4, KB `workspace/open-knowledge-terminal/stories.md`).
