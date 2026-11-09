---
status: unshaped
kind: fix
appetite: small
---

# A highlighted token named `l` is drawn as a source row

## Problem

The Inspector's source view draws each line as `<div class="l">`, and the stylesheet's source-row
rule is `.src .l` (`okt-client/static/app.css`: a grid of the blame gutter, the line number and the
code, `grid-template-columns: var(--gut, 0px) 46px minmax(0, 1fr)`). Pygments' class for literal
tokens is also `l`: the highlighter emits a YAML scalar as
`<span class="l l-Scalar l-Scalar-Plain">`. `.src .l` matches that span too, so with the blame on,
each literal span inside a line becomes a three-column grid of its own, with its own 210 px gutter
column.

Observed by the pane-arrangements blind review, cycle 3 (2026-11-09), on
`workspace/pearc28/sites/_common.yaml` with the blame on: `#ibody` scrolls sideways at 360–440 px
(scroll width 466 in a 440 px body). The pane itself doesn't overflow. Evidence:
`.local/pane-arrangements/review3/r3-yaml2b.log` (the nested `.l` spans and their widths), with
`r3-yaml.mjs` and `r3-yaml2.mjs` beside it.

## Why it was deferred

It is pre-existing: the `.src .l` rule and the token markup are the same on `main`. It sits outside
the pane-arrangements cycle, whose review recorded it as an observation out of scope, to be seeded
with Geoffrey's decision on that cycle's F1.

## Outcome / vision

The source-row rule matches only source rows. No token class the highlighter emits can take a
row's layout, in any language, with the blame on or off, so a literal span stays inline in its
line's code column.

## Sketch of the acceptance criteria

- **R1** — WHEN a source file whose highlighted tokens include the literal class is shown with the
  blame on, each line SHALL draw as one row of gutter, line number and code, and `#ibody` SHALL
  not scroll sideways at any width at which the same file without literal tokens doesn't.
- **R2** — The source-row rule SHALL NOT match any element inside a row's code column.

## Notes

- Fix sketch: scope the row rule to the rows themselves (`.src > .l`, or a row class the
  highlighter can't emit, such as `.srow`), and check the other `.src` rules for the same overlap
  with Pygments' short class names (`.n`, `.k`, `.s`, …).
- Related: `spec/pane-arrangements/REVIEW.md`, review cycle 3, "Observation, out of Scope".
- Found by: `pane-arrangements` blind review, cycle 3 (2026-11-09).
