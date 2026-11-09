# 02 — Settings persistence, `insp` migration, and the borrow rule

Read at `edcbed4`. Anchors by symbol; `state.rs` is high-blast-radius (AGENTS.md § Invariants).

## 1. `Settings` today (`state.rs`, `struct Settings`, `Settings::load`, `Settings::to_json`)

- Hand-rolled over `serde_json::Value` (no `#[derive(Deserialize)]` on `Settings`; serde derive *is*
  available, `live.rs`/`model.rs` use it). `preserve_order` is on. **No version field.**
- Fields → JSON keys: `theme` (str, default boot theme or `"terminal"`), `accent` (str, boot or
  `"#ffa028"`), `reader` (str|null), `ticker` (bool, true), `labels` (bool, false), `insp` (number,
  46.0), `collapsed` (obj id→bool; graph folders), `collapse` (int|null), `treeOpen` (obj id→bool),
  `tableCols` (obj → [str], omitted when None), `badge` (bool, omitted when None), `pins` ([str] ≤9,
  omitted when empty), plus private `extra: Map`.
- Load: `localStorage.getItem("okt:<boot.kb or 'kb'>")` → `serde_json::from_str::<Value>`. Malformed
  JSON or a non-object → **all defaults** (and the next save overwrites the whole blob). Per key, a
  `match (key, &value)` takes only the expected JSON type; a known key with the wrong type, and every
  unknown key, falls to `_ => extra.insert(k, v)`. So a bad field resets only itself (to_json then
  overwrites the stale `extra` copy, since it inserts known keys after cloning `extra`).
- **Unknown keys round-trip**: `to_json` starts from `extra.clone()`. This is the downgrade story:
  an older okt reading a new `arrangement` key keeps it in `extra` and writes it back untouched. No
  crash, no loss.
- Loaded lazily on the first `cfg()`/`cfg_mut()` (thread_local `SETTINGS`), after `read_boot()`.
- Save: `save_settings()` → `static SAVE: dom::Debounce = 200 ms`; the closure serialises `cfg()` at
  fire time and `setItem`s it. Cheap (a few KB of JSON). No `pagehide`/`beforeunload` flush, so a
  change is lost if the page unloads within 200 ms; no cross-tab `storage` listener (last writer
  wins on the whole blob).

## 2. `insp` and the migration

- `insp: f64`, default 46.0. Clamped to 22–75 and rounded to 0.1 **only in the drag handler**
  (`keys::wire`, the `#split` pointerdown closure); `Settings::load` does not clamp. Applied once at
  wire time as CSS var `--insp-w` on `#panes` (`.panes { grid-template-columns: minmax(0,1fr) 5px
  var(--insp-w,46%) }`), and on every pointermove.
- **Key finding:** `to_json` always writes `insp`, so *every* user who ever saved any setting
  (theme, a pin, a tree toggle) has `"insp": 46.0` stored. "insp saved" does not mean "the user
  dragged the splitter". See Questions.

Recommended shape (JSON, inside the existing blob):

```json
"arrangement": { "v": 1, "root": NODE }
NODE = { "split": "row" | "col", "sizes": [54, 46], "kids": [NODE, …] }
     | { "views": ["graph","files","log","table","list"], "shown": "files", "collapsed": false }
```

- `sizes` are each child's **expanded** share (percent, summing to 100). A collapsed child keeps its
  share; layout gives it the title-bar strip and the expanded siblings split the rest in proportion
  (flex-grow = share). That makes R10's "return to the size it had before" free and needs no
  separate size-before-collapse field.
- View names are **strings, not a serde enum** — an unknown variant would fail the whole parse;
  strings let the sanitiser drop unknowns (future views) individually. Use `#[derive(Serialize,
  Deserialize)]` on the node types without `deny_unknown_fields`, parse via
  `serde_json::from_value` in a new `("arrangement", v)` arm; on error, drop it (don't put it in
  `extra`, or it would be written back alongside a different live tree).
- Pane ids: runtime-only (`#[serde(skip)]`, assigned by the sanitiser), not persisted.

"Chosen means saved" (Q4) has a trap: any unrelated save (`theme`, pins) calls `to_json`. If the
default/migrated tree sat in `Settings` as a plain value it would be persisted by a theme change and
become "chosen". Prescribe: `Settings { arr: Arrangement, arr_saved: bool, … }` — `load` fills
`arr` from the saved key (`arr_saved = true`) or from the default (width < 900 → stacked, else
classic with the Inspector at `clamp(insp, 22, 75)`), and `to_json` writes `arrangement` only when
`arr_saved`. Every user change sets `arr_saved = true` then `save_settings()`. The default is chosen
once per load and is not re-evaluated on window resize (otherwise it is the automatic 900 px rule
again). The window width read for the default: `dom::win().inner_width()` (`< 900`).

`insp`: keep the field, keep reading it (migration source) and keep writing it, but **frozen** — the
new splitter no longer updates it. A downgrade then shows exactly the pre-upgrade width, and the new
`arrangement` survives in the old build's `extra`. Dropping it from `to_json` would only make the
old build fall back to 46%; removing the field would park it in `extra` forever. Neither helps.

## 3. Sanitiser (pure fn, run on load and after every op as a debug assert)

1. Views: keep known names only (`graph files log table list insp`); dedupe across the tree in DFS
   order (first holder wins).
2. Pane: no views → remove; `shown` not held → first view; `collapsed` non-bool → false.
3. Split: orientation not `row`/`col` → `row`; `sizes` re-derived per surviving kid (missing,
   non-finite or ≤0 → equal share), renormalised to 100, each ≥ a floor (e.g. 5).
4. Split with 0 kids → remove; 1 kid → replace by the kid (it inherits the split's slot).
5. Every split keeps ≥1 expanded child along its axis (else expand the last); a lone root pane is
   never collapsed.
6. Empty root → the width default. Bounds fall out of uniqueness: ≤6 panes.
7. A missing view (Inspector, Graph, …) is **legal** (R9 adds it on demand). But `dom::id` panics on
   a missing id, and `graph::wire` (canvas + `ResizeObserver` on `#v-graph`), `insp::render`
   (`#ibody`), the tree/log/table wires all look up their hosts by id: un-placed views must stay in
   the DOM in a hidden parking container, never be removed. Likewise `#split`/`#panes` are wired by
   id in `keys::wire` — replace that wiring, don't leave it dangling.

## 4. `S`, the borrow helpers, and § Invariants 1 in practice

- `STATE`, `SETTINGS`: `thread_local! { static …: &'static RefCell<…> = Box::leak(…) }`. Helpers
  `s()`/`s_mut()`/`cfg()`/`cfg_mut()` return `Ref`/`RefMut<'static>`; `idx()` and `sel()` clone out.
  The graph has its own `GS` with `g()`. A double borrow is a runtime panic, not a compile error.
- Existing patterns to copy: scoped write block then act (`nav::set_tab`, `nav::set_sel`,
  `graph::toggle_labels` then `let on = cfg().labels;`); copy-out before dispatch (`keys::on_key`:
  `let view = s().view;`); statement-scoped temporaries (`s_mut().view = v;` in `nav::set_view`,
  `cfg_mut().insp = v;` in the drag); compute-then-act (`actions::pin_toggle` returns `(msg, err)`
  from the block, toasts after); explicit `drop(gs)` before `apply_view()`/`schedule()`
  (`graph::resize`, `graph::declutter`); `go.rs` clones out of `SUG` before `pick`/`blur`.
- Footgun (edition 2024): `if let Some(x) = s().foo.clone() { … }` and `match s().view { … }` keep
  the `Ref` alive through the body/arms (e.g. `go.rs` `if let Some(ld) = s().log_data.clone()`).
  Bind with `let` first in pane code.

Calls a pane system makes, by re-entrancy:

| Call | Re-enters synchronously? |
|---|---|
| `HtmlElement.focus()`/`blur()` (`dom::focus`) | **Yes**: focus/blur/focusin/focusout run inside the call (`#go` has a `focus` → `render_sug` listener; a pane `focusin`/`pointerdown` focus handler would too) |
| Re-parenting a view's DOM that contains the focused element (e.g. the Files filter) | **Yes** in Chromium: removal fires `blur`/`focusout` synchronously. Treat moves like `focus()` |
| `dispatch_event`, `el.click()` | Yes |
| `set_view` and view renderers (`tree::render`, `log::render`, `insp::render`, `graph::schedule`) | Not events, but they borrow `s()`/`cfg()`/`g()` internally — call with nothing held, never under `s_mut`/`cfg_mut` |
| `history.pushState/replaceState` (`nav::nav`) | No events |
| `location.set_hash` | `hashchange` async (unused today; okt listens to `popstate` only) |
| `scrollIntoView`, `getBoundingClientRect` | No sync events (scroll is async); forces layout |
| `ResizeObserver` callback | Delivered later in the rendering steps, never inside the call that changed size |
| `spawn(async …)` | Runs on a microtask after the current sync code; no borrow may span its `await`s |
| `localStorage.setItem` | `storage` fires only in other documents |

Prescription: **compute under a short borrow, drop, then act.** Tree ops are pure functions over
`&mut Arrangement` returning a plain-data outcome (panes to remount, view/pane to focus, toast
text, whether to save); run them in `{ let mut c = cfg_mut(); let out = op(&mut c.arr, …); c.arr_saved = true; out }`,
then, with nothing borrowed: mutate DOM, `save_settings()`, render, `nav()`, and `dom::focus` last.

Where state lives: the tree (views, shown tab, collapsed, sizes) in `Settings` (persisted). The
focused pane id is transient in `S` (R7 does not ask for it on reload; default it to the pane holding
the hash's `v`, else the first). Keep `S.view` as the focused pane's shown view, updated on focus
change, so `state_hash`, `keys::on_key` and the `s().view == "log"` checks keep working.

## 5. Save frequency

The splitter mutates `cfg_mut().insp` on every pointermove without saving and calls
`save_settings()` once on pointerup (then re-renders the Log if shown). Keep that for every split:
write sizes on move (and the CSS), save on pointerup; the 200 ms debounce makes extra calls harmless
anyway. Recommend a synchronous flush on `pagehide` (reuse the debounce body) so a quick reload after
a change keeps it; at minimum the R7 browser check must wait > 200 ms before reloading.

## 6. Tests

`cargo test -p okt-client` runs natively (verified: 6 pass in ~11 s cold, `js::tests`,
`model::tests`, plain `#[cfg(test)] mod tests` + `#[test]`; no `wasm-bindgen-test`). web-sys and
js-sys compile for the host, but calling any import panics off-wasm, so `Settings::load` (which
calls `boot()` and `localStorage`) is not testable as is. Put the arrangement types, presets,
sanitiser and ops (swap, flip, close, move, collapse/expand, resize) in a new module with no
`dom`/`state` imports, plus a pure `Settings::from_map(defaults, Map)` split out of `load`, and test
them natively: round-trip, migration (`insp` only, neither, corrupt, unknown view, duplicate view),
and each op's refusals. No existing test covers settings. `tests/ui/keys.mjs` uses a fresh context
at 1600×1000 (empty localStorage → classic by R6).

## Questions for the planner

- **R6 vs R7/Q14 when narrow.** Window < 900 px, no arrangement saved, `insp` saved: stacked (R6) or
  classic at `insp` (R7)? Because `insp` is written with every settings save, nearly every existing
  user has it, and before this cycle a narrow window showed the stacked layout. "Looks as before"
  therefore reads as: the width picks the preset; if classic, the Inspector takes `insp`. Recommend
  that reading and saying so in PLAN.md.
- **R7 "rect within 1 px" with collapsed panes.** Storing expanded shares (above) restores rects
  exactly; confirm the planner accepts that a collapsed pane's stored share is its pre-collapse size.
- **R13/R14 "neighbouring pane"** is ambiguous when a pane's sibling is a split (move into which
  leaf?). The sanitiser doesn't decide it; the ops do — PLAN should define it (e.g. the nearest leaf
  of the next sibling, else the previous).
