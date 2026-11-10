---
status: adopted:preset-bar
kind: fix
appetite: small
---

# The always-on preset bar takes a row of vertical space

## Problem

Geoffrey, 2026-11-09, after driving 0.4.0 (screenshot of the upper-left of the client):

> Visually, I don't love the new vertical space taken to tell of the ';', ''', '\', '`' keybindings
> to switch through the visual modes with the panes. I spent some time trying to get familiar with
> the new switching keys, the 'p' cycler and the 'w' prefix to expand/collapse/swap panes. All of
> this is great. I just don't like the always displayed helper bar there. If we did keep them, I
> would want them to exist inside the top bar float all the way to the right. Honestly though I
> think we should just get rid of it.

Evidence at `0.4.0` (`74ab810`):
- `okt-client/static/index.html:37-45`: `<nav class="fkeys">` is a full-width row under the header,
  holding `#presets` (four buttons, CLASSIC `;`, SWAPPED `'`, STACKED `\`, THREE `` ` ``) and
  `#crumbs`.
- `okt-client/static/app.css:147` styles `.presets`; `app.css:152` pushes `.crumbs` right in the
  same row.
- `okt-client/src/panes.rs:126` wires the buttons' clicks and marks the button whose preset equals
  the current arrangement (`.on`).
- `tests/ui/panes.mjs:725` and `:779-784` grade that mark and a click on a preset button.

The presets stay reachable without the bar: the keys `;` `'` `\` `` ` ``, the `p` cycler, the
help overlay (`?`) and README § Keys (pane-arrangements R5).

## Why it was deferred

Found by Geoffrey on first use of 0.4.0, after the cycle landed.

## Outcome / vision

No helper row for the presets. Geoffrey's preference is to remove the bar outright. His fallback,
if it stays: the preset buttons sit inside the top bar, floated all the way to the right.

## Sketch of the acceptance criteria

- The preset buttons no longer take a row of their own. Removal is the default; the fallback above
  is the alternative to settle at shaping.
- Every preset key, the `p` cycler and the `w` leader work as before, and `?` and README § Keys
  still list them.
- `#crumbs` keeps a home. Shaping decides whether the `fkeys` row goes entirely, which would move
  the crumbs into the header, or stays for the crumbs alone.
- The suite checks that read `#presets` (`panes.mjs:725`, `:779-784`) are replaced, not left
  failing: grade the arrangement itself, not the bar.

## Notes

- Removing the bar drops the only mouse path to a preset. That's accepted: Geoffrey drives okt by
  keys, and drag-to-dock (`issues/drag-to-dock.md`) is the mouse follow-up.
