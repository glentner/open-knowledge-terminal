# 01 — Today's graph (research brief, read-only)

Scope: `okt-client/src/graph.rs` (2311 lines), the graph rules in `okt-client/static/app.css`,
every caller, the state it reads, the index model it consumes, and the test contracts. Refs are at
the worktree's HEAD. G = `graph.rs`.

## Module structure (G)

| Part | Where | Notes |
|---|---|---|
| Layout constants | G:23-39 | `R_ROOTFILES 58`, `R_TOP 190`, `ROW`, `SPACING`, `dr_dir`/`dr_file` radius steps, `polar(a,r)` (degrees, 0° = up) |
| `P`, `VEdge`, `Layout` | G:44-78 | `Layout{pos, rep, vis, vedges, bbox, r_out}`; `rep` maps every id to its drawn representative (collapsed folder or chain end) |
| Collapse-aware placement | G:80-212 | `chain_end` folds single-child folder chains into one label "a/b/"; `weight` = 0.7/file + subfolder weights, 2.2 when collapsed; `Placer::place` recursion splits angle by weight |
| `layout(idx)` | G:215-415 | root at 0,0; root files on a ring at 200°-520° (G:236-247); top dirs share 360° minus 1.6° gaps (G:253-260); surfaces (`@sid`) on outer ring `r_out` at the circular mean of incoming edges, relaxed 60 iterations (G:267-342); visible edges aggregated per `(rep_s, rep_t, group)` with summed `n` plus synthetic `tree` edges (G:344-397); `bbox` (G:398-413) |
| Label geometry | G:417-443 | radial rotated labels (`rot`, `anchor`) |
| `shape_path` (pub) | G:445-492 | SVG path per family shape; **also used by `views/tree.rs:11`** and the legend |
| `curve` | G:495-516 | straight for tree, quadratic toward the centre for links |
| `VNode`, `Hot`, `Colors`, `Drag`, `Anim` | G:520-630 | per-frame draw data; `Colors::read()` G:580 |
| `G` state | G:632-667 | camera `k, x, y` (pub), `lay: Rc<Layout>`, `nodes`, `at` (id→index), `base` edge paths, `hot`, `sel_vis`, `nbr`, `spotlit`, `spot`, `shown` (label on), `sk`, `hover`, `cursor` (walk mark), `flash`, `w,h,dpr,host_rect`, `raf`, `anim`, `drag`, `built`, `layers` (2 offscreen canvases) |
| Storage | G:669-681 | `thread_local! GS: &'static RefCell<G>` (leaked Box); **`g()` always `borrow_mut()`**, even for reads (`lay()` G:683, `view_k()` G:1010). Not inside `S`. |
| Walk state | G:1829-1837 | separate `thread_local! WALK: RefCell<Option<Walk{from, list, i}>>` |

### Pipeline
- `build(flash)` G:694-878: `layout()` → `VNode`s (size: dirs 8/5.6, clusters 7.5/9.5, files
  `2.9 + min(sqrt(degree)*0.55, 3.4)` G:716; root diamond 8; surfaces hexo 8.5) → base edges as 3
  `Path2d` groups tree/link/surface (G:832-850) → stores into G, re-reads `Colors`, clears `hover`
  and `cursor`, re-resolves `cursor` from `walk_target()` (G:870-874) → `apply_view`, `update`,
  `render_legend` (G:880, writes `#glegend`). **Camera `k,x,y` is not reset by build.**
- `update()` G:1113-1146: from `state::sel()`: `sel_vis`, `nbr` (via `nbrs_of` G:1017: rep'd
  in/out neighbours, parent, expanded kids), `spot` radial gradient, `hot_edges` G:1053 (arrows on
  out-links), collapsed hint "X is collapsed · E to expand" via `set_hint` G:1148 (lazily creates
  `#ghint` in `#v-graph`).
- Labels: `declutter()` G:1218-1330, greedy screen-space segment placement on a 56px grid, priority
  sel/root 0 → surfaces & depth-1 dirs 1 → neighbours 2/3 → other dirs → files; files only when
  `cfg().labels` or zoomed in (`sk >= 1.05`); throttled by `declutter_soon` G:1198 (140 ms / 90 ms).
- Scheduling: `schedule()` G:1332 coalesces one `requestAnimationFrame` (`gs.raf` as dirty flag);
  `frame_tick` G:1343 steps `anim` (cubic ease-out), draws, records time, re-schedules while `anim`
  or `flash` (3200 ms) is live. No continuous loop.
- `record_draw` G:1385: pushes draw ms onto `window.__oktDraw` when a test installed it
  (`tests/perf/profile.mjs:39,46,57`).
- `draw()` G:1424-1612: one `CanvasRenderingContext2d`, holds `g()` for the whole paint; world
  transform `a = dpr*sk`; dotted ring guides at `R_TOP` and `r_out`; spotlight gradient; base
  edges (alpha .35 when spotlit); hot edges + arrowheads; nodes via `Painter::node` G:1661-1817
  (halo/flash ring, walk dashed ring G:1690, shape, cluster ring + count, dirty dot, label). Dimmed
  nodes painted into offscreen layers at 0.16 / 0.4 group opacity (G:1546-1600). Labels constant
  screen size, culled 400px off-screen.

### Styling source
- Colors from CSS custom properties on `:root` via `theme::css_var` = `getComputedStyle`
  (`theme.rs:209`), read once per `build()` (G:580-608): `--accent --accent-text --graph-bg --ink
  --ink-2 --warn --edge --edge-tree --edge-hot --line --mono --f-{core,cards,rules,memory,records,
  workspace,other,world}`.
- `app.css:1-5` declares the `.n/.lab/.ebase/.ehot` rules the "spec graph.rs paints from";
  graph block `app.css:166-219`: `#graph` cursors (`.onnode`, `.panning`), `.gtools`, `.glegend`,
  `.gtip`, `.n…` label sizes/weights, `.halo`, `.flash` keyframes, `.spotlit` opacities .16/.4/.35,
  `.ebase.*` widths/dash, `.ehot.*`, `.ring-guide`. Tokens per theme at `app.css:40-43, 59-64,
  80-88`. `.nchip.kb` (walk mark in inspector) `app.css:336`. Only reduced-motion rule:
  `app.css:513` (ticker, live dot) — the graph has no reduced-motion handling today.
- Markup: `index.html:48-57` — `#v-graph > canvas#graph, .gtools (in/out/fit/labels), #glegend,
  #gtip`.
- Theme repaint: theme switch calls `graph::build(&[])` (`chrome.rs:360-363`, `go.rs:262-268`).
  **Accent change does not** (`actions.rs:174-197` → `theme::apply_theme` only), so the canvas keeps
  the old accent until the next build (pre-existing gap). No `prefers-color-scheme` listener.

### View, input, resize
- Camera is 2D: `k` (clamped 0.4-12), `x,y` pan in viewBox units; `screen_scale` fits `bbox` to
  the pane (G:906), `frame()` G:934, `user_point`/`world_point` G:944-956.
- `animate_to` G:958 (420 ms everywhere), `zoom_by` G:976, `focus_node(id, k)` G:991 (k ≥ 1.7),
  `fit()` G:1006 (k=1, origin), `frame_selection()` G:1942-1984 (2D bbox of rep(sel) +
  `nbrs_of`, pad 60, min 240; no selection → `fit()`).
- Size: `ResizeObserver` on `#v-graph` + window `scroll` → `resize()` G:2128 (host rect, dpr).
  Already pane-fitted, not window-fitted.
- `wire()` G:2141-2311: wheel (active listener, `exp(-dy*0.0016)`, ×7.5 with ctrl = pinch)
  G:2172; pointerdown on canvas, pointermove/up on **window**; 4px drag threshold → pan + pointer
  capture; hover `hit()` G:1987 (reverse-order radius test in world space) → `set_hover` G:2046
  (tooltip `#gtip` via `tip_html` G:2002, hot edges switch to the hovered node); click → `nav::select(…, from:"graph")`, empty click deselects; dblclick folder → `toggle_collapse`, empty → `fit`; `.gtools` buttons G:2296-2307.

## The walk (G:1829-1940)
- `walk(step)` builds `Walk.list = insp::walk_list(idx, sel)` (`insp.rs:486-519`: folder kids in
  tree order, then out-edges, then in-edges by `ORDER` `insp.rs:460`, each id once), steps with
  wrap, sets `gs.cursor` to the target's rep, re-centres via `focus_node` if within 60px of an edge,
  `insp::mark_chip(Some(target))` (`insp.rs:523`, toggles `#ibody .nchip.kb`), hint
  `"{at}/{of} · {label} · {name} · ⏎ go · esc"` (G:1916).
- `walk_target()` G:1840 (only while `Walk.from == sel`), `walk_reset()` G:1921 (called by
  `nav::select` on a moved selection, `nav.rs:27-29`), `walk_stop()` G:1932 (esc, `keys.rs:377`).
- Go: `keys.rs:413-425` Enter/space → `nav::select(Some(t))` → `nav.rs:53-58` `graph::update` +
  `focus_node` (because `from != "graph"`) + `nav(true)` pushes history (`nav.rs:72-74,157-172`).
- `[`/`]` = `history.back/forward` (`keys.rs:493-500`) → `popstate` (`keys.rs:642`) →
  `apply_hash` → `graph::update` + `focus_node` when sel changed (`nav.rs:227-232`). Pop does not
  call `walk_reset`; the stale walk is ignored by the `from == sel` filter.
- Not re-marked: `insp::render()` rebuilds `#ibody` without re-applying `.kb`.

## State and borrows
- Reads: `state::idx()` (`Rc` clone, `state.rs:409`), `state::sel()` (`state.rs:413`),
  `cfg().labels` (G:1220), `is_collapsed` (`state.rs:426`, reads `cfg().collapsed` and
  `collapse_n()` = `cfg().collapse` ‖ `idx.collapse` ‖ 25, `state.rs:418`). Graph never reads
  `s().view`, `s().list`, `s().mark`.
- Writes: `cfg_mut().labels` (`toggle_labels` G:2094), `cfg_mut().collapsed` (`toggle_collapse`
  G:2108), then `save_settings()` (debounced 200 ms, `state.rs:241-244`).
- Persisted graph settings (localStorage `okt:{kb}`, `state.rs:79-213`): `labels: bool`,
  `collapsed: {id: bool}`, `collapse: i64|null` (auto-collapse threshold, settings `#colN`,
  `chrome.rs:437-447`); settings "reset" clears `collapsed` + build (`chrome.rs:384-387`). Camera
  is in neither settings nor the URL hash (`nav.rs:128-154`: v, s, tab, c, rev, t, q).
- Borrow patterns (§ 1): every handler takes `g()` in a short block and drops it before calling
  out — e.g. pointerup clones the id (`g().nodes[i].id.clone()`) then calls `nav::select`, which
  re-enters `graph::update` (G:2252-2262). Long holds: `draw()` (whole paint), `declutter()` and
  `update()` hold `gs` while `nbrs_of`/`is_collapsed` take `cfg()` (fine unless a `cfg_mut` is
  live), `build()` calls `layout()` (many `cfg()` reads) before borrowing G. `walk()` holds
  `WALK.borrow_mut()` across `insp::walk_list` (pure). A nested `g()` anywhere panics, since even
  reads are `borrow_mut`.

## Public API and callers

| Fn | Callers | Expectation |
|---|---|---|
| `wire()` | `lib.rs:36` | once at boot, before the index |
| `build(flash)` | `lib.rs:78` boot; `live.rs:95` live reload (flash = changed+added ids); `chrome.rs:362,386,445`, `go.rs:268` theme/reset/colN; `actions.rs:217` EXPAND/COLLAPSE ALL; `toggle_collapse` | full relayout + repaint, keeps camera |
| `update()` | `nav.rs:53,227`, `keys.rs:104` (`[data-at]` travel) | selection changed |
| `focus_node(id, None)` | `nav.rs:57` (select, not from graph), `nav.rs:231` (pop), `views/tree.rs:414` (space on a file in FILES) | ease to node |
| `schedule()` | `nav.rs:120` (view shown) | repaint |
| `fit()` | `keys.rs:504` (`0`) | |
| `zoom_by(f, None)` | `keys.rs:573,577` (`+`/`=`, `-`) | |
| `frame_selection()` | `keys.rs:521` (`z`; `keys.rs:517-520` first switches to graph view if not there, so the Results view is gone before `z` frames) | |
| `walk(±1)`, `walk_target()`, `walk_stop()`, `walk_reset()` | `keys.rs:406,410,414,377`; `nav.rs:28` | as above |
| `rep_of(id)` / `lay()` | `keys.rs:418,469`, `actions.rs:245` | collapse semantics: which folder E/space/EXPAND act on |
| `toggle_collapse(id, force)` | `keys.rs:173` (inspector `[data-collapse]`, `insp.rs:883`), `actions.rs:258` | toast "Collapsed"/"Expanded" |
| `toggle_labels()` | `chrome.rs:382`, `.gtools [data-z=labels]` | |
| `shape_path` | `views/tree.rs:11`, legend | SVG path string |

Live update (`live.rs:65-95`): new `Idx` into `S`, drops a removed selection, then `build(&flash)`:
relayout, camera kept, hover/cursor cleared then cursor re-resolved from the (possibly stale) walk
list, flash halos on changed/added reps for 3.2 s.

## Index model (`model.rs`)
- `Node` `model.rs:50-67`: `id` (dirs end `/`, root `.`, surfaces `@sid` with `parent: null`,
  family `world`, `okt-server/src/index.rs:875`), `t` ∈ root/dir/file/surface, `name`, `parent`,
  `family`, `kind`, `title`, `type`, `size`, **`lines`**, **`words`**, `aliases`, `c`, `lt`, `link`.
- `Edge` `model.rs:93-99`: `s, t, k, n, label`; wire is a tuple (`model.rs:111`), already
  aggregated per (s,t,k) with count `n`. Kinds: link, wiki, mention, field, embed, symlink, surface
  (`insp.rs:460`); `edge_group` → link|surface (`model.rs:380`). Adjacency `out`/`inn` only for
  edges with both ends present (`model.rs:441-447`).
- Counts: `degree(id)` = out+in edge records, not weighted by `n` (`model.rs:582`); `DInfo`
  per folder: `files, direct, dirs, lt, lines, types` (`model.rs:333`, built `model.rs:476-500`).
- Order: `sorted_kids(id, tree)` (`model.rs:587`); graph mode sorts root dirs by `FAM_ORDER`
  (`model.rs:342`) then collation; files put index.md, readme.md, agents.md, log.md first
  (`model.rs:528-541`). Deterministic (collator ranks).
- Scale: `~/KB` today has 21 top-level folders (regions), 4 root files, ~944 `.md` files.

## Test and API contracts that must survive
- `tests/ui/keys.mjs` GRAPH block (`:92-122`): `#ghint` text matches `^1/N · ` after `j`, `2/N`
  after the next `j`, wraps to `N/N` on `k k`; hint loses ` · ⏎ go` and `#ibody .nchip.kb`
  disappears on esc with selection kept; `.nchip.kb[data-go]` is the walk target and Enter selects
  exactly it; space on a selected folder with no walk toasts `EXPANDED|COLLAPSED`; `z` throws no
  page error. FILES block `:85-90`: space on a file switches to graph with it selected
  (`tree.rs:414`). The walk count `N` is the length of `walk_list` today.
- `tests/perf/profile.mjs`: `window.__oktDraw` hook, `#graph` bounding box, wheel zoom, drag,
  hover sweep, click scenarios (`:64-96`).
- `README.md:79` and `chrome.rs:478,528` (`?` help: click/double-click/drag-wheel/arrows/dashed)
  describe today's semantics.

## Survives / retires / must re-home

| Item | Call |
|---|---|
| Radial `layout()`, `polar`, `R_TOP`, `dr_dir/dr_file`, `weight`, label rotation (`label_geom`), ring guides, root-file ring, surface ring relaxation | retires |
| 2D camera `k,x,y`, `screen_scale`/`frame`/`bbox` mapping, `frame_selection` 2D bbox, pan on drag | retires (re-home as spherical camera; `fit`/`zoom_by`/`focus_node`/`frame_selection` names are called from keys/nav/tree) |
| Public fn names and signatures in the API table | survive (or callers in `keys.rs`, `nav.rs`, `live.rs`, `chrome.rs`, `go.rs`, `actions.rs`, `tree.rs`, `lib.rs` change with them) |
| `shape_path`, family shapes, legend `#glegend`, `#gtip` tooltip, `.gtools` | survive (`shape_path` used by tree view) |
| Walk: `WALK`, `walk_target/reset/stop`, hint format, `mark_chip`, history push via `nav::select` | survive; ordering source (`insp::walk_list`) must re-home to on-screen angle, the list content and `N` stay |
| `rep`/collapse model (`rep_of`, `toggle_collapse`, `is_collapsed`, `collapsed`/`collapse` settings, cluster glyph + count, collapsed hint) | must re-home: space/Enter/E/EXPAND/COLLAPSE and `keys.mjs` depend on it; folding has no role in tiers as written |
| `cfg().labels` "all labels" toggle and `.gtools [data-z=labels]` | must re-home (conflicts with R9's hard cap of 24) |
| Spotlight dim layers (0.16/0.4), `.spotlit` CSS | retires (tiers replace fading) |
| `declutter` greedy non-overlap placement | survives in spirit as the overlay placer (R9) |
| `schedule`/rAF coalescing, `frame_tick` easing, `record_draw` | survive (easing must cut under reduced motion, R11) |
| `Colors::read` from CSS vars, `app.css` graph block | survive; CSS and `graph.rs` change together |
| ResizeObserver on `#v-graph` | survives (already pane-fitted) |
| `from != "graph"` guard on `focus_node` (`nav.rs:56`) | must re-home (R3: camera eases on every selection) |
| `z` view switch before framing (`keys.rs:517-520`) | must re-home (R4 needs the Results view known first) |
| Live reload keeps camera, flashes changed nodes | survives |
