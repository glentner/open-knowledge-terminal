# Research 02 — keys, navigation and history

Read at the worktree HEAD. All paths under `okt-client/src/` unless noted.

## 1. Key dispatch (`keys.rs` `on_key`, `:288`-`:585`; one `keydown` on `document`, `:645`)

Order of gates, first match wins:

| Gate | Keys | Effect | Where |
|---|---|---|---|
| any Meta/Ctrl/Alt held | all | `return` with no `preventDefault`: browser/OS defaults run (Cmd/Alt+arrows = back/scroll, Ctrl+arrows = macOS Spaces) | `:289` |
| focus in input/textarea/select/contenteditable | Escape blurs; rest ignored | `:292`-`:303` |
| `#help` open | Escape, `?` close; everything else swallowed | `:305`-`:313` |
| any view | `/` `:` focus GO | `:315` |
| view `files` | `views::tree::key` (j/k/↑/↓, ←/→ fold, space, u…) | `:321`, `views/tree.rs:362`-`:370` |
| view `log` | `views::log::key` (j/k/↑/↓) | `:325`, `views/log.rs:309` |
| `rev` + `travel_nav` set | `,` `.` older/newer commit | `:334`-`:344` |
| no `rev`, tracked file selected | `,` step back to last differing commit (async) | `:345`-`:375` |
| any view | Escape: graph walk stop → close settings → close commit → leave rev → deselect | `:376`-`:399` |
| view `graph` | `j`/`ArrowDown` walk +1; `k`/`ArrowUp` walk −1; Enter/space go to walk target, else folder expand/collapse (both) or content tab (Enter, file) | `:403`-`:433` |
| global | `g` `f` `l` views; `t` table; `o` `c` `h` tabs; `b` blame; `e` expand; `u` parent; `?`; `R`; `[` `history.back()`; `]` `history.forward()`; `0` `graph::fit()`; `J`/`K` scroll `#ibody`; `z` (switch to graph, `frame_selection()`); `s`; `y`; `n`/`N`; `m`; `1`-`9`; `E` `O` `F`; `+`/`=` `zoom_by(1.35)`; `-` `zoom_by(1/1.35)` | `:434`-`:580` |

`preventDefault` only on a handled key (`:430`, `:583`). **Shift is never tested anywhere in `keys.rs`**: dispatch is on `e.key()`, so Shift+↑ arrives as `"ArrowUp"` and Shift+h as `"H"`. Other keydown listeners: only `#go` (`go.rs:761`, input-focused). The graph canvas has no key listener (`graph.rs:2177`-`:2300`: wheel zoom, drag pan, click select/deselect, dblclick expand/fit).

## 2. Orbit-key proof (view `graph` active, no input focus, help closed)

| Candidate | Status | Proof |
|---|---|---|
| ArrowLeft / ArrowRight | **free** | Not in `:404`-`:428` nor `:434`-`:580`; tree's ←/→ (`tree.rs:364`) runs only when view is `files` (`:321`). Default: horizontal scroll of a focused scrollable (`#ibody` is `overflow:auto`, `app.css:321`; `body` is `overflow:hidden`, `app.css:93`) — suppressed once handled |
| Shift+ArrowUp / Down | **taken** | `e.key()` is `"ArrowUp"`/`"ArrowDown"` → walk at `:405`/`:409`. GOAL's suggestion collides; using it means gating the walk branch on `!e.shift_key()`, a meaning change R7 forbids unless the plan argues it away |
| Shift+ArrowLeft / Right | free | same as unshifted ←/→ (no branch); default Shift+arrow text selection only with a live selection/caret |
| `h` / `l` | taken | `h` history tab `:460`; `l` log view `:443` |
| `H` / `L` | free | no `"H"`/`"L"` branch (grep: only `go.rs:45` command alias strings) |
| `J` / `K` | taken | scroll inspector `:508` |
| `w` `a` `d` | free | no branch |
| `s` | taken | source/rendered `:524` (so WASD is out) |
| `,` / `.` | taken (conditionally) | travel `:334`; `,` history step `:345`; falls through unhandled otherwise — not free |
| `<` / `>` | free | `e.key()` is `"<"`/`">"`, no branch |
| PageUp / PageDown | free | no branch; default page-scroll of focused scrollable |
| Alt/Ctrl/Meta+anything | unusable | dropped at `:289`, owned by browser/OS |

Other free letters for reference: `a d i p q r v w x`, `A B C D G H I L M P Q S T U V W X Y Z`.

**Recommendation.** Azimuth: ArrowLeft/ArrowRight (Shift+←/→ optional coarse step, free). Elevation: no up/down arrow variant is free (plain and Shift walk; Alt/Ctrl/Meta are the browser's), so pick a non-arrow pair: PageUp/PageDown (on a Mac keyboard that is fn+↑/↓, so it rides the vertical arrows physically) — proved free, costs only page-scroll of a focused `#ibody`, which `J`/`K` already cover. Fallback letter pairs, all free but weaker mnemonics: `<`/`>` or `w`/`x` for elevation, `H`/`L` as an azimuth alias. If the plan prefers the GOAL's Shift+↑/↓, it must state the change (Shift+↑/↓ stops walking) explicitly for the reviewer. Insert orbit arms in the graph block (`:404`), before the global match, so they never reach `:434`.

## 3. The walk today

- **State:** `graph.rs:1829`-`:1837` `WALK: RefCell<Option<Walk{from, list, i}>>`; `G.cursor` (`graph.rs:655`) is the drawn index; paint is a dashed ring (`graph.rs:1689`).
- **Start:** first `j`/`k` (`graph.rs:1852`). No selection → toast. Builds `list = insp::walk_list(idx, sel)` when none or `from != sel`; `i: None`, so `j` → 0, `k` → last; wraps (`rem_euclid`).
- **Order source:** `insp.rs:486` `walk_list`: for a folder, subfolders then files in tree order; then out-edges by `ORDER`, then in-edges by `ORDER`; each node once, self excluded. Labels from `edge_label`.
- **Per step:** sets `cursor`; keeps the target in view by `focus_node(target, Some(k))` when within 60 px of an edge (`:1895`-`:1908`); `insp::mark_chip(Some(target))` (`:1910`); hint `"{at}/{of} · {label} · {name} · ⏎ go · esc"` (`:1915`).
- **Go:** `keys.rs:413`-`:415` `nav::select(Some(t), default)` → `walk_reset()` (`nav.rs:28`) → `focus_node` (420 ms, `nav.rs:56`) → `nav(true)` push (`nav.rs:71`).
- **Inspector mark:** `insp.rs:523` `mark_chip`: strips `.kb` from `#ibody .nchip.kb`, adds it to the first `.nchip[data-go=id]`, `scroll_into_view_nearest`. CSS `app.css:336`. Gaps: chips are capped at 40 per group (`insp.rs:587`, "+N more"), so a walk target past the cap gets no mark; `insp::render()` rebuilds `#ibody` and nothing re-applies the mark.
- **End:** Escape (`keys.rs:377` → `walk_stop` `:1932`), any `select` that moves (`nav.rs:28`), or `walk_target()` going stale because `from != sel` (popstate sets `sel` directly in `apply_hash_inner`, `nav.rs:217`-`:226`, without `walk_reset`; `G.cursor` stays until rebuilt/reset — latent).

**Angular order owned by the graph (recommendation).** Keep `insp::walk_list` as the *membership* (same set, labels, dedupe); have `graph::walk` sort it by on-screen angle around the selection's projected position (`atan2` of projected target − projected focus, clockwise from 12 o'clock, ties by distance then path), computed once when the walk starts (freeze the order for the walk so orbiting mid-walk doesn't reshuffle `i`; or re-sort and re-find the current target by id). Portals (targets in other regions): use the projected rim-cluster anchor as the target's screen point. The inspector already learns the row by **id**, not index (`mark_chip(Some(&target))`), so no order coupling is needed; add a re-mark after `insp::render()` (e.g. `insp::render` ends with `mark_chip(graph::walk_target().as_deref())`) and accept/flag the 40-cap gap. § 1: compute projections inside a `g()` borrow, drop it, then touch `WALK` and the DOM; never hold `g()`/`WALK`/`s()` across `mark_chip`, `nav::select` or `focus_node`.

## 4. History

- **Push/replace:** `nav.rs:158` `nav(push)`: no-op while `restoring`, or when the hash is unchanged (`:163`); `push_state_with_url(NULL, "", hash)` / `replace_state_with_url`. **State object is always `null`**; everything lives in the hash from `state_hash()` (`nav.rs:128`): `v`, `s`, `tab`, `c`, `rev`, `t` (table), `q` (list query). No camera.
- **Who pushes:** `select` (unless `from == "pop"`, `nav.rs:71`) — so every selection change pushes: graph click/deselect (`graph.rs:2256`/`:2266`), walk go, `u`, pins, GO, tree, lists, table. `set_view` pushes (`nav.rs:122`). `set_tab` and `s` replace (`nav.rs:84`, `keys.rs:537`). Not pushed: camera moves (`z`, `0`, `+`/`-`, wheel, drag), the walk itself, and the `[data-at]` path click (`keys.rs:99`-`:110` sets `sel` directly before `travel`).
- **Back/forward:** `[`/`]` → `history.back()`/`forward()` (`keys.rs:495`-`:502`); `[data-back]` button too (`keys.rs:210`).
- **popstate:** `keys.rs:642` → `spawn(nav::apply_hash())` (`nav.rs:175`): sets `restoring`, may `await` table/list loads (`:202`, `:209`), sets `sel` directly, `graph::update()`, then `graph::focus_node(n, None)` **only if** `sel` changed and view is graph (`nav.rs:229`-`:233`).
- **Hook for "ease back":** that `focus_node` call is the hook — replace it with the R3 camera solve for the new focus, eased (cut under reduced motion). Since the hash holds no camera, the deterministic R3 solve from the focus is enough for `[`/`]`. If the plan wants the exact prior camera (after manual orbit), stash it in the history **state object** with `replace_state` on the current entry just before each push, and read `history.state` in `apply_hash`; leave the hash format alone (so `h == location_hash` dedupe and shared URLs are unaffected). Also add `walk_reset()` in `apply_hash_inner` when `sel` changes. § 1: `apply_hash` awaits — take no `s()`/`g()` borrow across those awaits; read the camera after the last await.

## 5. "Results view active" (R4)

The Results view is the main view `"list"` (`views/list.rs:1`; `nav::set_view("list")` hides `#v-graph`, `nav.rs:97`). Detect with `s().view == "list"` **before** `keys.rs:518` switches to graph — capture it first. Matches live in `S.list: Option<Rc<ListData>>` (`state.rs:323`): `ListData::Find { files: Vec<SearchFile{path}>, names: Vec<String>, .. }` or `ListData::Paths { items: Vec<ListItem{id}> }` (`state.rs:264`-`:277`). Clone the `Rc` out of `s()` and drop the borrow before framing. Selecting a match: `views/list.rs:155` `select(from:"list")` → normal R3 path.

## 6. `?` list and README

- `?` is a **static Rust table** in `chrome.rs:469` `show_help()` (`:472`-`:495`), plus a hard-coded GRAPH `<dl>` in the `format!` (`:528`: "drag, wheel — pan, zoom"; "click", "double-click"). Rows to change: `:477` `("j k", "graph: step through the selection's connections")` → add angular order; `:478` `⏎ space` → warp; `:481` `z` → "frame the selection and links, or the Results matches"; `:489` `[ ]`; `:491` `+ − 0` → camera distance; add the orbit-key row; GRAPH dl "drag, wheel: pan, zoom" → orbit, zoom.
- README § Keys, `README.md:79`-`:82`: "`j`/`k` step through the selection's connections in the order the inspector lists them (a dashed ring, the card and the hint show where), Enter or space goes there, esc stops; with no walk, space opens or closes the selected folder and Enter shows a file's content; `z` zooms to the selection and its neighbors." Replace the order phrase, "goes there" (flight + `[` back), the `z` clause, and add orbit keys. README `:9`-`:11` ("draws the graph on a canvas … panning") may also drift.
- Doc comments restating the order: `keys.rs:400`-`:402`, `graph.rs:1849`-`:1851`, `insp.rs:484`-`:485`.
- `tests/ui/keys.mjs`: header `:4`-`:5`; GRAPH block `:92`-`:122` asserts `1/N`, `2/N`, wrap `N/N` via the hint and the target via the marked chip (`st.chip`) — still valid under angular order if hint and `mark_chip` keep their shapes; Enter test waits 800 ms (flight ~600 ms fits).
