# 04 — Per-view fitting (R15, R16, R2)

Read at `edcbed4`. Measured in the throwaway env (port 6586, Chromium, dpr 1) with probes that
drive `--insp-w` on `#panes` to set exact pane widths; numbers below are rendered rects.

## 1. Graph canvas: already pane-driven, one gap

- **Today.** `graph::wire` creates a `ResizeObserver` on `#v-graph` ("size: follow the pane") that
  calls `graph::resize`. `resize` reads `#v-graph.getBoundingClientRect()` into `G.w/h`, `G.host_rect`
  and `G.dpr`, drops the borrow, then `apply_view()`. `draw` sizes the backing store
  (`w·dpr × h·dpr`) when it differs. A window `scroll` listener also calls `resize` (to refresh
  `host_rect`). There is **no window `resize` listener in graph.rs**; window resizes reach it through
  the observer. Measured: changing the split to 70% / 30% gives a backing store of 475×890 /
  1115×890 within three frames, equal to the canvas's client size.
- **Refit?** No explicit zoom-to-fit. `frame`/`screen_scale` is an xMidYMid-meet mapping recomputed
  from `G.w/h` every frame, and `k, x, y` are relative to it, so a resize rescales the picture and
  keeps the user's relative zoom and pan. `apply_view` re-declutters when the scale moves >3%.
  Nothing to change.
- **What the plan must keep:** move the `#v-graph` element into its pane (`appendChild`/`append`),
  never re-create it or re-render its HTML. The observer, `G.canvas`, `G.ctx` and the `.gtools`
  click handlers are bound to those exact nodes. An observer follows its element across reparenting.
- **The gap: position-only moves.** A `ResizeObserver` fires on size, not position. `host_rect`
  (left, top) feeds `user_point` → `hit`, `world_point` (wheel zoom), and the tooltip position in
  `set_hover`. Any change that moves the Graph pane without resizing it leaves `host_rect` stale and
  hover, click-select and wheel-zoom land on the wrong node: classic → swapped (the Graph pane keeps
  ~54% but moves right by the Inspector's width), a swap of equal panes, or a neighbour above it
  changing height in a way that only shifts it. Fix: make `resize` public (e.g. `graph::fit_pane()`)
  and call it once after every arrangement apply (the same place that saves the arrangement), or
  re-read the rect at `pointerdown`/`wheel` time. The former is one call and covers keyboard-driven
  moves; recommended.
- **Invariants 1.** `G` is a leaked `RefCell` reached through `g()`. `draw` holds `g()` for the whole
  draw but calls nothing that runs script (canvas calls and `set_width` dispatch no events). Observer
  callbacks run in the rendering step after rAF callbacks and layout, never nested in a rAF callback
  or in another handler, and `getBoundingClientRect` forces layout without delivering observer
  notifications. So `resize` cannot collide with a held borrow. The one rule for the arrangement
  code: call `graph::fit_pane()` with no borrow of `S`, settings or `G` held (it borrows `G` and
  ends in `schedule`). No resize loop either: the canvas is CSS `width/height: 100%`, so changing its
  backing store does not change `#v-graph`'s size.
- **rAF coalescing.** Already there: an observer delivers once per frame, and `schedule` coalesces
  draws into one rAF (`G.raf`). The draw lands one frame after the resize; in that frame the old
  bitmap is CSS-stretched, which is the behaviour today. No extra coalescing needed. A per-pane
  observer is not needed for the Graph: keep the one on `#v-graph`.

## 2. Hidden or collapsed Graph and Log

- **Graph at 0×0** (measured with `#v-graph` hidden): the observer sets `w = h = 0`; `draw` returns
  early (`if gs.w <= 0.0 || gs.h <= 0.0`), so the backing store keeps its last size (1115×890 stayed).
  No division by zero: `screen_scale` falls back to 800×600, `hit` and `frame_selection` return
  early on `w <= 0`. rAF does not spin: `frame_tick` re-arms only while an animation (420 ms) or a
  flash (3.2 s) is live. On reshow the observer fires with the new size and the next draw resizes the
  store (measured 795×890 at 50%). The plan should hide a non-shown tab or a collapsed pane's body
  with `hidden`/`display:none` (not `visibility` or clipping) so the Graph keeps skipping work.
- **Log chart at 0 width:** `log::render` reads `#lchart.clientWidth`; at 0 it falls back to 800
  (min 200). The SVG uses `preserveAspectRatio="none"`, so a chart rendered while hidden is drawn at
  800 and stretched when shown. Rows don't care.

## 3. Log

Only the chart depends on width: bar width `(w − 36) / days`, axis labels dropped when
`x + 40 ≥ w`, the SVG's `viewBox` = `w × 92`, stretched with `preserveAspectRatio="none"` (text
distorts until a re-render). Rows (`.crow`) are a CSS grid that fits by itself, inside `#ltable`
(its own scroller). No virtualization; `render` writes up to 1,500 rows with `set_html`.
Recommendation: one `ResizeObserver` on `#lchart` (it moves with `#v-log`). When the observed width
changes and is > 0, redraw the chart, trailing-debounced like `BLAME_RO` (`dom::Debounce`, ~100 ms).
Split the chart out of `log::render` into `log::render_chart()` so the observer doesn't rebuild 1,500
rows on every drag frame. The chart sets its own `innerHTML` at a fixed box size (`flex: none;
height: 92px`, full width), so redrawing can't loop. The callback borrows `s()` only to clone
`log_data` and drops it before `set_html` (Invariants 1). This replaces the Log calls in the
splitter's `pointerup` (`keys.rs`, `views::log::render()` under `s().view == "log"`) and the
window-resize debounce. It also covers collapse and expand, presets, and reshowing a hidden tab.

## 4. Files

`tree.rs` has no row windowing. It renders every visible row and keeps `scrollTop` across renders.
Nothing depends on height, so no resize hook is needed. files-column-priority (its PLAN) puts the
container on a wrapper `.tcols` *inside* `#tree` (`container: tcols / inline-size`) and queries it
by name (`@container tcols (…)`). It moves with `#tree`/`#v-files`, so reparenting is safe. A pane
container higher up can't capture those queries because they are named. Today, at a 300 px pane,
`#tree` scrolls inside itself (334 vs 300) while the pane doesn't overflow; after that cycle it
shouldn't overflow at all.

## 5. Table and Results

The inner scrollers `.tree, .ltable, .tgrid, .list { flex: 1; overflow: auto }` carry wide content.
`#tgrid` scrolls horizontally inside itself (`table { min-width: 100% }`, sticky `td.k`); `#list`
likewise. At 300 px, `#tgrid` scrolls to 394 and `#list` to 399, with the pane unaffected. R15's
**real risk is the `.vhead` bars, not the bodies.** `.vhead` is a no-wrap flex row with no overflow
clip. Its min-content widths are Table **386 px** (`TABLE`, `.tfold`, input `min-width: 80px`,
`.vmeta` nowrap, `COLUMNS`, `CSV`), Log 279, Files 222. Measured: a 300 px Table pane has
`scrollWidth` 386 > 300 on `.vpane` and `#view`, which fails R15. Absolutely positioned `.vpane`s do
count in the parent's `scrollWidth`. Fix in CSS: `.vhead { overflow: hidden }` plus a shrinkable
`.vmeta` (`min-width: 0; overflow: hidden; text-overflow: ellipsis`), or a pane container query that
drops `.vmeta` and shortens the input's `min-width` below ~400 px. The pane's minimum width (the
split clamp) decides how far down this must hold. Today's clamp of 22% of 900 px is 198 px.

## 6. Inspector

`#ibody { container-type: inline-size; }` (app.css, after the `.md table` comment). It has no
`@container` rules; it serves the `100cqw` in `.md > table { max-width: calc(100cqw − 36px) }` and
`.md.blame > table` (52px). It travels with `#insp`. `BLAME_RO` already observes the rendered `.md`
and redraws the blame bars, which is pane-safe. Measured with a card selected: `#ihead` never
overflows (`.ipath`/`.ititle` ellipsize; `.ibadges` wraps, `.iacts` included). **`#itabs` does:**
the three buttons are 98 + 90 + 90 px (more with the History count), so the min-content is
**≈290 px**. `#insp` measures 290 vs 260 and 290 vs 200, while 300, 340, 400 and 460 are clean.
"three" at 900×1000 gives the Inspector ≈450 px, which is fine. A side-by-side drag or a narrow
column below ~290 px fails R15. Fix: `.itabs { min-width: 0; overflow: hidden }` and, better,
`@container pane (width < 300px)` hiding the tab words but keeping `O`/`C`/`H` `kbd`. That needs the
words in a span (`insp.rs` writes the History count into the button, around the `#itabs
button[data-tab=history]` update).

## 7. Legend (R16)

`.glegend` is absolutely positioned at bottom-left, `width: fit-content; max-width: calc(100% − 20px)`,
with flex-wrap. Today it's hidden only by `@media (max-width: 900px)`. Max-content is **1214 px**
(9 spans; the "◌ collapsed folder…" note alone is 302). Measured height by Graph-pane width: ≥700 →
43 px (2 rows); 500–600 → 62 (3 rows); 400 → 80 (4); 300 → 113 (6, 13% of the pane). Today's
smallest window that shows it, 901 px in classic at the default 46%, gives a Graph pane of ≈482 px.
Recommend
`@container pane (width < 480px) { .glegend { display: none } }`. The threshold equals today's
boundary in the default layout and hides the legend exactly when it would need a 4th row. Note the
behaviour change R16 asks for: stacked at 900 px now *shows* the legend (pane 900 wide). If the plan
gives the pane `container-type: size` (safe: pane bodies have definite sizes), add
`or (height < 240px)` so a short stacked Graph pane doesn't lose a fifth of its height to it.

**Pane container advice.** Name it (`container: pane / inline-size`) on the pane *body* (the element
that holds the views, below the title bar). The body's width must come from the split, never from
content: inline-size containment gives a content-sized track zero width. Containment also makes the
body a containing block for fixed descendants. None exist in panes (`.toasts`, `.pop`, `.overlay`
and `.offbar` sit outside `#app`'s panes). `#ibody`'s `cqw` and `.tcols`'s named queries keep
resolving to their nearer containers.

## 8. Every resize listener in the client

| Listener | Does | Becomes |
|---|---|---|
| `graph::wire` → `ResizeObserver(#v-graph)` → `resize` | size, dpr, `host_rect`, redraw | keep; add `graph::fit_pane()` after each arrangement apply (position-only moves) |
| `graph::wire` → window `scroll` → `resize` | refresh `host_rect` | keep (harmless; body is `overflow: hidden`) |
| `keys.rs` window `resize` → `RESIZE` Debounce(150) | `log::render` if `s().view == "log"`; `chrome::render_ticker` | keep for the ticker only (window chrome); the Log branch moves to the `#lchart` observer |
| `keys.rs` splitter `pointerup` | `save_settings`; `log::render` if view is log | the splitter is replaced by split drags; Log is covered by its observer |
| `insp.rs` `BLAME_RO` on `.md` + Debounce(80) | redraw blame bars | keep (element-scoped already) |

Also keyed on the one view and due for "is this view shown in any open pane": `nav::set_view` (it
hides the four other `.vpane`s), `live.rs` and `go.rs` `if s().view == "log"`, and the 60 s interval
in `keys.rs` `if s().view == "files" { tree::render() }`.
