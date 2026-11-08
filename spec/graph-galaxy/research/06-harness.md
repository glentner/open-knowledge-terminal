# 06 — Measurement and test harness (R11, R12)

Research for the plan step, 2026-11-08, at the worktree's HEAD. Both runs below were made through
`tests/env.sh` on 6581, on mac-m4 (Apple M4 Max).

## Baseline runs

| Run | Result | Wall |
|---|---|---|
| `tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"'` | 33/33 ok, exit 0 | 70 s (incremental build: wasm 24 s, server 14 s) |
| scratch rAF probe, headed | idle p50 6.9 / p95 7.3 ms; drag p95 7.2, max 34.7; wheel p95 7.1; walk+Enter p95 7.2; `__oktDraw` p95 1.3–1.5 ms | 33 s for both runs, with a warm build (0.1 s) |
| same probe, headless | idle p50 16.7 / **p95 16.7**; drag **p95 16.8**; wheel **p95 16.8** | (same) |

- **Headed:** WebGL2 renders through `ANGLE Metal Renderer: Apple M4 Max`. rAF ticks at about 144 Hz
  (6.9 ms), so this display is high-refresh. The current 2D graph sits far under the bound.
- **Headless:** WebGL2 renders through **SwiftShader**, and rAF is pinned at 60 Hz. **Vsync jitter
  alone fails the bound (16.8 > 16.7) on an idle page.** A 16.7 ms p95 gate therefore means
  nothing headless, or on any 60 Hz display. This backs the GOAL's "headed" clause and needs a
  rule in the harness (below).
- **Playwright resolution:** both suites use `await import(process.env.PLAYWRIGHT_MODULE ||
  'playwright')` (`profile.mjs:4`, `keys.mjs:9`). `$WT/tests/node_modules` is absent, and
  `NODE_PATH` does not apply to ESM imports. Two fixes work without changing any tracked file:
  `export PLAYWRIGHT_MODULE=<main checkout>/tests/node_modules/playwright/index.mjs` (Playwright
  1.65.0; this is what I used), or `npm install` in `$WT/tests`. That path is gitignored
  (`.gitignore:3`, confirmed with `git check-ignore`). The run also needs `WASM_BINDGEN` at the
  pinned version and `PLAYWRIGHT_BROWSERS_PATH` (chromium-1254).
- **Sandbox:** env.sh binds a local port, so verify lines must run with the sandbox off. A
  concurrent worker held 6581 once: env.sh exited 2 ("port 6581 is busy"), and a later retry
  worked.
- **Cleanup check:** `pgrep -fl okt-server` never matches the throwaway server, whose binary is
  `target/fast/okt`. Use `pgrep -fl 'target/fast/okt'; pgrep -fl ms-playwright`. Both came back
  empty after every run here.

## 1. `tests/perf/profile.mjs` today

- It measures both things. **rAF pacing:** an in-page loop pushes `performance.now` stamps
  (`:23-29`), and the percentiles come from the intervals between them (`:47,53`; `pct()` at `:35`,
  floor index, `toFixed(1)`). **Main-thread cost:** CDP `Performance.getMetrics` deltas give
  Task/Script/Style/Layout time (`:31-33,49`). It also records long tasks, slow events (>16 ms)
  and `window.__oktDraw`. `__oktDraw` is the client's own draw-time hook (`graph.rs:1383-1395`: if
  a test has put an array on `window`, it gets the last 600 draw durations). This is the existing
  precedent for a test hook that needs no wasm-bindgen export.
- **Launch:** `chromium.launch({ args: [...] })` (`:14`) has no `headless` option, so it runs
  **headless**. Extra flags come from `$FLAGS`. Playwright's defaults already include
  `--disable-background-timer-throttling`, `--disable-backgrounding-occluded-windows` and
  `--disable-renderer-backgrounding` (seen in the process list). Viewport `1920x1080@1` from argv.
- **Output:** one JSON line per scenario on stdout, plus `results-<label>.json` in `$OUT`.
  `compare.py` prints a side-by-side table of two runs. It applies no threshold and gives no
  PASS/FAIL.
- **Broken as it stands:** `points.json` is 0 bytes, so `JSON.parse` at `:67` throws before the
  first scenario runs. The default target is **6580** (`:7`), and so is `keys.mjs:11`'s. Both are
  safe only while `OKT` is passed in. The scenarios (`pan_drag`, `click_select` on fixed screen
  points, `#tree`/`#ltable`) are written for the 2D graph.

**Recommendation:** write a new `tests/perf/graph.mjs` rather than adding modes to `profile.mjs`.
Leave `profile.mjs` alone; repairing `points.json` is out of scope.

### `tests/perf/graph.mjs`: proposed spec

- **Target:** `$OKT` only. Exit 2 if it is unset or names port 6580.
- **Headed by default.** `--headless` (or `HEADLESS=1`) runs a smoke test: it prints the numbers
  but gives **no verdict** and exits 0, because the headless numbers are SwiftShader at 60 Hz.
- **Context:** `viewport 1600x1000` and `deviceScaleFactor: 2`. Real use is on a Retina display,
  and fill cost scales with pixels; the baseline above ran at dpr 1. Allow `--dpr` to override.
- **Recording:** inject an in-page rAF loop with `page.addInitScript` and keep two buffers:
  - all frame intervals while a scenario is active;
  - `anim` intervals, recorded only while `#graph[data-anim="1"]` (hook below), so idle vsync
    frames don't dilute the warp p95.

  Also collect `__oktDraw` and long tasks.
- **Calibrate:** take 1 s idle after load and use the median interval as `refresh_ms`; print it.
  - If `refresh_ms > 12`, the display is about 60 Hz: print `WARN 60Hz: p95 is jitter-limited`,
    and also report `dropped` (intervals > 1.5 × refresh). On a 60 Hz display, 16.7 ms is one
    vsync, and jitter crosses it. On 120/144 Hz, 16.7 ms allows one missed vsync.
  - Run the gate on the high-refresh display measured here, where Chromium opens.
- **Warm-up:** wait for `#graph[data-layout]` and `data-anim="0"`. Then run one untimed orbit pass
  (1 s) and one untimed warp, to compile shaders and warm the JIT.
- **Scenario `orbit` (≥ 5 s, ≥ 600 frames at 144 Hz):**
  - Playwright's `keyboard.down` does not auto-repeat, so repeat `page.keyboard.press` on the plan's
    azimuth key every 16 ms for 2 s, then on the elevation key for 1 s.
  - Then 2 s of pointer drag: `mouse.down` followed by `mouse.move(..., {steps})` in a circle.
- **Scenario `warp` (≥ 6 round trips):** select the most-connected card (`keys.mjs:22-24` has the
  method) and wait for anim 0. Then repeat six times: `j`, `space`, wait anim 0, `[`, wait anim 0.
  Alternate space and Enter. Use the `anim` buffer: 12 eased moves × ~600 ms is about 1000 frames
  at 144 Hz. A single 600 ms warp gives only about 86 frames, too few for a stable p95.
- **Output:**
  - one JSON line per scenario: `refresh_ms, frames, p50, p95, max, dropped, draw_p95, longtask_ms`;
  - then `PASS orbit p95=… ≤ 16.7`, or `FAIL …`;
  - exit 1 on any FAIL.

  Compute percentiles with `profile.mjs`'s `pct()` and round to 0.1 ms the same way.
- **Loaded runner:**
  - The bound stays 16.7 ms. env.sh's port check already makes runs mutually exclusive (exit 2
    when busy), but cargo builds in other worktrees still load the CPU.
  - Retry once on FAIL and report both runs, or take the median of 3. The plan decides.
  - Close the browser in `finally`.

## 2. `tests/ui/keys.mjs` today

**How it asserts:** DOM state through `state()` (`:36-54`). It reads:
- `#tree` row classes, the view from the `#v-*` `hidden` flags, and the selection parsed from
  `location.hash` (`s=`);
- the `#ghint` text, the inspector chip `#ibody .nchip.kb[data-go]`, and the `.toast` text.

It waits with fixed `waitForTimeout` sleeps (`key()` 300 ms, `go()` 700 ms). Checks print `ok` or
`FAIL`, and the exit code is the failure count (`:187`). The browser is closed outside any
`finally` (`:186`).

**Graph assertions at risk in the rewrite:**

| Line | Check | Depends on |
|---|---|---|
| `:90` | space on a file shows the graph | `#v-graph` not hidden |
| `:96-97` | j starts the walk | `#ghint` text `^1/N · ` and `#ibody .nchip.kb` |
| `:101` | j steps to the 2nd | `#ghint` `2/N · ` |
| `:105` | k wraps to the last | `#ghint` `N/N · ` |
| `:108` | esc stops the walk | `#ghint` lacks ` · ⏎ go`, no chip, selection kept |
| `:114` | Enter goes to the marked target | hash `s=` equals the chip's `data-go`, after a fixed 800 ms |
| `:119` | space toggles the folder in the graph | toast `EXPANDED\|COLLAPSED` (keys.rs:415-421) |
| `:122` | z frames | page errors only; nothing is asserted |

**Keep these elements so the checks survive:**
- `#ghint` (made in `graph.rs:1148-1162`) and its `i/N · … ⏎ go` text format;
- the `.nchip.kb` class on the inspector chip that marks the walk target, which is also R5's
  "inspector marks the row".

The checks at `:114` and `:119` need waits on the anim flag instead of sleeps, because a warp takes
about 600 ms.

**What the client exposes for tests today:** only `__oktDraw`. The `data-*` attributes in
`okt-client/src` are UI routing (`keys.rs:29-267`, `insp.rs`). There is no camera state, and no
code reads `prefers-reduced-motion`; the only use is CSS, at `app.css:513`.

### Hook: `data-*` attributes on `#graph`, the GL canvas

Recommended over a `window.__okt_graph` getter:
- Attributes are DOM-native, cost nothing to read, and work with `waitForFunction` and
  `MutationObserver`. Playwright can wait on a selector such as `#graph[data-anim="0"]`.
- A live getter would need a wasm-bindgen export or a `Closure` on `window`.

Rules:
- Write the attributes at the end of a frame, and only when the rounded string has changed.
- No CSS selects on them, so a change costs no style recalculation.
- Keep `__oktDraw`.

| Attribute | Value |
|---|---|
| `data-scale` | `galaxy` \| `system` \| `focus` |
| `data-theta`, `data-psi` | degrees, 1 decimal |
| `data-dist` | camera distance, 3 significant figures |
| `data-anim` | `1` while any camera ease is running, else `0` |
| `data-focus` | focused path (or region) |
| `data-focus-n` | focus-set size |
| `data-walk` | current walk-target path, empty with no walk |
| `data-walk-i` / `data-walk-n` | walk position |
| `data-walk-angle` | the target's on-screen angle around the focus, degrees clockwise from 12 o'clock |
| `data-layout` | hash (FNV-1a) of the quantized layout positions, computed once per layout build |
| `data-labels` | card labels drawn in the last frame (R9: at most 24) |
| `data-drawn` | cards drawn (R8: show-all off is less than the total) |
| `data-showall` | `0` \| `1` |

### R12 checks to add

Wait with `page.waitForSelector('#graph[data-anim="0"]')` after each key.

- **`z`:** from a card, read `{theta, psi, dist, layout}`, orbit once so the camera is off its
  solved angle, then press `z`. Pass when theta, psi or dist changed, and `data-layout` is unchanged
  (R4). A second case: `/FIND <word>`, Results view active, then `z`; `data-focus-n` equals the
  match count.
- **`j`/`k` angular order:** start a walk on the hub and step `j` up to min(N, 12) times, recording
  `data-walk-angle`. Pass when the sequence is non-decreasing (at most one descent at the wrap). Keep
  the `1/N`, `2/N` and `N/N` hint checks. The camera must stay still during j/k, or the order must
  freeze at walk start; the plan picks which.
- **Warp on space, and on Enter:** record `history.length` and the camera, then press `j` and
  `space`. Pass when:
  - the hash `s=` equals the old `data-walk`;
  - `history.length` went up by 1;
  - `data-anim` was seen at `1` (a `MutationObserver` in `addInitScript` records it).

  Repeat with Enter.
- **`[` after a warp:** pass when the hash `s=` is back to the hub, `data-focus` equals the hub, and
  theta, psi and dist each match their pre-warp values within 0.5. `]` goes forward again.
- **Orbit keys:** each of the plan's keys changes theta (azimuth keys) or psi (elevation keys).
  `data-layout`, the selection and `data-walk` stay unchanged. Plain ArrowUp/ArrowDown still walk
  (`keys.rs:405-411`), so the elevation keys must be matched before that arm.
- **Reduced motion:** use a second context, `browser.newContext({ reducedMotion: 'reduce' })`, with
  the MutationObserver installed. Select a card, warp, then `[`. Pass when `data-anim` never became
  `1`, and the end state is there one frame after the key (`waitForFunction` with a 100 ms timeout).
  The client has to read `matchMedia('(prefers-reduced-motion: reduce)')`, which is new code.

## 3. env.sh rules a `verify:` line must respect

- **Run it from the worktree root.** It calls `git rev-parse --show-toplevel`, so `$OKT_ROOT` is
  the worktree.
- **Paths:** the command runs in `$site/run`, a temp dir, so relative paths break. Single-quote the
  `sh -c` body so `"$OKT_ROOT"` expands inside it; that is one quoting level.
- **Exit codes:**
  - 2 means a busy port or a usage error: retry, don't count it as a test failure;
  - 1 means the server didn't come up;
  - anything else is the command's own status;
  - `set -eu`, so a build failure aborts with make's status.
- **Builds every time:** `make client`, then `cargo build --profile fast`. That took 38 s
  incremental and 0.1 s warm; a cold target is untimed. `make client` fails fast unless
  `WASM_BINDGEN` matches the pinned version.
- **Clones `~/KB` every run,** with `--no-hardlinks`; that is fast.
- **Environment it needs:** `WASM_BINDGEN`, `PLAYWRIGHT_BROWSERS_PATH`, and `PLAYWRIGHT_MODULE` (or
  `npm install` in `$WT/tests`). The sandbox must be off for the local bind.
- **Cleanup:** env.sh stops the server on every exit path. The browser is the script's job, so close
  it in `finally`.

## Proposed verify lines

```sh
tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"'
tests/env.sh sh -c 'node "$OKT_ROOT/tests/perf/graph.mjs"'
tests/env.sh sh -c 'node "$OKT_ROOT/tests/perf/graph.mjs" --headless'
pgrep -fl 'target/fast/okt'; pgrep -fl ms-playwright
```

1. Line 1 is R12, and runs headless.
2. Line 2 is R11. It is headed, and gives a verdict and exit code.
3. Line 3 is a smoke test only: numbers, no verdict.
4. Line 4 must print nothing.
