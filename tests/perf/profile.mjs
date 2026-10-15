// Main-thread cost and frame pacing of a fixed set of interactions in okt.
//   node tests/perf/profile.mjs <url> <label> [width height dpr]
// Writes results-<label>.json. Run two builds and compare (tests/perf/compare.py).
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import fs from 'node:fs';

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
