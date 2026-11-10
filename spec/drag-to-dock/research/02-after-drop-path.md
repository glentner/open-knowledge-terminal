# Research 02 — the path after a drop (operate, focus, key view, persistence, events)

Read-only survey of the worktree at `6f175fe` (base `50c9533`). Anchors are `file:line` + symbol.

## 1. `nav::operate` and § 1 in a `pointerup` handler

`nav::operate` (`nav.rs:156-188`) runs, in order:
1. `before` = views `panes::visible` now (DOM reads, no borrow).
2. `focus = s().focus` (a temporary `Ref`).
3. `op(&mut c.arr, focus)` **inside `cfg_mut()`**, then `c.arr_saved |= changed`, and the borrow drops (`:162-167`).
4. A toast if there is one. **Early return when `!changed && focus.is_none()`** (`:171`).
5. `s_mut().focus = f` → `sync_view()` (`:214`) → `panes::apply()` → `save_settings()` if changed → `nav::show(v)` for each view not in `before` and visible now → `nav(false)`.

`panes::apply` (`panes.rs:83-134`) clones the tree, then drops the borrow (`:84`). On a shape change it builds a new skeleton, `move_into` each root, parks views that no pane holds, and `o.remove()`s the old skeleton (`:110`). It then re-observes `BODIES`, runs `update`, refreshes the preset buttons, `graph::fit_pane`, `ON`, and `draw_key_view`.

These calls can re-enter:
- `move_into`'s `appendChild` fallback (`:249`) fires `blur` synchronously when the moved root holds DOM focus.
- `o.remove()` detaches the captured `.ptab`. The browser then releases capture and queues `lostpointercapture` at the document, not at `#panes`.
- `set_pointer_capture`/`release_pointer_capture` queue `got`/`lostpointercapture`.
- `panes::focus` (`:352`) runs `apply` + `nav`.
- The existing wiring routes `lostpointercapture` to the sash's `on_up` (`:491`).

Today no listener takes `S`/settings on blur (only `#go` blur → a timeout, `go.rs:758`; the window blur clears `LEADER`, `keys.rs:715`). § 1 still binds.

**Design consequences:**
- The handler must `take()` its drag state out of the thread_local and drop every `RefMut` before calling `operate`, `set/release_pointer_capture`, `panes::focus`, `chrome::toast` or the overlay's removal. The sash's `on_up` does this at `:451`.
- The op closure must `move` plain values in: view, target id, zone, and container size measured beforehand. It runs under `cfg_mut()`, so a `cfg()` inside it panics, and DOM measurement there is out.
- A pointer handler that reads the tree per move clones it under a short `cfg()` (as `start_drag` does at `:398`) and releases the borrow before touching the DOM.

## 2. Focus and the key view after a drop (R10)

`Outcome.focus` → `S.focus` → `sync_view` sets `S.view` only when the focused pane shows a main view (`nav.rs:220`). For an Inspector drop, `S.view` keeps the last main view (Q20), and `key_view` (`panes.rs:334`) returns it.

The press itself already focused the source pane (`on_down` → `focus`, `panes.rs:380-385`). So `S.view` at drop time is the source pane's shown main view, not what was focused before the press. That is consistent with Q14/Q20, but the test should expect it.

`draw_key_view` (`:143-149`) draws an off-screen key view once and latches it in `KEY_OFF`. Every `apply` passes through it, so the drop path is covered as long as it goes through `operate`.

Routes by which the key view can end up undrawn or stale:
- **(a) Centre join into a pane under the 200 px floor** (only a shrunken window makes one). The view is not "came on screen", so `operate` doesn't render it. `draw_key_view` draws it unless `KEY_OFF` already latched that same view. In that case it shows the content drawn when it was latched: the H1 class (REVIEW.md:290).
- **(b) An Inspector drop that covers `S.view`'s pane.** For example, the Inspector joins the centre of the pane showing `S.view`. `draw_key_view` draws it. This is covered if the drop's `Outcome.focus` is the Inspector's new pane, so that the focused pane is the Inspector's.
- **(c) A wrong `Outcome.focus` id** (see 4, duplicate ids). `focused_view` reads the first `[data-focused="1"]` (`:344`), so the keys go to the wrong view.
- **(d) A drop applied to a tree that changed mid-drag** (see risks). The focus then names a pane that no longer holds the view.
- **(e) A source pane whose shown tab leaves.** It shows the next view, which is "came on screen" and rendered by `operate` (`:182-186`). No hole.

A collapsed target is expanded by the join (R3). A split's new pane is expanded. `accordion` only expands. So the focused pane is never collapsed after a correct drop.

## 3. View state across a move (R10)

Roots are moved, never rebuilt: `move_into` (`panes.rs:235-253`).
- `moveBefore` keeps scroll and focus and fires no events.
- The fallback `appendChild` restores `scrollTop` only on descendants with `scrollTop > 0`, read before `update` unhides them. A root hidden at the move loses its scroll, as `w v` does. `scrollLeft` is never restored.

The Inspector's tab, selection and mode live in `S`, not the DOM. The Graph is refitted at the end of `apply` (`graph::fit_pane`, `graph.rs:2135`; `apply` `:130`) and by its own ResizeObserver on `#v-graph` (`graph.rs:2176-2180`).

`BODIES` re-observes the new bodies after a rebuild (`:113-120`). Its first callback runs `bodies_resized` (`:160`). `ON` was already updated in `apply`, so nothing renders twice.

A split adds nothing `w v` doesn't. Its new pane is one more `build` leaf (`:195-209`) with a new `data-pane`, drawn by the same shape-change rebuild. No new mechanism is needed.

## 4. Pane ids

- `number` (`arrange.rs:299`) assigns ids in reading order from 1. It runs only in the preset builders and in `sanitise` (`:490`).
- **No operation renumbers.** `close`/`move_view` `lift` (`:820`) and keep the survivors' ids, so ids become non-contiguous after any emptying move. `swap` keeps slot ids.
- `PaneId` is held across calls only by `S.focus` (`state.rs:363`). `KEY_OFF` and `ON` hold views, `SHAPE` holds the shape string, and the sash's `Drag` holds a path.

**Consequences:**
- A new pane must take `max id + 1`. `panes().len() + 1` can collide after a lift: ids {1,3} → 3. Renumbering is safe for the DOM, but it breaks R8's "exactly the tree `w v` produces", since `Pane: PartialEq` includes `id`. So don't renumber.
- `shape` (`:383`) includes ids and views. Every successful drop changes it (a join changes a pane's views; a split adds a leaf), so every drop takes the full rebuild path, including the detach of the pressed tab.

## 5. Persistence (R9)

- `operate` sets `arr_saved`. `Settings::to_json` writes `"arrangement"` when `arr_saved` (`state.rs:264`).
- `save_settings` is debounced 200 ms (`state.rs:285-296`); `flush_settings` runs on `pagehide` (`keys.rs:717`).
- On load: `from_json` → `sanitise` (`state.rs:229-232`). `parse_node` recurses with no depth limit. `clean` keeps each view once, drops empty panes, resets `f` outside (0,1) to 0.5, and fixes `shown`. `accordion` and the collapsed-root rule follow, then `number`.

Every tree a valid drop makes passes through unchanged except the ids. That covers deep nesting, one-view panes, same-orientation nesting and `f = 0.5`, provided the drop op itself runs `accordion` and never leaves a collapsed root. The ids are renumbered into reading order, which is harmless because a fresh page holds no `PaneId`. Rects come from the same `f`s, so they are equal.

**No load-path change and no `state.rs` change are needed.** Suite note: wait out the 200 ms debounce, or rely on reload (which flushes on `pagehide`), before reading `okt:<kb>`.

## 6. Hash and history

`nav(false)` = `replaceState`, and only if the hash differs (`nav.rs:298-313`). It is skipped while `S.restoring` (during `apply_hash`).

The hash carries `v` (`S.view`), `s`, `tab`, `c`, `rev`, `t`, `q` (`:269-296`), and no arrangement. A main-view drop changes `v` → replaced. An Inspector drop usually leaves it equal → untouched. Nothing pushes.

Caveat for R6: the press itself can replace the hash (`on_down` → `focus` → `nav(false)`, `panes.rs:360`) before the drag begins. Snapshot "before" at drag start, or press in the already-focused pane.

## 7. The tab click path

`keys::on_click` (`keys.rs:136-143`, delegated on `document`, `keys.rs:673`):
- `[data-view]` "insp" → `nav::show_insp` (`place(Insp, false)` + `insp::render`, no focus change, **no hash call**).
- Anything else → `set_view(v, false)` (`place(v, true)`, `show` if visible, **`nav(true)`, a pushed entry**).

A below-threshold release must let that native click through untouched. So don't `preventDefault` anything that suppresses `click`. Capture on the tab is fine, since both the down and up targets are the tab.

**Swallowing:**
- After a cancelled or no-op drag the tab is still in the DOM and captured, so the click **does** fire on it. It would show the tab, against Q14.
- After a drop the pressed tab is detached, and whether or where a click fires is engine-dependent.

Put the swallow in a capture-phase `click` listener on `window`, set when a moved drag ends, that calls `stop_immediate_propagation` + `prevent_default`. Clear it on the next `pointerdown` or with `timeout(0)`, never only "on the swallowed click", or a stale flag eats the next real click. The alternative is a check at the top of `on_click` before `:64`, but only the `[data-view]` branch matters.

## 8. Esc and the keys during a drag

`on_key` order (`keys.rs:333-371`):
- returns on meta/ctrl/alt (`:334`);
- returns early when an input/textarea/select/contentEditable is the target (Esc only blurs it, `:337-348`);
- swallows everything while the help overlay is open (`:350-358`);
- then the leader (`:362-371`: modifier-only keys `Shift|Control|Alt|Meta|CapsLock` leave it pending; Esc cancels and does nothing else).

Also, `#go` has its own `keydown` (`go.rs:761`, Esc blurs `#go` at `~:802-804`). It runs at the target before the document listener, so a drag check inside `on_key` cannot make Esc "do nothing else" when `#go` has focus. That can happen in engines where pressing a button doesn't move focus.

**Recommendation:** a capture-phase `keydown` on `window`, checked before everything:
- modifier-only key → ignore;
- Esc → cancel, `prevent_default` + `stop_immediate_propagation`;
- any other key → cancel and let it through (Q13).

Failing that, the check goes at the very top of `on_key`, before `:334`, and accepts the `#go` gap.

Window `blur`: add the cancel beside `LEADER.set(-1.0)` (`keys.rs:715`). `pointercancel` is **not wired anywhere today**; add it.

## 9. Other pointer listeners

- **The Graph:**
  - `pointerdown` on the canvas only, button 0 (`graph.rs:2190`), so a tab press never pans.
  - A window `pointermove` hover (`:2201-2244`). With capture on the tab, the target is the tab, so `over` is false and the hover clears. Without capture, a drag across the canvas hit-tests and shows hover. Capture is the fix.
  - A window `pointerup` (`:2246`) is a no-op without `g().drag`.
- **The sash** (`panes.rs:374-462`) shares `#panes`' `pointermove`/`pointerup`/`lostpointercapture`. Keep the tab drag in a separate state, or branch on it, so `on_move` doesn't run `arrange::drag` for a tab drag.
- **Mouse listeners on `document`:** `mousedown` in `chrome.rs:345` (closes the settings popover) and `go.rs:830` (`#gosug`). A `preventDefault` on `pointerdown` suppresses the compat `mousedown`, so the popover would stay open.
- Nothing listens on `.ptitle`/`.ptab` directly. `on_down` doesn't check `e.button()`. The tab drag should require button 0.
- Text selection: `.ptab` has no `user-select: none`. Only `#graph`, `.tgrid th` and a few others do. The sash `prevent_default`s at down (`:389`). Use `user-select: none` on `.ptitle`, or a `selectstart` guard, rather than `pointerdown` `preventDefault`.
- `pane_at` (`:465`) uses `elementsFromPoint` and can be reused for the target if the overlay is `pointer-events: none`, or lives outside `.pane`.

## 10. Where the validity rules live

| Rule | Where |
|---|---|
| floors `MIN_W`/`MIN_H`/`TITLE` | `arrange.rs:8-12` |
| `min_extent` | `arrange.rs:842` |
| `accordion` | `arrange.rs:530` |
| collapsed-root rule | `sanitise` (`arrange.rs:483-492`) |
| `drag`'s clamps | `arrange.rs:868` |
| CSS mirror | `app.css:187-208`: `@container pane (width < 200px)` hides `.pbody`; collapsed `flex: 0 0 24px`, set inline by `panes::update` (`panes.rs:289-301`) |
| `visible` (`width >= MIN_W`) | `panes.rs:310-326` |

A test-only pure layout already exists: `rects`/`layout` (`arrange.rs:1078-1106`, under `cfg(test)`). It matches the CSS: shares, collapsed 24 px, absolute sashes and separators. Promote it for the overlay's predicted layout and for the floor check.

## Risks for the plan (most severe first)

1. **The overlay drawn from the target's current rect fails R2.**
   - When the source pane empties and the target lies in its sibling subtree, `lift` grows the target, so a centre join is larger than the target, and an edge split halves the grown extent, not the "former" one.
   - When a collapsed target expands.
   - When `accordion` expands a second collapsed pane after a lift. Example: `[[X(c) | [S | C(c)]] | Y]`, S's only view → Y gives `[X(c)|C(c)]` → C expands.

   Compute the overlay and the R7 floors from the predicted tree's layout. Also, R4's "former extent" and Q12's "≥ 400 px target" disagree with the post-lift geometry; the plan must rule which one governs.
2. **Lost capture treated as a drop.** `lostpointercapture` is wired to `on_up`, which commits for the sash (`panes.rs:491`). Reusing it for the tab drag makes Esc (whose release fires it) or a lost pointer drop the view, against R6.
3. **The post-drag click shows the tab** (Q14 broken on cancel and no-op drops). A stale swallow flag instead eats the next real click.
4. **The arrangement changing under a live drag.** Async `set_view` from table/list loads (`table.rs:41`, `list.rs:26`, `go.rs:224`), `popstate` (mouse back button, `keys.rs:709`), a key (Q13). Each runs `place` → `apply`, possibly a rebuild that detaches the captured tab, whose lost capture goes to the document and not to `#panes`, leaving the drag live and stale. Store the dragged view rather than its source id, re-resolve the holder in the op, and cancel when `shape` differs from drag start.
5. **New-pane id collision** (`len + 1` after a lift). This yields duplicate `data-pane`, the wrong focus mark and the wrong key view (2c).
6. **A borrow held across `operate`/capture calls** (§ 1, CRITICAL in review). This includes `cfg()` called inside the op closure, which runs under `cfg_mut()`.
7. **Esc not "nothing else"** when `#go` has focus (its own keydown) or the help overlay is open, if the check sits inside `on_key` after `:334`.
8. **Key view under the floor stays stale** after a join into a narrow pane (the `KEY_OFF` latch, H1 class), plus a fractional-px pane at exactly 400 px splitting to 199.9x px halves. `visible` would then hide the body, so the floor check should allow an epsilon or compare against `layout` rounding.
9. **The R12 suite reads `okt:<kb>` before the 200 ms debounce**, or measures the hash before the press's own `replaceState`. Both give false failures.
