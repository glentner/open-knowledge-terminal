// Main-thread cost and frame pacing of a fixed set of interactions in okt.
//   node tests/perf/profile.mjs <url> <label> [width height dpr]
// Writes results-<label>.json. Run two builds and compare (tests/perf/compare.py).
//
//   node tests/perf/profile.mjs --graph [--headless]
// The graph's frame time, with a verdict: the camera orbiting by keys, by tilt keys and by a
// circular drag ("orbit"), then the same with every card drawn ("orbit_all"), then flights
// ("warp"): from the most connected card, j and space (Enter on alternate trips) fly to the walk's
// target and [ eases back, at least six round trips and on until 900 frames are recorded, frames
// counting only while the camera moves (#graph data-anim="1"). It targets $OKT
// only (tests/env.sh sets it; never 6580), runs headed in a 1600x1000 window at DPR 2, calibrates
// the display's refresh for 1 s and warms up with one untimed orbit. It prints target=, mode=,
// renderer=, refresh_ms=, one JSON line per scenario ({"scenario","frames","draws","p50","p95",
// "max"}, frames being the rAF intervals and draws the graph's own drawn frames), then
// "PASS <scenario> p95=<x> <= 16.7" or "FAIL ...". Exit 1 on a FAIL or a page error; 2 when $OKT
// is unset or names 6580. --headless prints the numbers with no verdict (headless Chromium draws
// WebGL in software at 60 Hz, so its p95 says nothing about the display).
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import fs from 'node:fs';

if (process.argv.includes('--graph')) process.exit(await graphFrameTime());

async function graphFrameTime() {
  const OKT = process.env.OKT || '';
  if (!OKT || /:6580(\/|$)/.test(OKT)) { console.error('profile --graph: set OKT to the tests/env.sh server (never 6580)'); return 2; }
  const headless = process.argv.includes('--headless');
  const BOUND = 16.7;
  const errors = [];
  const browser = await chromium.launch({ headless, args: ['--disable-backgrounding-occluded-windows', '--disable-renderer-backgrounding', '--disable-background-timer-throttling'] });
  try {
    const ctx = await browser.newContext({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 2 });
    // in the page: rAF intervals and the graph's drawn frames, recorded while __rec is set
    await ctx.addInitScript(() => {
      window.__rec = false; window.__iv = []; window.__draws = 0; window.__oktDraw = [];
      // __animOnly: only frames while the graph's camera moves count (an interval between two of them)
      window.__animOnly = false;
      let last = 0;
      const moving = () => !window.__animOnly || document.getElementById('graph')?.getAttribute('data-anim') === '1';
      const loop = t => {
        if (window.__rec && moving()) {
          if (last) window.__iv.push(t - last);
          window.__draws += window.__oktDraw.length;
          last = t;
        } else last = 0;
        window.__oktDraw.length = 0;
        requestAnimationFrame(loop);
      };
      requestAnimationFrame(loop);
    });
    const page = await ctx.newPage();
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(OKT + '#v=graph', { waitUntil: 'load' });
    await page.waitForFunction(() => document.getElementById('graph')?.getAttribute('data-dist'), null, { timeout: 30000 });
    await page.bringToFront();
    const renderer = await page.evaluate(() => {
      const gl = document.createElement('canvas').getContext('webgl2');
      const ext = gl && gl.getExtension('WEBGL_debug_renderer_info');
      return ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : (gl ? gl.getParameter(gl.RENDERER) : 'none');
    });
    const pct = (a, p) => { if (!a.length) return 0; const s = [...a].sort((x, y) => x - y); return s[Math.min(s.length - 1, Math.floor(p * s.length))]; };
    const record = async fn => {
      await page.evaluate(() => { window.__iv = []; window.__draws = 0; window.__rec = true; });
      await fn();
      await page.waitForTimeout(150);
      return page.evaluate(() => { window.__rec = false; return { iv: window.__iv, draws: window.__draws }; });
    };
    // refresh: the median frame interval of an idle second
    const idle = await record(() => page.waitForTimeout(1000));
    const refresh = +pct(idle.iv, 0.5).toFixed(1);
    console.log(`target=${OKT}`);
    console.log(`mode=${headless ? 'headless' : 'headed'}`);
    console.log(`renderer=${renderer}`);
    console.log(`refresh_ms=${refresh}`);
    const box = await page.locator('#graph').boundingBox();
    const cx = box.x + box.width / 2, cy = box.y + box.height / 2;
    const press = async (k, ms) => { const t1 = Date.now() + ms; while (Date.now() < t1) { await page.keyboard.press(k); await page.waitForTimeout(16); } };
    const orbit = async () => {
      await press('ArrowRight', 2500);
      await press('PageUp', 500);
      await press('PageDown', 500);
      // a circular drag, about 2 s, a pointer move at a time
      const r = Math.min(box.width, box.height) * 0.3;
      await page.mouse.move(cx + r, cy);
      await page.mouse.down();
      const t1 = Date.now() + 2000;
      for (let i = 0; Date.now() < t1; i++) {
        const a = i * 0.05;
        await page.mouse.move(cx + r * Math.cos(a), cy + r * Math.sin(a));
      }
      await page.mouse.up();
    };
    // warm-up: one untimed orbit (shaders, JIT)
    await press('ArrowRight', 1000);
    // the flights: a card to walk from (the most connected markdown file), then round trips
    const idx = await (await fetch(OKT + 'api/index')).json();
    const degree = {};
    for (const e of idx.edges) { const [s, t] = Array.isArray(e) ? e : [e.s, e.t]; degree[s] = (degree[s] || 0) + 1; degree[t] = (degree[t] || 0) + 1; }
    const hub = idx.nodes.filter(n => n.t === 'file' && n.id.endsWith('.md')).sort((a, b) => (degree[b.id] || 0) - (degree[a.id] || 0))[0].id;
    const settle = async act => {
      const m0 = await page.evaluate(() => document.getElementById('graph').getAttribute('data-move'));
      await act();
      await page.waitForFunction(m => document.getElementById('graph').getAttribute('data-move') !== m, m0, { timeout: 3000 }).catch(() => {});
      await page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
      await page.waitForFunction(() => document.getElementById('graph').getAttribute('data-anim') === '0', null, { timeout: 5000 }).catch(() => {});
    };
    let trips = 0;
    const warp = async () => {
      for (trips = 0; trips < 24 && (trips < 6 || (await page.evaluate(() => window.__iv.length)) < 900); trips++) {
        await page.keyboard.press('j');
        await page.waitForTimeout(80);
        await page.waitForFunction(() => document.getElementById('graph').getAttribute('data-anim') === '0', null, { timeout: 5000 }).catch(() => {});
        await settle(() => page.keyboard.press(trips % 2 ? 'Enter' : ' '));
        await settle(() => page.keyboard.press('['));
      }
    };
    let fail = false;
    for (const name of ['orbit', 'orbit_all', 'warp']) {
      // every card is drawn already; a show-all toggle, when the toolbar has one, is switched on
      if (name === 'orbit_all') {
        const all = page.locator('.gtools [data-z=all]');
        if (await all.count() && !(await all.evaluate(b => b.classList.contains('on')))) await all.click();
        await page.waitForTimeout(300);
      }
      // the flights run as the graph is by default: show all off, from a selected card
      if (name === 'warp') {
        const all = page.locator('.gtools [data-z=all]');
        if (await all.count() && (await all.evaluate(b => b.classList.contains('on')))) await all.click();
        await settle(() => page.evaluate(h => { location.hash = '#v=graph&s=' + encodeURIComponent(h); }, hub));
        await page.waitForTimeout(300);
        await page.evaluate(() => { document.activeElement?.blur(); window.__animOnly = true; });
      }
      const r = await record(name === 'warp' ? warp : orbit);
      const out = { scenario: name, frames: r.iv.length, draws: r.draws, p50: +pct(r.iv, 0.5).toFixed(1), p95: +pct(r.iv, 0.95).toFixed(1), max: +Math.max(0, ...r.iv).toFixed(1), ...(name === 'warp' ? { trips } : {}) };
      console.log(JSON.stringify(out));
      if (headless) continue;
      if (out.p95 <= BOUND) console.log(`PASS ${name} p95=${out.p95} <= ${BOUND}`);
      else { console.log(`FAIL ${name} p95=${out.p95} > ${BOUND}`); fail = true; }
    }
    if (errors.length) { console.log('FAIL page errors: ' + errors.slice(0, 3).join(' | ')); fail = true; }
    return fail ? 1 : 0;
  } finally {
    await browser.close();
  }
}

const TARGET = process.argv[2] || 'http://127.0.0.1:6580/';
const LABEL = process.argv[3] || 'okt';
const W = +(process.argv[4] || 1920), H = +(process.argv[5] || 1080), DPR = +(process.argv[6] || 1);
const SCEN = (process.env.SCEN || '').split(',').filter(Boolean);
const OUT = process.env.OUT || '.';

const XF = (process.env.FLAGS || '').split(' ').filter(Boolean);
const browser = await chromium.launch({ args: ['--enable-precise-memory-info', '--disable-renderer-backgrounding', ...XF] });
const ctx = await browser.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: DPR });
const page = await ctx.newPage();
page.on('pageerror', e => console.log('pageerror:', e.message));
const cdp = await ctx.newCDPSession(page);
await cdp.send('Performance.enable');
await page.goto(TARGET, { waitUntil: 'load' });
await page.waitForTimeout(2500);

await page.evaluate(() => {
  window.__frames = []; window.__long = []; window.__events = [];
  const loop = t => { window.__frames.push(t); requestAnimationFrame(loop); };
  requestAnimationFrame(loop);
  new PerformanceObserver(l => { for (const e of l.getEntries()) window.__long.push([e.startTime, e.duration]); }).observe({ type: 'longtask', buffered: false });
  new PerformanceObserver(l => { for (const e of l.getEntries()) window.__events.push([e.name, e.startTime, e.duration]); }).observe({ type: 'event', durationThreshold: 16, buffered: false });
});

async function metrics() {
  const { metrics } = await cdp.send('Performance.getMetrics');
  return Object.fromEntries(metrics.map(m => [m.name, m.value]));
}
const pct = (a, p) => { if (!a.length) return 0; const s = [...a].sort((x, y) => x - y); return s[Math.min(s.length - 1, Math.floor(p * s.length))]; };

async function scenario(name, fn) {
  if (SCEN.length && !SCEN.includes(name)) return null;
  await page.evaluate(() => { window.__frames = []; window.__long = []; window.__events = []; window.__oktDraw = []; });
  const m0 = await metrics();
  const t0 = Date.now();
  await fn();
  await page.waitForTimeout(400);
  const wall = Date.now() - t0;
  const m1 = await metrics();
  const r = await page.evaluate(() => ({ frames: window.__frames, long: window.__long, events: window.__events, draws: (window.__oktDraw || []).splice(0) }));
  const iv = []; for (let i = 1; i < r.frames.length; i++) iv.push(r.frames[i] - r.frames[i - 1]);
  const d = k => +(((m1[k] || 0) - (m0[k] || 0)) * 1000).toFixed(1);
  const task = d('TaskDuration'), script = d('ScriptDuration'), style = d('RecalcStyleDuration'), layout = d('LayoutDuration');
  const out = {
    scenario: name, wall_ms: wall,
    main_busy_ms: task, script_ms: script, style_ms: style, layout_ms: layout, other_ms: +(task - script - style - layout).toFixed(1),
    frames: iv.length, frame_p50: +pct(iv, 0.5).toFixed(1), frame_p95: +pct(iv, 0.95).toFixed(1), frame_max: +Math.max(0, ...iv).toFixed(1),
    over33: iv.filter(x => x > 33.4).length,
    longtasks: r.long.length, longtask_ms: +r.long.reduce((s, x) => s + x[1], 0).toFixed(1),
    slow_events: r.events.length, event_max: +Math.max(0, ...r.events.map(e => e[2])).toFixed(1),
    draws: r.draws.length, draw_p50: +pct(r.draws, 0.5).toFixed(2), draw_p95: +pct(r.draws, 0.95).toFixed(2), draw_max: +Math.max(0, ...r.draws).toFixed(2),
  };
  console.log(JSON.stringify(out));
  return out;
}

const results = [];
const box = await page.locator('#graph').boundingBox();
const cx = box.x + box.width / 2, cy = box.y + box.height / 2;
// fixed screen points of a spread of file nodes at 1920x1080 (fit view), the same for every build
const pts = JSON.parse(fs.readFileSync(new URL('./points.json', import.meta.url)));

results.push(await scenario('idle_3s', async () => { await page.waitForTimeout(3000); }));
results.push(await scenario('wheel_zoom', async () => {
  await page.mouse.move(cx + 40, cy - 30);
  for (let i = 0; i < 50; i++) { await page.mouse.wheel(0, -25); await page.waitForTimeout(8); }
  for (let i = 0; i < 50; i++) { await page.mouse.wheel(0, 25); await page.waitForTimeout(8); }
}));
results.push(await scenario('pan_drag', async () => {
  await page.mouse.move(cx - 300, cy + 200);
  await page.mouse.down();
  for (let i = 0; i < 80; i++) { await page.mouse.move(cx - 300 + i * 6, cy + 200 - i * 3); await page.waitForTimeout(8); }
  await page.mouse.up();
}));
await page.keyboard.press('0'); await page.waitForTimeout(700);
results.push(await scenario('hover_sweep', async () => {
  for (let i = 0; i < 120; i++) { await page.mouse.move(box.x + 100 + i * 5, cy - 150 + Math.sin(i / 6) * 120); await page.waitForTimeout(8); }
}));
results.push(await scenario('click_select', async () => {
  for (const [x, y] of pts.slice(0, 8)) {
    await page.mouse.click(x, y);
    await page.waitForTimeout(700);
    await page.keyboard.press('0'); await page.waitForTimeout(500);
  }
}));
results.push(await scenario('zoom_selected', async () => {
  await page.mouse.click(pts[0][0], pts[0][1]); await page.waitForTimeout(800);
  await page.mouse.move(cx, cy);
  for (let i = 0; i < 40; i++) { await page.mouse.wheel(0, -30); await page.waitForTimeout(8); }
  for (let i = 0; i < 40; i++) { await page.mouse.wheel(0, 30); await page.waitForTimeout(8); }
  await page.keyboard.press('Escape'); await page.keyboard.press('0'); await page.waitForTimeout(500);
}));
results.push(await scenario('tree_jk', async () => {
  await page.keyboard.press('Escape'); await page.keyboard.press('f'); await page.waitForTimeout(400);
  const row = page.locator('#tree .trow:not(.thead)').nth(3); await row.click(); await page.waitForTimeout(300);
  for (let i = 0; i < 40; i++) { await page.keyboard.press('j'); await page.waitForTimeout(33); }
  for (let i = 0; i < 20; i++) { await page.keyboard.press('k'); await page.waitForTimeout(33); }
}));
results.push(await scenario('log_jk', async () => {
  await page.keyboard.press('l'); await page.waitForTimeout(500);
  await page.locator('#ltable .crow').nth(2).click(); await page.waitForTimeout(300);
  for (let i = 0; i < 30; i++) { await page.keyboard.press('j'); await page.waitForTimeout(40); }
}));
results.push(await scenario('go_typing', async () => {
  await page.keyboard.press('g'); await page.waitForTimeout(300);
  await page.keyboard.press('/'); await page.waitForTimeout(100);
  await page.keyboard.type('contact geoffrey lentner', { delay: 60 });
  await page.keyboard.press('Escape');
}));
results.push(await scenario('content_load', async () => {
  for (const p of ['workspace/pearc28/research/location/floorplan.md', 'memory/inbox.md', 'skill/dream.md', 'AGENTS.md', 'policy/attention.md', 'bin/kb-lint']) {
    await page.evaluate(p => { location.hash = '#v=graph&s=' + encodeURIComponent(p) + '&tab=content'; }, p);
    await page.waitForTimeout(1000);
  }
}));
results.push(await scenario('view_switch', async () => {
  await page.keyboard.press('Escape');
  for (const k of ['f', 'l', 'g', 't', 'g', 'f', 'g']) { await page.keyboard.press(k); await page.waitForTimeout(350); }
}));

fs.writeFileSync(`${OUT}/results-${LABEL}.json`, JSON.stringify(results.filter(Boolean), null, 1));
const mem = await page.evaluate(() => performance.memory ? { used: performance.memory.usedJSHeapSize } : null);
console.log('heap', JSON.stringify(mem), 'dom', await page.evaluate(() => document.querySelectorAll('*').length));
await browser.close();
