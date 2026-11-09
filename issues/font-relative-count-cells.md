---
status: unshaped
kind: fix
appetite: small
---

# Count cells sized in characters, not pixels, so a `--font` change can't cut them

## Problem

The History counts cell (`okt-client/static/app.css`, `.hrow.hl .ad`, 105 px) and the Log's older
`.ad` column (96 px) are sized in pixels, for the default monospace font's 7.5 px advance. Under
okt's documented `--font` flag with a wider monospace font, their counts are cut at every width.
For example, the real `+21,899 −7,071` row in
`workspace/pearc28/research/location/method.md`'s History shows `+21,899 −7,0…` under Menlo
(105.375 px of text) and under SF Mono (108.2 px), even at 75%, where the subject has 653 px.

Found by the history-log-rows blind review, cycle 3 (2026-11-09, finding F-font). It was out of that
cycle's scope, because its GOAL defines the cell at the History font's 7.5 px advance. The Log's
coupling predates that cycle.

## Outcome / vision

The counts cells, and every width and container-query threshold derived from them (the History
row's actions cell and its `@container` threshold), are written in `ch`. A cell built to hold N
characters then holds N characters in any monospace font okt is started with.

## Draft acceptance criteria (input to shaping, not a contract)

- R1: WHEN okt runs with `--font` naming a monospace font wider than the default, a History row's
  counts of up to 14 characters, and a Log row's counts that fit today at the default font, SHALL
  show whole wherever they show.
- R2: The History row's counts-hide threshold SHALL still fall exactly where the subject cell reaches 0,
  under the default font and under at least one wider font.

## Notes

- Fix sketch from the review: `width: 14ch` for `.hrow.hl .ad`, with `.rt` and the `@container`
  threshold in `ch` too; the Log's `.ad` likewise.
- The suites run Chromium with the default font stack; a check under `--font Menlo` would need the
  server started with that flag inside `tests/env.sh`.
