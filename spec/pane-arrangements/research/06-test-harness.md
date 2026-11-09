# 06 — Test harness: the rect suite, keys.mjs, phase gates

Research for `code-plan`, 2026-11-08, measured on the prototype worktree (detached at `edcbed4`,
same code), port 6585, mac-m4. Probe scripts lived in a temp dir and are gone.

## 1. Measured timings

| Part | Cold (empty `target/`) | No source change | After a client edit (derived) |
|---|---|---|---|
| `make client` (wasm-release + bindgen + copy) | 26.6 s (okt-client crate alone ≈ 12 s) | 0.1 s | ≈ 13 s |
| `cargo build --profile fast -p okt-server` (embeds `okt-client/dist` via rust-embed, `http.rs` `#[folder]`) | 14.3 s | 0.2 s | ≈ 3–4 s (okt-server crate only) |
| `git clone` of ~/KB | 1.9 s | 1.8 s | |
| server answering `/api/index` | < 0.1 s | | |
| `keys.mjs` (33 checks, all ok) | 29.8 s | | |
| **env.sh + keys.mjs total** | **72.9 s** | **≈ 32 s** | **≈ 50 s** |
| `cargo test -p okt-server -p okt-pygments -p okt-client` | 18 s (build 12.5 s) | 4 s | |

Browser costs (headless Chromium 1254): launch 89 ms; a new context + load + ready ≈ 1.2 s
(ready — `#ihead` has text and `#graph.width > 0` — lands ~90 ms after `load`); a reload to ready
≈ 0.5 s; a full open/measure/key/reload/close cycle 1.65 s (dpr 1), 1.97 s (dpr 2).

`cargo test -p okt-client` compiles and runs natively today (6 tests in `js.rs`, 1 in `model.rs`):
`crate-type = ["cdylib"]` and web-sys don't block a native test build; web-sys calls would only
panic at run time. So the arrangement tree (presets, split/collapse/swap/flip/close/move, the
`insp` migration, serde round trip) should be a pure module — std + serde only, no web_sys — and
its rules unit-tested natively, cheaply (warm 4 s).

## 2. The rect suite (`tests/ui/panes.mjs`, or `arrangement.mjs`)

**Setting a preset.** Press its key: the real path, and it exercises the save. Wait for layout by
polling until every `[data-pane]` rect is unchanged across two `requestAnimationFrame`s, not a
fixed sleep. Seed localStorage only for states real users have (the old `insp` settings); never
inject an arrangement or DOM state the app wouldn't produce (graph-galaxy's hollow gate set
`tabIndex` on `#ibody`).

**The storage key** is `skey()` in `state.rs`: `"okt:" + boot().kb` (or `"okt:kb"` if empty);
`boot().kb` is the KB root's basename (`http.rs` boot JSON). In env.sh the clone is `$site/kb`,
so the key is **`okt:kb`** (probed). Derive it in-page:
`'okt:' + (JSON.parse(document.getElementById('boot').textContent).kb || 'kb')`.
Nothing is stored until a setting changes (`save_settings`, debounced 200 ms), so a fresh
context is "no settings saved". To seed: `goto(OKT + 'api/index')` (same origin, app not
running), `localStorage.setItem`, then `goto(OKT)`. Don't use `addInitScript` for it: it re-seeds
on every reload.

**DOM contract** (hooks to *find* things; the grade comes from rects):

- `.pane[data-pane="<stable id>"]` with `data-views="graph files …"`, `data-shown`,
  `data-collapsed="0|1"`, `data-focused="0|1"`; children `.ptitle` (title bar, holding one
  `button[data-view]` per tab) and `.pbody`.
- `.psplit[data-orient="row|col"]` for each split, with its handle.
- The arrangement container keeps `#panes`; view roots keep `#v-graph`, `#v-files`, `#v-log`,
  `#v-table`, `#v-list`, `#insp` and every id `keys.mjs` reads (`#tree`, `#ibody`, `#ghint`,
  `#itabs`, `#ffilter`, `#toasts`, `#go`).

**Grading from the rendered page** (every attribute is cross-checked; a mismatch fails):

- *visible(e)* = `e.checkVisibility({checkOpacity:true, checkVisibilityCSS:true})` and rect
  width, height > 0.
- *Pane geometry*: pane rect vs the preset's expected rect, computed from `#panes`' rect (don't
  hard-code the 66 px top: header + `.fkeys` today, may change), PLAN's fractions, the 5 px
  handle and the title-bar height; within 2 px.
- *Views held*: the visible tab buttons whose rects lie inside that pane's `.ptitle` rect, by
  `data-view` **and** text — must equal `data-views`.
- *Shown view*: the one view root that is visible and whose rect lies inside the pane's `.pbody`
  rect (±1 px) — must equal `data-shown`; every other root it holds is not visible.
- *Collapsed*: stacked — pane height = `.ptitle` height ±1 and no visible view root inside;
  side by side — pane width = strip width ±1, title rect taller than wide. Must equal
  `data-collapsed`.
- *Focused*: exactly one `data-focused="1"`, and its `.ptitle` computed style differs from the
  others (e.g. border colour), so the mark is drawn, not just set.
- *No overflow*: for each `.pbody` and for `#tree`, `#ibody`: `scrollWidth ≤ clientWidth`;
  also `documentElement.scrollWidth ≤ innerWidth`.

**"three"** (1600×1000 and 900×1000, after selecting the hub card from `keys.mjs`'s method):
the Graph pane's `.ptitle` has `left ≤ #panes.left + 1`, `width ≥ #panes.width − 2`,
`top = #panes.top ± 1`, and the pane is title-bar tall with `#graph` not visible. Inspector pane
`left = #panes.left ± 1`; `#insp` visible inside it; `#ihead` innerText contains the hash's `s=`
id; the three `#itabs button[data-tab]` (O/C/H) visible inside the pane. Files pane to the right
(`left ≥ inspector.right`); `#ffilter` visible inside it; **≥ 10** `#tree .trow:not(.thead) .nm`
whose `span.t` has width > 0, non-empty text, and a rect wholly inside the pane's `.pbody` and
`#tree`'s client box. Probe today: 32 such rows at 890 px tall, 21 at 445 px, so 10 has room.

## 3. Reload, migration, first load

- **Reload**: after each preset (and after one each of collapse, split drag via `page.mouse`,
  swap, flip, close, move), wait for the save — poll `localStorage.getItem(key)` until it
  changes, since the debounce is 200 ms — then record per pane `{rect, views, shown, collapsed}`
  (rect-derived as above, sorted by top, left), `page.reload()`, wait for ready, record again,
  compare: same panes, same views/shown/collapsed, every rect within 1 px. Today's layout
  reloads to identical rects (probe: `#view` 859, `#insp` 736 before and after, integer px).
- **Migration**: seed `{"theme":"terminal","insp":30}` (no arrangement key), load at 1600×1000:
  the Inspector pane's width = 30 % of `#panes` (±2 px) and the left pane takes the rest minus
  the handle. Today's code gives exactly 480 px (probe).
- **First load** with an empty store: 899×1000 → stacked (two stacked panes, equal ±2 px), 900×1000
  → classic (Inspector 46 %). Today both are stacked (`max-width: 900px` matches 900), so 900 is
  the boundary case that proves the media query is gone. Also resize a saved classic page to
  800×1000 with `page.setViewportSize` and check that the panes, orientation, views and collapsed
  states are unchanged.
- **History**: any back/forward check opens a fresh page; `history.length` caps at 50 (probed:
  70 `pushState`s → 50).

## 4. Graph and Files at once

Measured on today's page: a **hidden** Graph keeps its backing store and its drawn pixels
(Files shown: canvas CSS 0×0, backing 859×890, 336 825 non-transparent pixels). So neither a
pixel count nor a backing-size check alone proves the Graph is on screen. Require all of:

1. `#graph` visible, its rect inside the Graph pane's `.pbody` ±1;
2. `canvas.width = round(rect.width × dpr)` and height likewise (±1) — run this context at
   `deviceScaleFactor: 2` (probe: 1718×1780 for 859×890);
3. `document.elementFromPoint` at 3 points inside the rect returns the canvas (not covered);
4. drawn pixels: `getImageData` over the full backing store, count alpha > 0 (the client clears
   with `clear_rect`, so background is alpha 0) — require > 1 % (today 44 %);
5. it responds: hash the pixel buffer, wheel over the canvas or select another card, wait two
   frames, hash differs.

Files beside it: `#tree` visible inside its pane, wheel over it → `scrollTop` grows; `j` after
clicking a row moves `.trow.on`; then the canvas still passes 1–4. Repeat 1–2 after a split
drag, a neighbour's collapse/expand, a preset change and `setViewportSize` (canvas fits its pane).
Never trust `__oktDraw` or any client-written "drawn" flag for this.

## 5. Gate structure and footprint

- One env.sh call per gate so the build is paid once:
  `tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"; a=$?; node "$OKT_ROOT/tests/ui/panes.mjs" $GROUPS; b=$?; [ $a -eq 0 ] && [ $b -eq 0 ]'`
  (both run; `&&` would hide the second suite's failures).
- One `chromium.launch` per suite, one `newContext` per scenario (fresh storage), browser closed
  in `finally`. Headless is fine: nothing here grades frame time.
- `panes.mjs` takes behaviour-named groups for phase-only runs: `presets`, `reload`, `first-load`,
  `migration`, `together` (Graph+Files), `three`, `operations`, `fit`. Each check prints
  `ok   <label>` / `FAIL <label>: …`; the gate also requires each phase's labels to be printed
  (archive's `checks.txt` pattern), so a deleted check can't pass silently. Labels describe
  behaviour, never ids.
- Estimate: 8 preset×size layouts at ≈ 2.5 s (apply, grade, reload, grade) ≈ 20 s; first-load,
  migration, together, three, operations ≈ 20–30 s more → **suite ≈ 45–60 s**. Phase gate after
  a client edit: static < 1 s + cargo test ≈ 10 s (incremental) + build ≈ 17 s + clone 2 s +
  keys.mjs 30 s + panes.mjs ≤ 60 s ≈ **2 min**; cold ≈ 2.5 min. Under the 5 min target and the
  600 s Bash cap.
- Gate honours `$OKT_TEST_PORT` (the archive's port poll hard-coded 6581); exits 75 on a busy port,
  never "red". Leftover-process check matches `--kb $TMPDIR/okt-env.` and `ms-playwright` (the
  server binary is `target/fast/okt`, so `pgrep okt-server` never matches); `pgrep` and `del` need
  the sandbox off.
- **keys.mjs stays green** if classic stays the first-load default at 1600×1000 (it holds all five
  views on the left, so every existing check sees today's behaviour) and the ids above survive.
  Its one fragile read is `state().view` — the first `#v-*` without `hidden`. Once two views can
  be on screen, change it (allowed: "selectors may change") to the visible view root in the
  `data-focused` pane, keeping every expected outcome. `profile.mjs`'s fixed points
  (`points.json`, 1920×1080) and `soak.mjs` also assume classic.

## 6. Static checks and the ledger (last phase)

**Spec-id scan**, added lines only:
`git diff -U0 <oldest plan commit>..HEAD -- okt-client tests | grep '^+' | grep -v '^+++'`, plus
`git diff --name-only --diff-filter=A` for file names. Regex (ERE, case-sensitive classes):

    (^|[^A-Za-z0-9])[RrPp][0-9]{1,3}([^A-Za-z0-9]|$)

after deleting percentile keys `(^|[^A-Za-z0-9])[Pp](50|90|95|99)([^A-Za-z0-9]|$)` (apply the sed
twice; adjacent matches overlap). Don't use `\b`: `_` is a word character, so `\br10\b` misses
`r10_name` (verified). Tested against all 13 085 lines ever added under `okt-client/` and `tests/`:
only 7 hits, all geometry locals in `graph.rs` (`r0` in `place_files`, `p92`, `p1`/`p2` in
`ease_out`'s bezier), none a spec id; hex colours never match (no r/p in hex). Those lines are
not in this cycle's diff, but pane geometry is exactly where `r1`, `r2`, `p0`, `p1` would be
written: the plan should forbid `[rp]<digit>` names (use `a`/`b`, `lo`/`hi`, `ra`) rather than
allowlist them.

**Deferral ledger** (`verify/ledger.sh`): the archive's version passes on its own final text and
its awk is reusable — units are paragraphs and list items with continuation lines joined; drop
frontmatter, headings (REVIEW's "Out of scope (recorded, not blocking)" tripped a ledger before,
KB inbox 2026-10-31) and fenced code. Add: strip inline code spans before matching, so amendment
text quoting the vocabulary (`` `defer` ``) can't trip it; remove negations ("not deferred",
"nothing deferred", "no deferral") before re-matching; skip units naming `verify/ledger.sh` (the
checklist line describing the check) instead of any unit containing "deferral ledger". Every
remaining unit names `issues/<slug>.md` that exists, has a valid `status:`, and is in
ROADMAP.md. Give it a `--self-test` that writes fixtures to a `mktemp -d`: an unseeded deferral
(must fail), the REVIEW heading, a "fixed, not deferred" triage line and a backticked amendment
(all must pass).
