# 05 — Keys (research brief, read at `edcbed4`)

## 1. The dispatcher today

One `keydown` listener on `document`: `keys::on_key` (wired in `keys::wire`). The order of the gates in
`on_key` decides everything below:

1. **Any Meta/Ctrl/Alt → return** (not prevented). Every modified key belongs to the browser/OS.
   Shift is *not* filtered: it arrives as `e.key()` = `J`, `?`, `+`, `!`…
2. **Target is input/textarea/select/contenteditable** (`dom::target_el` = `e.target`) → Escape blurs,
   everything else returns untouched. Covers `#go`, `#ffilter`, `#lfilter`, `#tfilter`, settings inputs.
   `#go` has its own listener (`go::wire`: ↑ ↓ Tab Escape).
3. **`#help` shown** → Escape or `?` hide it; every other key is swallowed (returned, not prevented).
4. `/` `:` → focus `#go`.
5. `view == "files"` → `views::tree::key`; `view == "log"` → `views::log::key` (both return true if handled).
6. `,` `.` time-travel arm; `,` from the working tree (tracked file selected).
7. Escape chain; then the Graph walk arm (`view == "graph"`); then the global `match k.as_str()`.

No pending/prefix state exists anywhere (no `g g`, no marks, no timeouts on keys). The Graph walk is
a mode inside `graph.rs` (`walk_stop` returns whether one was active), not a key prefix.

| Key | Scope | Action | Source |
|---|---|---|---|
| `/` `:` | global | focus `#go` | `on_key` |
| `g` `f` `l` | global | `nav::set_view(graph/files/log)`, pushes history | `on_key` match |
| `t` | global | `views::table::open(current_folder)` (re-targets the table each press; `set_view("table")` inside) | match |
| `o` `c` `h` | global | `nav::set_tab(overview/content/history)` (replaceState, clears commit) | match |
| `b` | global | toggle blame | match |
| `e` | global | expand/collapse the selection's folder in the graph model | match |
| `u` | files view | parent folder, fold everything under it | `tree::key` |
| `u` | other views | select parent | match |
| `j` `k` `↓` `↑` | graph view | walk the selection's connections | walk arm |
| `j` `k` `↓` `↑` | files / log view | move the row (Files: select; Log: open commit) | `tree::key`, `log::key` |
| `→` `←` | files view | open folder / fold or go to parent | `tree::key` |
| Enter | graph view | go to walk target; else open/close folder; else `set_tab("content")` | walk arm |
| Enter | files view | toggle folder / `set_tab("content")` | `tree::key` |
| space | graph view | as Enter but never opens content | walk arm |
| space | files view | toggle folder / `set_view("graph")` + `focus_node` | `tree::key` |
| Escape | global chain | stop walk (only `view=="graph"`) → close `#settings` → close commit → leave travel → clear selection | Escape arm |
| `,` `.` | travelling (`rev` + `travel_nav`) | older / newer version | `,`/`.` arm |
| `,` | not travelling, tracked file selected | step to last committed version | `,` arm |
| `?` | global | help (also closes it) | match / help gate |
| `R` | global | rebuild index | match |
| `[` `]` | global | history back / forward | match |
| `0` `+` `=` `-` | global (graph camera even when hidden) | fit, zoom in, zoom out | match |
| `J` `K` | global | scroll `#ibody` ½ page | match |
| `z` | global | `set_view("graph")` if not graph, then `frame_selection` | match |
| `s` | global | source/rendered, forces tab content | match |
| `y` | global | press `#ihead [data-copy]` | match / `press` |
| `n` `N` | global | `list::find_step(±1)` (uses `#ibody mark.fm`) | match |
| `m`, `1`–`9` | global | pin toggle, pin jump | match |
| `E` `O` `F` | global | press `#ihead [data-open=editor/app/reveal]` | match / `press` |

Table and Results have **no** key handlers (the help line "↑ ↓ ⏎ move and open in lists" describes
Files/Log; `#tgrid`/`#list` only get native scroll via `tabindex`).

Browser/OS-reserved (never reach okt, by gate 1, or browser-default when not prevented): every
Cmd/Ctrl/Alt chord — notably Cmd/Ctrl-W, T, N (Chrome does not deliver them to the page, PWA window
included), Cmd-[ ] back/forward, Cmd-1…9 tab switch, Cmd-L, Cmd-R, Cmd-`, Cmd-H/M/Q, Ctrl-Tab;
Tab/Shift-Tab (focus traversal — keep unbound); F-keys. Firefox Quick Find on `'` and `/` when not
prevented. On US-International and most European layouts `` ` `` `~` `^` `'` `"` are dead keys
(`e.key()` = `"Dead"`), and `\` `[` `]` `{` `}` `|` need AltGr/Option (filtered by gate 1). CapsLock
turns lowercase into the capital key (`e`→`E` edit, existing quirk).

## 2. Free unmodified keys (proven)

Checked against every arm: the help gate, `/`/`:`, `tree::key` (tree.rs `match k.as_str()`: ↓ j ↑ k → ←
Enter space u), `log::key` (↓ j ↑ k), the `,`/`.` arms, Escape, the walk arm (j ↓ k ↑ Enter space),
and the global match (g f l t o c h b e u ? R [ ] 0 J K z s y n N m 1–9 E O F + = -).
`tests/ui/keys.mjs` presses only u space j k Escape Enter z J K s y m 1 2 3 E O F / n N (plus text typed
into `#go`, swallowed by gate 2); no assertion relies on a key doing nothing.

- **Lowercase free:** `a d i p q r v w x`. (`j`/`k` are unbound in table/list/insp but are view keys —
  not free.)
- **Uppercase free:** `A B C D G H I L M P Q S T U V W X Y Z`.
- **Digits:** none free (`1`–`9` pins at the `d if … b'1'..=b'9'` arm, `0` fit).
- **Punctuation free:** `!` `@` `#` `$` `%` `^` `&` `*` `(` `)` `_` `{` `}` `|` `\` `;` `'` `"` `<` `>`
  `` ` `` `~`. `.` has no arm outside travel but is reserved by help (`, .`); treat as taken.

## 3. Proposed keys

**Presets and cycle — single unmodified keys (R5):**

| Key | Preset | Why |
|---|---|---|
| `!` | classic | Shift+1…4: "arrangement 1–4", the pin row shifted; CapsLock-proof |
| `@` | swapped | |
| `#` | stacked | |
| `$` | three | |
| `\` | cycle classic → swapped → stacked → three → classic | unshifted, free, looks like a split |

Match on `e.key()` like the rest of the dispatcher (Playwright `keyboard.press('!')` sends `key: "!"`).
US-layout glyphs; on other layouts Shift+digit differs (accepted: okt's `[`/`]` already assume US).

**Pane operations — leader `w` (vim's Ctrl-w, unmodified because Ctrl-W closes the tab):**

| Keys | Op | Mnemonic |
|---|---|---|
| `w w` / `w W` | focus next / previous pane (reading order) | vim `^W w`/`W` |
| `w c` | collapse / expand the focused pane | collapse |
| `w x` | swap the focused pane with its neighbour | vim `^W x` exchange |
| `w r` | flip the parent split (side by side ↔ stacked) | rotate |
| `w q` | close the focused pane | quit |
| `w m` / `w M` | move the shown view to the next / previous pane | move |
| `w n` / `w p` | next / previous tab in the focused pane (optional) | tmux n/p |
| `w Escape` | cancel | |

Recommendation: the leader. Single keys would spend six of the nine free lowercase letters (`a d i
p q r v w x`) on rarely used operations, and the coming views (agent chat, dream inbox) will want
show-keys from that same set (`a`, `i`/`d`). A leader costs one free key, puts every pane op behind
one discoverable prefix, and doesn't touch any existing meaning, because the second key never reaches
the normal match. Cost: a new pending state. Sketch: a `thread_local! { static PANE: Cell<bool> }` in
`keys.rs` (a `Cell`, not a field in `S`, so no borrow exists to hold across re-entry), checked right
after the help gate and before `/`. While it's pending, the next key is always consumed. An unknown
key gets a toast (`PANE · no op for X`), Escape cancels, and the input gate and a click clear it. Show
it as a toast or title-bar marker listing the ops. Don't add a timeout (tmux style); if the plan adds
one, 2 s is enough. An optional `w ?` can open help.

## 4. Focus semantics (R8)

Every view-acting branch keys on `S.view`. None reads `document.activeElement`. The only target check is
gate 2 (inputs), and `#tree`/`#ltable` `tabindex` is used only by native scrolling.

- On `S.view`: `tree::key` (Files), `log::key` (Log), the walk arm (Graph `j k ↑ ↓ Enter space`), the
  Escape chain's `walk_stop`, `u` (tree vs global), and `z` (switches to graph only if not shown).
- **These move to "the focused pane's shown view"**. The simplest form is `let view = focused_view();` at the
  top of the dispatcher, where the Inspector is a fifth value that matches no view branch. With the
  Inspector focused, `j`/`k` then do nothing, as they do in Table/Results today.
- **Inspector-wide, focus-independent:** `J K s y , . b n N E O F` (they act on `#ihead`/`#ibody`,
  wherever it is), plus `m 1–9 [ ] R ? e`, the non-walk part of Escape, and `0 + = -` (the one
  Graph's camera, wherever it is).
- Principle that keeps "no key changes meaning": **today's `S.view` becomes the focused pane's shown
  view.** So everything that calls `nav::set_view` today also focuses the holding pane: keys,
  `.fk` clicks, the GO commands GRAPH/FILES/LOG, Files space→graph, `table::open`, `list::show`, and
  `apply_hash`. `set_tab` focuses only for the `o`/`c`/`h` keys. Its other callers (Files/Graph Enter →
  content, outline clicks, `select{tab}`) reveal the Inspector, expanding it if collapsed, without
  moving focus, so Enter-then-`j` keeps walking Files as it does today.
- `apply_hash` only calls `set_view` when `s().view != v`. It has to place and focus whenever `v` isn't
  the focused pane's shown view, or back/forward (and keys.mjs's `go('#v=files…')`) won't refocus.
- Follow-the-selection is gated on `S.view` too (`nav::select`: `view=="graph"` → `focus_node`,
  `view=="files"` → tree render; `apply_hash` the same; `tree::render` at tree.rs ~262 when
  `view=="files"`; `live.rs` 85/96/101; `insp.rs` ~1957; the resize/splitter/60 s timers in
  `keys::wire`). Under R3 they become "is shown in some pane". That belongs to the R3 brief, but it's
  the same `S.view` split.

## 5. View keys under R9

Today: `g f l` call `set_view` (it hides the other four `#v-*`, toggles `.fk.on`, renders, and
`nav(true)` pushes). `g` on a shown Graph re-runs and `graph::schedule` (`nav` no-ops when the hash
is the same). `t` always re-opens the table on `current_folder`. `o c h` call `set_tab` (replaceState,
clear commit), and the Inspector is always visible today. `/` `:` focus `#go`. Nothing pushes when
already there.

R9: make `nav::set_view(v)` the single place the rule lives: find the pane holding `v`, make it that
pane's shown tab, expand it if collapsed, and focus it; if no pane holds it, add it to the focused pane.
Every caller listed in §4 then inherits the rule, including Results from a search (`list::show`) and
`t`. `o c h`: the same for the Inspector, then the tab. Edge cases:
- `g` when the Graph is already shown and focused: idempotent (no push). When it's shown but not
  focused: focus only. When it's collapsed (three): expand, then `graph::schedule` once the pane has
  its size back.
- `o`/`c`/`h` with the Inspector pane collapsed: expand, then focus. `J`/`K`/`s`/`n`/`N` on a collapsed
  Inspector have no visible effect (zero `client_height`); see the questions.
- `z` keeps meaning "show the Graph, then frame": rule applies.
- "No pane holds it" can't arise from the four presets or close/move (all six views are always held);
  only a future view or a stale saved arrangement produces it.

## 6. tests/ui/keys.mjs

`state()` reads `#tree .trow:not(.thead)` (`.tw` ▾, `.on`, `data-id`), `#v-<v>.hidden` (first visible
of graph, files, log, table, list), `location.hash` `s=`, `#ghint` (hidden/text),
`#ibody .nchip.kb[data-go]`, `#ibody .src`, `#ibody article.md`, `#ibody.scrollTop`,
`#ibody mark.fm(.cur)`, `#toasts .toast`. Navigation is by `go(hash)` (popstate → `apply_hash`), with
no clicks.

| # | Assertion | Relies on |
|---|---|---|
| 1 | deep file: its folders open | `#v=files` → tree rendered; `.tw` |
| 2 | `u` → parent (loop), folds below | `u` routed to `tree::key` (Files focused); `.on`, `.tw` |
| 3 | `u` at top folds all | same |
| 4 | outer and inner open | hash nav twice |
| 5–6 | space closes / opens one level | `tree::key` space |
| 7 | space on a file shows the graph | `state().view==='graph'` (`#v-graph` not hidden), hash `s=` |
| 8–10 | `j` starts walk, `j` steps, `k` wraps | walk arm on Graph focus; `#ghint`, `#ibody .nchip.kb` |
| 11 | esc stops walk, keeps selection | Escape `walk_stop` gated on graph view |
| 12 | Enter goes to marked | walk arm |
| 13 | space toggles folder in graph | toast EXPANDED/COLLAPSED |
| 14 | `z` frames | no page errors |
| 15 | J/K scroll inspector | `#ibody.scrollTop` (needs a non-collapsed Inspector) |
| 16–17 | `s` source / rendered | `#ibody .src`, `article.md` |
| 18 | `y` copies path | `#ihead [data-copy]`, clipboard |
| 19–22 | pins 1, 2, "No pin 3", unpin | hash `s=`, toasts |
| 23 | E O F | `#ihead [data-open=…]`, intercepted `/api/open` |
| 24–26 | FIND then `n` `n` `N` | `/`→`#go`, submit blurs, `#ibody mark.fm.cur` |
| 27 | no page errors | |

- (a) **Reparenting `.vpane`s into pane bodies with their ids kept breaks nothing.** No selector is scoped by a
  parent (`#view`, `#insp`, `#panes` are never queried).
- (b) **Replacing `hidden` with another mechanism** breaks #7. `state().view` would then return `graph`
  always, or `undefined`, and #7 would pass vacuously or fail. Recommend keeping the `hidden` attribute
  on non-shown `#v-<view>` tabs. Collapse should use the pane container, not `hidden` on the view, but
  note that a collapsed Graph pane then still counts as "visible" to `state().view`.
- (c) **Classic at 1600×1000 as the default** (fresh context, no saved arrangement) holds all five in one
  pane, so `state().view` is unambiguous and every outcome holds. That depends on #2/#5/#8 reaching
  the right view: the left pane must be focused at load, and `apply_hash` must focus the holding pane.
- "First visible" with several views visible: graph is first in the list, so in **three** it would
  report `graph` whenever the Graph pane exists. To run keys.mjs under other presets, read the
  focused pane's shown tab instead (for example a `data-view` on the focused pane's shown view).

## 7. Where the new keys get documented

- Help: `chrome::show_help`, the `keys: &[(&str, &str)]` array, rendered as
  `<dt><kbd>{k}</kbd></dt><dd>{d}</dd>` under `<h4>KEYS</h4>`. Add rows `("! @ # $", "arrangement:
  classic, swapped, stacked, three")` and `("\\", "next arrangement")`, plus a `<h4>PANES</h4><dl>`
  section built the same way, like the PINS block, for the `w` ops. Fix the stale "lists" line while
  there (it should read "files, log").
- README § Keys: prose bullets `- AREA: …`. Add `- PANES:` (presets, cycle, `w` ops, and the focus
  rule: view keys act on the focused pane, and `g f l t o c h` show and focus).
- The pane tab titles can carry `<kbd>` hints the way `.fk` and `#itabs` buttons do today
  (`<kbd>G</kbd>GRAPH`).

## Questions for the planner

1. **R5 "unmodified":** I read it as "no Ctrl/Alt/Meta", the dispatcher's own gate. Shifted keys
   (`!`, capitals) already count as unmodified there. If Shift counts as a modifier, only `a d i p q r
   v w x` and unshifted punctuation (`\` `;` `'` `` ` ``) remain for five keys.
2. **R5 "one key per preset":** does a leader sequence (`w 1`) satisfy it? I assumed not, so presets
   get single keys and the leader is only for R10–R14.
3. **R9 focus by `o`/`c`/`h`:** it moves focus to the Inspector, so `c` then `j` no longer walks the
   Graph, which it does today. The text requires it, but it's a sequence-level change worth stating in
   PLAN.
4. **Inspector-wide keys on a collapsed Inspector** (`J K s n N y`): should they reveal it? Revealing
   doesn't change their meaning, and leaving it collapsed makes them silent no-ops.
5. **"Neighbour" for swap (R12) and move (R14):** the sibling node in the parent split (it may be a
   subtree, as in three's Graph pane) or the next pane in reading order? The keys above assume next and
   previous in reading order for move and focus, and the sibling for swap.
