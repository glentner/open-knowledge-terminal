---
status: unshaped
kind: fix
appetite: small
---

# The SERVER STOPPED banner covers the pane title bars

## Problem

While the event stream is down, the SERVER STOPPED banner (`#offbar`) sits over the top of the
panes and hides most of their title bars: the tabs that name each pane's views, and the
Inspector's.

Evidence on `fix/preset-bar`:
- `okt-client/static/index.html:86`: `<div class="offbar" id="offbar" hidden>`.
- `okt-client/static/app.css:712`: the `.offbar` rule fixes it at `top: 44px`, centred
  (`left: 50%`, `translateX(-50%)`), at `z-index: 90`.
- `okt-client/src/chrome.rs:37`: `set_live` shows it 2.5 s after the stream errors; `:31` hides it
  once the stream is back, and `:557` while the offline page waits for the server. The rule is the
  same on `main`; only the grid under it changed.
- `okt-client/static/app.css:104-105`: the `#app` grid is now `38px minmax(0, 1fr) …`. On `main`
  it was `38px 28px minmax(0, 1fr) …`, and the 28 px row (y 38–66) held the preset buttons and the
  crumbs. `top: 44px` was placed for that row: the banner sat over it.
- With the row gone the panes start at y 38, and their title bars (`.ptitle`, 24 px,
  `app.css:166`) span y 38–62. At 1600×1000 the banner spans y 44–72.7 and x 501–1099, so it
  covers both title bars where they meet at x 864: 18 of their 24 px (y 44–62; the top 6 px stay
  visible), and about 11 px of the pane bodies below (preset-bar review cycle 1, O1: `probe.log`
  line 40, `.local/preset-bar/review1/` in the main checkout, with the banner forced visible).
- The banner wraps below 1196 px. It is `position: fixed` with `left: 50%` and no width, so its
  shrink-to-fit width is capped at half the window (500 px at 1000, 450 px at 900); at 1195 px and
  narrower the message takes two lines and the banner is 45.34 px tall (y 44–89.3). It still covers
  18 px of the title bars, and about 27 px of the pane bodies (preset-bar review cycle 2, G1:
  `probe-r2b.log` and `probe-r2.log`, `.local/preset-bar/review2/` in the main checkout).

## Why it was deferred

Out of preset-bar's scope: its R1 grades the steady state, with the stream up, and the banner shows
only while the stream is down. The foreman accepted it at preset-bar's PLAN gate as a known visual
effect of the removed row (`spec/preset-bar/PLAN.md`, "`#offbar` over the panes' top while the
event stream is down"; also its L3), and the review's triage seeded it as O1.

**Partly pre-existing.** The overlap depends on the width: below 1196 px, where the banner wraps,
`main` already covered more of the title bars than the branch does (preset-bar review cycle 2, G1:
`probe-r2b.log`; on `main` the title bars span y 66–90):

| Width | Banner | Title bar covered on `main` | Title bar covered on `fix/preset-bar` |
|---|---|---|---|
| 1196 px and wider | one line, 28.67 px | 6.67 of 24 px | 18 of 24 px |
| 1195 px and narrower | two lines, 45.34 px | 23.34 of 24 px | 18 of 24 px |

So preset-bar raised the overlap from 6.7 to 18 px at 1196 px and wider, and lowered it from 23.3
to 18 px below that. At every width on the branch the title bars' top 6 px stay visible.

## Outcome / vision

While the server is stopped, the banner is plainly visible and the panes' title bars stay
readable at every width, the narrow ones where it wraps included: it sits at the header's bottom
edge without covering them, or it overlays them by a deliberate design (a full-width strip that
replaces them for the outage, say), chosen at shaping. A fix that restores `main`'s placement alone
is not enough: `main` covered 23.3 of the 24 px below 1196 px.

## Sketch of the acceptance criteria

- **R1** — WHILE the event stream is down, the SERVER STOPPED banner SHALL NOT overlap any pane
  title bar (`.ptitle`) at 1600×1000, 1196×1000, 1195×1000 and 900×1000, whether its message takes
  one line (about 29 px tall) or wraps to two (about 45 px), unless shaping picks a deliberate
  overlay.
- **R2** — WHEN the stream comes back, the banner SHALL hide, as today.
- **R3** — WHILE the stream is up, the header and the panes SHALL sit exactly as preset-bar left
  them (the header 38 px, the panes from y 38).

## Notes

- Related: `spec/preset-bar/` (PLAN, the PLAN gate's accepted effects; REVIEW, O1);
  `issues/crumbs-kb-name-twice.md` (the other header follow-up from the same cycle).
- Found by: `preset-bar` plan (PLAN L3), confirmed by the cycle 1 review (O1), seeded in that
  cycle's review remediation; corrected for the wrap and `main`'s baseline after the cycle 2 review
  (G1).
