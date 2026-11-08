// The navigation keys (README § Keys), checked on their own against a running okt:
//   FILES  u walks up and folds what is under the new parent; space opens or closes a folder
//          (closing folds everything inside) and shows a file in the graph
//   GRAPH  j/k step through the selection's connections, Enter/space go there, esc stops;
//          space with no walk opens or closes the selected folder; z frames the selection
//   ANY    J/K scroll the inspector, s source/rendered, y copies the path, m and 1-9 pins,
//          n/N FIND matches, E/O/F edit/open/reveal (the request is intercepted, nothing opens)
//   LAYOUT the 3D layout: stable across a shuffled reload, and adding cards (written into the
//          env.sh clone, $OKT_KB) moves nothing outside their folders
//   node tests/ui/keys.mjs [okt-url]
// A page error, or a console message holding "panicked at" or "already borrowed", fails the run.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');

const OKT = process.argv[2] || process.env.OKT || 'http://127.0.0.1:6580/';
const idx = await (await fetch(OKT + 'api/index')).json();
const dirs = new Set(idx.nodes.filter(n => n.t === 'dir').map(n => n.id));
const parent = Object.fromEntries(idx.nodes.map(n => [n.id, n.parent]));
const ancestors = id => { const out = []; for (let p = parent[id]; p && p !== '.'; p = parent[p]) out.push(p); return out; };
const kind = Object.fromEntries(idx.nodes.map(n => [n.id, n.t]));
const files = idx.nodes.filter(n => n.t === 'file');
// the deepest file, to walk up from; a folder with a subfolder, to fold; the most connected
// markdown file, to walk the graph from and to read
const deep = [...files].sort((a, b) => ancestors(b.id).length - ancestors(a.id).length)[0].id;
const outer = [...dirs].find(d => [...dirs].some(s => parent[s] === d && [...dirs].some(t => parent[t] === s)));
const inner = [...dirs].find(s => parent[s] === outer && [...dirs].some(t => parent[t] === s));
const degree = {};
for (const e of idx.edges) { const [s, t] = Array.isArray(e) ? e : [e.s, e.t]; degree[s] = (degree[s] || 0) + 1; degree[t] = (degree[t] || 0) + 1; }
const hub = files.filter(n => n.id.endsWith('.md')).sort((a, b) => (degree[b.id] || 0) - (degree[a.id] || 0))[0].id;
const word = hub.split('/').pop().replace(/\.md$/, '').split(/[_-]/)[0];

const browser = await chromium.launch();
const ctx = await browser.newContext({ viewport: { width: 1600, height: 1000 }, permissions: ['clipboard-read', 'clipboard-write'] });
// window.__oktPos, installed empty: the client fills it with {id: [x, y, z]} on every layout build
await ctx.addInitScript(() => { window.__oktPos = {}; });
const page = await ctx.newPage();
const errors = [];
page.on('pageerror', e => errors.push(e.message));
page.on('console', m => { const t = m.text(); if (t.includes('panicked at') || t.includes('already borrowed')) errors.push('console: ' + t); });
const opened = [];
await page.route('**/api/open', route => { opened.push(JSON.parse(route.request().postData() || '{}')); route.fulfill({ status: 200, contentType: 'application/json', body: '{"ok":true}' }); });
const go = async hash => { await page.evaluate(h => { location.hash = h; }, hash); await page.waitForTimeout(700); };
const key = async (k, ms = 300) => { await page.keyboard.press(k); await page.waitForTimeout(ms); };
const state = () => page.evaluate(() => {
  const rows = [...document.querySelectorAll('#tree .trow:not(.thead)')];
  const marks = [...document.querySelectorAll('#ibody mark.fm')];
  const hint = document.getElementById('ghint');
  return {
    open: rows.filter(r => r.querySelector('.tw')?.textContent === '▾').map(r => r.dataset.id),
    on: rows.find(r => r.classList.contains('on'))?.dataset.id ?? null,
    view: ['graph', 'files', 'log', 'table', 'list'].find(v => !document.getElementById('v-' + v).hidden),
    sel: decodeURIComponent((location.hash.match(/[#&]s=([^&]*)/) || [])[1] || ''),
    hint: hint && !hint.hidden ? hint.textContent : '',
    chip: document.querySelector('#ibody .nchip.kb')?.dataset.go ?? null,
    src: !!document.querySelector('#ibody .src'),
    md: !!document.querySelector('#ibody article.md'),
    scroll: document.getElementById('ibody').scrollTop,
    marks: marks.length,
    cur: marks.findIndex(m => m.classList.contains('cur')),
    toast: [...document.querySelectorAll('#toasts .toast')].map(t => t.innerText.replace(/\n/g, ' ')).pop() || '',
  };
});
let fails = 0;
const check = (label, ok, detail) => { if (!ok) fails++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok ? '' : ': ' + JSON.stringify(detail)}`); };

await page.goto(OKT, { waitUntil: 'load' });
await page.waitForTimeout(2000);

// ---- FILES ----------------------------------------------------------------------------------
await go('#v=files&s=' + encodeURIComponent(deep));
let st = await state();
check(`${deep}: its folders open`, ancestors(deep).every(a => st.open.includes(a)), st.open);
let cur = deep;
while (parent[cur] && parent[cur] !== '.') {
  const p = parent[cur];
  await key('u');
  st = await state();
  const under = st.open.filter(o => o !== p && o.startsWith(p));
  check(`u -> ${p}`, st.on === p && st.open.includes(p) && under.length === 0, st);
  cur = p;
}
await key('u');
st = await state();
check('u at the top folds the whole tree', st.open.length === 0, st.open);
await go('#v=files&s=' + encodeURIComponent(inner));
await go('#v=files&s=' + encodeURIComponent(outer));
st = await state();
check(`${outer} and ${inner} open`, st.on === outer && st.open.includes(outer) && st.open.includes(inner), st);
await key(' ');
st = await state();
check(`space closes ${outer}`, !st.open.includes(outer) && !st.open.some(o => o.startsWith(outer)), st.open);
await key(' ');
st = await state();
check(`space opens ${outer} one level (${inner} stays closed)`, st.open.includes(outer) && !st.open.includes(inner), st.open);
await go('#v=files&s=' + encodeURIComponent(deep));
await key(' ');
st = await state();
check('space on a file shows the graph', st.view === 'graph' && st.sel === deep, st);

// ---- GRAPH ----------------------------------------------------------------------------------
await go('#v=graph&s=' + encodeURIComponent(hub));
await key('j');
st = await state();
const m1 = st.hint.match(/^1\/(\d+) · /);
check(`j starts the walk from ${hub}`, !!m1 && !!st.chip, st);
const total = m1 ? +m1[1] : 0;
await key('j');
st = await state();
check('j steps to the second connection', st.hint.startsWith(`2/${total} · `), st.hint);
await key('k');
await key('k');
st = await state();
check('k from the first wraps to the last', st.hint.startsWith(`${total}/${total} · `), st.hint);
await key('Escape');
st = await state();
check('esc stops the walk, keeping the selection', !st.hint.includes(' · ⏎ go') && !st.chip && st.sel === hub, st);
await key('j');
st = await state();
const target = st.chip;
await key('Enter', 800);
st = await state();
check('Enter goes to the marked connection', st.sel === target, { target, sel: st.sel });
const folder = parent[hub];
await go('#v=graph&s=' + encodeURIComponent(folder));
await key(' ', 500);
st = await state();
check(`space opens or closes ${folder} in the graph`, /EXPANDED|COLLAPSED/.test(st.toast), st.toast);
await key(' ', 500);
await key('z', 600);
check('z frames the selection', errors.length === 0, errors);

// ---- reading ----------------------------------------------------------------------------------
await go('#v=graph&s=' + encodeURIComponent(hub) + '&tab=content');
st = await state();
const top0 = st.scroll;
await key('J', 700);
const top1 = (await state()).scroll;
await key('K', 700);
const top2 = (await state()).scroll;
check('J scrolls the inspector down, K back up', top1 > top0 && top2 < top1, { top0, top1, top2 });
await key('s', 800);
st = await state();
check('s shows the source', st.src && !st.md, st);
await key('s', 800);
st = await state();
check('s again shows it rendered', st.md && !st.src, st);
await key('y', 500);
const clip = await page.evaluate(() => navigator.clipboard.readText());
check('y copies the path', clip.endsWith('/' + hub), clip);

// ---- pins -----------------------------------------------------------------------------------
await key('m');
await go('#v=graph&s=' + encodeURIComponent(deep));
await key('m');
await key('1', 600);
st = await state();
check('1 goes to the first pin', st.sel === hub, st.sel);
await key('2', 600);
st = await state();
check('2 goes to the second pin', st.sel === deep, st.sel);
await key('3', 400);
st = await state();
check('3 with two pins says so', st.toast.includes('No pin 3'), st.toast);
await key('m');
await key('2', 400);
st = await state();
check('m again unpins', st.toast.includes('No pin 2'), st.toast);

// ---- hand-offs ------------------------------------------------------------------------------
await go('#v=graph&s=' + encodeURIComponent(hub));
await key('E', 500);
await key('O', 500);
await key('F', 500);
check('E O F ask for editor, app, reveal', JSON.stringify(opened.map(o => o.how)) === '["editor","app","reveal"]' && opened.every(o => o.path === hub), opened);

// ---- FIND -----------------------------------------------------------------------------------
await page.keyboard.press('/');
await page.keyboard.type(`FIND ${word}`);
await page.keyboard.press('Enter');
await page.waitForTimeout(1500);
await key('n', 1500);
st = await state();
check(`n opens the first FIND ${word} match`, st.marks > 0 && st.cur >= 0, st);
const first = { sel: st.sel, cur: st.cur };
await key('n', 900);
st = await state();
check('n moves to the next match', st.sel !== first.sel || st.cur > first.cur, { first, now: { sel: st.sel, cur: st.cur } });
await key('N', 900);
st = await state();
check('N moves back', st.sel === first.sel && st.cur === first.cur, { first, now: { sel: st.sel, cur: st.cur } });

// ---- LAYOUT ---------------------------------------------------------------------------------
const gattr = name => page.evaluate(n => document.getElementById('graph').getAttribute(n), name);
const positions = () => page.evaluate(() => window.__oktPos);
// every entry of `before` (but those `skip` names) present in `after` with bitwise-equal coordinates
const samePos = (before, after, skip = () => false) => {
  const moved = [];
  let checked = 0;
  for (const [id, p] of Object.entries(before || {})) {
    if (skip(id)) continue;
    checked++;
    const q = (after || {})[id];
    if (!q || q.length !== 3 || !p.every((c, i) => Object.is(c, q[i]))) moved.push(id);
  }
  return { checked, moved: moved.slice(0, 5), nmoved: moved.length };
};
check('layout publishes the card total', +(await gattr('data-total')) === files.length, { total: await gattr('data-total'), files: files.length });

const lay0 = await gattr('data-layout');
const pos0 = await positions();
let seed = 20261108;
const rnd = () => { seed = (seed + 0x6D2B79F5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const shuffle = a => { for (let i = a.length - 1; i > 0; i--) { const j = Math.floor(rnd() * (i + 1)); [a[i], a[j]] = [a[j], a[i]]; } return a; };
let shuffled = 0;
// the context, not the page, so a service-worker-controlled reload is routed too
await ctx.route('**/api/index', async route => {
  const r = await route.fetch();
  const j = await r.json();
  const ids = j.nodes.map(n => n.id).join('\n');
  shuffle(j.nodes);
  shuffle(j.edges);
  if (j.nodes.map(n => n.id).join('\n') !== ids) shuffled++;
  await route.fulfill({ response: r, json: j });
});
await page.reload({ waitUntil: 'load' });
await page.waitForFunction(() => document.getElementById('graph')?.getAttribute('data-layout') && Object.keys(window.__oktPos || {}).length > 0, null, { timeout: 30000 }).catch(() => {});
const lay1 = await gattr('data-layout');
const pos1 = await positions();
await ctx.unroute('**/api/index');
const same1 = samePos(pos0, pos1);
check('layout is stable across a shuffled reload',
  shuffled > 0 && !!lay0 && lay0 === lay1 && same1.checked > 0 && same1.nmoved === 0 && Object.keys(pos0 || {}).length === Object.keys(pos1 || {}).length,
  { shuffled, lay0, lay1, ...same1 });

// probe cards go into the env.sh clone only, never a real KB
const kb = process.env.OKT_KB || '';
const probeDir = dirs.has(parent[hub]) ? parent[hub] : parent[deep];
// folder ids end in '/', so a folder id is also its path prefix
const probes = [`${probeDir.replace(/\/?$/, '/')}zz-probe.md`, 'zz-probe-region/probe.md'];
if (!/\/okt-env\.[^/]+\/kb$/.test(kb)) {
  check('adding a card moves nothing outside its folder', false, { error: 'OKT_KB is not the tests/env.sh clone', kb });
} else {
  const { mkdirSync, writeFileSync } = await import('node:fs');
  const posA = await positions();
  const totalA = +(await gattr('data-total'));
  writeFileSync(`${kb}/${probes[0]}`, '# probe\n');
  mkdirSync(`${kb}/zz-probe-region`, { recursive: true });
  writeFileSync(`${kb}/${probes[1]}`, '# probe\n');
  await page.waitForFunction(t => +document.getElementById('graph').getAttribute('data-total') === t, totalA + 2, { timeout: 30000 }).catch(() => {});
  const posB = await positions();
  const totalB = +(await gattr('data-total'));
  const inProbed = id => kind[id] === 'file' && (parent[id] === probeDir || parent[id] === 'zz-probe-region/');
  const sameB = samePos(posA, posB, inProbed);
  check('adding a card moves nothing outside its folder',
    totalB === totalA + 2 && probes.every(p => posB && posB[p]) && sameB.checked > 0 && sameB.nmoved === 0,
    { probeDir, totalA, totalB, probes: probes.map(p => !!(posB && posB[p])), ...sameB });
}

if (errors.length) { fails++; console.log('page errors:', errors.slice(0, 5).join(' | ')); }
console.log(fails ? `${fails} failed` : 'all passed');
await browser.close();
process.exit(fails ? 1 : 0);
