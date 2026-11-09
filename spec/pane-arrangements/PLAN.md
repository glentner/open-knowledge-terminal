# PLAN — Pane arrangements: place, swap, stack and collapse views

> **Status:** Draft for review · **Last updated:** 2026-11-08
> **Authoritative technical design.** The *how*. Vision/contract is [`GOAL.md`](GOAL.md);
> the phased executable roadmap is [`TECH.md`](TECH.md). Backing detail is in
> [`research/`](research/). Every design element traces to a GOAL R-ID.

## 1. Summary

The fixed grid (`#view` | `#split` | `#insp`) becomes a binary split tree rendered as nested flex
boxes whose leaves are panes with title bars and tabs. The six existing view roots are moved into
the panes, never rebuilt. The tree is a pure Rust module, tested natively, and persisted in the KB's
settings only once the user changes it. Four presets, a leader key for pane operations, a container
per pane, and a frozen rect probe that grades what renders make up the rest. Five vertical slices:
the frame, presets and collapse, operations, fitting, and the full rendered grid.

The code this builds on is `main` after **files-column-priority** and **history-log-rows** land: the
Files columns follow their own named container inside `#tree`, the window-scoped `.trow` rules are
gone, History rows are one line, `tests/ui/keys.mjs` carries a FILES columns section, and
`tests/ui/history.mjs` exists. Anchors below are cited by symbol, because those cycles move lines.

## 2. Design

### 2.1 The model: `okt-client/src/arrange.rs` (new, pure)

No `dom`, `state` or `web_sys` imports, so `cargo test -p okt-client` tests it natively.

- `View`: `graph | files | log | table | list | insp`. Persisted as strings, and unknown strings are
  dropped one by one, never failing the parse, so a later okt's views don't break an older one.
- `Node::Split { orient: Row | Col, f: f64, kids: Box<[Node; 2]> }`, where `f` is the first child's
  share of the split's extent, and `Node::Pane { id, views: Vec<View>, shown: View, collapsed: bool }`.
  Pane ids are runtime only (assigned on load), never saved.
- **JSON** (inside the existing `okt:<kb>` blob): `"arrangement": {"v": 1, "root": NODE}`, where
  NODE is `{"split": "row"|"col", "f": 0.54, "kids": [NODE, NODE]}` or
  `{"views": ["graph", …], "shown": "graph", "collapsed": false}`.
- **Sanitiser**, run on load:
  - keep known views and drop duplicates, the first holder in reading order winning (R1: a view is
    held by at most one pane);
  - drop empty panes, and replace one-child splits by the child;
  - clamp a non-finite `f`, or one outside (0, 1), to 0.5;
  - `shown` not held becomes the first view;
  - never leave two collapsed leaves in one split (the accordion rule below);
  - never leave the root pane collapsed;
  - an empty tree becomes the width default.

  A view no pane holds is legal; R9 adds it on demand.
- **Presets** (R4): `classic(f)`, `swapped`, `stacked`, `three`, with the sizes in § 2.4.
- **Default** (R6, R7 as clarified, Q16): `default_for(width, legacy_insp)` gives `stacked` when
  `width < 900`, else `classic(f)`. Here `f = 1 − clamp(insp, 22, 75)/100` when the settings blob
  holds an `insp` number, else `0.54`.
- **Operations**, each a pure function returning an `Outcome` (remount needed, pane to focus, toast
  text, changed or refused):
  - `place(view, focused, take_focus)` (R9): find the holder, make the view its shown tab, expand
    it, and focus it when `take_focus` is set; if no pane holds the view, add it to the focused
    pane. The view keys set `take_focus`; the Inspector keys never do (Q18);
  - `collapse_toggle(pane)` (R10);
  - `drag(split, f)` (R11);
  - `swap(pane, dir)` and `flip(pane)` (R12);
  - `close(pane)` (R13);
  - `move_view(pane, dir)` (R14);
  - `neighbour(pane, dir)`, defined in § 2.5.
- **Accordion rule** (R10): collapsing a pane whose sibling is a collapsed leaf expands that
  sibling. The space then always has a taker, and no split ends up with nothing to grow. Collapsing
  the only pane is refused with a toast. That pane is in no split, so R10 doesn't reach it.
  Collapse and expand leave focus on the pane itself, so `w d` twice restores what it changed.
- **Floors** (R11, Q17): each side of a split has a minimum extent along the split's axis.
  - An expanded pane's minimum is 200 px across and 120 px down; a collapsed pane's is 24.
  - A subtree's minimum along the axis it splits is the sum of its children's minimums, and the
    larger of them across it.

  `drag` keeps both sides at or above their minimums. Q17's single-pane case is the leaf case, 200 px
  each side. Nested panes are covered too: a Graph beside `row[insp, files]` stops where the inner
  panes reach 200 px each, so only a shrinking window ever makes a pane narrower than its floor.
  When the split is smaller than the two minimums together, `f` sits at their ratio, which is half
  for two panes, as Q17 states. The extent is measured at drag time, and the stored `f` is a
  fraction, so a window resize never changes the tree (R6).

### 2.2 Settings: `okt-client/src/state.rs` (high blast radius)

- `Settings` gains `arr: Node` and `arr_saved: bool`. `Settings::load` reads `"arrangement"` in a
  new match arm: parse, then sanitise. A parse failure drops the key; it is not parked in `extra`,
  or it would be written back beside a different live tree. With no arrangement it calls
  `default_for(innerWidth, insp_if_present)`, choosing once per load and never again on a window
  resize (R6).
- `to_json` writes `"arrangement"` **only when `arr_saved`**. Every user change sets it before
  `save_settings()`. A theme change or a pin therefore never saves the default as a choice (R6:
  chosen means saved).
- `insp` stays: read, frozen, still written. The new splits never update it. An older okt then
  shows the pre-upgrade width, and its `extra` map round-trips the new key.
- A pure `Settings::from_map` is split out of `load` so the migration is tested natively. Detecting
  `insp` as present means the key exists in the blob, not that the value differs from 46. Q16 says
  that is the intent: a saved 46 sizes classic at 46.
- `save_settings` gains a synchronous flush on `pagehide`, so a reload within the 200 ms debounce
  keeps the change (R7).
- **Boot and the hash** (R7, R9).
  - A saved arrangement is restored untouched. The hash's `v` only chooses which pane gets focus:
    the pane already showing it, or else the default focus. Boot never changes a saved tab,
    collapse or size, so a reload shows exactly what was saved.
  - With nothing saved, boot places the hash's `v` in the width default in memory, without setting
    `arr_saved`. A bookmark such as `#v=log` still opens the Log, and the default is not saved as a
    choice.
  - Back and forward (`popstate`) place by R9's rule, and that change is saved like any other.
  - Every arrangement change replaces the hash (`nav(false)`), so `v` always names the view `S.view`
    holds.
- `S` gains `focus: PaneId` (transient). `S.view` keeps meaning "the current one of the five main
  views": what the focused pane shows, or, when the Inspector pane has focus, the last of the five
  that had it. It keeps feeding the hash `v` and the `t`/`q` hash keys exactly as today.
- **Borrow rule** (§ Invariants 1): each operation runs inside a block
  `{ let mut c = cfg_mut(); let out = arrange::op(&mut c.arr, …); c.arr_saved |= out.changed; out }`.
  The borrow is dropped, then come the DOM moves, `save_settings()`, the renders, `nav`, and
  `dom::focus` last. Reads of `s()` are bound with `let` before an `if let` or `match` (edition 2024
  keeps temporaries alive through the body).

### 2.3 Rendering: `okt-client/src/panes.rs` (new) and `okt-client/static/index.html`

`#panes` stays the arrangement root. `index.html` keeps the six view roots
(`#v-graph`, `#v-files`, `#v-log`, `#v-table`, `#v-list`, `#insp`) inside a hidden `#parking`
element, and `#view` and `#split` are deleted. `panes::render()` builds the skeleton:

```
#panes
 └ .psplit[data-orient=row|col] style="--f:0.54"   (position: relative; display: flex)
    ├ section.pane[data-pane][data-views][data-shown][data-collapsed][data-focused]
    │   ├ header.ptitle   one button[data-view] per held view: <kbd>G</kbd>GRAPH …
    │   └ div.pbody       the held view roots; non-shown ones keep the `hidden` attribute
    ├ div.sash            (absolute, 5 px, centred on the boundary)
    └ …second child…
```

The `data-*` attributes help tests *find* things. Every grade comes from rects (§ 6).

- **Moving view roots.** `panes::render()` builds the new skeleton beside the old one, moves each
  view root into its pane body, then removes the old skeleton.
  - It uses `Element.moveBefore` through a small `#[wasm_bindgen]` extern when the browser has it:
    the move keeps scroll and focus and fires no events.
  - Otherwise it falls back to `appendChild`, saving and restoring `scrollTop`. That fallback fires
    `blur` and `focusout` synchronously, so every move counts as a re-entrant call: no borrow is
    held across it.
  - Views no pane holds go back to `#parking`, because `dom::id` panics on a missing id.
  - Re-parenting happens only on structural changes (preset, close, move, swap). A drag, collapse,
    expand or flip changes only `--f`, classes and attributes.
- **Overlay sash.** The two children of a split tile its box exactly, as `f·E` and `(1 − f)·E`
  (`flex: <f·1000> 1 0px` and `flex: <(1−f)·1000> 1 0px`), or `24` and `E − 24` when one child is a
  collapsed leaf (`flex: 0 0 24px`).
  - Each child carries its own inline `flex`, written from its parent's `f`. No CSS variable is
    inherited, so a nested split can't read its parent's share by mistake.
  - Tests set a split's share only by dragging its sash, never by writing styles.
  - The drag handle is a 5 px absolutely positioned `.sash` centred on the boundary, above both
    panes. Its visible part is the second child's 1 px leading border.
  - Drag: one delegated `pointerdown` on `#panes` for `.sash`, with `setPointerCapture`. Pointer
    moves set `--f` through `arrange::drag`, which applies the floors; `pointerup` and
    `lostpointercapture` save.
  - The sash is inert, and hidden, beside a collapsed leaf.
  - This replaces `keys::wire`'s `#split` handlers, which leaked two closures per drag (03 § 3).
- **Title bars** (R1, R8): 24 px, border-box, holding the tabs (`.ptab`, `.on` when shown).
  - Tabs shrink to fit: each is `flex: 0 1 auto; min-width: 0`, its label ellipsised, its kbd fixed.
    The title bar never overflows, whatever the tab count or pane width.
  - Kbd hints: `G F L T /` for the five views, and `O` on the Inspector tab, labelled INSPECTOR.
  - A tab click goes through the existing `[data-view]` click delegate in `keys::on_click`, which
    routes `insp` to the Inspector placement, never to `set_view`'s `_ => graph` fallback.
  - The focused pane's title bar is drawn differently: an accent top edge and a lighter
    background.
  - A delegated `pointerdown` on `#panes` focuses the pane under the pointer.
- **Collapsed panes** (R10): `.pane.collapsed > .pbody { display: none }`.
  - In a column split the pane is its 24 px title bar.
  - In a row split it is a 24 px strip whose title bar fills it with `writing-mode: vertical-rl`,
    reading "GRAPH · FILES · …" top to bottom.
  - Because the look follows the parent's orientation, a flip turns a strip into a title bar.
- **The window bar** (`nav.fkeys`, 28 px, kept, so the panes' height stays `innerHeight − 110`).
  - Its five view buttons (`.fk[data-view]`) are removed; their tabs now live in the pane title
    bars (a visible change).
  - It keeps the crumbs.
  - It gains a preset strip: buttons `CLASSIC ; · SWAPPED ' · STACKED \ · THREE `` with the key
    hints, the current preset marked when the tree equals it. A button click applies the preset.
- **The 900 px block** (R6, R16, GOAL Q4):
  - `.panes`, `.split` and `.insp` are deleted;
  - `.glegend` moves to the pane container (§ 2.6);
  - `.word, .kbname, .crumbs` stay as a window `@media (max-width: 900px)` rule, unchanged.

  After the rebase no `.trow` rule may remain under any `@media`; the predecessor cycle removes
  them, and P1 confirms it.

### 2.4 Presets and their sizes (R4; the sizes R17 grades)

Let the arrangement root `#panes` have width `W` and height `H`; at 1000 px tall `H = 890`
(`innerHeight − 110`). Every rect is relative to `#panes`; a collapsed pane is 24 px (`C`).

| preset | tree | panes (x, y, w, h) |
|---|---|---|
| classic | `row f=c [views*, insp]`, `c = 0.54`, or `1 − insp/100` from a pre-cycle `insp` | views (0, 0, cW, H); Inspector (cW, 0, (1−c)W, H) |
| swapped | `row f=0.46 [insp, views*]` | Inspector (0, 0, 0.46W, H); views (0.46W, 0, 0.54W, H) |
| stacked | `col f=0.5 [views*, insp]` | views (0, 0, W, H/2); Inspector (0, H/2, W, H/2) |
| three | `col f=0.4 [graph (collapsed), row f=0.5 [insp, views⁻]]` | Graph (0, 0, W, 24); Inspector (0, 24, W/2, H−24); views⁻ (W/2, 24, W/2, H−24) |

`views*` is Graph, Files, Log, Table, Results with Graph shown; `views⁻` is Files, Log, Table,
Results with Files shown.

| preset | 1600×1000 | 900×1000 |
|---|---|---|
| classic | 864 / 736 (× 890) | 486 / 414 |
| swapped | 736 / 864 | 414 / 486 |
| stacked | 1600 × 445, 445 | 900 × 445, 445 |
| three | Graph 1600 × 24; 800 / 800 (× 866) | Graph 900 × 24; 450 / 450 (× 866) |

- Expanding three's Graph gives it `0.4·H` (356 px at 1000 tall).
- Classic's Inspector is exactly 46% of the width, as today (736 px at 1600). The left pane is 5 px
  wider than today's 859, because the gutter became an overlay.
- Applying classic by key later uses the same `c` (R4: "classic takes a splitter width saved before
  this cycle").
- The cycle key steps classic → swapped → stacked → three → classic, starting after the preset the
  current tree equals, or from classic when it equals none.
- Focus after a preset goes to the pane holding `S.view` if that pane is expanded, else to the first
  expanded pane holding one of the five views. In three, with the Graph collapsed, that is Files.
- At both sizes every pane clears the 200 px floor, so no preset trips R15's narrow-pane rule.

### 2.5 Neighbour, swap, flip, close, move (R12–R14)

- **Neighbour** of pane P in direction `next` (`prev`): a *pane*. Take P's parent split. The
  sibling subtree after P (before P) is used if one exists, else the one on the other side. Descend
  that subtree: in a split with the parent's orientation take the child nearest P, otherwise the
  first child.
  - The leaf reached touches the edge it shares with P. Since P spans its parent's full cross axis,
    the two share an edge on screen, and they are adjacent in the tree.
  - The root pane has no neighbour.
- **Swap** (`w x`): P and its neighbour exchange their contents (views, shown tab, collapsed) in
  place. "The two exchange places, each keeping its views." The slot sizes stay. Focus follows P's
  contents.
- **Flip** (`w r`): toggle the orientation of P's parent split, keeping the order and `f`.
- **Close** (`w q`): P's views join the neighbour.
  - The neighbour shows the view P showed, is expanded if it was collapsed, and takes focus.
  - P's parent split is replaced by the sibling subtree, so the neighbours take the space.
  - Afterwards the sanitiser's accordion step runs: a split left with two collapsed leaves expands
    the receiver.
  - The root pane refuses with the toast "only pane", and the arrangement is unchanged.
- **Move** (`w v` next, `w V` previous): P's shown view goes to the neighbour, as its shown tab,
  expanded and focused.
  - P shows its next held view.
  - If P is left empty, it closes exactly as Close does.
  - The root pane refuses with the toast "no other pane", and nothing changes.

### 2.6 Fitting each pane (R2, R15, R16)

- `section.pane { container: pane / size }`. A pane's extents come from flex and never from its
  content, so size containment is safe (04 § 7).
- Queries are always named. `#ibody` keeps its unnamed inline-size container for `.md` tables, and
  the Files columns keep their named container inside `#tree` from files-column-priority.
- **Narrow panes** (R15, Q17): `@container pane (width < 200px) { .pbody { display: none } }`.
  Only a shrinking window can produce one (§ 2.1 floors), and the title bar stays.
- **Overflow fixes** (R15), down to 200 px. The rows of Files, the Log and Results fit a 200 px
  pane without a sideways scroll (truncated cells, fewer columns under a pane query). The Table's
  grid (`#tgrid`) and the Inspector's body (`#ibody`: rendered Markdown, wide tables, and History
  rows as history-log-rows shipped them) keep their own sideways scroll, which R15's pane measure
  allows. Measured min-content today: Table bar 386 px, Log 279,
  Files 222, Inspector tabs about 290 (03 § 6, 04 § 5–6).
  - `.vhead`: shrinkable `.vmeta` and `.tfold` (`min-width: 0; overflow: hidden;
    text-overflow: ellipsis`), the input at `min-width: 0`, and `.mini` at `flex: none`.
  - Under `@container pane (width < 360px)` the bar drops `.vmeta` and `.tfold`.
  - `#itabs` and pane tabs show only their kbd under `@container pane (width < 300px)`. That needs the
    tab words in a span; `insp.rs` writes the History count into its button.
  - `.gtip` is capped at 300 px and clamped into the pane.
- **Legend** (R16): `@container pane (width < 480px) { .glegend { display: none } }`. 480 px is
  today's boundary in the default layout (a 901 px window, 46%), and below it the legend would
  need a fourth row (04 § 7). A visible change: a Graph pane 480 px or wider in a narrow window now
  shows the legend.
- **Graph** (R15, R2): the existing `ResizeObserver` on `#v-graph` keeps the backing store sized.
  `graph::resize` becomes `pub fn fit_pane()`, called after every arrangement apply with no borrow
  held. A pane moved without being resized (classic → swapped) would otherwise leave
  `host_rect` stale, and clicks, hover and wheel would land on the wrong node (01, 03, 04).
- **Log** (R15): a debounced `ResizeObserver` on `#lchart`, with a chart-only `log::render_chart()`
  (the rows fit by CSS). It replaces the splitter's `pointerup` hook and the window-resize Log
  branch; the window `resize` listener keeps only the ticker.
- **Visibility gates rendering** (R2, R3). `nav::set_view` splits three ways: placement
  (`arrange::place`), render-on-show, and history.
  - Render-on-show runs on every path that makes a view visible: tab click, expand, preset, swap,
    move, back/forward.
  - A `ResizeObserver` on each `.pbody` re-runs render-on-show when a pane crosses 200 px, so a
    pane that grows back from a small window shows the current selection.
  - These checks change from `s().view == "x"` to `panes::visible("x")` (held, the shown tab, and
    the pane neither collapsed nor under 200 px wide): `nav::select` (Graph camera, Files row,
    Table marks), `tree::render`, `insp::open_commit`, `live.rs`, `go.rs`, and `keys::wire`'s 60 s
    Files interval.

### 2.7 Focus and keys (R5, R8, R9): `okt-client/src/keys.rs`, `nav.rs`, `chrome.rs`, `README.md`

- **Dispatch** reads `let view = panes::focused_view();` once.
  - The Files, Log and Graph-walk branches, Escape's walk stop, `u` and `z` act on that view. The
    Inspector matches none of them, so `j`/`k` do nothing there, as in Table and Results today.
  - The Inspector keys (`J K s y , . b n N E O F`) and the Graph camera keys (`0 + = -`) act on
    their view wherever it is (05 § 4).
- **View keys** (R9). `g f l t` and every other `set_view` caller (the GO commands, Files' space,
  `table::open`, `list::show` for searches, `apply_hash`) go through `place`, which shows the view,
  expands its pane and focuses it.
  - `o c h` set the Inspector's tab and show it, expanding its pane if collapsed, or adding it to
    the focused pane when no pane holds it. **They never move focus** (R9 as clarified, Q18). In
    classic with the Graph focused, `c` then `j` still steps the Graph walk, as today, and the
    Graph's pane keeps focus.
  - `set_tab`'s other callers (Enter → content, outline clicks) reveal the Inspector the same way,
    also without moving focus, so Enter followed by `j` keeps walking Files as today.
  - `apply_hash` always places and focuses the hash's `v`. Today it skips when `S.view` already
    matches.
  - `/` and `:` focus the GO bar as today.
- **Focus on load**: the pane holding the hash's `v`, else the first expanded pane holding one of
  the five views.
- **New keys**, all unmodified (no Shift, Ctrl, Alt or Cmd) and proven free (05 § 2):
  - presets: `;` classic, `'` swapped, `\` stacked, backtick three; `p` cycles them (R5);
  - a leader `w`, then a key that is free today: `w w`/`w W` focus the next/previous pane,
    `w d` collapse or expand, `w x` swap, `w r` flip, `w q` close, `w v`/`w V` move the shown view
    to the next/previous neighbour. The pending flag is a `Cell` in `keys.rs`, outside `S`.
    - Modifier-only keydowns (Shift, Control, Alt, Meta, CapsLock) leave the leader pending, so
      `w W` and `w V` work on a real keyboard.
    - Escape cancels the leader.
    - Any other key cancels the leader and is then handled as usual, so `w` then `f` still shows
      Files.
    - Blur, or two seconds without a key, also clears it;
  - `a` and `i` stay free for the agent chat and the dream inbox.
- **Help and README** (R5).
  - `chrome::show_help` gives each preset its own row: `;` classic, `'` swapped, `\` stacked,
    backtick three, and `p` "cycle the presets". It also gains a PANES block for `w`.
  - README § Keys gains a PANES bullet naming the four presets, `p`, the `w` operations and the
    focus rule.

### 2.8 Ports of the predecessors' suites (after the rebase)

- **The rule for every port:** selectors and the means of setting a width may change. Each
  existing check keeps its label text and its expected outcome. `verify/kept.mjs` enforces this:
  every check label as the suites stand at the merge base with `main` must still be in the file,
  and must print as passed in the gate's run.
- **keys.mjs's own reads.** `state().view` (the first `#v-*` without `hidden`) becomes the shown
  view in the focused pane. The expected outcomes are unchanged (R9: "selectors may change where
  the DOM moved").
- **keys.mjs's FILES columns section** (from files-column-priority):
  - its pane-width sweep through `--insp-w` becomes one held drag of the classic sash, moved 2 px
    a step, with the same checks at each step;
  - its splitter drags at 1200, 760 and 480 px use the `.sash`;
  - its "stacked windows" come from a fresh context loaded below 900 px, the stacked default, then
    resized.

  Every expected outcome is kept.
- **tests/ui/history.mjs** (from history-log-rows):
  - its drags of `#split` to 22% and to the default use the `.sash` to the same widths;
  - its stacked window comes from a fresh narrow context.

  Every expected outcome is kept. The new floor (200 px) is narrower than 22% of some windows. In
  that range History must still not overflow its *pane*; its rows sit in `#ibody`, an inner
  scroller, so R15's pane measure holds.

### Requirement → design map

| R-ID | Design element(s) that satisfy it |
|------|-----------------------------------|
| R1 | `arrange::Node`; `panes::render` (title bars, tabs, `hidden` on non-shown roots); tab click delegate; `place`/`move_view` put any view in any pane; sanitiser dedupes (§ 2.1, 2.3, 2.5) |
| R2 | The roots live in separate panes and render independently; visibility-gated rendering; `fit_pane`; the Graph's own ResizeObserver (§ 2.6) |
| R3 | Selection followers ask `panes::visible`, never `S.view`; the Inspector renders wherever it is (§ 2.6) |
| R4 | `arrange` presets with the sizes of § 2.4 |
| R5 | `;` `'` `\` backtick, `p`; help and README (§ 2.7) |
| R6 | `default_for` once per load; the stored `f` fractions; no window media rule for panes (§ 2.1–2.3) |
| R7 | `arr_saved` + `to_json`; `pagehide` flush; legacy `insp` sizes classic when classic starts (Q16) (§ 2.2) |
| R8 | `S.focus`; pointerdown focus; the title-bar mark; `focused_view()` dispatch (§ 2.3, 2.7) |
| R9 | `place` behind every `set_view` caller, `apply_hash`, `list::show`; `o c h` show the Inspector without moving focus (Q18) (§ 2.7) |
| R10 | `collapse_toggle`, `w d`, the 24 px bar or strip, the accordion rule, `f` kept (§ 2.1, 2.3) |
| R11 | `.sash` drag with the floors, counted per pane through nested splits (D5); save on pointerup (§ 2.1, 2.3) |
| R12 | `swap` (contents in place) and `flip` (§ 2.5) |
| R13 | `close` to the neighbour; the root refuses (§ 2.5) |
| R14 | `move_view`; an emptied source closes; the root refuses (§ 2.5) |
| R15 | `container: pane / size`; overflow fixes to 200 px; the narrow body hidden below 200 px; Graph and Log observers (§ 2.6) |
| R16 | The legend's pane query at 480 px; the chrome `@media` kept; no `.trow` under `@media` (§ 2.3, 2.6) |
| R17 | `tests/ui/panes.mjs`, graded by rects, plus the frozen `verify/rects.mjs` (§ 6) |

## 3. Invariant gate (AGENTS.md constitution check)

Walked before research (from the GOAL) and again against this design.

- **High-blast-radius paths:** `okt-client/src/state.rs` is touched (`Settings.arr`, `arr_saved`,
  `from_map`, `pagehide` flush, `S.focus`). The `insp` key keeps its meaning and is still written.
  Unknown keys keep round-tripping through `extra`. A parse failure of the new key drops only that
  key; it never resets other settings. `http.rs`, `launch.rs` and `index.rs` are untouched: no
  server change (a GOAL non-goal).
- **§ 1, client re-entrancy:** the hazards are view moves (`appendChild` fires `blur`/`focusout`
  synchronously), `dom::focus`, ResizeObserver callbacks, and renders that borrow `S`, settings or
  `G`.
  - Every operation runs pure under one short `cfg_mut()` borrow, which is dropped before any
    move, save, render, `nav` or focus; focus comes last (§ 2.2).
  - `fit_pane` is called with nothing held.
  - The Log observer clones `log_data` out before `set_html`.
  - The leader's pending flag is a `Cell` outside `S`.
- **§ 2, Pygments data:** not touched.
- **§ Conventions:**
  - commit subjects follow the profile;
  - no spec ids in source or test names, enforced on added lines (§ 6);
  - `app.css` and `graph.rs` change together where graph paint rules change. This cycle changes no
    graph paint rule: the legend query is layout, and `fit_pane` is a visibility change, not paint.
    So no commit is forced to pair them, and no gate enforces pairing (the graph-galaxy lesson);
  - the same-commit rules: README § Keys with the keys, AGENTS.md's Map with the new modules,
    AGENTS.md § Build and test and README with the new suite.

### Deviation justifications

| Deviation | Why needed | Simpler alternative rejected because |
|-----------|-----------|--------------------------------------|
| D1. The 5 px gutter becomes a 1 px rule with a 5 px overlay grab zone (visible change, accepted at the sign-off). | R4 and R7 put classic's Inspector at exactly 46% (or `insp`%) of the width, and R17 grades within 2 px. A 5 px gutter item makes the Inspector `0.46·(W − 5)`, 2.3 px short at 1600. | A gutter item with the fraction converted at load (`f = p·W/(W−5)`) depends on the window width, so it would drift as the window resizes. |
| D2. The `.fkeys` view buttons move into pane title bars, and the bar gains a preset strip (visible change, accepted at the sign-off). | R1 puts the tabs on each pane, and two strips of the same buttons would contradict each other once views sit in different panes. | Keeping the global strip as a second set of tabs: it would show one "current" view while two are on screen. |
| D3. A leader key (`w`) for pane operations, a new pending state in `keys.rs`. | Seven operations need keys, and only nine unshifted letters are free, two of them wanted by the coming views. | Single keys, which would use up the free letters. |
| D4. `Element.moveBefore` through a hand-written extern, with an `appendChild` fallback. | It keeps scroll and focus and fires no `blur` mid-move (§ 1). | `appendChild` alone, which loses `scrollTop` and fires synchronous `blur` on every structural change. |
| D5. R11's floor is counted per pane: a side that is itself a split stops where its own panes reach their floors (200 px each across, 120 px down). | Q17 says only a shrinking window can make a pane under 200 px. Applying a flat 200 px to a side holding two panes lets a drag squeeze each to 100 px (the review's three-then-flip case). | The flat 200 px per side, which contradicts Q17's stated rationale and R15's narrow-pane rule. |

Each row read against the R-IDs it touches:
- **D1** sharpens R4 and R7's sizes. Arithmetic: classic at 1600 gives 0.46 × 1600 = 736.0, and the
  left pane 864.0. R17's 2 px holds exactly, and R11's drag floors are unaffected.
- **D2:** R1 asks only that each pane show tabs. R9's keys are unchanged, and `.fk` clicks were not
  keys.
- **D3:** R5 constrains only the preset and cycle keys, and those are single unmodified keys.
  R10–R14 say "by key on the focused pane". The leader's second keys are all free today, so no
  existing key changes meaning (R9).
- **D4** touches no R-ID's wording.
- **D5:** R11 reads "each side at least 200 px wide … where the drag stops". For a side that is a
  single pane, the design stops at exactly 200 px (120 px down, half below 400 px), as R11 and Q17
  state. For a side that is a split, it stops where that split's panes reach 200 px each. The side
  is then at least 200 px wide, as R11 requires, and no pane falls under 200 px, as Q17 requires.
  Arithmetic for the review's case: three, `g`, then `w r` at 1600 gives `row[graph, row[insp,
  files]]`. The inner split's minimum is 200 + 200 = 400, so the Graph can grow to at most
  1600 − 400 = 1200 px, and each inner pane stays at 200 px or more. The report names this reading
  for the sign-off.

## 4. Rabbit holes (resolved)

- **Move views or rebuild them?** Move: listeners, the canvas context, observers and the tree cache
  are bound to the nodes. `moveBefore` keeps scroll and focus. ([01](research/01-pane-dom-and-views.md),
  [03](research/03-layout-engine.md))
- **Exact sizes.** Flex with weights `f·1000` lays out within 0.02 px of the formula; Chrome uses
  1/64 px units. The overlay sash makes the formula exact. ([03](research/03-layout-engine.md))
- **"Chosen means saved" vs incidental saves.** `arr_saved` gates writing the key; and `insp` is
  always present, which raised Q16 (answered). ([02](research/02-settings-and-borrows.md))
- **The Graph after a move.** The ResizeObserver misses position-only moves, so `fit_pane` runs
  after every apply. A hidden Graph at 0×0 is safe (`draw` returns early).
  ([04](research/04-view-fitting.md))
- **Free keys.** Proven by walking every arm of `keys::on_key`; there is no prefix state today.
  ([05](research/05-keys.md))
- **What overflows.** `overflow: hidden` doesn't fix `scrollWidth`; content must shrink. The bars
  and the Inspector tabs are the culprits, and inner scrollers don't count.
  ([03](research/03-layout-engine.md), [04](research/04-view-fitting.md))
- **Grading a hidden Graph.** It keeps its pixels, so the R2 check needs visibility,
  `elementFromPoint`, backing = rect × DPR, and pixels that change. ([06](research/06-harness.md))
- **R6 and R7 on a narrow first load, and R11 against R15.** These were questions at the
  checkpoint, ruled 2026-11-08 (GOAL Q16, Q17).
- **Adversarial review:** see § 7.

## 5. Risks & open questions

- **The frozen probe reads the KB's content.** It uses the most-connected Markdown file, the
  deepest file, and a FIND for "inbox". The throwaway clone is Geoffrey's KB, so the facts hold
  today. A KB with no `inbox` match would turn the Results check red.

- **The predecessors' suites must be ported** (§ 2.8). The FILES columns section's 2 px sweep and
  history.mjs's drags are the riskiest. P1 owns the ports and keeps every expected outcome. If the
  landed suites differ from their PLANs, P1 follows the landed code.
- **The rebase** (P1's first step) can conflict in `app.css` (the 900 px block, `.trow`), `keys.rs`
  and `insp.rs`. Resolve nothing by guesswork: stop and report.
- **moveBefore** exists in Chromium 158, which the suites use. The `appendChild` fallback path is
  exercised only by older browsers. A native test can't reach it, so its correctness rests on review.
- **The 200 ms save debounce:** `pagehide` flushes it. The suites still wait for the stored value
  to change before reloading.
- **Window shrink below the floors:** panes keep their fractions and hide their body below 200 px
  (R15). A very small window shows title bars only; that is the ruling.
- **profile.mjs and soak.mjs** assume today's single view pane at 1920×1080. They still run under
  classic, the default at that width; P5 runs soak once.

## 6. Verification strategy

Each phase's `verify:` is `sh spec/pane-arrangements/verify/gate.sh P<n>` (cumulative), with a
phase-only mode for build's fail-once mutation and a static mode for pre-push. The gate runs:

1. **`static.sh`**:
   - committed state: nothing unstaged or untracked anywhere;
   - the gate frozen: `verify/` exactly as the plan commit left it;
   - the spec-id scan on lines and file names added since the plan commit, regex
     `(^|[^A-Za-z0-9])[RrPp][0-9]{1,3}([^A-Za-z0-9]|$)` after stripping p50/p90/p95/p99;
   - every existing check label of `keys.mjs` and `history.mjs` still present (`kept.mjs`);
   - from P2, README § Keys naming the presets and `p`;
   - from P4, no `.trow` selector under any `@media`, at any depth (R16's literal "no
     window-scoped rule");
   - in P5, the deferral ledger.
2. **`cargo test`**, the profile's crates, plus the phase's required native tests by exact name
   (`required.txt`), all passing in one run.
3. **One `tests/env.sh` run on `$OKT_TEST_PORT`** (6584 for this cycle), under a private `TMPDIR`. It
   runs `keys.mjs`, `history.mjs`, the cycle's `tests/ui/panes.mjs`, and the frozen probe
   `verify/rects.mjs` with the phase's groups, each to its own exit code. Then:
   - no process with the private `TMPDIR` in its argv is left (another session's are never
     touched);
   - `kept.mjs` finds every existing check printed as passed;
   - `panes.mjs` printed every label `checks.txt` names for the phase, as `ok   <label>`.

**The frozen probe** (`verify/rects.mjs`, planner-written, attacked by the review) grades the
rendered page alone.
- Panes are found through `.pane`, `.ptitle`, `.pbody` and the view-root ids.
- Every `data-*` value is cross-checked against geometry. Views held are the visible tab buttons
  inside the title bar that `elementFromPoint` reaches. The shown view is the one visible root
  inside the body, reached at its centre and holding content.
- Only the focused title bar is drawn differently from the rest.
- Collapsed panes are measured against their own title bars.
- Overflow: nothing in a pane lies outside it, and no scroller other than `#tgrid` and `#ibody`
  scrolls sideways.
- A reload is graded from a stored arrangement, and again on a new page at the bare URL.
- Its groups:
  - `first-load`: 899 vs 900 px, an empty store, a bookmark's `v`, a settings save that must not
    save the arrangement;
  - `migration`: a seeded `insp`, at 1600, 900 and 899 (Q16);
  - `keys`: the view keys' placement and focus, the Inspector keys keeping focus (`c` then `j`
    walks the Graph), a tab click, the selection followed, a view no pane holds;
  - `drag`: following the pointer, the floors, a reload within 1 px;
  - `presets`: each preset at 1600 and 900, the rects of § 2.4 within 2 px, a reload, the cycle, a
    window resize, `insp` by key, the hover tip after a position-only move, the help rows;
  - `collapse`: strip, bar, restore, accordion, `g` expanding three's Graph;
  - `three`: the R17 extras;
  - `ops`: swap, flip, move, nested floors, close and the refusals, the last view, a reload;
  - `together`: the Graph and Files both live, keys following focus, the selection in every view,
    the canvas refitting after each kind of size change (R2, R3, R8);
  - `fit`: every view at a 200 px pane, the Log chart, a pane under 200 px, the legend.

The suite is sized from the research run: env.sh plus keys.mjs takes about 50 s after a client
edit. TECH § Gate timings records the dry-run figures.

## 7. Adversarial review

A fresh reviewer received the GOAL, this PLAN, TECH and the drafted `verify/` (2026-11-08). It
returned 27 findings: 8 HIGH, 11 MEDIUM, 8 LOW. Each is folded in as follows.

- **HIGH 1, `export GROUPS` kills the gate.** `GROUPS` is read-only in macOS's bash-as-`sh`. The
  probe's groups now go in as positional arguments.
- **HIGH 2, nothing kept the gate frozen.** `static.sh` now fails when `verify/` differs from the
  plan commit, and on any unstaged or untracked file anywhere in the repo.
- **HIGH 3, the leftover check could kill other sessions' processes.** Step 3 runs under a private
  `TMPDIR` and matches only processes whose argv carries it. It also runs on the red path.
- **HIGH 4, the hash's `v` overrode a restored arrangement on reload, breaking R7.** Fixed by the
  boot rule (§ 2.2). The probe now also reloads into a new page at the bare URL.
- **HIGH 5, the 380 px document-overflow check was red-but-good.** The header overflows there,
  which no R-ID covers. The probe now makes a narrow pane by a floor drag and a 440 px window, and
  measures panes only.
- **HIGH 6, focus was trusted from `data-focused`.** The focused title bar must now be drawn
  unlike every other. `together` checks that `j` moves the Files row with Files focused and starts
  the Graph walk with the Graph focused.
- **HIGH 7, the ports could loosen the existing suites.** `verify/kept.mjs`: every existing check
  label is still in the file (static) and printed as passed (step 3).
- **HIGH 8, R-IDs graded only by labels the build prints.** The frozen probe now covers:
  - a tab click (R1);
  - the selection followed in Files, the Inspector and the Graph (R3);
  - a view no pane holds, added to the focused pane (R9);
  - collapse and the accordion in a P2 group (R10);
  - the Log chart refit (R15);
  - README § Keys statically (R5).
- **MEDIUM 9, a stale `host_rect` after a move without a resize.** `presets` hovers the moved Graph
  until its tip shows, and requires the tip beside the pointer.
- **MEDIUM 10, a reload that never stored passed.** The store wait is now a check, plus the
  bare-URL page.
- **MEDIUM 11, focus after a collapse was unspecified.** It stays on the pane (§ 2.1).
- **MEDIUM 12, Shift cancelled the leader on real keyboards.** Modifier-only keys leave it pending;
  a cancelling key is still handled; blur or a timeout clears it (§ 2.7).
- **MEDIUM 13, floors ignored nested splits.** Floors are counted per pane (§ 2.1, D5).
- **MEDIUM 14, inner scrollers hid sideways overflow.** Any sideways scroll fails except `#tgrid`
  and `#ibody`. Files, Log and Results rows must fit 200 px (§ 2.6).
- **MEDIUM 15, an empty or covered view passed.** The shown root must hold content and be reached
  at its centre, and every tab must be reached at its centre.
- **MEDIUM 16, tabs could overflow between 300 and 490 px.** Tabs shrink to fit, and the title
  bar's overflow is graded (§ 2.3).
- **MEDIUM 17, `ready()` could race the boot.** It now also waits for the status bar to fill.
- **MEDIUM 18, expiring toasts raced the count.** A MutationObserver counts added toasts.
- **MEDIUM 19, `c` then `j`.** Put to the foreman at the sign-off and ruled as GOAL Q18: the
  Inspector keys never move focus, and R9 is reworded to say so. § 2.7 states the rule, and the
  probe's `keys` group checks that `c` then `j` steps the Graph walk while the Graph's pane keeps
  focus.
- **LOW 20, the help check was partly vacuous.** Each key must now sit beside its own preset name.
- **LOW 21, `--f` inheritance in nested splits.** Each child gets an inline flex; tests drag the
  sash and never set styles.
- **LOW 22, the `@media` awk.** It now strips comments and catches `@media` nested in other
  blocks; tested on a fixture.
- **LOW 23, the spec-id scan.** TECH says not to reflow `graph.rs` functions holding `r0`/`p1`, and
  to rename them if touched.
- **LOW 24, small probe gaps.** An `else` now fails each skipped branch; collapsed panes are
  measured against their own title; the shown tab is compared across reloads.
- **LOW 25, a pane growing back over 200 px could show a stale view.** A `.pbody` observer re-runs
  render-on-show (§ 2.6); `fit` checks it.
- **LOW 26, a bookmark saved the default.** The boot rule keeps the arrangement unsaved;
  `first-load` checks it.
- **LOW 27, phase coverage.** P1 grades R3 and R9, and P2 grades R10, in the frozen probe.

---

*Backing research: [`research/00-digest.md`](research/00-digest.md).*
