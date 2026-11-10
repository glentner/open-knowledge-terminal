---
status: unshaped
kind: fix
appetite: small
---

# The KB name shows twice in the header when nothing is selected

## Problem

With the crumbs in the header (preset-bar), a window 1350 px wide or wider (with `~/KB`) shows the
KB's display name twice when nothing is selected: once in the brand (`#kbname`), and again in the
crumbs, right after the GO form.

Evidence on `fix/preset-bar`:
- `okt-client/src/chrome.rs:173` writes the KB's display name into `#kbname`, in the brand.
- `okt-client/src/nav.rs:396-397`: `render_crumbs` with no selection writes the same name into
  `#crumbs`, as its only crumb.
- `okt-client/static/index.html`: `#crumbs` now sits in `header.top`, between `form#goform` and
  `div.chips`, and gets only the room GO leaves at its 900 px cap (preset-bar PLAN § 2.1). With
  `~/KB` the crumbs box opens at about 1339 px wide, the name first paints at 1350 px and is whole
  from 1377 px (preset-bar review cycle 2: `probe-r2.log`, `.local/preset-bar/review2/` in the main
  checkout).
- On `main` the crumbs sat in their own row, `nav.fkeys` (`okt-client/static/index.html:37-45` on
  `main`), under the header, and showed the same repeat there: with nothing selected, the brand
  held the KB name and the row's only crumb held it again, at 1600 and at 1000 px (preset-bar
  review cycle 1, F2: `mainemu.mjs`, `probe2.log` and `emu-main-1600.png` under
  `.local/preset-bar/review1/` in the main checkout).

### A related limit: a long KB display name leaves the crumbs no room

The crumbs get only the room GO leaves, and the brand takes its full width first, so every pixel
the KB name adds to the brand comes out of the crumbs. On `fix/preset-bar`:
- `okt-client/static/app.css:110`: `.brand` is `flex: none`, so it never shrinks;
- `okt-client/static/app.css:113`: `.kbname` has no width bound (no `max-width`, no ellipsis);
- `okt-client/static/app.css:114`: `.go` is `flex: 0 1 900px`, and keeps its 900 px cap at 1600 px;
- `okt-client/static/app.css:131`: `.crumbs` is `flex: 1 1 0; min-width: 0`, so it takes what is
  left and nothing more.

Measured at 1600 px with a deep selection (preset-bar review cycle 1, F1: `probe.log`,
`probe3.log`, `deep-1600.png`): with `~/KB` the crumbs are 261 px wide with 8 pieces painted; under
the throwaway clone's temp path (a KB outside `~` is shown as its full path) they are 5.78 px with
nothing painted; under a 74-character macOS temp path (`/private/var/folders/…/okt-env.…/kb`),
0 px. The long path squeezes GO as well: the brand is 618.7 px wide and GO shrinks from its
900 px cap to 678.1 px (preset-bar review cycle 2: `probe-r2.log`). Geoffrey's KB is `~/KB`, so his
window is unaffected; a KB outside `~` with a long path is.

## Why it was deferred

The crumbs' content is a non-goal of preset-bar's GOAL, which moved the crumbs without changing
what they hold. The foreman accepted the double name for that cycle at the PLAN gate, with
Geoffrey to decide whether it needs changing (PLAN § PLAN gate, Q4).

**Pre-existing.** `main` already showed the repeat, across two rows: the KB name in the brand, and
again as the only crumb in the `nav.fkeys` row below it. preset-bar moved the crumbs into the
header, so the two copies now sit side by side in one row; it did not introduce the repeat.

The long-name limit is preset-bar's: the GOAL-gate ruling gives the crumbs only the room GO leaves,
and the review's triage judged F1 as designed for `~/KB` and recorded it here rather than fixing
it in that cycle.

## Outcome / vision

If Geoffrey dislikes it: with no selection, the header shows the KB name once. And a long KB
display name no longer starves the crumbs: the brand's name gives way (bounded, or elided) before
the trail does.

## Sketch of the acceptance criteria

- **R1** — WHILE nothing is selected, the header SHALL show the KB's display name at most once.
- **R2** — WHEN a card is selected, the crumbs SHALL show its trail as they do today.
- **R3** — WHILE a card is selected at 1600 px, the crumbs SHALL show at least the trail's first
  piece, whatever the length of the KB's display name.

## Notes

- Related: `spec/preset-bar/` (PLAN § 4.1, "For the foreman, not the gate"; Q4).
- Found by: `preset-bar` plan (the adversarial review's note to the foreman), seeded in P1;
  corrected and widened in that cycle's review remediation (review cycle 1, F2 and F1).
