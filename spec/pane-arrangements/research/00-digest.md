# 00 — Digest: pane-arrangements research

> **State (2026-11-08):** research done, briefs 01–06. The checkpoint's two questions were ruled the
> same day: GOAL Q16 (R6/R7, the first-load preset) and Q17 (R11/R15, the pane floor), each as
> recommended below. PLAN, TECH and `verify/` follow from this digest. The adversarial review's
> changes are recorded in PLAN § 7, and they supersede this digest where the two differ: the boot
> rule, floors counted per pane, tabs that shrink to fit, and the leader ignoring modifier keys.

Briefs: [01 pane DOM and views](01-pane-dom-and-views.md) · [02 settings and borrows](02-settings-and-borrows.md)
· [03 layout engine](03-layout-engine.md) · [04 view fitting](04-view-fitting.md) · [05 keys](05-keys.md)
· [06 test harness](06-test-harness.md).

## Consolidated decisions

Each resolves the briefs' recommendations into one; where two briefs disagreed, the losing option
is named.

1. **Model: a binary split tree** (03 over 02's n-ary `sizes`). A split is
   `{split: row|col, f, kids: [A, B]}` with `f` the first child's share of the split's extent; a
   leaf is a pane `{views: [..], shown, collapsed}`. Every preset is binary; close = replace the
   parent by the sibling, so "the neighbours take the space" falls out. View names persist as
   strings (unknown ones dropped one by one, never failing the parse) (02). Pane ids are runtime
   only. The tree lives in a new pure module (no `dom`/`state` imports), tested natively by
   `cargo test -p okt-client` (02, 06: native tests run today).
2. **Settings** (02): `Settings.arr` plus `arr_saved`; `to_json` writes `arrangement` only once the
   user has changed it, so a theme change never persists the default as "chosen". Load fills `arr`
   from the saved key, else the width default. `insp` stays read and written but frozen (never
   updated by the new splits), so a downgrade keeps the old width and an old build round-trips the
   new key through `extra`. A sanitiser runs on load (known views only, dedupe, drop empty panes,
   collapse one-child splits, at least one expanded child per split). Views not placed in any pane
   stay in the DOM in a hidden parking element, because `dom::id` panics on a missing id. A
   `pagehide` flush closes the 200 ms debounce gap before a reload.
3. **Layout: nested flex, panes tile exactly; the gutter becomes an overlay sash** (planner's call,
   between 03's 5 px gutter item and the GOAL's "46% of the width"). Today's grid makes the
   Inspector 46% of the *whole* width (736 px at 1600) with a 5 px gutter taken from the left pane.
   03's flex formula `(E − 5)·f` misses that by 2.3 px, over R17's 2 px. Instead the two children of a
   split are exactly `f·E` and `(1 − f)·E` (or `24` and `E − 24` with one collapsed), and the drag
   handle is a 5 px absolutely positioned sash centred on the boundary, drawn as a 1 px rule (the
   second child's leading border). Classic's Inspector is then exactly 46% (or the saved `insp`%) of
   the width, and every R17 size is a one-line formula. Visible change: the 5 px gutter bar becomes a
   1 px rule with a 5 px grab zone.
4. **Collapse**: 24 px title bar in a stacked split; a 24 px strip with `writing-mode: vertical-rl`
   title in a side-by-side split (03). The stored `f` is untouched by collapse, so expand restores
   the exact size (R10). **Accordion rule** (03): collapsing a pane whose sibling is a collapsed leaf
   expands that sibling, so a split never has two collapsed leaves and the space always has a taker;
   collapsing the only pane is refused with a toast (it is in no split, so R10 doesn't reach it).
5. **Neighbour** (03 said "sibling, possibly a subtree"; 02 said "nearest leaf"): the neighbour of
   pane P is a *pane*, namely the leaf of P's adjacent sibling subtree (the next sibling, else the
   previous) that touches the edge it shares with P. Descend: in a split of the parent's orientation
   take the child nearest P, otherwise the first child. That leaf shares an edge with P on screen
   (P spans its parent's full cross axis) and is adjacent in the tree, so it is a neighbour under
   any reading. Swap exchanges the two panes' contents (views, shown tab, collapsed) in place; flip
   toggles P's parent split; close moves P's views to the neighbour, which shows the view P showed,
   is expanded if collapsed, and takes focus; move sends P's shown view there the same way. The root
   pane has no neighbour: close and move refuse and say so (R13, R14).
6. **Moving views between panes**: re-parent the existing view roots (`#v-graph`, `#v-files`,
   `#v-log`, `#v-table`, `#v-list`, `#insp`), never regenerate them (01, 03, 04: listeners, the canvas
   context, `ResizeObserver`s and the tree cache are bound to those nodes). Use `Element.moveBefore`
   through a small `wasm_bindgen` extern when present (keeps scroll and focus, fires no events),
   else `appendChild` with saved and restored `scrollTop`. `appendChild` fires `blur`/`focusout`
   synchronously: treat every move like `focus()` under § Invariants 1. Re-parent only on structural
   changes (preset, close, move, swap); drag, collapse and flip are style and class changes.
7. **Borrow pattern** (02): tree ops are pure functions on `&mut Arrangement` that return a plain
   outcome (panes to remount, pane to focus, toast, whether to save). Run them inside a short
   `cfg_mut()` block, drop it, then do the DOM moves, `save_settings`, renders, `nav`, and `focus`
   last. Bind `s()` reads with `let` before `if let`/`match` (edition 2024 keeps temporaries alive).
8. **Visibility, not `S.view`, gates rendering** (01): `set_view` splits into "place", "render on
   show" and "push history". Render-on-show runs on every path that makes a view visible (tab click,
   expand, preset, swap, move, back/forward). The selection followers in `nav::select`,
   `tree::render`, `insp::open_commit`, `live.rs`, `go.rs` and the 60 s Files interval ask "is this
   view visible" (held, shown tab, pane not collapsed). `S.view` stays "the current one of the five
   views": what the focused pane shows, or, when the Inspector pane has focus, the last of the five
   focused. It keeps feeding the hash `v` and the `t`/`q` keys as today.
9. **Focus** (01, 05): new transient `S.focus` (a pane id), set by a delegated `pointerdown` on the
   arrangement root and by the view keys; marked on the title bar. Key dispatch's `S.view` branches
   (Files, Log, the Graph walk, Escape, `z`) read the focused pane's shown view. The Inspector keys
   (`J K s y , . b n N E O F`) and the graph zoom keys keep acting on their view wherever it is. On
   load the focused pane is the one holding the hash's `v`, else the first pane. `apply_hash` always
   places and focuses (today it skips when `S.view` already matches). The hash `v`
   restores by the placement rule; `z` and Files' space show the Graph by the same rule and focus
   it, so the next `j` walks the graph as today.
10. **Keys** (05; R5's "unmodified" read strictly, no Shift, so no question arises):
    - presets `;` classic, `'` swapped, `\` stacked, backtick three; `p` cycles
      classic → swapped → stacked → three;
    - pane operations under a leader `w`, followed only by keys that are free today: `w w` / `w W`
      focus the next / previous pane, `w d` collapse or expand, `w x` swap, `w r` flip, `w q` close,
      `w v` / `w V` move the shown view to the next / previous neighbour. The pending flag is a `Cell`
      in `keys.rs`; Escape or any other key cancels it.
    - `a` and `i` stay free for the agent chat and the dream inbox.
    - The help overlay and README § Keys gain a Panes block.
11. **Window chrome**: the `.fkeys` view buttons go (each pane's title bar now holds the tabs). The
    28 px bar stays as window chrome holding the crumbs and a preset strip (CLASSIC, SWAPPED,
    STACKED, THREE with their keys, the current one marked), so `H = window height − 110` is
    unchanged and the crumbs' 900 px rule stays window-scoped (Q4).
12. **Containers and fitting** (03, 04): `container: pane / size` on each pane element (its extents
    come from flex, never from content). Named queries only: `#ibody` keeps its unnamed
    inline-size container, and files-column-priority's named `tcols` container inside `#tree` is
    unaffected. Legend: `@container pane (width < 480px) { .glegend { display: none } }`, today's
    boundary in the default layout (04). The Graph keeps its `ResizeObserver`, plus a public
    `graph::fit_pane()` called after every arrangement apply, because a move without a resize
    leaves `host_rect` stale (01, 03, 04). The Log gets a debounced `ResizeObserver` on `#lchart` and a
    chart-only redraw, replacing the splitter and window-resize Log hooks. The window resize
    listener keeps only the ticker.
13. **Overflow is graded on boxes, not clipped away** (03): `overflow: hidden` still reports
    `scrollWidth > clientWidth` on that element, but it also hides the problem from the pane's
    `scrollWidth`. So R15's suite checks the pane's `scrollWidth ≤ clientWidth` and also that every
    rendered descendant outside an inner scroller lies inside the pane rect. Fixes needed today:
    the `.vhead` bars (Table 386 px min-content, Log 279, Files 222) get shrinkable `.vmeta`,
    `.tfold` and input; `#itabs` (≈290 px) drops its words under `@container pane (width < 300px)`,
    keeping the O/C/H kbd; pane tab labels shrink to the kbd chip in narrow panes.
14. **The 900 px block** (03, Q4): `.panes`, `.split` and `.insp` rules deleted; `.glegend` moves to
    the pane query; `.word, .kbname, .crumbs` stay as a window `@media`; the `.trow` rules are
    already gone with files-column-priority (P1 confirms after the rebase).
15. **Harness** (06):
    - The settings key in env.sh is `okt:kb`; suites read it from `#boot`, seed it after loading
      `api/index` and before loading the app, never with `addInitScript`.
    - History checks open a fresh tab.
    - A Graph check needs the canvas visible in its pane, backing size = rect × DPR at DPR 2,
      `elementFromPoint` hitting the canvas, more than 1% drawn pixels, and pixels that change
      after an input. A hidden canvas keeps its old pixels.
    - Measured: env.sh with keys.mjs takes 73 s cold, ≈ 32 s with nothing changed, ≈ 50 s after a
      client edit; `cargo test` 18 s cold, 4 s warm.
    - Spec-id regex for added lines: `(^|[^A-Za-z0-9])[RrPp][0-9]{1,3}([^A-Za-z0-9]|$)` after
      stripping `p50/p90/p95/p99`.

## Questions (the checkpoint: readings that add a precondition to an R-ID)

1. **R6 against R7 on a narrow first load.** With no arrangement saved, a window under 900 px, and an
   `insp` key present, R6 says start in stacked and R7 says classic starts at `insp`. The case is
   the common one: `Settings::to_json` writes `insp` on every save of any setting, so nearly every
   existing user has `"insp": 46` (02). **Recommended reading:** the window width picks the preset
   (R6), and `insp` sizes classic whenever classic is the one that starts (R7). That matches "looks
   as before", because a narrow window showed the stacked layout before this cycle. It conditions R7
   on the width.
2. **R11 against R15: a minimum pane size.** R11 says the panes follow the pointer, without bound.
   R15 says no view's content overflows its pane, at every size. Content can't fit a pane a few
   pixels wide, and today's splitter clamps at 22–75%. **Recommended:**
   - a drag clamps each side of a split to at least 200 px, or to half the split when the split is
     narrower than 400 px; vertically the floor is 120 px;
   - views are made to fit down to 200 px;
   - a pane under 200 px, reachable only by shrinking the window, hides its body's content rather
     than overflow.

   That conditions R11 on the bound. The alternative, no clamp beyond the 24 px title-bar size and
   bodies hidden below 200 px, keeps R11 literal but lets a drag empty a pane's view.

Not questions (design readings that every wording of the GOAL accepts; PLAN states them):
- the strict "unmodified" (decision 10);
- the neighbour definition (decision 5);
- the accordion rule (decision 4);
- `size` containers (decision 12);
- the overlay sash (decision 3);
- `c` then `j`: R9 has `c` focus the Inspector pane, so a following `j` has no Graph walk to drive.
  `keys.mjs` never does that sequence: it reaches the Inspector tab through the hash
  (`&tab=content`), and a hash's `tab` sets the tab without moving focus (only its `v` focuses).
