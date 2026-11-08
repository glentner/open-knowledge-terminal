// The navigation keys (README § Keys), checked on their own against a running okt:
//   FILES  u walks up and folds what is under the new parent; space opens or closes a folder
//          (closing folds everything inside) and shows a file in the graph
//   GRAPH  j/k step through the selection's connections, Enter/space go there, esc stops;
//          space with no walk opens or closes the selected folder; z frames the selection
//   ANY    J/K scroll the inspector, s source/rendered, y copies the path, m and 1-9 pins,
//          n/N FIND matches, E/O/F edit/open/reveal (the request is intercepted, nothing opens)
//   CAMERA the WebGL scene and its camera: the canvas draws, orbit keys turn by fixed steps and
//          leave the layout, the selection and a focused inspector alone, + = - 0 act on the
//          distance, drag orbits, wheel and pinch zoom, moves ease (and cut under reduced
//          motion), and the scene comes back after a lost GL context
//   SCALES nothing selected shows the galaxy (a glyph per region, the bundles, region names only);
//          a folder brings its region to system scale with the others on the rim; zooming out
//          crosses focus, system, galaxy and zooming in at galaxy enters the region nearest the
//          centre, neither touching the selection; show all draws every card and edge
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
// window.__oktPos, installed empty: the client fills it with {id: [x, y, z]} on every upload of
// the positions it draws. window.__tl, the timeline: every write of the camera hooks on #graph,
// with performance.now(), recorded by a MutationObserver from the first frame on.
const hooks = () => {
  window.__oktPos = {};
  window.__tl = [];
  const watch = () => {
    const g = document.getElementById('graph');
    if (!g) return false;
    new MutationObserver(ms => { for (const m of ms) window.__tl.push([performance.now(), m.attributeName, g.getAttribute(m.attributeName)]); })
      .observe(g, { attributes: true, attributeFilter: ['data-theta', 'data-psi', 'data-dist', 'data-anim', 'data-labels'] });
    return true;
  };
  if (!watch()) document.addEventListener('DOMContentLoaded', watch);
};
await ctx.addInitScript(hooks);
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

// ---- CAMERA ---------------------------------------------------------------------------------
const { inflateSync } = await import('node:zlib');
// a PNG (8-bit RGB or RGBA, as Chromium writes them) to raw pixels
const decodePng = buf => {
  let o = 8, w = 0, h = 0, ct = 6;
  const idat = [];
  while (o < buf.length) {
    const len = buf.readUInt32BE(o), type = buf.toString('ascii', o + 4, o + 8), data = buf.subarray(o + 8, o + 8 + len);
    if (type === 'IHDR') { w = data.readUInt32BE(0); h = data.readUInt32BE(4); ct = data[9]; }
    else if (type === 'IDAT') idat.push(data);
    else if (type === 'IEND') break;
    o += 12 + len;
  }
  const bpp = ct === 6 ? 4 : 3, stride = w * bpp, raw = inflateSync(Buffer.concat(idat)), px = Buffer.alloc(h * stride);
  for (let y = 0; y < h; y++) {
    const f = raw[y * (stride + 1)], src = y * (stride + 1) + 1, dst = y * stride;
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? px[dst + x - bpp] : 0, b = y > 0 ? px[dst - stride + x] : 0, c = x >= bpp && y > 0 ? px[dst - stride + x - bpp] : 0;
      let v = raw[src + x];
      if (f === 1) v += a; else if (f === 2) v += b; else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) { const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c); v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c; }
      px[dst + x] = v & 255;
    }
  }
  return { w, h, bpp, px };
};
// #graph alone, with the overlay canvas and the chrome over it hidden: its colours (4 bits per
// channel) each covering more than 1% of the pixels, and the pixels themselves
const shot = async (pg = page) => {
  const hide = ['#goverlay', '.gtools', '#glegend', '#ghint', '#gtip'];
  await pg.evaluate(h => h.forEach(s => document.querySelectorAll(s).forEach(e => { e.dataset.vis = e.style.visibility; e.style.visibility = 'hidden'; })), hide);
  const buf = await pg.locator('#graph').screenshot();
  await pg.evaluate(h => h.forEach(s => document.querySelectorAll(s).forEach(e => { e.style.visibility = e.dataset.vis || ''; })), hide);
  const { w, h, bpp, px } = decodePng(buf);
  const count = new Map();
  for (let i = 0; i < w * h; i++) { const k = (px[i * bpp] >> 4) << 8 | (px[i * bpp + 1] >> 4) << 4 | (px[i * bpp + 2] >> 4); count.set(k, (count.get(k) || 0) + 1); }
  return { colours: [...count.values()].filter(c => c > w * h * 0.01).length, px };
};
const gattrOf = (pg, name) => pg.evaluate(n => document.getElementById('graph').getAttribute(n), name);
const twoFrames = pg => pg.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
// "waits": data-move changes from its value before the action, then data-anim is "0" (two frames
// in between, so the "0" read is one the move's own frames wrote)
const waits = async (act, pg = page) => {
  const m0 = await gattrOf(pg, 'data-move');
  await act();
  await pg.waitForFunction(m => document.getElementById('graph').getAttribute('data-move') !== m, m0, { timeout: 5000 }).catch(() => {});
  await twoFrames(pg);
  await pg.waitForFunction(() => document.getElementById('graph').getAttribute('data-anim') === '0', null, { timeout: 5000 }).catch(() => {});
};
const cam = async (pg = page) => {
  const v = await pg.evaluate(() => { const g = document.getElementById('graph'); return ['data-theta', 'data-psi', 'data-dist', 'data-move', 'data-layout', 'data-pos-hash', 'data-dist-galaxy'].map(k => g.getAttribute(k)); });
  return { theta: +v[0], psi: +v[1], dist: +v[2], move: v[3], layout: v[4], posHash: v[5], galaxy: +v[6], sel: decodeURIComponent((await pg.evaluate(() => location.hash)).match(/[#&]s=([^&]*)/)?.[1] || '') };
};
const dang = (a, b) => ((b - a + 540) % 360) - 180;
const tlLen = (pg = page) => pg.evaluate(() => window.__tl.length);
const tlFrom = (i, pg = page) => pg.evaluate(i => window.__tl.slice(i), i);
// a move's writes of one hook: how many came before its end value, and the time they span
const eased = (tl, attr) => {
  const w = tl.filter(e => e[1] === attr);
  if (w.length < 2) return { mid: 0, span: 0, writes: w.length };
  return { mid: w.length - 1, span: +(w[w.length - 1][0] - w[0][0]).toFixed(1), writes: w.length };
};

await go('#v=graph&s=' + encodeURIComponent(hub));
await page.evaluate(() => document.activeElement?.blur());
{
  const a = await shot();
  await waits(() => page.keyboard.press('ArrowRight'));
  const b = await shot();
  const differs = a.px.length !== b.px.length || !a.px.equals(b.px);
  check('canvas renders the scene', a.colours >= 2 && b.colours >= 2 && differs, { before: a.colours, after: b.colours, differs });
}
{
  const out = {};
  for (const [k, attr] of [['ArrowRight', 'data-theta'], ['+', 'data-dist'], ['0', 'data-dist']]) {
    const i = await tlLen();
    await waits(() => page.keyboard.press(k));
    out[k] = eased(await tlFrom(i), attr);
  }
  check('camera moves ease', Object.values(out).every(e => e.mid >= 3 && e.span >= 100), out);
}
{
  const c0 = await cam();
  await waits(() => page.keyboard.press('ArrowRight'));
  const c1 = await cam();
  check('ArrowRight orbits the azimuth', Math.abs(dang(c0.theta, c1.theta) - 15) <= 1, { from: c0.theta, to: c1.theta });
  await waits(() => page.keyboard.press('ArrowLeft'));
  const c2 = await cam();
  check('ArrowLeft orbits the azimuth back', Math.abs(dang(c0.theta, c2.theta)) <= 1, { from: c0.theta, back: c2.theta });
  await waits(() => page.keyboard.press('PageUp'));
  const c3 = await cam();
  check('PageUp tilts the elevation', Math.abs(c3.psi - c2.psi - 10) <= 1, { from: c2.psi, to: c3.psi });
  await waits(() => page.keyboard.press('PageDown'));
  const c4 = await cam();
  check('PageDown tilts the elevation back', Math.abs(c4.psi - c2.psi) <= 1, { from: c2.psi, back: c4.psi });
  const all = [c0, c1, c2, c3, c4];
  check('orbit keys leave the layout and the selection',
    !!c0.layout && !!c0.posHash && c0.sel === hub && all.every(c => c.layout === c0.layout && c.posHash === c0.posHash && c.sel === c0.sel),
    all.map(c => ({ layout: c.layout, posHash: c.posHash, sel: c.sel })));
}
{
  await go('#v=graph&s=' + encodeURIComponent(hub) + '&tab=content');
  const focused = await page.evaluate(() => {
    const b = document.getElementById('ibody');
    if (b.tabIndex < 0 && !b.hasAttribute('tabindex')) b.tabIndex = -1;
    b.focus();
    return { inInsp: !!document.activeElement?.closest('#insp'), tall: b.scrollHeight > document.getElementById('v-graph').clientHeight };
  });
  const c0 = await cam();
  for (const k of ['ArrowRight', 'ArrowLeft', 'PageUp', 'PageDown']) await key(k, 250);
  await twoFrames(page);
  const c1 = await cam();
  check('orbit keys leave a focused inspector alone',
    focused.inInsp && focused.tall && c1.move === c0.move && c1.theta === c0.theta && c1.psi === c0.psi,
    { focused, before: c0, after: c1 });
  await page.evaluate(() => document.activeElement?.blur());
}
{
  await go('#v=graph&s=' + encodeURIComponent(hub));
  await key('ArrowDown');
  st = await state();
  const m = st.hint.match(/^1\/(\d+) · /);
  const n = m ? +m[1] : 0;
  await key('ArrowUp');
  const back = (await state()).hint;
  check('ArrowDown still walks in the graph', !!m && !!st.chip && back.startsWith(`${n}/${n} · `), { first: st.hint, chip: st.chip, back });
  await key('Escape');
}
{
  await waits(() => page.keyboard.press('0'));
  const r = {};
  for (const [k, want] of [['+', 1 / 1.35], ['=', 1 / 1.35], ['-', 1.35]]) {
    const c0 = await cam();
    await waits(() => page.keyboard.press(k));
    const c1 = await cam();
    r[k] = { from: c0.dist, to: c1.dist, factor: +(c1.dist / c0.dist).toFixed(4), ok: Math.abs(c1.dist / c0.dist / want - 1) <= 0.02 };
  }
  check('+ moves the camera closer', r['+'].ok, r['+']);
  check('= moves the camera closer', r['='].ok, r['=']);
  check('- moves the camera away', r['-'].ok, r['-']);
  await waits(() => page.keyboard.press('+'));
  const c0 = await cam();
  await waits(() => page.keyboard.press('0'));
  const c1 = await cam();
  check('0 fits the galaxy and keeps the selection',
    c1.galaxy > 0 && Math.abs(c1.dist / c1.galaxy - 1) <= 0.02 && c1.dist !== c0.dist && c1.sel === hub && c0.sel === hub, { before: c0, after: c1 });
}
{
  const box = await page.locator('#graph').boundingBox();
  const cx = box.x + box.width / 2, cy = box.y + box.height / 2;
  const c0 = await cam();
  await page.mouse.move(cx - 100, cy + 7);
  await page.mouse.down();
  await page.mouse.move(cx + 100, cy + 7, { steps: 12 });
  await page.mouse.up();
  await twoFrames(page);
  await page.waitForTimeout(100);
  const c1 = await cam();
  check('drag orbits the camera', Math.abs(dang(c0.theta, c1.theta)) >= 30 && c1.sel === c0.sel && c0.sel === hub, { before: c0, after: c1 });
  await waits(() => page.keyboard.press('0'));
  const w0 = await cam();
  await page.mouse.move(cx, cy);
  await page.mouse.wheel(0, 200);
  await twoFrames(page);
  await page.waitForTimeout(100);
  const w1 = await cam();
  await page.keyboard.down('Control');
  await page.mouse.wheel(0, -100);
  await page.keyboard.up('Control');
  await twoFrames(page);
  await page.waitForTimeout(100);
  const w2 = await cam();
  check('wheel zooms the camera', w1.dist > w0.dist && w2.dist < w1.dist, { start: w0.dist, wheel: w1.dist, pinch: w2.dist });
}
{
  const lost = await page.evaluate(async () => {
    const gl = document.getElementById('graph').getContext('webgl2');
    const ext = gl && gl.getExtension('WEBGL_lose_context');
    if (!ext) return 'no WEBGL_lose_context';
    ext.loseContext();
    await new Promise(r => setTimeout(r, 300));
    const was = gl.isContextLost();
    ext.restoreContext();
    await new Promise(r => setTimeout(r, 500));
    return was && !gl.isContextLost() ? 'ok' : 'context did not cycle';
  });
  await waits(() => page.keyboard.press('ArrowRight'));
  const a = await shot();
  await waits(() => page.keyboard.press('ArrowRight'));
  const b = await shot();
  const differs = a.px.length !== b.px.length || !a.px.equals(b.px);
  check('the scene survives a lost GL context', lost === 'ok' && a.colours >= 2 && b.colours >= 2 && differs && errors.length === 0, { lost, before: a.colours, after: b.colours, differs, errors });
}
{
  // reduced motion: a context that asks for it; every move writes its end value once
  const rctx = await browser.newContext({ viewport: { width: 1600, height: 1000 }, reducedMotion: 'reduce' });
  try {
    await rctx.addInitScript(hooks);
    const rp = await rctx.newPage();
    rp.on('pageerror', e => errors.push(e.message));
    await rp.goto(OKT + '#v=graph', { waitUntil: 'load' });
    await rp.waitForFunction(() => document.getElementById('graph')?.getAttribute('data-dist'), null, { timeout: 30000 }).catch(() => {});
    await rp.waitForTimeout(500);
    const moves = {};
    const acts = [
      ['ArrowRight', () => rp.keyboard.press('ArrowRight')],
      ['+', () => rp.keyboard.press('+')],
      ['0', () => rp.keyboard.press('0')],
      ['select', () => rp.evaluate(h => { location.hash = '#v=graph&s=' + encodeURIComponent(h); }, hub)],
    ];
    for (const [name, act] of acts) {
      const i = await tlLen(rp);
      await waits(act, rp);
      const tl = await tlFrom(i, rp);
      const end = await cam(rp);
      const th = tl.filter(e => e[1] === 'data-theta'), di = tl.filter(e => e[1] === 'data-dist');
      moves[name] = {
        theta: th.length, dist: di.length,
        once: th.length <= 1 && di.length <= 1 && th.length + di.length >= 1
          && th.every(e => +e[2] === end.theta) && di.every(e => +e[2] === end.dist),
      };
    }
    const anim1 = (await rp.evaluate(() => window.__tl)).filter(e => e[1] === 'data-anim' && e[2] === '1').length;
    check('reduced motion cuts camera moves', Object.values(moves).every(m => m.once) && anim1 === 0, { moves, anim1 });
  } finally {
    await rctx.close();
  }
}

// ---- SCALES ---------------------------------------------------------------------------------
// counts come from idx: a region is a top-level folder; the root's cards and the external
// systems belong to none
const gattr = name => page.evaluate(n => document.getElementById('graph').getAttribute(n), name);
const nodeIds = new Set(idx.nodes.map(n => n.id));
const topDirs = [...dirs].filter(d => parent[d] === '.').sort();
const topSet = new Set(topDirs);
const regionOf = id => {
  if (kind[id] === 'surface' || !id.includes('/')) return null;
  const r = id.slice(0, id.indexOf('/') + 1);
  return topSet.has(r) ? r : null;
};
const regionCards = {};
for (const f of files) { const r = regionOf(f.id); if (r) regionCards[r] = (regionCards[r] || 0) + 1; }
const edgeEnds = idx.edges.map(e => Array.isArray(e) ? [e[0], e[1]] : [e.s, e.t]).filter(([s, t]) => nodeIds.has(s) && nodeIds.has(t));
const regionPairs = new Set();
for (const [s, t] of edgeEnds) {
  const a = regionOf(s), b = regionOf(t);
  if (a && b && a !== b) regionPairs.add(a < b ? `${a}|${b}` : `${b}|${a}`);
}
// a position under the last frame's view-projection: ndc, pane px from the centre, in front
const projectAll = (view, pts, w, h) => pts.map(p => {
  const m = view;
  const x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
  const y = m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13];
  const cw = m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15];
  return { front: cw > 0, nx: x / cw, ny: y / cw, px: Math.hypot(x / cw * w / 2, y / cw * h / 2) };
});
const scaleState = async () => {
  const v = await page.evaluate(() => { const g = document.getElementById('graph'); return ['data-scale', 'data-region', 'data-regions', 'data-rim', 'data-bundles', 'data-drawn', 'data-edges', 'data-dist', 'data-dist-galaxy', 'data-labels', 'data-view', 'data-showall'].map(k => g.getAttribute(k)); });
  return { scale: v[0], region: v[1], regions: +v[2], rim: +v[3], bundles: +v[4], drawn: +v[5], edges: +v[6], dist: +v[7], galaxy: +v[8], labels: JSON.parse(v[9] || '[]'), view: JSON.parse(v[10] || '[]'), showall: v[11], sel: (await state()).sel };
};
{
  await waits(() => go('#v=graph'));
  const s0 = await scaleState();
  check('nothing selected shows galaxy scale',
    s0.sel === '' && s0.scale === 'galaxy' && s0.drawn === 0 && s0.galaxy > 0 && Math.abs(s0.dist / s0.galaxy - 1) <= 0.02,
    { sel: s0.sel, scale: s0.scale, drawn: s0.drawn, dist: s0.dist, galaxy: s0.galaxy });
  check('galaxy draws one cluster per region and the bundles',
    topDirs.length > 1 && s0.regions === topDirs.length && s0.bundles === regionPairs.size,
    { regions: s0.regions, topDirs: topDirs.length, bundles: s0.bundles, pairs: regionPairs.size });
  check('galaxy labels region names only',
    s0.labels.length > 0 && s0.labels.every(l => l[4] === 'region'),
    { labels: s0.labels.slice(0, 4), n: s0.labels.length });
}
{
  // a subfolder of the region with the most cards
  const big = [...topDirs].sort((a, b) => (regionCards[b] || 0) - (regionCards[a] || 0)).find(r => [...dirs].some(d => parent[d] === r));
  const sub = [...dirs].filter(d => parent[d] === big).sort()[0];
  await waits(() => go('#v=graph&s=' + encodeURIComponent(sub)));
  const s1 = await scaleState();
  const box = await page.locator('#graph').boundingBox();
  const pos = await page.evaluate(() => window.__oktPos);
  const members = Object.keys(pos || {}).filter(id => id.startsWith(big));
  const pr = projectAll(s1.view, members.map(id => pos[id]), box.width, box.height);
  const extent = Math.max(0, ...pr.map(p => Math.max(Math.abs(p.nx), Math.abs(p.ny))));
  check('a folder selects system scale on its region',
    s1.sel === sub && s1.scale === 'system' && s1.region === big && members.length > 1 && pr.every(p => p.front) && extent >= 0.6 && extent <= 0.8,
    { sub, big, sel: s1.sel, scale: s1.scale, region: s1.region, members: members.length, extent: +extent.toFixed(3) });
  check('system scale puts the other regions on the rim',
    s1.regions > 1 && s1.rim === s1.regions - 1,
    { regions: s1.regions, rim: s1.rim });
}
{
  await waits(() => go('#v=graph&s=' + encodeURIComponent(hub)));
  const seen = [(await scaleState()).scale];
  const sels = [(await state()).sel];
  for (let i = 0; i < 24 && seen[seen.length - 1] !== 'galaxy'; i++) {
    await waits(() => page.keyboard.press('-'));
    const s = await scaleState();
    if (s.scale !== seen[seen.length - 1]) seen.push(s.scale);
    sels.push(s.sel);
  }
  check('zooming out crosses to galaxy and keeps the selection',
    JSON.stringify(seen) === '["focus","system","galaxy"]' && sels.every(s => s === hub),
    { seen, sels: [...new Set(sels)] });
}
{
  await waits(() => go('#v=graph'));
  await waits(() => page.keyboard.press('ArrowRight'));
  await waits(() => page.keyboard.press('ArrowRight'));
  await waits(() => page.keyboard.press('PageUp'));
  const box = await page.locator('#graph').boundingBox();
  let before = null, s = await scaleState();
  for (let i = 0; i < 24 && s.scale !== 'system'; i++) {
    before = s;
    await waits(() => page.keyboard.press('+'));
    s = await scaleState();
  }
  // the region the test expects, from the view the crossing press was made under
  let want = null;
  if (before) {
    const pos = await page.evaluate(() => window.__oktPos);
    const pr = projectAll(before.view, topDirs.map(r => pos[r]), box.width, box.height);
    const cands = topDirs.map((r, k) => ({ r, ...pr[k], cards: regionCards[r] || 0 })).filter(c => c.front);
    cands.sort((a, b) => (Math.abs(a.px - b.px) > 1e-6 * (1 + a.px) ? a.px - b.px : 0) || b.cards - a.cards || (a.r < b.r ? -1 : 1));
    want = cands[0]?.r ?? null;
  }
  check('zooming in at galaxy enters system scale on the region nearest the centre',
    s.scale === 'system' && !!want && s.region === want && s.sel === '',
    { scale: s.scale, region: s.region, want, sel: s.sel });
}
{
  // collapse a folder (space toggles; press again if it opened), then show all at the galaxy fit
  await waits(() => go('#v=graph&s=' + encodeURIComponent(outer)));
  await key(' ', 500);
  if (!/COLLAPSED/i.test((await state()).toast)) await key(' ', 500);
  const collapsed = /COLLAPSED/i.test((await state()).toast);
  await waits(() => page.keyboard.press('0'));
  await page.locator('.gtools [data-z=all]').click();
  await page.waitForFunction(() => document.getElementById('graph').getAttribute('data-showall') === '1', null, { timeout: 5000 }).catch(() => {});
  await twoFrames(page);
  const s2 = await scaleState();
  check('show all draws every card and every edge',
    collapsed && s2.showall === '1' && s2.drawn === files.length && s2.edges === edgeEnds.length,
    { collapsed, showall: s2.showall, drawn: s2.drawn, files: files.length, edges: s2.edges, want: edgeEnds.length });
  // put things back: show all off, the folder open again
  await page.locator('.gtools [data-z=all]').click();
  await page.waitForFunction(() => document.getElementById('graph').getAttribute('data-showall') === '0', null, { timeout: 5000 }).catch(() => {});
  await key(' ', 500);
}

// ---- LAYOUT ---------------------------------------------------------------------------------
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
