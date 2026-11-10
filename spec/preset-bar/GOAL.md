# GOAL — No preset bar: the crumbs move into the header

> **Origin spec.** The *what* and *why* — the locked contract `code-review` grades against.
> The *how* lives in [`PLAN.md`](PLAN.md) and [`TECH.md`](TECH.md) (written by `code-plan`).
> Keep this at the right altitude: solved and bounded, but not over-specified — leave design
> freedom for the plan. Edit requirements here; do **not** silently drift them during build.

- **slug:** preset-bar
- **kind:** fix
- **appetite:** small

## Problem

0.4.0 (pane-arrangements, `e5b94ec`) put a full-width row under the header, `nav.fkeys`
(`okt-client/static/index.html:37-45`, the `#app` grid's 28 px second row at `app.css:104-105`).
It holds four always-on preset buttons, `#presets` (CLASSIC `;`, SWAPPED `'`, STACKED `\`, THREE
`` ` ``), and the selection's breadcrumb trail, `#crumbs` (`nav::render_crumbs`), pushed to its
right. `panes.rs:125-129` marks the button whose preset the arrangement equals (`.on`), and
`tests/ui/panes.mjs:725` and `:779-786` grade that mark and a click on a button.

Geoffrey, 2026-11-09, after driving 0.4.0:

> Visually, I don't love the new vertical space taken to tell of the ';', ''', '\', '`' keybindings
> to switch through the visual modes with the panes. […] I just don't like the always displayed
> helper bar there. […] Honestly though I think we should just get rid of it.

The row spends 28 px of every window's height on a reminder of keys he already uses, which the
help overlay (`?`) and README § Keys already list. The crumbs are the row's only other tenant, so
keeping the row for them alone would keep most of the cost.

## Outcome / vision

The header sits directly on the panes. The preset buttons are gone, the presets stay one key away
as before, and the crumbs live in the header between the GO bar and the chips, the first thing to
give way as the window narrows.

## Acceptance criteria (the contract)

Every criterion is graded on the rendered page (`getBoundingClientRect`, computed visibility),
never on the stylesheet or the markup.

- **R1** — At each preset (classic, swapped, stacked, three) and at window widths of 1600 and
  900 px, the panes' top edge SHALL meet the header's bottom edge (within 0.5 px), and no visible
  element SHALL lie between them across the window's width.
- **R2** — The page SHALL show no preset buttons, at any width and in any arrangement.
- **R3** — WHILE the window is wider than 900 px, the crumbs SHALL render inside the header's box,
  after the GO form and before the chips, overlapping neither, at every width from 1600 px down to
  901 px; and the header SHALL keep its single-row height (38 px today) at every width.
- **R4** — At any window width, the GO form's and each chip's rendered width SHALL be the same
  (within 1 px) whatever the crumbs hold, comparing a deep selection's trail with no selection:
  the crumbs truncate or hide, and the GO form and the chips do not shrink for them. WHEN a card is
  selected in a 1600 px window, the crumbs SHALL be visible with a non-zero width.
- **R5** — WHILE the window is 900 px wide or narrower, the crumbs SHALL be hidden, as the brand
  word and the KB name are.
- **R6** — WHEN `;`, `'`, `\` or `` ` `` is pressed, okt SHALL apply that preset; WHEN `p` is
  pressed, it SHALL cycle the presets in that order; and the `w` leader's pane operations SHALL
  work as before. The help overlay (`?`) and README § Keys SHALL still list every one of them.
- **R7** — `tests/ui/panes.mjs` SHALL grade the presets by the arrangement each key, the cycle and
  a re-applied preset after a drag produce (each pane's rendered rect against the preset's
  definition), in place of its checks of the bar's mark and click; no check SHALL read a preset
  button. The suite's PRESETS header comment, README's and `AGENTS.md`'s test descriptions, and
  any other prose naming a window bar or preset buttons SHALL describe the page as it is.

## Non-goals (no-gos)

- Preset buttons anywhere else, the top bar included (the seed's fallback, ruled out below).
- Any new mouse path to a preset; drag-to-dock (`issues/drag-to-dock.md`) is the mouse follow-up.
- The crumbs' content and click behaviour, the 900 px breakpoint, and the brand and KB name's
  hiding rule: unchanged, apart from where the crumbs sit.
- The presets themselves: their trees, sizes, keys, cycle order, and how an arrangement is saved.
- Stylesheet cleanup beyond the rules the removed row and buttons leave dead.
- The landed `spec/pane-arrangements/` records (`verify/rects.mjs` among them) stay as they are.

## Clarifications

- **Q:** Remove the preset bar, or move the buttons into the top bar at the far right (the seed's
  fallback)? — **A:** Remove it entirely, Geoffrey's stated preference; the buttons do not move
  into the top bar (resolved 2026-11-09, foreman ruling under Geoffrey's word).
- **Q:** Does the `nav.fkeys` row stay for the crumbs alone, or go entirely? — **A:** The whole row
  goes, leaving no row of vertical space under the header. `#crumbs` moves into the header between
  the GO form and the chips, and gives way first as the header narrows: it may truncate or hide at
  narrow widths, while the GO form and the chips keep their space (R3, R4, R5) (resolved
  2026-11-09, foreman ruling under Geoffrey's word).
- **Q:** What happens to the keys and to the suite's checks that read `#presets`? — **A:** Every
  preset key, the `p` cycler and the `w` leader keep working and stay listed in `?` and README §
  Keys. The `#presets` checks are replaced by checks grading the arrangement itself, not deleted
  (R6, R7) (resolved 2026-11-09, foreman ruling under Geoffrey's word).
- **Q:** How is the fix graded? — **A:** By the rendered page: nothing between the header's bottom
  and the panes' top, the panes right under the header at every preset and at 1600 and 900 px; the
  crumbs inside the header's box, overlapping neither the GO form nor the chips (R1, R3) (resolved
  2026-11-09, foreman ruling under Geoffrey's word).
- **Q:** How much room do the crumbs get? (GOAL gate) — **A:** Only the space GO leaves. GO keeps its
  full flexible width, and the header never shifts when the selection changes (R4). So the crumbs
  show only where the header has room to spare, about 1350 px and wider, and hide below that. The
  Inspector already shows the selection's path. (resolved 2026-11-09, foreman ruling at the GOAL
  gate under Geoffrey's word in session)

## Related materials

- Seed: `issues/preset-bar.md` (Geoffrey's report, 2026-11-09).
- The cycle that added the row: `spec/pane-arrangements/` (`e5b94ec`; PLAN § window bar and D2,
  research `01-pane-dom-and-views.md` § 7, whose option B is this fix).
- Sources: `okt-client/static/index.html:37-45`; `okt-client/static/app.css:104-105`, `:140-155`,
  `:709-711`; `okt-client/src/panes.rs:125-129`; `okt-client/src/nav.rs:384` (`render_crumbs`);
  `okt-client/src/chrome.rs:496-509` (the help's pane keys); `tests/ui/panes.mjs:20`, `:725`,
  `:779-786`; README § Keys and § Tests; `AGENTS.md` § Build and test.
