// The pane arrangement, graded on what renders, against a running okt:
//   FIRST LOAD  an empty store starts stacked below 900px and classic from 900px; a splitter width
//               saved before arrangements sizes classic; the default is not saved by another
//               setting, nor by a bookmarked view, and a saved arrangement is restored as it was
//   PANES       a title bar with the views held as tabs; a tab click shows its view; the view keys
//               show their view where it is held and focus that pane, a view no pane holds joins
//               the focused pane; o, c and h show the Inspector and leave focus alone (c then j
//               still walks the Graph); a click focuses a pane and marks its title bar; the
//               selection followed in every shown view; while the Inspector's pane is focused
//               the view keys act on the view focused before it, wherever a swap, move or close of
//               the Inspector's pane leaves that view; the Log's and Files' keys step from their
//               selection with their pane collapsed or off screen, and a commit reaches an
//               off-screen Log (an empty commit in $OKT_KB: under tests/env.sh only); the Log
//               drawn as the key view under the 200 px floor (by l, or bookmarked at load) and
//               when focus steps onto it collapsed, its keys stepping as on screen; back and
//               forward place the view; a view coming on screen by itself (a reload, a bookmark,
//               a window grown) moves no focus, hash or history
//   DRAG        a split follows the pointer, stops at its floors, and survives a reload
//   PRESETS     classic, swapped, stacked and three at 1600 and 900 px wide, by key, by the cycle
//               key and by the window bar; each survives a reload and a window resize, and a reload
//               inside the save delay; three's
//               Graph bar, Inspector and Files; the Graph hit-tested after a move; the help rows
//   COLLAPSE    a strip side by side, a title bar stacked, expand restoring the size, the
//               accordion, and the pane leader's waiting, cancelling and timing out
//   OPERATIONS  focus stepping; swap, flip, close and move on the focused pane, the refusals of the
//               only pane, moving a last view closing its pane, every view moved into a pane that
//               didn't hold it, the floors of a nested split, and a reload after it all
//   FITTING     the Graph and Files drawn and responding side by side; the canvas filling its pane
//               after a drag, a preset, a neighbour's collapse and expand and a window resize (its
//               pixels and backing store read at DPR 2); every view in a 200 px pane without
//               sideways overflow, the Inspector's tabs also at 240 and 272 px with its body never
//               scrolling sideways, its source with the blame at 200, 240 and 268 px keeping a code
//               column of 120 px, and its time-travel bar at 200 px; a narrower pane down to its
//               title bar, and grown back showing
//               its view as it is now; the legend by the Graph pane's width; the Log chart redrawn
//               for its pane; the keys acting on the focused pane's view
//   WHAT SHOWS  every preset at 1600 x 1000 and 900 x 1000 in one pass, graded from rects, with
//               three's Graph bar, Inspector and Files
// Every pane, view and size comes from getBoundingClientRect, checkVisibility and
// elementFromPoint; the data-* attributes the client writes only find elements. Each check opens
// a fresh context (an empty store) unless it seeds one, as a user's earlier settings would.
//   node tests/ui/panes.mjs [okt-url]
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');

const OKT = process.argv[2] || process.env.OKT || 'http://127.0.0.1:6580/';
const idx = await (await fetch(OKT + 'api/index')).json();
const degree = {};
for (const e of idx.edges) { const [s, t] = Array.isArray(e) ? e : [e.s, e.t]; degree[s] = (degree[s] || 0) + 1; degree[t] = (degree[t] || 0) + 1; }
const md = idx.nodes.filter(n => n.t === 'file' && n.id.endsWith('.md'));
const hub = md.slice().sort((a, b) => (degree[b.id] || 0) - (degree[a.id] || 0))[0].id;
const deep = md.slice().sort((a, b) => b.id.split('/').length - a.id.split('/').length)[0].id;
const stem = id => id.split('/').pop().replace(/\.md$/, '');
const FIVE = ['graph', 'files', 'log', 'table', 'list'];
const near = (a, b, tol) => Math.abs(a - b) <= tol;

let fails = 0;
const check = (label, ok, detail) => { if (!ok) fails++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok ? '' : ': ' + JSON.stringify(detail).slice(0, 700)}`); };
const browser = await chromium.launch();
const errors = [];

// A fresh context at width × 1000, its store seeded first when `seed` is given.
async function open(width, { seed = null, hash = '', dpr = 1, init = null } = {}) {
  const ctx = await browser.newContext({ viewport: { width, height: 1000 }, deviceScaleFactor: dpr });
  const page = await ctx.newPage();
  page.on('pageerror', e => errors.push(e.message));
  if (init) await page.addInitScript(init);
  await page.goto(OKT + 'api/index');
  const kb = await (async () => { const r = await fetch(OKT); const t = await r.text(); const m = t.match(/<script id="boot"[^>]*>([^<]*)</); try { return JSON.parse(m[1]).kb || 'kb'; } catch { return 'kb'; } })();
  const key = 'okt:' + kb;
  if (seed) await page.evaluate(([k, v]) => localStorage.setItem(k, v), [key, JSON.stringify(seed)]);
  await page.goto(OKT + hash, { waitUntil: 'load' });
  await page.waitForFunction(() => document.querySelectorAll('#panes .pane').length > 0 && (document.getElementById('status')?.innerText || '').trim().length > 0, null, { timeout: 15000 });
  await page.waitForTimeout(500);
  return { ctx, page, key, stored: () => page.evaluate(k => localStorage.getItem(k) || '', key) };
}
const press = async (page, k, ms = 500) => { await page.keyboard.press(k); await page.waitForTimeout(ms); };
const hashTo = async (page, h) => { await page.evaluate(h => { location.hash = h; }, h); await page.waitForTimeout(800); };

// The panes as drawn, in reading order: each pane's rect relative to #panes; its title bar's rect
// and whether it is drawn as the focused one (an accent edge: a box-shadow); the views its title
// bar shows (visible tab buttons whose centre hits the tab), and which tab is drawn as current
// (an accent underline); the view roots it holds; the one root visible in its body, reached at
// its centre.
const ROOT = { graph: 'v-graph', files: 'v-files', log: 'v-log', table: 'v-table', list: 'v-list', insp: 'insp' };
const snap = page => page.evaluate(ROOT => {
  const vis = e => !!e && e.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true }) && e.getBoundingClientRect().width > 0 && e.getBoundingClientRect().height > 0;
  const hits = e => { const r = e.getBoundingClientRect(); const at = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2); return !!at && e.contains(at); };
  const R = document.getElementById('panes').getBoundingClientRect();
  const panes = [...document.querySelectorAll('#panes .pane')].map(p => {
    const r = p.getBoundingClientRect();
    const t = p.querySelector(':scope > .ptitle');
    const tr = t.getBoundingClientRect();
    const tabs = [...t.querySelectorAll('button[data-view]')].filter(b => vis(b) && hits(b));
    const on = tabs.filter(b => { const c = getComputedStyle(b).borderBottomColor; return c !== 'rgba(0, 0, 0, 0)' && c !== 'transparent'; }).map(b => b.dataset.view);
    const roots = Object.entries(ROOT).filter(([, id]) => p.contains(document.getElementById(id))).map(([v]) => v);
    const shown = roots.filter(v => vis(document.getElementById(ROOT[v])) && hits(document.getElementById(ROOT[v])));
    const body = p.querySelector(':scope > .pbody');
    return {
      x: r.left - R.left, y: r.top - R.top, w: r.width, h: r.height,
      title: { x: tr.left - R.left, top: tr.top - R.top, y: tr.top - r.top, w: tr.width, h: tr.height },
      body: vis(body),
      marked: getComputedStyle(t).boxShadow !== 'none',
      tabs: tabs.map(b => b.dataset.view), on, roots, shown,
      id: p.dataset.pane,
    };
  }).sort((a, b) => a.y - b.y || a.x - b.x);
  return { W: R.width, H: R.height, panes };
}, ROOT);
const focusOf = s => { const m = s.panes.map((p, i) => p.marked ? i : -1).filter(i => i >= 0); return m.length === 1 ? m[0] : -1; };
const views = p => p.tabs.slice().sort().join();
// a two-pane arrangement as drawn: [x, y, w, h] each within 2px, the views held and the one shown
function grade2(s, rects, held, shown) {
  const bad = [];
  if (s.panes.length !== rects.length) bad.push(`${s.panes.length} panes`);
  rects.forEach((r, i) => {
    const p = s.panes[i];
    if (!p) return;
    if (![p.x, p.y, p.w, p.h].every((v, j) => near(v, r[j], 2))) bad.push(`pane ${i} at ${[p.x, p.y, p.w, p.h].map(v => v.toFixed(1))}, want ${r.map(v => v.toFixed(1))}`);
    if (views(p) !== held[i].slice().sort().join() || p.roots.slice().sort().join() !== views(p)) bad.push(`pane ${i} tabs ${p.tabs} roots ${p.roots}, want ${held[i]}`);
    if (p.shown.join() !== shown[i]) bad.push(`pane ${i} shows ${p.shown}, want ${shown[i]}`);
  });
  return bad;
}
const classic = (s, c = 0.54) => [[0, 0, c * s.W, s.H], [c * s.W, 0, (1 - c) * s.W, s.H]];
const stacked = s => [[0, 0, s.W, s.H / 2], [0, s.H / 2, s.W, s.H / 2]];
const HELD = [FIVE, ['insp']];
// click a pane's title bar at its empty far end
async function clickTitle(page, i) {
  const pt = await page.evaluate(i => {
    const ps = [...document.querySelectorAll('#panes .pane')].sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top || a.getBoundingClientRect().left - b.getBoundingClientRect().left);
    const t = ps[i].querySelector('.ptitle').getBoundingClientRect();
    return { x: t.right - 12, y: t.top + t.height / 2 };
  }, i);
  await page.mouse.click(pt.x, pt.y);
  await page.waitForTimeout(300);
}
const sash = page => page.evaluate(() => { const s = [...document.querySelectorAll('#panes .sash')].find(e => e.checkVisibility()); if (!s) return null; const r = s.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
async function drag(page, from, to, mid) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 5 });
  const during = mid ? await mid() : null;
  await page.mouse.move(to.x, to.y, { steps: 5 });
  await page.mouse.up();
  await page.waitForTimeout(300);
  return during;
}

// ---- first load ---------------------------------------------------------------------------------
{
  const rows = [];
  for (const [w, want, label] of [[899, stacked, 'first load below 900 px starts stacked'], [900, classic, 'first load at 900 px starts classic']]) {
    const { ctx, page, stored } = await open(w);
    const s = await snap(page);
    const bad = grade2(s, want(s), HELD, ['graph', 'insp']);
    check(label, bad.length === 0 && near(s.W, w, 1) && near(s.H, 890, 1), { bad, W: s.W, H: s.H });
    // the graph's all-labels toggle saves the settings; the default stays unsaved
    await page.click('#v-graph .gtools button[data-z="labels"]');
    await page.waitForTimeout(700);
    const st = await stored();
    rows.push({ w, labels: st.includes('"labels"'), arrangement: st.includes('"arrangement"') });
    await ctx.close();
  }
  check('the default arrangement is not saved by an unrelated setting', rows.every(r => r.labels && !r.arrangement), rows);
  // a splitter width saved before arrangements
  {
    const { ctx, page } = await open(1600, { seed: { theme: 'terminal', insp: 30 } });
    const s = await snap(page);
    const bad = grade2(s, classic(s, 0.7), HELD, ['graph', 'insp']);
    check('a saved splitter width sizes classic when classic starts', bad.length === 0, bad);
    await ctx.close();
  }
  {
    const { ctx, page } = await open(899, { seed: { theme: 'terminal', insp: 30 } });
    const s = await snap(page);
    const bad = grade2(s, stacked(s), HELD, ['graph', 'insp']);
    check('a saved splitter width leaves a narrow window stacked', bad.length === 0, bad);
    await ctx.close();
  }
  // a bookmark's view shows, placed in the default, which stays unsaved
  {
    const { ctx, page, stored } = await open(1600, { hash: '#v=log' });
    const s = await snap(page);
    const bad = grade2(s, classic(s), HELD, ['log', 'insp']);
    const st = await stored();
    check('a bookmarked view on a first load shows without saving the arrangement', bad.length === 0 && focusOf(s) === 0 && !st.includes('"arrangement"'), { bad, focus: focusOf(s), st: st.slice(0, 200) });
    await ctx.close();
  }
  // a saved arrangement comes back as saved: the hash's view picks only the focus
  {
    const arrangement = { v: 1, root: { split: 'row', f: 0.6, kids: [{ views: FIVE, shown: 'files', collapsed: false }, { views: ['insp'], shown: 'insp', collapsed: false }] } };
    const { ctx, page, stored } = await open(1600, { seed: { arrangement }, hash: '#v=log' });
    const s = await snap(page);
    const bad = grade2(s, classic(s, 0.6), HELD, ['files', 'insp']);
    const st = JSON.parse((await stored()) || '{}');
    check('a saved arrangement is restored untouched by the hash', bad.length === 0 && JSON.stringify(st.arrangement) === JSON.stringify(arrangement), { bad, st: st.arrangement });
    await ctx.close();
  }
}

// ---- panes, tabs, keys and focus ------------------------------------------------------------------
{
  const { ctx, page } = await open(1600);
  let s = await snap(page);
  const bars = s.panes.map(p => ({ top: p.title.y, h: p.title.h, w: p.title.w - p.w, tabs: p.tabs, roots: p.roots }));
  check('each pane shows a title bar with its views as tabs', s.panes.length === 2 && s.panes.every(p => near(p.title.y, 0, 0.5) && near(p.title.h, 24, 0.5) && near(p.title.w, p.w, 1.5) && views(p) === p.roots.slice().sort().join())
    && views(s.panes[0]) === FIVE.slice().sort().join() && views(s.panes[1]) === 'insp', bars);
  // a tab click
  const tab = async (i, v) => {
    const pt = await page.evaluate(([i, v]) => { const ps = [...document.querySelectorAll('#panes .pane')].sort((a, b) => a.getBoundingClientRect().left - b.getBoundingClientRect().left); const r = ps[i].querySelector(`.ptitle button[data-view="${v}"]`).getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; }, [i, v]);
    await page.mouse.click(pt.x, pt.y);
    await page.waitForTimeout(700);
    return snap(page);
  };
  const clicked = [];
  for (const v of ['log', 'files', 'graph']) { s = await tab(0, v); clicked.push({ v, shown: s.panes[0].shown, on: s.panes[0].on }); }
  check('clicking a tab shows that view', clicked.every(c => c.shown.join() === c.v && c.on.join() === c.v), clicked);
  // the view keys: each shows its view in the left pane and focuses it, from either pane
  const keyed = [];
  for (const [k, v] of [['f', 'files'], ['l', 'log'], ['t', 'table'], ['g', 'graph']]) {
    await clickTitle(page, 1);
    await press(page, k, 800);
    s = await snap(page);
    keyed.push({ k, shown: s.panes[0].shown.join(), focus: focusOf(s), right: s.panes[1].shown.join() });
  }
  check('view keys show their view in the holding pane and focus it', keyed.every((x, i) => x.shown === ['files', 'log', 'table', 'graph'][i] && x.focus === 0 && x.right === 'insp'), keyed);
  // o c h: the Inspector on each tab, the Graph's pane keeping focus
  const insp = [];
  for (const [k, t] of [['o', 'overview'], ['c', 'content'], ['h', 'history']]) {
    await press(page, k);
    s = await snap(page);
    const on = await page.evaluate(t => !!document.querySelector(`#itabs button[data-tab=${t}].on`), t);
    insp.push({ k, on, focus: focusOf(s), left: s.panes[0].shown.join(), right: s.panes[1].shown.join() });
  }
  check('o c h show the Inspector on their tab and keep focus where it was', insp.every(x => x.on && x.focus === 0 && x.left === 'graph' && x.right === 'insp'), insp);
  // c then j walks the Graph from the hub
  await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
  await press(page, 'c');
  await press(page, 'j', 400);
  s = await snap(page);
  const walk = await page.evaluate(() => { const h = document.getElementById('ghint'); return h && !h.hidden ? h.textContent : ''; });
  check('c then j still steps the Graph walk with the Graph focused', /^1\/\d+ · /.test(walk) && focusOf(s) === 0, { walk, focus: focusOf(s) });
  await press(page, 'Escape', 300);
  // a click focuses its pane: the title bar is marked, and the view keys follow (j in Files)
  await clickTitle(page, 1);
  const a = await snap(page);
  await clickTitle(page, 0);
  const b = await snap(page);
  check('clicking in a pane focuses it and marks its title bar', focusOf(a) === 1 && focusOf(b) === 0 && a.panes[0].marked === false && b.panes[1].marked === false, { a: a.panes.map(p => p.marked), b: b.panes.map(p => p.marked) });
  // the selection: Files and the Inspector, in two panes, both follow it
  await hashTo(page, '#v=files&s=' + encodeURIComponent(deep));
  s = await snap(page);
  const fol = await page.evaluate(() => ({ on: document.querySelector('#tree .trow.on')?.dataset.id || null, head: document.getElementById('ihead').innerText }));
  await hashTo(page, '#v=files&s=' + encodeURIComponent(hub));
  const fol2 = await page.evaluate(() => ({ on: document.querySelector('#tree .trow.on')?.dataset.id || null, head: document.getElementById('ihead').innerText }));
  check('a selection shows in the Inspector and every shown view follows it', s.panes[0].shown.join() === 'files' && s.panes[1].shown.join() === 'insp' && fol.on === deep && fol.head.includes(stem(deep)) && fol2.on === hub && fol2.head.includes(stem(hub)), { shown: s.panes.map(p => p.shown), fol, fol2 });
  await ctx.close();
}
// A view that comes on screen by itself only draws: focus, the hash and the history move only by a
// user's action. The Table is the view that loads on its first showing. Each case reads the
// history's length from the page's own start (an init script) and checks the Table was drawn, so
// the render did happen.
{
  const START = () => { window.__start = { len: history.length, hash: decodeURIComponent(location.hash) }; };
  const now = page => page.evaluate(() => ({ len: history.length, hash: decodeURIComponent(location.hash), start: window.__start, focus: [...document.querySelectorAll('#panes .pane')].find(p => p.dataset.focused === '1')?.dataset.shown || null, rows: document.querySelectorAll('#tgrid tr[data-id]').length }));
  const settled = page => page.waitForFunction(() => document.querySelectorAll('#tgrid tr[data-id]').length > 0, null, { timeout: 8000 }).then(() => page.waitForTimeout(400), () => {});
  const walks = page => page.evaluate(() => { const h = document.getElementById('ghint'); return !!h && !h.hidden && /^1\/\d+ · /.test(h.textContent); });
  // three with its Table tab shown at the right, the Graph focused on a card, then a reload
  {
    const { ctx, page } = await open(1600, { init: START });
    await press(page, '`', 700); // three
    await press(page, 't', 1500);
    await press(page, 'g', 800);
    await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
    await page.waitForTimeout(400);
    const before = await now(page);
    await page.reload({ waitUntil: 'load' });
    await page.waitForFunction(() => document.querySelectorAll('#panes .pane').length > 0 && (document.getElementById('status')?.innerText || '').trim().length > 0, null, { timeout: 15000 });
    await settled(page);
    const after = await now(page);
    await press(page, 'j', 500);
    const walk = await walks(page);
    check('a Table on screen at a reload leaves focus, the hash and the history alone', before.focus === 'graph' && after.focus === 'graph' && after.hash === before.hash && after.len === after.start.len && after.rows > 0 && walk, { before, after, walk });
    await ctx.close();
  }
  // a saved arrangement with the Table in a pane of its own, loaded from a bookmark of the Graph
  {
    const arrangement = { v: 1, root: { split: 'row', f: 0.5, kids: [{ views: ['graph', 'files', 'log', 'list'], shown: 'graph', collapsed: false }, { split: 'col', f: 0.5, kids: [{ views: ['table'], shown: 'table', collapsed: false }, { views: ['insp'], shown: 'insp', collapsed: false }] }] } };
    const hash = '#v=graph&s=' + encodeURIComponent(hub);
    const { ctx, page } = await open(1600, { seed: { arrangement }, hash, init: START });
    await settled(page);
    const at = await now(page);
    await press(page, 'j', 500);
    const walk = await walks(page);
    check('a bookmarked Graph with a Table on screen keeps the Graph focused, the hash and the history', at.focus === 'graph' && at.hash === decodeURIComponent(hash) && at.start.hash === at.hash && at.len === at.start.len && at.rows > 0 && walk, { at, walk });
    await ctx.close();
  }
  // the Table's pane under the 200 px floor at load, then the window grows it back on screen
  {
    const arrangement = { v: 1, root: { split: 'row', f: 0.8, kids: [{ views: ['graph', 'files', 'log', 'list', 'insp'], shown: 'graph', collapsed: false }, { views: ['table'], shown: 'table', collapsed: false }] } };
    const hash = '#v=graph&s=' + encodeURIComponent(hub);
    const { ctx, page } = await open(900, { seed: { arrangement }, hash, init: START });
    await page.waitForTimeout(800);
    const before = await now(page);
    await page.setViewportSize({ width: 1600, height: 1000 });
    await settled(page);
    const after = await now(page);
    check('a Table grown back on screen leaves focus, the hash and the history alone', before.rows === 0 && before.focus === 'graph' && after.focus === 'graph' && after.hash === before.hash && after.len === before.len && after.rows > 0, { before, after });
    await ctx.close();
  }
}
// The Inspector's pane takes focus like any pane, but its view has no view keys of its own: after a
// click on its text, in classic and in three, its title bar is marked, j steps the Graph walk and
// ArrowDown walks it (the view focused before), and w d collapses the Inspector's pane to its 24 px strip
{
  const hint = page => page.evaluate(() => { const h = document.getElementById('ghint'); return !!h && !h.hidden ? h.textContent : ''; });
  const inspPane = s => s.panes.findIndex(p => p.roots.includes('insp'));
  const out = [];
  for (const [name, k] of [['classic', ';'], ['three', '`']]) {
    const { ctx, page } = await open(1600);
    await press(page, k, 700);
    await press(page, 'g', 700);
    await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
    await press(page, 'c', 700);
    const at = await page.evaluate(() => { const r = document.getElementById('ibody').getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + 60 }; });
    await page.mouse.click(at.x, at.y); await page.waitForTimeout(300);
    let s = await snap(page);
    const r = { name, focus: focusOf(s), insp: inspPane(s) };
    await press(page, 'j', 400); r.j = await hint(page);
    await press(page, 'Escape', 300); r.stopped = await hint(page);
    await press(page, 'ArrowDown', 400); r.down = await hint(page);
    await press(page, 'Escape', 300);
    s = await snap(page);
    r.still = focusOf(s) === r.insp;
    const w0 = s.panes[r.insp].w;
    await press(page, 'w', 80); await press(page, 'd', 700);
    s = await snap(page);
    const ip = s.panes[inspPane(s)];
    r.collapsed = { w0, w: ip.w, body: ip.body };
    out.push(r);
    await ctx.close();
  }
  check('a click in the Inspector focuses its pane and the view keys still act on the view before', out.every(r => r.insp >= 0 && r.focus === r.insp && r.still && /^1\/\d+ · /.test(r.j) && !/^\d+\/\d+ · /.test(r.stopped) && /^1\/\d+ · /.test(r.down) && !r.collapsed.body && near(r.collapsed.w, 24, 1) && r.collapsed.w0 > 300), out);
}
// click the Inspector's body on plain text, away from its links and buttons
const inspText = async page => {
  const pt = await page.evaluate(() => {
    const b = document.getElementById('ibody'), r = b.getBoundingClientRect();
    for (let y = r.top + 20; y < r.bottom - 20; y += 7) for (let x = r.left + 30; x < r.right - 30; x += 23) {
      const e = document.elementFromPoint(x, y);
      if (e && b.contains(e) && !e.closest('a,button,input,summary,[data-go],[data-commit],[data-at],[data-more],[data-view]') && (e.textContent || '').trim()) return { x, y };
    }
    return { x: r.left + r.width / 2, y: r.bottom - 10 };
  });
  await page.mouse.click(pt.x, pt.y); await page.waitForTimeout(300);
};
// While the Inspector's pane is focused, the view keys act on the view that last held key focus,
// wherever that view now lives: a swap, a move or a close of the Inspector's pane leaves them on
// it. Files is focused, by its title bar after the Graph's, and is never the first pane showing a
// main view. Each case is graded against a reference run of the same arrangement with Files
// focused and j j k pressed at once: the same rows, the same selection and hash, and no Graph walk
{
  const folder = idx.nodes.find(n => n.id === hub)?.parent ?? hub;
  const trace = page => page.evaluate(() => {
    const h = document.getElementById('ghint');
    return {
      s: decodeURIComponent((location.hash.match(/[#&]s=([^&]*)/) || [])[1] || ''),
      v: (location.hash.match(/[#&]v=([^&]*)/) || [])[1] || '',
      walk: !!h && !h.hidden && /^\d+\/\d+ · /.test(h.textContent),
      on: document.querySelector('#tree .trow.on')?.dataset.id ?? null,
    };
  });
  const paneOf = (s, v) => s.panes.findIndex(p => p.roots.includes(v));
  // Graph and Files side by side with the Inspector at the right; and the Graph and the Inspector
  // side by side with Files at the right, so the Inspector's neighbour is the Graph's pane
  const ROW3 = { v: 1, root: { split: 'row', f: 0.34, kids: [{ views: ['graph', 'log', 'table', 'list'], shown: 'graph', collapsed: false }, { split: 'row', f: 0.5, kids: [{ views: ['files'], shown: 'files', collapsed: false }, { views: ['insp'], shown: 'insp', collapsed: false }] }] } };
  const GIF = { v: 1, root: { split: 'row', f: 0.66, kids: [{ split: 'row', f: 0.5, kids: [{ views: ['graph', 'log', 'table', 'list'], shown: 'graph', collapsed: false }, { views: ['insp'], shown: 'insp', collapsed: false }] }, { views: ['files'], shown: 'files', collapsed: false }] } };
  const run = async (arr, op) => {
    const { ctx, page } = await open(1600, arr.seed ? { seed: { arrangement: arr.seed } } : {});
    if (arr.key) await press(page, arr.key, 700);
    await hashTo(page, '#v=files&s=' + encodeURIComponent(folder));
    let s = await snap(page);
    await clickTitle(page, paneOf(s, 'graph'));
    await press(page, 'Escape', 200);
    s = await snap(page);
    await clickTitle(page, paneOf(s, 'files'));
    s = await snap(page);
    const r = { files: s.panes[focusOf(s)]?.shown.join() === 'files', first: s.panes.findIndex(p => p.roots.some(v => FIVE.includes(v))) !== focusOf(s) };
    if (op !== null) {
      await inspText(page);
      s = await snap(page);
      r.insp = s.panes[focusOf(s)]?.roots.includes('insp');
      for (const k of op) await press(page, k, k === 'w' ? 80 : 700);
      s = await snap(page);
      r.after = { focus: s.panes[focusOf(s)]?.shown.join(), panes: s.panes.map(p => p.roots.join(' ')) };
    }
    r.t = [];
    for (const k of ['j', 'j', 'k']) { await press(page, k, 600); r.t.push(await trace(page)); }
    await ctx.close();
    return r;
  };
  const out = [];
  for (const [name, arr, ops] of [
    ['three', { key: '`' }, [[], ['w', 'x']]],
    ['Graph | Files | Inspector', { seed: ROW3 }, [['w', 'x']]],
    ['Graph | Inspector, Files', { seed: GIF }, [['w', 'v'], ['w', 'q']]],
  ]) {
    const ref = await run(arr, null);
    for (const op of ops) {
      const r = await run(arr, op);
      out.push({ name, op: op.join(' ') || 'click', ref: ref.t, ...r, same: JSON.stringify(r.t) === JSON.stringify(ref.t) && ref.files && ref.first && ref.t[0].s !== folder && ref.t.every(x => !x.walk && x.v === 'files' && x.on === x.s) });
    }
  }
  check('while the Inspector is focused the view keys act on the view focused before it, wherever a swap, move or close leaves it', out.length === 5 && out.every(r => r.same && r.files && r.first && r.insp && r.after.focus === 'insp'), out.filter(r => !(r.same && r.files && r.first && r.insp && r.after.focus === 'insp')));
}
// The Log's keys step from the commit it has selected, on screen or not: its pane collapsed, its
// slot showing the Inspector after a move or a close of the Inspector's pane, or a collapsed pane
// restored at load and then focused. In each case j j j k opens rows 0, 1, 2 and 1, as with the Log
// on screen, and the Log shown again marks row 1
{
  const LOGSHUT = { v: 1, root: { split: 'row', f: 0.54, kids: [{ views: ['graph', 'files', 'table', 'list'], shown: 'graph', collapsed: false }, { split: 'col', f: 0.5, kids: [{ views: ['log'], shown: 'log', collapsed: true }, { views: ['insp'], shown: 'insp', collapsed: false }] }] } };
  const rows = page => page.evaluate(() => [...document.querySelectorAll('#ltable .crow')].map(r => r.dataset.h));
  const opened = page => page.evaluate(() => (location.hash.match(/[#&]c=([^&]*)/) || [])[1] || '');
  const out = [];
  for (const [name, start, op] of [
    ['three, on screen', '`', []],
    ['three', '`', ['click', 'w', 'v']],
    ['three', '`', ['click', 'w', 'q']],
    ['classic', ';', ['click', 'w', 'q']],
    ['three', '`', ['w', 'd']],
    ['classic', ';', ['w', 'd']],
    ['collapsed at load', LOGSHUT, ['title']],
  ]) {
    const { ctx, page } = await open(1600, typeof start === 'string' ? {} : { seed: { arrangement: start } });
    if (typeof start === 'string') { await press(page, start, 700); await press(page, 'l', 1500); }
    for (const k of op) {
      if (k === 'click') await inspText(page);
      else if (k === 'title') { await clickTitle(page, (await snap(page)).panes.findIndex(p => p.roots.includes('log'))); await page.waitForTimeout(1200); }
      else await press(page, k, k === 'w' ? 80 : 700);
    }
    let s = await snap(page);
    const r = { name, op: op.join(' '), off: !s.panes.some(p => p.shown.includes('log')), rows: await rows(page), opened: [] };
    for (const k of ['j', 'j', 'j', 'k']) { await press(page, k, 600); r.opened.push(r.rows.indexOf(await opened(page))); }
    await press(page, 'l', 1200);
    s = await snap(page);
    r.back = s.panes.some(p => p.shown.includes('log'));
    r.marked = await page.evaluate(() => [...document.querySelectorAll('#ltable .crow')].map((x, i) => x.classList.contains('on') ? i : -1).filter(i => i >= 0));
    r.ok = r.rows.length >= 4 && r.off === (op.length > 0) && r.opened.join() === '0,1,2,1' && r.back && r.marked.join() === '1';
    out.push(r);
    await ctx.close();
  }
  check('the Log\'s keys step from its selected commit, its pane on screen or not', out.length === 7 && out.every(r => r.ok), out.map(({ rows, ...r }) => ({ ...r, rows: rows.length })).filter(r => !r.ok));
}
// Files' keys step from the selection, on screen or not: after a node is chosen in the Inspector
// while Files' pane is collapsed, or its slot shows the Inspector, j and k step from that node,
// graded against the same steps with Files on screen
{
  const sel = page => page.evaluate(() => decodeURIComponent((location.hash.match(/[#&]s=([^&]*)/) || [])[1] || ''));
  const rowIds = page => page.evaluate(() => [...document.querySelectorAll('#tree .trow:not(.thead)')].map(r => r.dataset.id));
  // a node chip in the Inspector, other than the selection; clicked, it returns its node
  const chip = async (page, not) => {
    const pt = await page.evaluate(not => {
      const c = [...document.querySelectorAll('#ibody [data-go]')].find(e => e.dataset.go !== not && e.checkVisibility() && e.getBoundingClientRect().width > 0);
      if (!c) return null;
      c.scrollIntoView({ block: 'center' });
      const r = c.getBoundingClientRect();
      return { x: r.left + Math.min(10, r.width / 2), y: r.top + r.height / 2, id: c.dataset.go };
    }, not);
    if (pt) { await page.mouse.click(pt.x, pt.y); await page.waitForTimeout(500); }
    return pt?.id ?? null;
  };
  const run = async (key, op) => {
    const { ctx, page } = await open(1600);
    await press(page, key, 700);
    await press(page, 'f', 700);
    await press(page, 'j', 500); await press(page, 'j', 500);
    const r = { a: await sel(page), rows: await rowIds(page) };
    for (const k of op) {
      if (k === 'click') await inspText(page);
      else await press(page, k, k === 'w' ? 80 : 700);
    }
    let s = await snap(page);
    r.off = !s.panes.some(p => p.shown.includes('files'));
    r.b = await chip(page, r.a);
    s = await snap(page);
    r.insp = !!s.panes[focusOf(s)]?.roots.includes('insp');
    r.t = [];
    for (const k of ['j', 'k']) { await press(page, k, 600); r.t.push(await sel(page)); }
    await ctx.close();
    return r;
  };
  const out = [];
  for (const [name, key, ops] of [['classic', ';', [['w', 'd']]], ['three', '`', [['w', 'd'], ['click', 'w', 'v']]]]) {
    const ref = await run(key, []);
    for (const op of ops) {
      const r = await run(key, op);
      // the step from the stale row would land on the row after the earlier selection
      const stale = r.rows[r.rows.indexOf(r.a) + 1];
      out.push({ name, op: op.join(' '), ref: ref.t, refb: ref.b, a: r.a, b: r.b, t: r.t, off: r.off, insp: r.insp, ok: !ref.off && r.off && r.insp && !!r.b && r.b === ref.b && JSON.stringify(r.t) === JSON.stringify(ref.t) && r.t[0] !== stale });
    }
  }
  check('Files\' keys step from the selection, its pane on screen or not', out.length === 3 && out.every(r => r.ok), out.filter(r => !r.ok));
}
// A commit arriving while the Log's pane is collapsed: the Log is redrawn off screen, as it is on
// screen, so j opens the new commit, and the Log shown again marks it. The commit is an empty one
// in OKT_KB, the throwaway clone tests/env.sh serves, so no file or count changes for later checks
{
  const { execFileSync } = await import('node:child_process');
  const { realpathSync } = await import('node:fs');
  const { homedir } = await import('node:os');
  const real = p => { try { return realpathSync(p); } catch { return null; } };
  const KB = process.env.OKT_KB ? real(process.env.OKT_KB) : null, home = real(homedir() + '/KB');
  if (!KB || KB === home || (home && KB.startsWith(home + '/'))) console.log('skip a commit reaches the Log off screen: needs OKT_KB, a throwaway clone (run it inside tests/env.sh)');
  else {
    const git = (...a) => execFileSync('git', ['-C', KB, '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', '-c', 'user.name=okt tests', '-c', 'user.email=tests@okt.invalid', ...a], { encoding: 'utf8' }).trim();
    const { ctx, page } = await open(1600);
    await press(page, '`', 700);
    await press(page, 'l', 1500);
    await press(page, 'w', 80); await press(page, 'd', 700);
    const s = await snap(page);
    const r = { off: !s.panes.some(p => p.shown.includes('log')) };
    git('commit', '-q', '--allow-empty', '-m', 'Panes: a commit while the Log is off screen');
    r.head = git('rev-parse', 'HEAD');
    r.drawn = await page.waitForFunction(h => document.querySelector('#ltable .crow')?.dataset.h === h, r.head, { timeout: 20000 }).then(() => true, () => false);
    await press(page, 'j', 600);
    r.opened = await page.evaluate(() => (location.hash.match(/[#&]c=([^&]*)/) || [])[1] || '');
    await press(page, 'l', 1200);
    r.marked = await page.evaluate(() => [...document.querySelectorAll('#ltable .crow.on')].map(x => x.dataset.h));
    await ctx.close();
    check('a commit reaches the Log off screen, and j opens it', r.off && r.drawn && r.opened === r.head && r.marked.join() === r.head, r);
  }
}
// The view the keys go to is drawn however it became the key view, its pane on screen or not. The
// five views' pane is saved at a quarter of the window, so a window shrunk from 1000 to 760 px puts
// it under the 200 px floor: the Log made the key view by l there, or drawn on screen first and
// then shrunk under it; the Log bookmarked as the view of a load already under the floor; and focus
// stepped by w w onto a Log collapsed at load. In each, j j j k opens rows 0, 1, 2 and 1, as with
// the Log on screen, and a window grown back over the floor shows row 1 marked
{
  const QUARTER = shown => ({ v: 1, root: { split: 'row', f: 0.25, kids: [{ views: FIVE, shown, collapsed: false }, { views: ['insp'], shown: 'insp', collapsed: false }] } });
  const LOGSHUT = { v: 1, root: { split: 'row', f: 0.54, kids: [{ views: ['graph', 'files', 'table', 'list'], shown: 'graph', collapsed: false }, { split: 'col', f: 0.5, kids: [{ views: ['log'], shown: 'log', collapsed: true }, { views: ['insp'], shown: 'insp', collapsed: false }] }] } };
  const rows = page => page.evaluate(() => [...document.querySelectorAll('#ltable .crow')].map(r => r.dataset.h));
  const opened = page => page.evaluate(() => (location.hash.match(/[#&]c=([^&]*)/) || [])[1] || '');
  const marked = page => page.evaluate(() => [...document.querySelectorAll('#ltable .crow')].map((x, i) => x.classList.contains('on') ? i : -1).filter(i => i >= 0).join());
  // the Log's pane as drawn: whether the Log shows, its width, and whether it holds the focus
  const logPane = async page => {
    const s = await snap(page);
    const i = s.panes.findIndex(p => p.roots.includes('log'));
    return { on: !!s.panes[i]?.shown.includes('log'), w: Math.round(s.panes[i]?.w ?? 0), focused: i >= 0 && focusOf(s) === i };
  };
  const steps = async (page, r) => {
    const rs = await rows(page);
    r.rows = rs.length;
    const o = [];
    for (const k of ['j', 'j', 'j', 'k']) { await press(page, k, 600); o.push(rs.indexOf(await opened(page))); }
    r.opened = o.join();
  };
  const grow = async (page, r) => { await page.setViewportSize({ width: 1000, height: 1000 }); await page.waitForTimeout(1200); r.grown = await logPane(page); r.marked = await marked(page); };
  const fine = r => r.rows >= 4 && r.opened === '0,1,2,1';
  const under = r => !r.pane.on && r.pane.w < 200 && r.pane.focused && fine(r) && r.grown.on && r.marked === '1';
  // made the key view by l: under the floor before l, or after it; and on screen for reference
  const made = [];
  for (const [name, before, after] of [['shrunk, then l', true, false], ['l, then shrunk', false, true], ['on screen', false, false]]) {
    const { ctx, page } = await open(1000, { seed: { arrangement: QUARTER('graph') } });
    if (before) { await page.setViewportSize({ width: 760, height: 1000 }); await page.waitForTimeout(800); }
    await press(page, 'l', 1500);
    if (after) { await page.setViewportSize({ width: 760, height: 1000 }); await page.waitForTimeout(800); }
    const r = { name, pane: await logPane(page) };
    await steps(page, r);
    if (before || after) await grow(page, r);
    else r.marked = await marked(page);
    r.ok = before || after ? under(r) : r.pane.on && r.pane.w >= 200 && fine(r) && r.marked === '1';
    made.push(r);
    await ctx.close();
  }
  check('the Log made the key view under the narrow floor is drawn, and its keys step as on screen', made.length === 3 && made.every(r => r.ok), made);
  // bookmarked as the view at load: under the floor at 760 px, on screen at 1000 px for reference
  const booted = [];
  for (const width of [760, 1000]) {
    const { ctx, page } = await open(width, { seed: { arrangement: QUARTER('log') }, hash: '#v=log' });
    await page.waitForTimeout(1200);
    const r = { width, pane: await logPane(page) };
    await steps(page, r);
    if (width < 1000) await grow(page, r);
    else r.marked = await marked(page);
    r.ok = width < 1000 ? under(r) : r.pane.on && r.pane.focused && fine(r) && r.marked === '1';
    booted.push(r);
    await ctx.close();
  }
  check('a Log bookmarked under the narrow floor is drawn at load, and its keys step as on screen', booted.length === 2 && booted.every(r => r.ok), booted);
  // focus stepped by a pane operation onto a Log collapsed at load, never drawn before
  {
    const { ctx, page } = await open(1600, { seed: { arrangement: LOGSHUT } });
    const r = { before: await logPane(page) };
    await press(page, 'w', 80); await press(page, 'w', 1200);
    r.pane = await logPane(page);
    await steps(page, r);
    r.ok = !r.before.focused && !r.pane.on && r.pane.focused && fine(r);
    await ctx.close();
    check('focus stepped onto a Log collapsed at load draws it, and its keys step as on screen', r.ok, r);
  }
}
// back and forward, on a fresh page (Chromium caps history.length)
{
  const { ctx, page } = await open(1600);
  await press(page, 'f', 700);
  await press(page, 'l', 700);
  await clickTitle(page, 1);
  await press(page, '[', 900);
  const back = await snap(page);
  await press(page, ']', 900);
  const fwd = await snap(page);
  check('back and forward restore the view by placement', back.panes[0].shown.join() === 'files' && focusOf(back) === 0 && fwd.panes[0].shown.join() === 'log' && focusOf(fwd) === 0, { back: [back.panes[0].shown, focusOf(back)], fwd: [fwd.panes[0].shown, focusOf(fwd)] });
  await ctx.close();
}
// a view no pane holds joins the focused pane
{
  const arrangement = { v: 1, root: { split: 'row', f: 0.54, kids: [{ views: ['graph', 'files', 'table', 'list'], shown: 'graph', collapsed: false }, { views: ['insp'], shown: 'insp', collapsed: false }] } };
  const { ctx, page } = await open(1600, { seed: { arrangement } });
  const before = await snap(page);
  await clickTitle(page, 1);
  await press(page, 'l', 900);
  const s = await snap(page);
  check('a view no pane holds is added to the focused pane', !before.panes.some(p => p.tabs.includes('log')) && s.panes[1].tabs.includes('log') && s.panes[1].shown.join() === 'log' && focusOf(s) === 1 && views(s.panes[0]) === 'files,graph,list,table', { before: before.panes.map(p => p.tabs), after: s.panes.map(p => [p.tabs, p.shown]), focus: focusOf(s) });
  await ctx.close();
}

// ---- drag -------------------------------------------------------------------------------------------
{
  const { ctx, page, stored } = await open(1600);
  const R = await page.evaluate(() => { const r = document.getElementById('panes').getBoundingClientRect(); return { left: r.left, right: r.right, top: r.top, W: r.width }; });
  const from = await sash(page);
  const out = {};
  if (from) {
    out.at = from.x - R.left;
    const during = await drag(page, from, { x: R.left + 1000, y: from.y }, () => snap(page));
    out.mid = during.panes.map(p => p.w);
    out.after = (await snap(page)).panes.map(p => p.w);
    await drag(page, await sash(page), { x: R.left + 30, y: from.y });
    out.left = (await snap(page)).panes.map(p => p.w);
    await drag(page, await sash(page), { x: R.right - 30, y: from.y });
    out.right = (await snap(page)).panes.map(p => p.w);
  }
  const mid = from ? (from.x - R.left + 1000) / 2 : 0;
  const tall = await open(899);
  const T = await tall.page.evaluate(() => document.getElementById('panes').getBoundingClientRect().top);
  const at = await sash(tall.page);
  if (at) { await drag(tall.page, at, { x: at.x, y: T + 15 }); out.top = (await snap(tall.page)).panes.map(p => p.h); }
  await tall.ctx.close();
  check('a split drag follows the pointer and stops at the floor', !!from && near(out.at, 0.54 * R.W, 3) && near(out.mid[0], mid, 3) && near(out.after[0], 1000, 2)
    && near(out.left[0], 200, 2) && near(out.right[1], 200, 2) && !!out.top && near(out.top[0], 120, 2), out);
  // a reload, then a new page at the bare URL, show the same arrangement
  if (from) await drag(page, await sash(page), { x: R.left + 700, y: from.y });
  const saved = await page.waitForFunction(k => (localStorage.getItem(k) || '').includes('"arrangement"'), 'okt:' + (await page.evaluate(() => JSON.parse(document.getElementById('boot').textContent).kb || 'kb')), { timeout: 3000 }).then(() => true, () => false);
  const before = await snap(page);
  await page.reload({ waitUntil: 'load' });
  await page.waitForFunction(() => document.querySelectorAll('#panes .pane').length > 0, null, { timeout: 15000 });
  await page.waitForTimeout(800);
  const after = await snap(page);
  const fresh = await ctx.newPage();
  await fresh.goto(OKT, { waitUntil: 'load' });
  await fresh.waitForFunction(() => document.querySelectorAll('#panes .pane').length > 0, null, { timeout: 15000 });
  await fresh.waitForTimeout(800);
  const bare = await snap(fresh);
  const same = (a, b) => a.panes.length === b.panes.length && a.panes.every((p, i) => { const q = b.panes[i]; return [[p.x, q.x], [p.y, q.y], [p.w, q.w], [p.h, q.h]].every(([u, v]) => near(u, v, 1)) && views(p) === views(q) && p.shown.join() === q.shown.join(); });
  check('a dragged split survives a reload within 1 px', saved && near(before.panes[0].w, 700, 2) && same(before, after) && same(before, bare), { saved, before: before.panes.map(p => p.w), after: after.panes.map(p => p.w), bare: bare.panes.map(p => p.w), st: (await stored()).slice(0, 120) });
  await ctx.close();
}

// ---- presets ----------------------------------------------------------------------------------------
const TITLE = 24;
const PRESET = { classic: ';', swapped: "'", stacked: '\\', three: '`' };
// the presets as drawn in a W × H root: [x, y, w, h, views held, view shown (none when collapsed), collapsed]
function preset(name, s, c = 0.54) {
  const W = s.W, H = s.H, FOUR = ['files', 'log', 'table', 'list'];
  return {
    classic: [[0, 0, c * W, H, FIVE, 'graph', false], [c * W, 0, (1 - c) * W, H, ['insp'], 'insp', false]],
    swapped: [[0, 0, 0.46 * W, H, ['insp'], 'insp', false], [0.46 * W, 0, 0.54 * W, H, FIVE, 'graph', false]],
    stacked: [[0, 0, W, H / 2, FIVE, 'graph', false], [0, H / 2, W, H / 2, ['insp'], 'insp', false]],
    three: [[0, 0, W, TITLE, ['graph'], null, true], [0, TITLE, W / 2, H - TITLE, ['insp'], 'insp', false], [W / 2, TITLE, W / 2, H - TITLE, FOUR, 'files', false]],
  }[name];
}
// an arrangement as drawn against its definition: each rect within 2 px, the views held (tabs and
// roots alike), the one shown and its tab marked, and a collapsed pane's body hidden
function grade(s, want) {
  const bad = [];
  if (s.panes.length !== want.length) bad.push(`${s.panes.length} panes, want ${want.length}`);
  want.forEach(([x, y, w, h, held, shown, folded], i) => {
    const p = s.panes[i];
    if (!p) return;
    if (![[p.x, x], [p.y, y], [p.w, w], [p.h, h]].every(([a, b]) => near(a, b, 2))) bad.push(`pane ${i} at ${[p.x, p.y, p.w, p.h].map(v => v.toFixed(1))}, want ${[x, y, w, h].map(v => v.toFixed(1))}`);
    if (views(p) !== held.slice().sort().join() || p.roots.slice().sort().join() !== views(p)) bad.push(`pane ${i} tabs ${p.tabs} roots ${p.roots}, want ${held}`);
    if (folded !== !p.body) bad.push(`pane ${i} body shown ${p.body}, want collapsed ${folded}`);
    if (!folded && (p.shown.join() !== shown || p.on.join() !== shown)) bad.push(`pane ${i} shows ${p.shown} (tab ${p.on}), want ${shown}`);
    if (folded && p.shown.length) bad.push(`pane ${i} collapsed but shows ${p.shown}`);
  });
  return bad;
}
// two drawings alike within 1 px: the rects, the views held, the view shown and its tab, the bodies
const alike = (a, b) => a.panes.length === b.panes.length && a.panes.every((p, i) => {
  const q = b.panes[i];
  return [[p.x, q.x], [p.y, q.y], [p.w, q.w], [p.h, q.h]].every(([u, v]) => near(u, v, 1)) && views(p) === views(q) && p.shown.join() === q.shown.join() && p.on.join() === q.on.join() && p.body === q.body;
});
const drawn = s => s.panes.map(p => `${views(p)}/${p.shown}/${p.body ? 'open' : 'shut'}/${[p.x, p.y, p.w, p.h].map(v => v.toFixed(1))}`).join(' | ');
// wait for the arrangement to be stored, then reload, and open a new page at the bare URL: both
// must draw what was there
async function survives(ctx, page) {
  const kb = await page.evaluate(() => JSON.parse(document.getElementById('boot').textContent).kb || 'kb');
  const stored = await page.waitForFunction(k => (localStorage.getItem(k) || '').includes('"arrangement"'), 'okt:' + kb, { timeout: 3000 }).then(() => true, () => false);
  await page.waitForTimeout(300);
  const before = await snap(page);
  await page.reload({ waitUntil: 'load' });
  await page.waitForFunction(() => document.querySelectorAll('#panes .pane').length > 0 && (document.getElementById('status')?.innerText || '').trim().length > 0, null, { timeout: 15000 });
  await page.waitForTimeout(700);
  const after = await snap(page);
  const fresh = await ctx.newPage();
  fresh.on('pageerror', e => errors.push(e.message));
  await fresh.goto(OKT, { waitUntil: 'load' });
  await fresh.waitForFunction(() => document.querySelectorAll('#panes .pane').length > 0 && (document.getElementById('status')?.innerText || '').trim().length > 0, null, { timeout: 15000 });
  await fresh.waitForTimeout(700);
  const bare = await snap(fresh);
  await fresh.close();
  return { ok: stored && alike(before, after) && alike(before, bare), stored, before: drawn(before), after: drawn(after), bare: drawn(bare) };
}
const marked = page => page.evaluate(() => [...document.querySelectorAll('#presets [data-preset].on')].filter(b => b.checkVisibility()).map(b => b.dataset.preset));
{
  const rects = { classic: [], swapped: [], stacked: [], three: [] };
  const keyed = [], cycled = [], reloads = [], strip = [];
  for (const w of [1600, 900]) {
    const { ctx, page } = await open(w);
    // each key from another preset, classic last
    for (const name of ['swapped', 'stacked', 'three', 'classic']) {
      await press(page, PRESET[name], 700);
      const s = await snap(page);
      const bad = grade(s, preset(name, s));
      rects[name].push({ w, bad });
      keyed.push({ w, name, ok: bad.length === 0 });
      strip.push({ w, name, marked: await marked(page) });
      const r = await survives(ctx, page);
      reloads.push({ w, name, ...r });
    }
    // the cycle: classic -> swapped -> stacked -> three -> classic
    for (const name of ['swapped', 'stacked', 'three', 'classic']) {
      await press(page, 'p', 700);
      const s = await snap(page);
      const bad = grade(s, preset(name, s));
      cycled.push({ w, name, bad });
    }
    await ctx.close();
  }
  for (const name of ['classic', 'swapped', 'stacked', 'three']) check(`${name} matches its rects at 1600 and 900`, rects[name].length === 2 && rects[name].every(r => r.bad.length === 0), rects[name]);
  // from an arrangement that equals no preset, the cycle key starts at classic
  const { ctx, page } = await open(1600);
  await press(page, PRESET.stacked, 700);
  const R = await page.evaluate(() => { const r = document.getElementById('panes').getBoundingClientRect(); return { top: r.top, H: r.height }; });
  const at = await sash(page);
  if (at) await drag(page, at, { x: at.x, y: R.top + 300 });
  const none = await marked(page);
  await press(page, 'p', 700);
  let s = await snap(page);
  const fromNone = grade(s, preset('classic', s));
  check('each preset key applies its preset and p cycles them in order', keyed.length === 8 && keyed.every(k => k.ok) && cycled.length === 8 && cycled.every(c => c.bad.length === 0) && !!at && fromNone.length === 0, { keyed: keyed.filter(k => !k.ok), cycled: cycled.filter(c => c.bad.length), fromNone });
  check('every preset survives a reload within 1 px', reloads.length === 8 && reloads.every(r => r.ok), reloads.filter(r => !r.ok));
  // a reload at once, inside the save's delay: the store is still without the arrangement when the
  // reload starts, so only the flush as the page goes away can keep the preset
  const quick = [];
  for (const name of ['stacked', 'three']) {
    const q = await open(1600);
    await q.page.keyboard.press(PRESET[name]);
    const pending = !(await q.stored()).includes('"arrangement"');
    await q.page.reload({ waitUntil: 'load' });
    await q.page.waitForFunction(() => document.querySelectorAll('#panes .pane').length > 0 && (document.getElementById('status')?.innerText || '').trim().length > 0, null, { timeout: 15000 });
    await q.page.waitForTimeout(500);
    const s = await snap(q.page);
    quick.push({ name, pending, stored: (await q.stored()).includes('"arrangement"'), bad: grade(s, preset(name, s)) });
    await q.ctx.close();
  }
  check('a preset applied just before a reload is kept', quick.every(x => x.pending && x.stored && x.bad.length === 0), quick);
  // the window bar: the preset the arrangement equals is marked, none after a drag, and a click applies one
  const btn = await page.evaluate(() => { const b = document.querySelector('#presets [data-preset="three"]'); const r = b.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
  await page.mouse.click(btn.x, btn.y);
  await page.waitForTimeout(700);
  s = await snap(page);
  const clicked = grade(s, preset('three', s));
  const after = await marked(page);
  check('the window bar marks the current preset and a click applies it', strip.every(x => x.marked.join() === x.name) && none.length === 0 && clicked.length === 0 && after.join() === 'three', { strip: strip.filter(x => x.marked.join() !== x.name), none, clicked, after });
  await ctx.close();
}
// a chosen arrangement is kept at any window width, its shares holding
{
  const { ctx, page } = await open(1600);
  const kept = [];
  for (const name of ['swapped', 'three']) {
    await press(page, PRESET[name], 700);
    for (const w of [880, 1300, 1600]) {
      await page.setViewportSize({ width: w, height: 1000 });
      await page.waitForTimeout(500);
      const s = await snap(page);
      kept.push({ name, w, bad: grade(s, preset(name, s)) });
    }
  }
  check('a window resize keeps the arrangement', kept.every(k => k.bad.length === 0), kept.filter(k => k.bad.length));
  await ctx.close();
}
// three's extras, with the hub selected: the Graph's bar across the top, the Inspector left with
// the card and its tabs, Files right with its filter and named rows, and nothing wider than its
// pane. Returns the problems found.
async function threeExtras(page, s) {
  const bad = [];
  const g = s.panes[0];
  if (!(g && near(g.title.x, 0, 1) && near(g.title.top, 0, 1) && near(g.title.w, s.W, 2) && near(g.h, g.title.h, 1) && g.tabs.join() === 'graph')) bad.push(`graph bar ${JSON.stringify(g && g.title)}`);
  const facts = await page.evaluate(hub => {
    const vis = e => !!e && e.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true }) && e.getBoundingClientRect().width > 0;
    const ps = [...document.querySelectorAll('#panes .pane')].sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top || a.getBoundingClientRect().left - b.getBoundingClientRect().left);
    const within = (e, b) => { const r = e.getBoundingClientRect(); return r.left >= b.left - 1 && r.right <= b.right + 1 && r.top >= b.top - 1 && r.bottom <= b.bottom + 1; };
    const ip = ps[1].getBoundingClientRect(), fp = ps[2].getBoundingClientRect();
    const tree = document.getElementById('tree'), tb = tree.getBoundingClientRect();
    const box = { left: tb.left, top: tb.top, right: tb.left + tree.clientWidth, bottom: tb.top + tree.clientHeight };
    const named = [...tree.querySelectorAll('.trow:not(.thead) .nm')].filter(n => vis(n) && n.innerText.trim() && within(n, box) && n.getBoundingClientRect().width >= 40).length;
    // sideways overflow: the pane scrolls sideways, something outside every scroller lies outside
    // the pane, or a scroller scrolls sideways (the Inspector's body included; free are only the
    // Table grid and the Inspector's rendered tables, code blocks and data tables)
    const wide = p => {
      const pr = p.getBoundingClientRect(), out = [];
      if (p.scrollWidth > p.clientWidth + 1) out.push(`pane ${p.scrollWidth} > ${p.clientWidth}`);
      const scroller = e => ['auto', 'scroll'].includes(getComputedStyle(e).overflowX);
      const walk = (e, inner) => {
        for (const c of e.children) {
          if (!c.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true })) continue;
          const r = c.getBoundingClientRect();
          const name = c.id || c.className || c.tagName;
          if (!inner && r.width > 0 && r.height > 0 && (r.left < pr.left - 1 || r.right > pr.right + 1)) out.push(`${name} outside`);
          if (scroller(c) && !c.closest('#tgrid, #ibody .md table, #ibody .md pre, #ibody .dtable') && c.scrollWidth > c.clientWidth + 1) out.push(`${name} scrolls sideways`);
          if (out.length < 6) walk(c, inner || scroller(c));
        }
      };
      walk(p, false);
      return out;
    };
    return {
      insp: vis(document.getElementById('insp')) && within(document.getElementById('insp'), ip),
      card: within(document.getElementById('ihead'), ip) && document.getElementById('ihead').innerText.includes(hub.split('/').pop().replace(/\.md$/, '')),
      tabs: [...document.querySelectorAll('#itabs button[data-tab]')].filter(b => vis(b) && within(b, ip)).map(b => b.dataset.tab),
      filter: vis(document.getElementById('ffilter')) && within(document.getElementById('ffilter'), fp),
      named, right: fp.left >= ip.right - 1,
      wide: [wide(ps[1]), wide(ps[2])],
    };
  }, hub);
  if (!(facts.insp && facts.card && ['overview', 'content', 'history'].every(t => facts.tabs.includes(t)))) bad.push(`inspector ${JSON.stringify(facts)}`);
  if (!(facts.filter && facts.named >= 10 && facts.right)) bad.push(`files ${JSON.stringify(facts)}`);
  if (facts.wide.some(x => x.length)) bad.push(`overflow ${JSON.stringify(facts.wide)}`);
  return bad;
}
// three, with the hub selected: the Graph's bar across the top, the Inspector left with the card and
// its tabs, Files right with its filter and named rows, and nothing wider than its pane
{
  const out = [];
  for (const w of [1600, 900]) {
    const { ctx, page } = await open(w);
    await press(page, PRESET.three, 700);
    await hashTo(page, '#v=files&s=' + encodeURIComponent(hub));
    const s = await snap(page);
    const bad = grade(s, preset('three', s));
    bad.push(...await threeExtras(page, s));
    out.push({ w, bad });
    await ctx.close();
  }
  check('three shows the Graph bar on top, the Inspector left and Files right', out.every(o => o.bad.length === 0), out);
}
// the Graph moved without a resize (classic -> swapped) still hit-tests where it is drawn: a point
// with a hover tip in classic shows the same tip, beside the pointer, at the same spot of the moved
// canvas
{
  const { ctx, page } = await open(1600);
  const box = () => page.evaluate(() => { const r = document.getElementById('graph').getBoundingClientRect(); return { left: r.left, top: r.top, w: r.width, h: r.height }; });
  const tipAt = async (x, y) => {
    await page.mouse.move(x, y);
    await page.waitForTimeout(40);
    return page.evaluate(() => { const e = document.getElementById('gtip'); if (!e || e.hidden || !e.checkVisibility()) return null; const r = e.getBoundingClientRect(); return { text: e.innerText, l: r.left, t: r.top }; });
  };
  const a = await box();
  let found = null;
  for (let gy = 1; gy < 12 && !found; gy++) for (let gx = 1; gx < 16 && !found; gx++) {
    const t = await tipAt(a.left + a.w * gx / 16, a.top + a.h * gy / 12);
    if (t) found = { gx, gy, text: t.text };
  }
  await page.mouse.move(2, 2);
  await press(page, PRESET.swapped, 900);
  const b = await box();
  let moved = null;
  if (found) {
    const x = b.left + b.w * found.gx / 16, y = b.top + b.h * found.gy / 12;
    const t = await tipAt(x, y);
    moved = t && { text: t.text, near: Math.abs(t.l - x) < 400 && Math.abs(t.t - y) < 400 && t.l >= b.left - 2 };
  }
  check('a Graph moved without a resize still hit-tests where it is drawn', !!found && near(b.left - a.left, 0.46 * 1600, 2) && near(b.w, a.w, 1) && !!moved && moved.text === found.text && moved.near, { found, moved, a, b });
  await ctx.close();
}
// the help pairs each preset key with its preset, p with the cycle, and the leader's keys
{
  const { ctx, page } = await open(1600);
  await press(page, '?', 400);
  const rows = await page.evaluate(() => [...document.querySelectorAll('#help dt')].map(dt => ({ k: dt.innerText.trim(), d: (dt.nextElementSibling?.innerText || '').toLowerCase() })));
  const has = (k, word) => rows.some(r => r.k === k && r.d.includes(word));
  check('the help lists the preset keys', has(';', 'classic') && has("'", 'swapped') && has('\\', 'stacked') && has('`', 'three') && has('p', 'cycle') && has('w', 'pane') && has('w d', 'collapse') && !rows.some(r => r.d.includes('in lists')), rows.slice(0, 40));
  await ctx.close();
}

// ---- collapse -----------------------------------------------------------------------------------------
{
  const { ctx, page } = await open(1600);
  const lead = async k => { await press(page, 'w', 80); await press(page, k, 600); };
  const { W, H } = await snap(page);
  await clickTitle(page, 0);
  await lead('d');
  let s = await snap(page);
  const strip = { w: s.panes[0].w, title: s.panes[0].title, body: s.panes[0].body, tabs: s.panes[0].tabs, right: s.panes[1].w, focus: focusOf(s) };
  const stripOk = near(strip.w, TITLE, 1) && near(strip.title.w, TITLE, 1) && strip.title.h > 400 && !strip.body && views(s.panes[0]) === FIVE.slice().sort().join() && near(strip.right, W - TITLE, 2) && strip.focus === 0;
  await lead('d');
  s = await snap(page);
  const back = grade(s, preset('classic', s));
  await press(page, PRESET.stacked, 700);
  await clickTitle(page, 0);
  await lead('d');
  s = await snap(page);
  const bar = { h: s.panes[0].h, title: s.panes[0].title, body: s.panes[0].body, below: s.panes[1].h, focus: focusOf(s) };
  const barOk = near(bar.h, TITLE, 1) && near(bar.title.h, TITLE, 1) && near(bar.title.w, W, 2) && !bar.body && near(bar.below, H - TITLE, 2) && bar.focus === 0;
  const kept = await survives(ctx, page);
  await clickTitle(page, 0);
  await lead('d');
  s = await snap(page);
  const back2 = grade(s, preset('stacked', s));
  check('collapse shrinks a pane to its title bar or a strip and expand restores it', stripOk && back.length === 0 && barOk && kept.ok && back2.length === 0, { strip, back, bar, kept, back2 });
  // the accordion: collapsing the Inspector beside the collapsed views pane expands that pane
  await clickTitle(page, 0);
  await lead('d');
  await clickTitle(page, 1);
  await lead('d');
  s = await snap(page);
  const acc = s.panes.map(p => [p.h, p.body]);
  // and three's collapsed Graph: g expands it to 40% of the height and focuses it
  await press(page, PRESET.three, 700);
  await press(page, 'g', 800);
  const t = await snap(page);
  check('collapsing beside a collapsed pane expands that pane', near(acc[0][0], H - TITLE, 2) && acc[0][1] && near(acc[1][0], TITLE, 1) && !acc[1][1] && focusOf(s) === 1
    && near(t.panes[0].h, 0.4 * H, 2) && t.panes[0].shown.join() === 'graph' && focusOf(t) === 0, { acc, focus: focusOf(s), three: t.panes.map(p => [p.h, p.shown]) });
  await ctx.close();
}
// the pane leader: a modifier alone leaves it waiting; Escape cancels it; another key cancels it
// and is handled as usual; two seconds clear it
{
  const { ctx, page } = await open(1600);
  const left = async () => { const s = await snap(page); return { w: s.panes[0].w, body: s.panes[0].body, shown: s.panes[0].shown.join() }; };
  await clickTitle(page, 0);
  await press(page, 'w', 80); await press(page, 'Shift', 80); await press(page, 'd', 600);
  const shifted = await left();
  await press(page, 'w', 80); await press(page, 'd', 600);
  await press(page, 'w', 80); await press(page, 'Escape', 80); await press(page, 'd', 600);
  const escaped = await left();
  await press(page, 'w', 80); await press(page, 'f', 700);
  const other = await left();
  await press(page, 'w', 2300); await press(page, 'd', 600);
  const late = await left();
  check('the pane leader waits through a modifier, Escape cancels it, another key is handled as usual, and it times out', near(shifted.w, TITLE, 1) && !shifted.body && escaped.body && near(escaped.w, 864, 2) && other.body && other.shown === 'files' && late.body && near(late.w, 864, 2), { shifted, escaped, other, late });
  await ctx.close();
}

// ---- operations -----------------------------------------------------------------------------------------
// count toasts as they are added (they expire, so the live list can't be counted)
const watchToasts = page => page.evaluate(() => {
  window.__toasts = 0;
  new MutationObserver(ms => { for (const m of ms) for (const n of m.addedNodes) if (n.classList?.contains('toast')) window.__toasts++; })
    .observe(document.getElementById('toasts'), { childList: true });
});
const toastCount = page => page.evaluate(() => window.__toasts || 0);
const SIX = [...FIVE, 'insp'];
{
  const { ctx, page } = await open(1600);
  const lead = async k => { await press(page, 'w', 80); await press(page, k, 700); };
  await watchToasts(page);
  const { W, H } = await snap(page);
  // focus steps through the panes in reading order, both ways, and wraps
  await clickTitle(page, 0);
  const steps = [];
  for (const k of ['w', 'w', 'W']) { await lead(k); steps.push(focusOf(await snap(page))); }
  check('w w and w W step the focus through the panes', steps.join() === '1,0,1', steps);
  // swap: the two exchange their contents, the slots keep their sizes, focus follows the contents
  await clickTitle(page, 0);
  await lead('x');
  let s = await snap(page);
  const swapped = grade(s, [[0, 0, 0.54 * W, H, ['insp'], 'insp', false], [0.54 * W, 0, 0.46 * W, H, FIVE, 'graph', false]]);
  const swapFocus = focusOf(s);
  await lead('x');
  s = await snap(page);
  const swappedBack = grade(s, preset('classic', s));
  check('swap exchanges the focused pane with its neighbour', swapped.length === 0 && swapFocus === 1 && swappedBack.length === 0 && focusOf(s) === 0, { swapped, swapFocus, swappedBack, focus: focusOf(s) });
  // flip: side by side becomes stacked, keeping the order and the share, and back
  await lead('r');
  s = await snap(page);
  const flipped = grade(s, [[0, 0, W, 0.54 * H, FIVE, 'graph', false], [0, 0.54 * H, W, 0.46 * H, ['insp'], 'insp', false]]);
  await lead('r');
  s = await snap(page);
  const flippedBack = grade(s, preset('classic', s));
  check('flip turns a split between side by side and stacked', flipped.length === 0 && flippedBack.length === 0, { flipped, flippedBack });
  // move: the shown view goes to the neighbour as its shown tab, and the source shows its next view
  await clickTitle(page, 0);
  await press(page, 'g', 700);
  await lead('v');
  s = await snap(page);
  const moved = grade(s, [[0, 0, 0.54 * W, H, ['files', 'log', 'table', 'list'], 'files', false], [0.54 * W, 0, 0.46 * W, H, ['insp', 'graph'], 'graph', false]]);
  check('moving a view shows it in the neighbour', moved.length === 0 && focusOf(s) === 1, { moved, focus: focusOf(s) });
  // close: the views join the neighbour, which shows the closed pane's view and takes the space
  await press(page, PRESET.classic, 700);
  await clickTitle(page, 0);
  await press(page, 't', 900);
  await lead('q');
  s = await snap(page);
  const closed = grade(s, [[0, 0, W, H, SIX, 'table', false]]);
  check('close gives the views to the neighbour and the neighbour takes the space', closed.length === 0 && focusOf(s) === 0, { closed, focus: focusOf(s) });
  // the only pane: close, move and collapse are refused, each saying so
  const refused = [];
  for (const k of ['q', 'v', 'V', 'd']) {
    const t0 = await toastCount(page);
    await lead(k);
    const after = await snap(page);
    refused.push({ k, same: alike(s, after), toast: (await toastCount(page)) > t0 });
  }
  check('closing the only pane is refused', refused[0].same && refused[0].toast && refused[3].same && refused[3].toast, refused);
  check('moving the only pane\'s view is refused', refused[1].same && refused[1].toast && refused[2].same && refused[2].toast, refused);
  // moving a pane's last view closes the pane: three's Inspector into Files' pane
  await press(page, PRESET.three, 700);
  await clickTitle(page, 1);
  await lead('v');
  s = await snap(page);
  const last = grade(s, [[0, 0, W, TITLE, ['graph'], null, true], [0, TITLE, W, H - TITLE, ['files', 'log', 'table', 'list', 'insp'], 'insp', false]]);
  check('moving a pane\'s last view closes the pane', last.length === 0 && focusOf(s) === 1, { last, focus: focusOf(s) });
  const kept = await survives(ctx, page);
  check('the arrangement after operations survives a reload within 1 px', kept.ok, kept);
  await ctx.close();
}
// any view can be moved into any pane: each of the five from classic's left pane into the
// Inspector's, then the Inspector into the left pane
{
  const { ctx, page } = await open(1600);
  const lead = async k => { await press(page, 'w', 80); await press(page, k, 700); };
  const into = [];
  for (const v of FIVE) {
    await clickTitle(page, 0);
    const pt = await page.evaluate(v => { const ps = [...document.querySelectorAll('#panes .pane')].sort((a, b) => a.getBoundingClientRect().left - b.getBoundingClientRect().left); const b = ps[0].querySelector(`.ptitle button[data-view="${v}"]`); if (!b) return null; const r = b.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; }, v);
    if (pt) { await page.mouse.click(pt.x, pt.y); await page.waitForTimeout(700); }
    const before = await snap(page);
    const had = before.panes[before.panes.length - 1].tabs.includes(v);
    await lead('v');
    const s = await snap(page);
    const dest = s.panes[s.panes.length - 1];
    into.push({ v, had, held: dest.tabs.includes(v), shown: dest.shown.join(), left: s.panes.length > 1 && s.panes[0].tabs.includes(v) });
  }
  await press(page, PRESET.classic, 700);
  await clickTitle(page, 1);
  await lead('v');
  const s = await snap(page);
  into.push({ v: 'insp', had: false, held: s.panes[0].tabs.includes('insp'), shown: s.panes[0].shown.join(), left: s.panes.length !== 1 });
  check('any view can be moved into any pane', into.length === 6 && into.every(x => !x.had && x.held && x.shown === x.v && !x.left), into);
  await ctx.close();
}
// nested floors: the Graph beside the Inspector and Files side by side (three, g, then w r) stops
// where the two inner panes reach 200 px each, and the outer sash is met first
{
  const { ctx, page } = await open(1600);
  await press(page, PRESET.three, 700);
  await press(page, 'g', 800);
  await press(page, 'w', 80); await press(page, 'r', 700);
  const R = await page.evaluate(() => { const r = document.getElementById('panes').getBoundingClientRect(); return { left: r.left, right: r.right, top: r.top, W: r.width }; });
  const outer = await sash(page);
  const out = { at: outer && outer.x - R.left };
  if (outer) {
    await drag(page, outer, { x: R.right - 10, y: outer.y });
    out.right = (await snap(page)).panes.map(p => p.w);
    await drag(page, await sash(page), { x: R.left + 10, y: outer.y });
    out.left = (await snap(page)).panes.map(p => p.w);
  }
  check('a drag stops where a nested split\'s panes reach their floor', !!outer && near(out.at, 0.4 * R.W, 3) && out.right.length === 3 && near(out.right[0], R.W - 400, 2) && near(out.right[1], 200, 2) && near(out.right[2], 200, 2) && near(out.left[0], 200, 2), out);
  await ctx.close();
}

// ---- fitting ----------------------------------------------------------------------------------------
// The pane i in reading order, as an expression for page.evaluate
const PANE = i => `[...document.querySelectorAll('#panes .pane')].sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top || a.getBoundingClientRect().left - b.getBoundingClientRect().left)[${i}]`;
// what spills out of pane i sideways: its own scroll width, any visible element outside its rect,
// and any scroller in it that scrolls sideways, the Inspector's body included. Free are only the
// Table's grid and, inside the Inspector, the blocks meant to scroll on their own: a rendered
// table, a code block, a data table
const FREE = '#tgrid, #ibody .md table, #ibody .md pre, #ibody .dtable';
const spill = (page, i) => page.evaluate(([q, FREE]) => {
  const p = eval(q), pr = p.getBoundingClientRect(), bad = [];
  if (p.scrollWidth > p.clientWidth + 1) bad.push(`pane ${p.scrollWidth} > ${p.clientWidth}`);
  const free = e => !!e.closest(FREE);
  const scrolls = e => ['auto', 'scroll'].includes(getComputedStyle(e).overflowX);
  const walk = (e, inside) => {
    for (const c of e.children) {
      if (!c.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true })) continue;
      const r = c.getBoundingClientRect();
      const name = c.tagName.toLowerCase() + (c.id ? '#' + c.id : '') + (c.className && typeof c.className === 'string' ? '.' + c.className.trim().split(/\s+/).join('.') : '');
      if (!inside && r.width > 0 && r.height > 0 && (r.left < pr.left - 1 || r.right > pr.right + 1)) bad.push(`${name} at [${r.left.toFixed(0)}, ${r.right.toFixed(0)}] past [${pr.left.toFixed(0)}, ${pr.right.toFixed(0)}]`);
      if (scrolls(c) && !free(c) && c.scrollWidth > c.clientWidth + 1) bad.push(`${name} scrolls sideways ${c.scrollWidth} > ${c.clientWidth}`);
      walk(c, inside || scrolls(c));
    }
  };
  walk(p, false);
  return bad.slice(0, 6);
}, [PANE(i), FREE]);
// the Graph as drawn: visible, its centre and two other points hitting the canvas, the canvas
// filling its pane body, its backing store at the device pixel ratio, and pixels painted
const canvas = page => page.evaluate(() => {
  const c = document.getElementById('graph'), body = c.closest('.pbody');
  const r = c.getBoundingClientRect(), b = body ? body.getBoundingClientRect() : null;
  const hit = [[0.5, 0.5], [0.25, 0.75], [0.75, 0.25]].every(([u, v]) => document.elementFromPoint(r.left + u * r.width, r.top + v * r.height) === c);
  let painted = 0;
  const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
  for (let k = 3; k < d.length; k += 4) if (d[k] > 0) painted++;
  return { shown: c.checkVisibility({ checkVisibilityCSS: true }) && r.width > 0, fills: !!b && Math.abs(r.width - b.width) <= 1 && Math.abs(r.height - b.height) <= 1, store: [c.width, c.height], css: [+r.width.toFixed(1), +r.height.toFixed(1)], dpr: devicePixelRatio, hit, painted: +(painted / (c.width * c.height)).toFixed(3) };
});
const fills = g => g.shown && g.fills && g.hit && near(g.store[0], g.css[0] * g.dpr, 2) && near(g.store[1], g.css[1] * g.dpr, 2) && g.painted > 0.01;
const pixels = page => page.evaluate(() => { const c = document.getElementById('graph'); const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let h = 0; for (let k = 0; k < d.length; k += 101) h = (h * 31 + d[k]) | 0; return h; });
const centre = (page, sel) => page.evaluate(sel => { const r = document.querySelector(sel).getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; }, sel);
const walking = page => page.evaluate(() => { const h = document.getElementById('ghint'); return !!h && !h.hidden && /^1\/\d+ · /.test(h.textContent); });
const fileRow = page => page.evaluate(() => document.querySelector('#tree .trow.on')?.dataset.id || null);
// the Graph and Files side by side in three, at DPR 2: both drawn, the Graph zooming under the
// wheel while Files scrolls under it, and the keys going to the focused pane's view
{
  const { ctx, page } = await open(1600, { dpr: 2 });
  await press(page, PRESET.three, 700);
  await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
  await hashTo(page, '#v=files&s=' + encodeURIComponent(deep));
  await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
  const s = await snap(page);
  const g0 = await canvas(page);
  const h0 = await pixels(page);
  const at = await centre(page, '#graph');
  await page.mouse.move(at.x, at.y); await page.mouse.wheel(0, -300); await page.waitForTimeout(500);
  const zoomed = (await pixels(page)) !== h0;
  await page.evaluate(() => { document.getElementById('tree').scrollTop = 0; });
  const t = await centre(page, '#tree');
  await page.mouse.move(t.x, t.y); await page.mouse.wheel(0, 300); await page.waitForTimeout(400);
  const scrolled = await page.evaluate(() => document.getElementById('tree').scrollTop);
  const g1 = await canvas(page);
  const rows = await page.evaluate(() => document.querySelectorAll('#tree .trow:not(.thead)').length);
  check('the Graph and Files render and respond side by side', s.panes.length === 3 && s.panes[0].shown.join() === 'graph' && s.panes[2].shown.join() === 'files' && fills(g0) && zoomed && scrolled > 0 && fills(g1) && rows >= 10, { shown: s.panes.map(p => p.shown), g0, zoomed, scrolled, g1, rows });
  // keys: Files focused, j moves its row and walks nothing; the Graph focused, j walks it; the
  // Log shown and focused in Files' pane, j selects a commit and moves no Files row
  const keyed = {};
  await clickTitle(page, 2);
  const rowA = await fileRow(page);
  await press(page, 'j', 500);
  keyed.files = { moved: (await fileRow(page)) !== rowA, walk: await walking(page) };
  await clickTitle(page, 0);
  await press(page, 'j', 500);
  keyed.graph = { walk: await walking(page) };
  await press(page, 'Escape', 300);
  await press(page, 'l', 1500);
  const rowB = await fileRow(page);
  await press(page, 'j', 600);
  keyed.log = await page.evaluate(() => ({ commit: !!document.querySelector('#ltable .crow.on') }));
  keyed.log.files = (await fileRow(page)) === rowB;
  keyed.log.walk = await walking(page);
  check('keys act on the focused pane\'s view', keyed.files.moved && !keyed.files.walk && keyed.graph.walk && keyed.log.commit && keyed.log.files && !keyed.log.walk, keyed);
  // the canvas after every change of its pane's size
  const after = {};
  await press(page, 'g', 700);
  const R = await page.evaluate(() => { const r = document.getElementById('panes').getBoundingClientRect(); return { top: r.top, H: r.height }; });
  const top = await sash(page);
  if (top) { await drag(page, top, { x: top.x, y: R.top + 0.65 * R.H }); after.drag = await canvas(page); }
  await press(page, PRESET.stacked, 700); after.preset = await canvas(page);
  await clickTitle(page, 1); await press(page, 'w', 80); await press(page, 'd', 700); after.collapse = await canvas(page);
  after.collapsedTall = (await snap(page)).panes[0].h;
  await press(page, 'w', 80); await press(page, 'd', 700); after.expand = await canvas(page);
  await press(page, PRESET.classic, 700); after.classic = await canvas(page);
  await page.setViewportSize({ width: 1100, height: 800 }); await page.waitForTimeout(700); after.window = await canvas(page);
  const H2 = await page.evaluate(() => document.getElementById('panes').getBoundingClientRect().height);
  check('the Graph canvas fills its pane after every size change', !!top && Object.keys(after).length === 7 && ['drag', 'preset', 'collapse', 'expand', 'classic', 'window'].every(k => fills(after[k])) && near(after.collapsedTall, s.H - TITLE, 2) && near(after.window.css[0], 0.54 * 1100, 2) && near(after.window.css[1], H2 - TITLE, 2), after);
  await ctx.close();
}
// every view in a 200 px pane: classic's split dragged to the floor on each side
{
  const { ctx, page } = await open(1600);
  await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
  const R = await page.evaluate(() => { const r = document.getElementById('panes').getBoundingClientRect(); return { left: r.left, right: r.right, top: r.top }; });
  const out = [];
  await drag(page, await sash(page), { x: R.right - 5, y: R.top + 300 });
  let s = await snap(page);
  out.push({ at: 'insp', w: s.panes[1].w });
  for (const k of ['o', 'c', 'h']) {
    await press(page, k, 900);
    out.push({ v: 'insp ' + k, shown: (await snap(page)).panes[1].shown.join(), bad: await spill(page, 1) });
  }
  await drag(page, await sash(page), { x: R.left + 5, y: R.top + 300 });
  s = await snap(page);
  out.push({ at: 'views', w: s.panes[0].w });
  for (const [k, v] of [['g', 'graph'], ['f', 'files'], ['l', 'log'], ['t', 'table']]) {
    await press(page, k, 1200);
    out.push({ v, shown: (await snap(page)).panes[0].shown.join(), bad: await spill(page, 0) });
  }
  await page.keyboard.press('/'); await page.keyboard.type('FIND plain'); await page.keyboard.press('Enter'); await page.waitForTimeout(1500);
  out.push({ v: 'list', shown: (await snap(page)).panes[0].shown.join(), rows: await page.evaluate(() => document.querySelectorAll('#list .lrow').length), bad: await spill(page, 0) });
  const want = { 'insp o': 'insp', 'insp c': 'insp', 'insp h': 'insp', graph: 'graph', files: 'files', log: 'log', table: 'table', list: 'list' };
  check('every view fits a 200 px pane without overflow', near(out[0].w, 200, 2) && near(out[4].w, 200, 2) && out.filter(o => o.v).length === 8 && out.filter(o => o.v).every(o => o.shown === want[o.v] && o.bad.length === 0) && out.at(-1).rows > 0, out);
  // the Inspector at 200, 240 and 272 px, on each tab and on Content with the blame on: its body
  // never scrolls sideways and nothing spills past the pane; the Overview's Last commit row and
  // the Content toolbar's buttons are there to be graded
  const ib = [];
  await press(page, 'g', 700);
  for (const w of [200, 240, 272]) {
    await drag(page, await sash(page), { x: R.right - w, y: R.top + 300 });
    for (const [k, t] of [['o', 'overview'], ['c', 'content'], ['b', 'blame'], ['b', 'content'], ['h', 'history']]) {
      await press(page, k, 900);
      const m = await page.evaluate(() => { const b = document.getElementById('ibody'); return { sw: b.scrollWidth, cw: b.clientWidth, last: !!b.querySelector('.hrow.last'), blame: !!b.querySelector('.ctool [data-blame]') }; });
      ib.push({ w: +(await snap(page)).panes[1].w.toFixed(1), t, ...m, bad: await spill(page, 1) });
    }
  }
  check('the Inspector fits its pane at 200, 240 and 272 px on every tab', ib.length === 15 && ib.every(x => near(x.w, [200, 240, 272][Math.floor(ib.indexOf(x) / 5)], 2) && x.sw <= x.cw && x.bad.length === 0) && ib.filter(x => x.t === 'overview').every(x => x.last) && ib.filter(x => x.t === 'content').every(x => x.blame), ib.filter(x => x.sw > x.cw || x.bad.length || !near(x.w, [200, 240, 272][Math.floor(ib.indexOf(x) / 5)], 2)).length ? ib.filter(x => x.sw > x.cw || x.bad.length) : ib);
  // the Log chart: shown in a pane dragged wider, then in a narrower window, drawn each time for
  // the chart's own width with its bars inside it
  const chart = page => page.evaluate(() => {
    const c = document.getElementById('lchart'), svg = c.querySelector('svg'), cr = c.getBoundingClientRect();
    const bars = [...c.querySelectorAll('rect.bar')].map(b => b.getBoundingClientRect());
    return { cw: c.clientWidth, vb: svg?.viewBox?.baseVal?.width ?? null, bars: bars.length, inside: bars.every(b => b.left >= cr.left - 0.5 && b.right <= cr.right + 0.5) };
  });
  await press(page, 'l', 900);
  const lc = [];
  for (const x of [700, 1100]) {
    await drag(page, await sash(page), { x: R.left + x, y: R.top + 300 });
    await page.waitForTimeout(500);
    lc.push({ pane: (await snap(page)).panes[0].w, ...(await chart(page)) });
  }
  await page.setViewportSize({ width: 1200, height: 1000 }); await page.waitForTimeout(800);
  lc.push({ pane: (await snap(page)).panes[0].w, ...(await chart(page)) });
  check('the Log chart fits its pane after a resize', lc.length === 3 && lc.every(c => c.vb !== null && near(c.vb, c.cw, 2) && near(c.cw, c.pane, 2) && c.bars > 0 && c.inside) && Math.abs(lc[0].cw - lc[1].cw) > 300, lc);
  await ctx.close();
}
// The Inspector's source view with the blame on, for a code file and for a card after s then b, at
// 200, 240 and 268 px: the blame gutter drawn, #ibody never scrolling sideways, nothing spilling
// past the pane, and the code column at least 120 px wide. Then the time-travel bar at 200 px:
// every button inside #ibody, which doesn't scroll sideways
{
  const CODE = /\.(py|sh|rs|js|mjs|ts)$/;
  const code = idx.nodes.filter(n => n.t === 'file' && CODE.test(n.id)).map(n => n.id).sort()[0];
  const { ctx, page } = await open(1600);
  await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
  const R = await page.evaluate(() => { const r = document.getElementById('panes').getBoundingClientRect(); return { right: r.right, top: r.top }; });
  const source = page => page.evaluate(() => {
    const b = document.getElementById('ibody'), l = b.querySelector('.src.blame .l');
    const w = sel => +(l?.querySelector(sel)?.getBoundingClientRect().width ?? 0).toFixed(1);
    return { sw: b.scrollWidth, cw: b.clientWidth, gutter: w('.bl'), code: w('.tx'), lines: b.querySelectorAll('.src.blame .l .bl').length };
  });
  const out = [];
  for (const w of [200, 240, 268]) {
    await drag(page, await sash(page), { x: R.right - w, y: R.top + 300 });
    const pw = +(await snap(page)).panes[1].w.toFixed(1);
    await hashTo(page, '#v=graph&s=' + encodeURIComponent(code));
    await press(page, 'c', 1200); await press(page, 'b', 1800);
    out.push({ w, pw, of: 'code', ...(await source(page)), bad: await spill(page, 1) });
    await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
    await press(page, 'c', 1200); await press(page, 's', 900); await press(page, 'b', 1800);
    out.push({ w, pw, of: 'card', ...(await source(page)), bad: await spill(page, 1) });
  }
  check('source with the blame fits the Inspector at 200, 240 and 268 px, the code column at least 120 px', !!code && out.length === 6 && out.every(o => near(o.pw, o.w, 2) && o.lines > 0 && o.gutter > 0 && o.code >= 120 && o.sw <= o.cw && o.bad.length === 0), { code, out });
  // time travel on the card, one change back, in a 200 px Inspector
  await drag(page, await sash(page), { x: R.right - 200, y: R.top + 300 });
  await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
  await press(page, 'c', 1200); await press(page, ',', 1800);
  const tr = await page.evaluate(() => {
    const b = document.getElementById('ibody'), br = b.getBoundingClientRect(), bar = b.querySelector('.travel');
    const btn = bar ? [...bar.querySelectorAll('button')].map(e => { const r = e.getBoundingClientRect(); return { t: e.textContent, l: +r.left.toFixed(1), r: +r.right.toFixed(1) }; }) : [];
    return { bar: !!bar, sw: b.scrollWidth, cw: b.clientWidth, left: +br.left.toFixed(1), right: +(br.left + b.clientWidth).toFixed(1), btn };
  });
  tr.pw = +(await snap(page)).panes[1].w.toFixed(1);
  tr.bad = await spill(page, 1);
  check('the time-travel bar fits a 200 px Inspector', tr.bar && near(tr.pw, 200, 2) && tr.btn.length === 3 && tr.btn.every(b => b.l >= tr.left - 0.5 && b.r <= tr.right + 0.5) && tr.sw <= tr.cw && tr.bad.length === 0, tr);
  await press(page, 'Escape', 600);
  await ctx.close();
}
// a pane under 200 px: the views pane at its floor, then a 440 px window makes it 55 px wide; it
// keeps a title bar with every tab, hides its body, spills nothing, and grown back shows its view
// as it is now (a selection made meanwhile marked in Files)
{
  const { ctx, page } = await open(1600);
  await hashTo(page, '#v=files&s=' + encodeURIComponent(hub));
  const R = await page.evaluate(() => { const r = document.getElementById('panes').getBoundingClientRect(); return { left: r.left, top: r.top }; });
  await drag(page, await sash(page), { x: R.left + 5, y: R.top + 300 });
  await page.setViewportSize({ width: 440, height: 1000 }); await page.waitForTimeout(700);
  let s = await snap(page);
  const tabsIn = await page.evaluate(q => { const p = eval(q), pr = p.getBoundingClientRect(); return [...p.querySelectorAll('.ptitle button[data-view]')].filter(b => b.checkVisibility()).map(b => { const r = b.getBoundingClientRect(); return r.width > 0 && r.left >= pr.left - 1 && r.right <= pr.right + 1; }); }, PANE(0));
  const narrow = { w: s.panes[0].w, body: s.panes[0].body, title: s.panes[0].title, tabs: s.panes[0].tabs, tabsIn, other: s.panes[1].body && s.panes[1].shown.join(), bad: [await spill(page, 0), await spill(page, 1)] };
  check('a pane under 200 px hides its body behind its title bar', narrow.w < 200 && !narrow.body && narrow.title.w > 0 && near(narrow.title.h, TITLE, 1) && narrow.tabs.slice().sort().join() === FIVE.slice().sort().join() && tabsIn.length === 5 && tabsIn.every(Boolean) && narrow.other === 'insp' && narrow.bad.every(b => b.length === 0), narrow);
  // a selection while Files is out of sight, then the window grown back
  const other = md.find(n => n.id !== hub && !n.id.includes('/'))?.id || md.find(n => n.id !== hub).id;
  await hashTo(page, '#v=files&s=' + encodeURIComponent(other));
  const hidden = await fileRow(page);
  await page.setViewportSize({ width: 1600, height: 1000 }); await page.waitForTimeout(900);
  s = await snap(page);
  const back = { w: s.panes[0].w, shown: s.panes[0].shown.join(), row: await fileRow(page), hidden, other };
  check('a pane grown back over 200 px shows its view', back.w >= 200 && back.shown === 'files' && back.row === other && hidden !== other, back);
  await ctx.close();
}
// the legend by the Graph pane's width, not the window's
{
  const legend = page => page.evaluate(() => { const l = document.getElementById('glegend'); return !!l && l.checkVisibility({ checkVisibilityCSS: true }) && l.getBoundingClientRect().width > 0; });
  let { ctx, page } = await open(1600);
  const got = { wide: await legend(page) };
  const R = await page.evaluate(() => { const r = document.getElementById('panes').getBoundingClientRect(); return { left: r.left, top: r.top }; });
  for (const x of [470, 490]) {
    await drag(page, await sash(page), { x: R.left + x, y: R.top + 300 });
    got[x] = { pane: (await snap(page)).panes[0].w, shown: await legend(page) };
  }
  await ctx.close();
  ({ ctx, page } = await open(899));
  got.window899 = { pane: (await snap(page)).panes[0].w, shown: await legend(page) };
  await ctx.close();
  check('the legend hides by the Graph pane\'s width', got.wide && got[470].pane < 480 && !got[470].shown && got[490].pane >= 480 && got[490].shown && got.window899.pane >= 480 && got.window899.shown, got);
}

// ---- what Geoffrey sees ---------------------------------------------------------------------------
// Every preset at 1600 x 1000 and 900 x 1000, with a card selected, graded from rects alone: each
// pane's place and size against its definition within 2 px; the views it holds, read from the tabs
// drawn inside its title bar; every collapsed pane at its title bar's height; and for three, the
// Graph's bar across the top, the Inspector left with the card and its tabs, Files right with its
// filter and at least 10 named rows, and nothing wider than either pane
{
  // the views whose tab is drawn inside each pane's title bar, in reading order
  const titled = page => page.evaluate(q => [...eval(q)].map(p => {
    const t = p.querySelector(':scope > .ptitle').getBoundingClientRect();
    return [...p.querySelectorAll(':scope > .ptitle button[data-view]')].filter(b => {
      const r = b.getBoundingClientRect();
      return b.checkVisibility({ checkVisibilityCSS: true }) && r.width > 0 && r.left >= t.left - 1 && r.right <= t.right + 1 && r.top >= t.top - 1 && r.bottom <= t.bottom + 1;
    }).map(b => b.dataset.view).sort().join();
  }), `[...document.querySelectorAll('#panes .pane')].sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top || a.getBoundingClientRect().left - b.getBoundingClientRect().left)`);
  const out = [];
  for (const w of [1600, 900]) {
    const { ctx, page } = await open(w);
    await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
    for (const name of ['swapped', 'stacked', 'three', 'classic']) {
      await press(page, PRESET[name], 800);
      const s = await snap(page);
      const want = preset(name, s);
      const bad = grade(s, want);
      const tabs = await titled(page);
      want.forEach(([, , , , held], i) => { if (tabs[i] !== held.slice().sort().join()) bad.push(`pane ${i} title bar holds ${tabs[i]}, want ${held}`); });
      want.forEach(([, , , , , , folded], i) => { const p = s.panes[i]; if (folded && p && !(near(p.h, TITLE, 1) && near(p.title.h, TITLE, 1))) bad.push(`pane ${i} collapsed at ${p.h} (title ${p.title.h})`); });
      if (name === 'three') bad.push(...await threeExtras(page, s));
      out.push({ w, name, panes: s.panes.length, bad });
    }
    await ctx.close();
  }
  check('every preset at both sizes graded from rects', out.length === 8 && out.every(o => o.panes > 1 && o.bad.length === 0), out.filter(o => o.bad.length).length ? out.filter(o => o.bad.length) : out.length);
}

check('no page errors', errors.length === 0, errors.slice(0, 5));
console.log(fails ? `${fails} failed` : 'all passed');
await browser.close();
process.exit(fails ? 1 : 0);
