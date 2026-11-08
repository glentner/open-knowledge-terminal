# 05 — Renderer benchmark (binding input to TECH)

Ruling (GOAL → Clarifications, Renderer): choose (a) Canvas 2D with software projection and depth sort, or (b) WebGL2 via `web-sys` with a 2D label overlay. The choice rests on frame time with the full live KB, on mac-m4, in headed Chromium. WebGL2 wins if both pass p95 ≤ 16.7 ms.

**Verdict: both pass by a wide margin, so WebGL2 is the choice.** WebGL2 also has the headroom: its JS cost per frame stays flat at about 0.1 ms whatever the scene size. Canvas 2D JS cost grows with the card count and starts missing vsync at 10× the KB.

## Setup (2026-11-08, two identical runs, run 2 reported)

- **Host:** mac-m4. **GPU path:** `ANGLE (Apple, ANGLE Metal Renderer: Apple M4 Max, Unspecified Version)`, read through `WEBGL_debug_renderer_info`. That is hardware Metal, not SwiftShader. `ALIASED_POINT_SIZE_RANGE` is [1, 511].
- **Browser:** Playwright Chromium 158.0.8127.18, `headless: false`. Launch args: `--disable-renderer-backgrounding --disable-backgrounding-occluded-windows --disable-background-timer-throttling`, which stop the window being throttled when it is behind others. GL is not forced either way.
- **Display:** main display is an external Dell Alienware AW3225DM at 2560×1440. The idle rAF delta is p50 6.9 ms and min 6.2 ms, so the display runs at **≈144 Hz** and the rAF floor is 6.94 ms. The bar stays at 16.7 ms p95. A p95 above 6.94 ms on this display means a vsync was missed, which is not a failure.
- **Canvas:** 1200×900 CSS px at `deviceScaleFactor: 2`, so the backing store is 2400×1800. DPR is emulated because the panel's native DPR is 1, but the backing store is drawn at full size either way.
- **Content:** the `tests/env.sh` clone of `~/KB`, served on 6581 and fetched same-origin from `/api/index`. It holds **1269 files, 149 folders, 3361 edges** (1427 non-root nodes in all) and **23 regions** (top-level folders). The `edges` entries are arrays `[source, target, kind, weight, …]`. Every edge resolved to two nodes.
- **Layout stand-in:**
  - Regions sit on a Fibonacci sphere.
  - Within a region, cards are scattered over a 0.45 rad cap with radius 0.5 + 0.18·depth.
  - Size comes from type and √degree.
  - The orbit turns θ at 0.0006 rad/ms with a 0.35 rad ψ wobble.
- **Scenarios:**
  - **show all:** every node and every edge, plus 24 labels.
  - **tiered focus:** the largest region drawn in full (`workspace/`, 710 cards, which is the worst case), 22 cluster glyphs for the other regions, 200 edges touching the region, and 24 labels.
  - **stress:** show all with the KB replicated 4× and 10× (jittered copies), to measure headroom.
- **Timing:** 1 s warm-up, then 5 s recorded (3 s for stress). Two measures are taken:
  - **rAF delta** is frame-to-frame time. It includes GPU and compositor time.
  - **draw** is `performance.now()` around the JS draw call. It is JS only: GPU raster for both Canvas 2D and WebGL happens off-thread and shows up only in rAF.
- A pixel check after each scenario confirmed the frames really rendered (lit fraction 0.05–0.29).

## Numbers (ms)

| renderer | scenario | frames | rAF p50 | rAF p95 | rAF max | draw p50 | draw p95 | draw max |
|---|---|---|---|---|---|---|---|---|
| Canvas 2D | show all (1427 n / 3361 e) | 721 | 6.9 | 7.6 | 7.9 | 1.8 | 2.5 | 3.4 |
| Canvas 2D | tiered focus | 721 | 6.9 | 7.8 | 7.9 | 0.8 | 1.1 | 1.5 |
| WebGL2 + overlay | show all | 720 | 6.9 | 7.8 | 7.9 | 0.1 | 0.3 | 0.3 |
| WebGL2 + overlay | tiered focus | 721 | 6.9 | 7.8 | 7.9 | 0.1 | 0.2 | 0.5 |
| Canvas 2D | stress 4× (5708 / 13444) | 433 | 6.9 | 7.7 | 7.8 | 3.2 | 3.5 | 3.6 |
| WebGL2 + overlay | stress 4× | 433 | 6.9 | 7.7 | 7.8 | 0.1 | 0.2 | 0.4 |
| Canvas 2D | stress 10× (14270 / 33610) | 403 | 6.9 | **13.9** | 14.6 | 7.1 | 7.6 | 8.1 |
| WebGL2 + overlay | stress 10× | 433 | 6.9 | 7.5 | 7.8 | 0.1 | 0.2 | 0.3 |

Run 1 agreed with these figures to within 0.5 ms on every cell. For example, Canvas 2D show all drew at p50 0.8 / p95 2.1 ms, and every rAF p95 was ≤ 7.7 ms.

Reading the table:
- On the real KB, every case holds 144 Hz, about 0.4× the 16.7 ms budget.
- Canvas 2D JS cost grows roughly linearly, at about 0.5 ms per 1k nodes plus edges. At 10× it already drops about 7% of vsyncs at 144 Hz, though it is still under 16.7 ms.
- WebGL2 does not grow measurably, because positions sit in a static VBO and the camera is one `mat3` uniform.

## Verdict

**Both candidates pass on the full live KB, in both scenarios: rAF p95 is 7.6–7.8 ms against the 16.7 ms bar. Under the ruling, TECH SHALL choose WebGL2 via `web-sys` with a 2D overlay canvas for labels.**

The margin is what matters for the planned features. A real glow pass, label-collision work and picking will all eat into the frame, and in WebGL2 that budget goes to the GPU. In Canvas 2D every extra effect is JS time on the main thread, alongside the app's other work.

## What the stand-in does not model (caveats)

- **Glow.** WebGL draws a single-pass soft sprite: `exp` falloff plus a smoothstep core, with additive blending. Canvas 2D draws a halo circle plus a core circle, batched into Path2D per depth band and region colour. Neither renderer models a real bloom pass (an offscreen FBO, blur, composite). A bloom pass at 2400×1800 is a few full-screen passes, cheap on an M4 Max. In Canvas 2D a comparable effect would need `shadowBlur` or per-node gradients, which cost far more.
- **Depth.** WebGL uses additive blending with no depth test, which is order-independent, so it needs no sort. Canvas 2D sorts every frame (`Array.sort` over the node count), and the sort is part of its cost. If TECH wants occluding alpha blending in GL, it needs a back-to-front sort or depth pre-pass. The sort is about 1 ms of JS at 1.4k nodes.
- **Labels and picking.** Labels are 24 `fillText` calls on the overlay, with no collision pass (R9). There is no picking (hit-testing) and no hover state. Both run in JS whichever renderer is chosen, over about 24–1.4k projected points, so they add to both renderers equally.
- **Lines.** Edges are 1 px `GL_LINES`, and Canvas 2D strokes them at `lineWidth = dpr`. Thick or curved bundle edges would need quads or triangle strips in GL.
- **Rest of the app.** The bench runs the renderer alone, with no okt client wasm on the page. Main-thread contention from the app (DOM, SSE) is not modeled, and the R11 measurement on the real client covers it.
- **Display.** The bench ran on a 144 Hz external panel. On a 60 Hz panel the rAF floor would be 16.7 ms and the p95 bar would bind on dropped frames only.
- **Context loss.** The bench does not handle `webglcontextlost` (sleep or wake, GPU switch). The client must restore the context from its static buffers.

## web-sys features WebGL2 would add (okt-client/Cargo.toml)

The client already has `HtmlCanvasElement`, `CanvasRenderingContext2d` and `Path2d`, and `js-sys` is a dependency (for `Float32Array`/`Uint32Array` views when calling `buffer_data_with_array_buffer_view`).

- **Needed:** `WebGl2RenderingContext`, `WebGlProgram`, `WebGlShader`, `WebGlBuffer`, `WebGlUniformLocation`, `WebGlVertexArrayObject`, `WebGlContextAttributes` (antialias and alpha flags for `get_context_with_context_options`), and `WebGlContextEvent` (context loss and restore).
- **For a bloom or label atlas:** `WebGlTexture`, `WebGlFramebuffer`, `WebGlRenderbuffer`.
- **Optional diagnostics:** `WebglDebugRendererInfo`, `WebglLoseContext`, `WebGlActiveInfo`.

All of these features exist in **web-sys 0.3.106**, which is locked with js-sys 0.3.106 and wasm-bindgen 0.2.129 (Cargo.lock). That is a matched set, so the wasm-bindgen pin needs no bump.

## CSP finding (okt-server/src/http.rs `page_csp`, read only)

The page CSP is:

```
default-src 'self'; script-src 'self' 'wasm-unsafe-eval' <hashes>; style-src 'self' 'unsafe-inline';
img-src 'self' data: blob:; connect-src 'self'; …; object-src 'none'
```

**WebGL2 needs no CSP change.** No CSP directive governs `getContext('webgl2')` or shader compilation, and wasm already has `'wasm-unsafe-eval'`. Textures made from the app's own canvases or same-origin images are fine. Workers would come under `script-src 'self'` (same-origin script only), and `blob:` workers are blocked. That matters only if TECH moves layout or picking to a `blob:` worker, and it would be a finding to raise then, not an edit now. No change is required for this choice.

## Reproduce

The lab was a throwaway `/tmp/claude-501/okt-bench.*/bench.mjs` and has since been deleted. Its key parts:

```js
const { chromium } = await import('<repo>/tests/node_modules/playwright/index.mjs');
const browser = await chromium.launch({ headless: false, args: ['--disable-renderer-backgrounding',
  '--disable-backgrounding-occluded-windows', '--disable-background-timer-throttling'] });
const ctx = await browser.newContext({ viewport: { width: 1240, height: 940 }, deviceScaleFactor: 2 });
const page = await ctx.newPage();
await page.goto(process.env.OKT + 'api/index');      // same origin, okt client not running
const res = await page.evaluate(bench, { WARM: 1000, REC: 5000, K: 1, modes: ['all', 'tier'] });
// bench(): fetch('/api/index'), lay out, replace <html> with 1200x900 canvases,
// per scenario: rAF loop {w0=performance.now(); draw(t); draw_ms=now-w0; raf_ms=t-last}, p50/p95/max.
// WebGL2: one VAO (P vec3, S float, C vec3) STATIC_DRAW; edges = ELEMENT_ARRAY Uint32 GL_LINES;
//   points = gl.POINTS, gl_PointSize = size*dpr*dist/z*4.4, frag: discard r>1, smoothstep core + exp glow;
//   blendFunc(ONE, ONE); camera = uniform mat3 + dist + focal; overlay 2d canvas: 24 fillText.
// Canvas 2D: project all in JS, edges into 6 alpha-bucket Path2D strokes, nodes depth-sorted into
//   8 bands x region-colour Path2D (halo 2.2r @0.18a + core), 24 fillText.
```

Run it with the sandbox off:

```
export WASM_BINDGEN=/tmp/claude-501/okt-tools/bin/wasm-bindgen PLAYWRIGHT_BROWSERS_PATH=/tmp/claude-501/ms-playwright TMPDIR=/tmp/claude-501
tests/env.sh sh -c 'node <lab>/bench.mjs'
```

The bench refuses to run if `OKT` is unset or points at 6580. 6581 is shared with other workers, and `env.sh` exits with "port 6581 is busy" if another run holds it, so retry once it is free.

R11's binding measurement is the real client under `tests/perf/profile.mjs` (headed). This bench only settles the renderer choice.
