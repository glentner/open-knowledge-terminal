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
//   FOCUS  a card brings focus scale, the camera looking along the smallest principal axis of the
//          card and its links in its region, fitted to the pane; only the tiers are drawn (the
//          focus and its links anywhere, two hops and the rest of its region inside it), other
//          regions' cards fold into their glyphs; a graph click eases to the card clicked
//   LABELS at most 24 cards labelled at once (region names not counted), flat on the overlay,
//          never two labels overlapping: at rest on the hub, under show all, and through a drag
//          orbit
//   WALK   z re-solves the camera for the selection and its links, or from Results for the
//          FIND matches across regions, moving no card; j/k step clockwise in screen order around
//          the selection, a target in another region a portal labelled with its path, its
//          inspector chip marked and kept marked through a re-render
//   WARP   Enter or space during a walk flies to the target (one ~600 ms move rising to at least
//          twice both end distances) and pushes a history entry; [ eases back to the camera the
//          entry left had, ] forward again; reduced motion cuts both; interleaved moves never
//          trip a borrow
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
// the most connected markdown file whose every Connections group fits the inspector's 40 chips, so
// each step of a walk from it has a chip to mark (issues/walk-mark-chip-cap.md)
const groups = {};
for (const e of idx.edges) {
  const [s, t, k] = Array.isArray(e) ? e : [e.s, e.t, e.k];
  for (const [id, g] of [[s, `out:${k}`], [t, `in:${k}`]]) { groups[id] = groups[id] || {}; groups[id][g] = (groups[id][g] || 0) + 1; }
}
const walker = files.filter(n => n.id.endsWith('.md') && (degree[n.id] || 0) >= 3 && Math.max(...Object.values(groups[n.id] || { none: 0 })) <= 40)
  .sort((a, b) => (degree[b.id] || 0) - (degree[a.id] || 0))[0].id;

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
// KEYS_VERBOSE=1 prints the detail of passing checks too
const verbose = !!process.env.KEYS_VERBOSE;
const check = (label, ok, detail) => { if (!ok) fails++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok && !verbose ? '' : ': ' + JSON.stringify(detail)}`); };

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
await go('#v=graph&s=' + encodeURIComponent(walker));
await key('j');
st = await state();
const m1 = st.hint.match(/^1\/(\d+) · /);
check(`j starts the walk from ${walker}`, !!m1 && !!st.chip, st);
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
check('esc stops the walk, keeping the selection', !st.hint.includes(' · ⏎ go') && !st.chip && st.sel === walker, st);
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
  await go('#v=graph&s=' + encodeURIComponent(walker));
  await key('ArrowDown');
  st = await state();
  const m = st.hint.match(/^1\/(\d+) · /);
  const n = m ? +m[1] : 0;
  await key('ArrowUp');
  const back = (await state()).hint;
  check('ArrowDown still walks in the graph', !!m && !!st.chip && back.startsWith(`${n}/${n} · `), { first: st.hint, chip: st.chip, back });
  await key('Escape');
  await go('#v=graph&s=' + encodeURIComponent(hub));
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

// ---- FOCUS ----------------------------------------------------------------------------------
// The tiers, from idx: link hops from the focus (both directions), never through a folder or the
// root; tier 0 and 1 drawn in any region, 2 and 3 (the rest) only in the focus's region (its
// top-level folder, '.' for the root's cards, '@' for the external systems); the root is never
// drawn. The edge rule: focus to its links, links to links, links to two hops; both ends drawn.
const regionKey = id => kind[id] === 'surface' ? '@' : id.includes('/') ? id.slice(0, id.indexOf('/') + 1) : '.';
const adjOf = new Map();
for (const [s, t] of edgeEnds) for (const [a, b] of [[s, t], [t, s]]) { if (!adjOf.has(a)) adjOf.set(a, new Set()); adjOf.get(a).add(b); }
const isFolder = id => kind[id] === 'dir' || kind[id] === 'root';
const tiersOf = f => {
  const hop = new Map([[f, 0]]);
  const one = [...(adjOf.get(f) || [])].filter(m => !hop.has(m));
  for (const m of one) hop.set(m, 1);
  for (const m of one) if (!isFolder(m)) for (const x of adjOf.get(m) || []) if (!hop.has(x)) hop.set(x, 2);
  const R = regionKey(f);
  const tier = id => { const h = hop.has(id) ? hop.get(id) : Infinity; return h <= 1 ? h : regionKey(id) === R ? Math.min(h, 3) : null; };
  const drawn = id => kind[id] !== 'root' && tier(id) !== null;
  return { hop, R, tier, drawn };
};
const ruleEdges = T => edgeEnds.filter(([s, t]) => {
  if (!T.drawn(s) || !T.drawn(t)) return false;
  const a = T.tier(s), b = T.tier(t), lo = Math.min(a, b), hi = Math.max(a, b);
  return (lo === 0 && hi <= 1) || (lo === 1 && hi <= 2);
});
// eigenvalues (ascending) and eigenvectors of a symmetric 3x3 matrix, by cyclic Jacobi
const eigen3 = m0 => {
  const a = m0.map(r => r.slice()), v = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
  for (let sweep = 0; sweep < 50; sweep++) {
    if (Math.abs(a[0][1]) + Math.abs(a[0][2]) + Math.abs(a[1][2]) < 1e-14 * (Math.abs(a[0][0]) + Math.abs(a[1][1]) + Math.abs(a[2][2]) + 1e-300)) break;
    for (const [p, q] of [[0, 1], [0, 2], [1, 2]]) {
      if (a[p][q] === 0) continue;
      const th = (a[q][q] - a[p][p]) / (2 * a[p][q]);
      const t = Math.sign(th || 1) / (Math.abs(th) + Math.sqrt(th * th + 1)), c = 1 / Math.sqrt(t * t + 1), s = t * c;
      for (let k = 0; k < 3; k++) { const akp = a[k][p], akq = a[k][q]; a[k][p] = c * akp - s * akq; a[k][q] = s * akp + c * akq; }
      for (let k = 0; k < 3; k++) { const apk = a[p][k], aqk = a[q][k]; a[p][k] = c * apk - s * aqk; a[q][k] = s * apk + c * aqk; }
      for (let k = 0; k < 3; k++) { const vkp = v[k][p], vkq = v[k][q]; v[k][p] = c * vkp - s * vkq; v[k][q] = s * vkp + c * vkq; }
    }
  }
  return [0, 1, 2].map(k => ({ val: a[k][k], vec: [v[0][k], v[1][k], v[2][k]] })).sort((x, y) => x.val - y.val);
};
const focusState = async (pg = page) => {
  const v = await pg.evaluate(() => { const g = document.getElementById('graph'); return ['data-scale', 'data-focus', 'data-focus-ids', 'data-focus-n', 'data-view', 'data-drawn', 'data-edges', 'data-drawn-regions', 'data-tiers', 'data-anim'].map(k => g.getAttribute(k)); });
  return { scale: v[0], focus: v[1], ids: JSON.parse(v[2] || '[]'), n: +v[3], view: JSON.parse(v[4] || '[]'), drawn: +v[5], edges: +v[6], regions: JSON.parse(v[7] || '{}'), tiers: v[8], anim: v[9] };
};
// a card's fit set as the client solves it: the card and its links in its own region
const fitSet = id => [id, ...[...(adjOf.get(id) || [])].filter(m => m !== id && kind[m] !== 'root' && regionKey(m) === regionKey(id))];
// a set's principal axes, from positions: the smallest axis and lambda1/lambda0
const axesOf = pts => {
  const mean = [0, 1, 2].map(k => pts.reduce((a, p) => a + p[k], 0) / Math.max(1, pts.length));
  const cov = [0, 1, 2].map(a => [0, 1, 2].map(b => pts.reduce((acc, p) => acc + (p[a] - mean[a]) * (p[b] - mean[b]), 0) / Math.max(1, pts.length)));
  const eig = eigen3(cov);
  return { e0: eig[0].vec, ratio: eig[1].val / Math.max(eig[0].val, 1e-300) };
};
// the camera against data-focus-ids: the axis (when it is defined) and the fitted extent
const facing = async () => {
  const f = await focusState();
  const box = await page.locator('#graph').boundingBox();
  const pos = await page.evaluate(() => window.__oktPos);
  const pts = f.ids.map(id => pos[id]).filter(Boolean);
  const { e0, ratio } = axesOf(pts);
  // the view direction (target toward the eye): clip w is dir . (eye - p), so row 4 is -dir
  const m = f.view, dl = Math.hypot(m[3], m[7], m[11]);
  const dir = [-m[3] / dl, -m[7] / dl, -m[11] / dl];
  const along = Math.abs(dir[0] * e0[0] + dir[1] * e0[1] + dir[2] * e0[2]);
  const pr = projectAll(f.view, pts, box.width, box.height);
  const extent = Math.max(0, ...pr.map(p => Math.max(Math.abs(p.nx), Math.abs(p.ny))));
  const ok = pts.length >= 3 && pts.length === f.ids.length && (ratio < 1.1 || along >= 0.99) && pr.every(p => p.front) && extent >= 0.6 && extent <= 0.8;
  return { f, ok, detail: { focus: f.focus, members: f.ids.length, ratio: +ratio.toFixed(3), along: +along.toFixed(4), extent: +extent.toFixed(3) } };
};
{
  await waits(() => go('#v=graph&s=' + encodeURIComponent(hub)));
  const a = await facing();
  check('a card selects focus scale', a.f.scale === 'focus' && a.f.focus === hub, { hub, scale: a.f.scale, focus: a.f.focus });
  const same = (ids, id) => { const w = new Set(fitSet(id)); return ids.length === w.size && ids.every(x => w.has(x)); };
  // when the hub's set has no well-defined axis, the most connected card whose set clearly has
  // one is solved for as well, so the axis itself is always checked
  let b = null;
  if (+a.detail.ratio < 1.1) {
    const pos = await page.evaluate(() => window.__oktPos);
    const clear = files.map(n => n.id).filter(id => id !== hub && pos[id])
      .sort((x, y) => (degree[y] || 0) - (degree[x] || 0))
      .find(id => { const set = fitSet(id); return set.length >= 5 && set.every(x => pos[x]) && axesOf(set.map(x => pos[x])).ratio >= 1.25; });
    if (clear) {
      await waits(() => go('#v=graph&s=' + encodeURIComponent(clear)));
      b = await facing();
      b.ok = b.ok && b.f.focus === clear && same(b.f.ids, clear) && +b.detail.ratio >= 1.1;
    } else {
      b = { ok: false, detail: { error: 'no card with a well-defined axis' } };
    }
  }
  check('the focus camera faces the smallest principal axis',
    a.ok && same(a.f.ids, hub) && (!b || b.ok),
    { hub: a.detail, clear: b && b.detail });
}
{
  // a second context, with auto-collapse off: the settings say collapse 1000000 before the load
  const cctx = await browser.newContext({ viewport: { width: 1600, height: 1000 } });
  try {
    await cctx.addInitScript(hooks);
    const cp = await cctx.newPage();
    cp.on('pageerror', e => errors.push(e.message));
    cp.on('console', m => { const t = m.text(); if (t.includes('panicked at') || t.includes('already borrowed')) errors.push('console: ' + t); });
    await cp.goto(OKT + '#v=graph', { waitUntil: 'load' });
    await cp.evaluate(() => {
      const kb = JSON.parse(document.getElementById('boot')?.textContent || '{}').kb || 'kb';
      const k = 'okt:' + kb;
      const cur = JSON.parse(localStorage.getItem(k) || '{}');
      localStorage.setItem(k, JSON.stringify({ ...cur, collapse: 1000000 }));
    });
    const settle = async h => {
      await cp.goto(OKT + '#v=graph&s=' + encodeURIComponent(h), { waitUntil: 'load' });
      await cp.waitForFunction(id => { const g = document.getElementById('graph'); return g?.getAttribute('data-focus') === id && g.getAttribute('data-scale') === 'focus' && g.getAttribute('data-anim') === '0'; }, h, { timeout: 30000 }).catch(() => {});
      await twoFrames(cp);
      await cp.waitForFunction(() => document.getElementById('graph').getAttribute('data-anim') === '0', null, { timeout: 5000 }).catch(() => {});
      return focusState(cp);
    };
    const collapse = await cp.evaluate(() => { const kb = JSON.parse(document.getElementById('boot')?.textContent || '{}').kb || 'kb'; return JSON.parse(localStorage.getItem('okt:' + kb) || '{}').collapse; });
    const T = tiersOf(hub);
    const f = await settle(hub);
    const wantDrawn = files.filter(n => T.drawn(n.id)).length;
    const regionCount = files.filter(n => regionKey(n.id) === T.R).length;
    const links = files.filter(n => T.hop.get(n.id) === 1 && regionKey(n.id) !== T.R).length;
    const wantEdges = ruleEdges(T).length;
    check('show all off draws only the tiers',
      collapse === 1000000 && f.focus === hub && wantDrawn === regionCount + links && f.drawn === wantDrawn && f.edges === wantEdges,
      { collapse, focus: f.focus, drawn: f.drawn, want: wantDrawn, region: regionCount, links, edges: f.edges, wantEdges });
    // a focus whose two hops reach another region past its own links there: the most connected
    // such card, the hub when it is one
    const reaches = id => { const T2 = tiersOf(id); return [...T2.hop].some(([x, h]) => h === 2 && kind[x] === 'file' && regionKey(x) !== T2.R); };
    const hub2 = [hub, ...files.map(n => n.id).sort((a, b) => (degree[b] || 0) - (degree[a] || 0))].find(reaches);
    if (!hub2) {
      check('cards in other regions fold into their cluster', false, { error: 'no card reaches another region in two hops' });
    } else {
      const T2 = tiersOf(hub2);
      const f2 = hub2 === hub ? f : await settle(hub2);
      const want = {};
      for (const n of files) if (T2.hop.get(n.id) === 1 && regionKey(n.id) !== T2.R) want[regionKey(n.id)] = (want[regionKey(n.id)] || 0) + 1;
      const keys = new Set([...Object.keys(want), ...Object.keys(f2.regions)].filter(k => k !== T2.R));
      const off = [...keys].filter(k => (f2.regions[k] || 0) !== (want[k] || 0)).map(k => ({ region: k, drawn: f2.regions[k] || 0, want: want[k] || 0 }));
      const twoHop = [...T2.hop].filter(([x, h]) => h === 2 && kind[x] === 'file' && regionKey(x) !== T2.R).length;
      check('cards in other regions fold into their cluster',
        f2.focus === hub2 && Object.keys(want).length > 0 && twoHop > 0 && off.length === 0,
        { hub2, focus: f2.focus, regions: Object.keys(want).length, twoHop, off: off.slice(0, 5) });
    }
  } finally {
    await cctx.close();
  }
}
{
  // click a drawn tier-1 card: one in the hub's region, on the pane clear of the chrome, and the
  // farthest from any other drawn point
  await waits(() => go('#v=graph&s=' + encodeURIComponent(hub)));
  await page.evaluate(() => document.activeElement?.blur());
  const f = await focusState();
  const box = await page.locator('#graph').boundingBox();
  const pos = await page.evaluate(() => window.__oktPos);
  const T = tiersOf(hub);
  const drawnIds = idx.nodes.map(n => n.id).filter(id => T.drawn(id) && pos[id]);
  const scr = new Map();
  const pr = projectAll(f.view, drawnIds.map(id => pos[id]), box.width, box.height);
  drawnIds.forEach((id, k) => { const p = pr[k]; if (p.front) scr.set(id, [(p.nx + 1) / 2 * box.width, (1 - p.ny) / 2 * box.height]); });
  const cands = [...scr].filter(([id, [x, y]]) => kind[id] === 'file' && T.hop.get(id) === 1 && regionKey(id) === T.R
    && x > 90 && x < box.width - 140 && y > 60 && y < box.height - 110)
    .map(([id, [x, y]]) => ({ id, x, y, room: Math.min(...[...scr].filter(([o]) => o !== id).map(([, [ox, oy]]) => Math.hypot(ox - x, oy - y))) }))
    .sort((a, b) => b.room - a.room);
  const c = cands[0];
  if (!c) {
    check('a graph click eases the camera to the card', false, { error: 'no tier-1 card on the pane', drawn: drawnIds.length });
  } else {
    const i = await tlLen();
    await waits(() => page.mouse.click(box.x + c.x, box.y + c.y));
    const tl = await tlFrom(i);
    const after = await focusState();
    const e = { theta: eased(tl, 'data-theta'), dist: eased(tl, 'data-dist') };
    check('a graph click eases the camera to the card',
      after.focus === c.id && after.scale === 'focus' && Object.values(e).some(x => x.mid >= 3 && x.span >= 100),
      { card: c.id, room: +c.room.toFixed(1), focus: after.focus, ...e });
  }
}

// ---- LABELS ---------------------------------------------------------------------------------
// data-labels rows are [x, y, w, h, kind, text]; every kind but "region" names a card. Two rects
// overlap when they share more than 1 px on both axes.
const cardLabels = ls => ls.filter(l => l[4] !== 'region');
const overlaps = ls => {
  const out = [];
  for (let i = 0; i < ls.length; i++) for (let j = i + 1; j < ls.length; j++) {
    const [ax, ay, aw, ah] = ls[i], [bx, by, bw, bh] = ls[j];
    if (ax < bx + bw - 1 && bx < ax + aw - 1 && ay < by + bh - 1 && by < ay + ah - 1) out.push([ls[i], ls[j]]);
  }
  return out;
};
// the labels once the camera has rested long enough for them to be chosen again
const restedLabels = async () => {
  await page.waitForTimeout(400);
  await twoFrames(page);
  const v = await page.evaluate(() => { const g = document.getElementById('graph'); return [g.getAttribute('data-labels'), g.getAttribute('data-label-cands'), g.getAttribute('data-anim')]; });
  return { labels: JSON.parse(v[0] || '[]'), cands: +v[1], anim: v[2] };
};
const showAll = async on => {
  if ((await gattr('data-showall')) !== (on ? '1' : '0')) await page.locator('.gtools [data-z=all]').click();
  await page.waitForFunction(w => document.getElementById('graph').getAttribute('data-showall') === w, on ? '1' : '0', { timeout: 5000 }).catch(() => {});
  await twoFrames(page);
};
{
  await waits(() => go('#v=graph&s=' + encodeURIComponent(hub)));
  await page.evaluate(() => document.activeElement?.blur());
  await showAll(false);
  const a = await restedLabels();
  await showAll(true);
  const b = await restedLabels();
  const n = r => cardLabels(r.labels).length;
  check('at most 24 card labels',
    a.cands > 24 && n(a) >= 12 && n(a) <= 24 && b.cands > 24 && n(b) >= 12 && n(b) <= 24,
    { hub: { cands: a.cands, cards: n(a), regions: a.labels.length - n(a) }, showAll: { cands: b.cands, cards: n(b), regions: b.labels.length - n(b) } });
  const both = [a, b].map(r => overlaps(r.labels));
  check('no two labels overlap',
    a.labels.length > 0 && b.labels.length > 0 && both.every(o => o.length === 0),
    { hub: a.labels.length, showAll: b.labels.length, overlaps: both.map(o => o.slice(0, 2)) });
  // show all at the galaxy fit with the hub still selected: every card a candidate
  await waits(() => page.keyboard.press('0'));
  const c = await restedLabels();
  check('labels stay capped in show all',
    n(b) <= 24 && n(c) <= 24 && c.cands > 24 && overlaps(b.labels).length === 0 && overlaps(c.labels).length === 0,
    { hub: n(b), galaxy: { cands: c.cands, cards: n(c), regions: c.labels.length - n(c) }, overlaps: overlaps(c.labels).slice(0, 2) });
  // show all with nothing selected: the galaxy still names its regions only
  await waits(() => go('#v=graph'));
  const d = await restedLabels();
  const dScale = await gattr('data-scale');
  check('show all at the galaxy labels region names only',
    (await gattr('data-showall')) === '1' && (await state()).sel === '' && dScale === 'galaxy' && d.cands === 0 && d.labels.length > 0 && d.labels.every(l => l[4] === 'region'),
    { scale: dScale, cands: d.cands, labels: d.labels.length, cards: n(d) });
  await showAll(false);
  // a 1 s drag orbit around the hub (reached again by way of another selection, since 0 kept it):
  // every data-labels value written on the way
  await waits(() => go('#v=graph&s=' + encodeURIComponent(outer)));
  await waits(() => go('#v=graph&s=' + encodeURIComponent(hub)));
  await restedLabels();
  const box = await page.locator('#graph').boundingBox();
  const cx = box.x + box.width / 2, cy = box.y + box.height / 2;
  const i = await tlLen();
  const start = JSON.parse((await gattr('data-labels')) || '[]');
  await page.mouse.move(cx - 150, cy + 40);
  await page.mouse.down();
  const t0 = Date.now();
  for (let k = 1; k <= 60 && Date.now() - t0 < 1000; k++) {
    await page.mouse.move(cx - 150 + k * 5, cy + 40 - k * 1.5);
    await page.waitForTimeout(16);
  }
  const span = Date.now() - t0;
  await page.mouse.up();
  await twoFrames(page);
  const seen = [start, ...(await tlFrom(i)).filter(e => e[1] === 'data-labels').map(e => JSON.parse(e[2] || '[]'))];
  const bad = seen.map((ls, k) => ({ k, cards: cardLabels(ls).length, overlaps: overlaps(ls).length })).filter(v => v.cards > 24 || v.overlaps > 0);
  const most = Math.max(...seen.map(ls => cardLabels(ls).length));
  check('labels stay capped and apart while orbiting',
    span >= 900 && seen.length >= 5 && most >= 1 && bad.length === 0,
    { span, values: seen.length, most, bad: bad.slice(0, 3) });
}

// ---- WALK -----------------------------------------------------------------------------------
// z re-solves the camera where things are: for the selection and its links, or, from the Results
// view, for the FIND matches across regions; no card moves. j/k step through the selection's
// connections clockwise in screen order, from 12 o'clock around the selection, in the order taken
// from the view the walk started under. The test computes it by projecting positions under that
// view: a card at its point; a portal (a link into another top-level folder) whose card is off the
// pane at its region's glyph on the rim, and a card behind the camera at the rim in its own
// direction, the rim lying in the view-plane direction from the pane centre (where the selection
// is). The inspector marks the target's chip; it shows 40 chips per group, so a target past them
// has none to mark (issues/walk-mark-chip-cap.md) and then no chip may be marked.
const clipOf = (m, p) => [m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12], m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13], m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15]];
const clockAngle = v => { const a = Math.atan2(v[0], -v[1]); return a < 0 ? a + 2 * Math.PI : a; };
const sameAt = (a, b) => { const ks = Object.keys(a || {}); return ks.length > 0 && ks.length === Object.keys(b || {}).length && ks.every(id => (b[id] || []).length === 3 && a[id].every((c, i) => Object.is(c, b[id][i]))); };
{
  // a card whose set has a well-defined axis (the hub when it has one), so the solve has an
  // orientation to come back to
  const pos = await page.evaluate(() => window.__oktPos);
  const card = [hub, ...files.map(n => n.id).filter(id => id !== hub && pos[id]).sort((x, y) => (degree[y] || 0) - (degree[x] || 0))]
    .find(id => { const set = fitSet(id); return set.length >= 5 && set.every(x => pos[x]) && axesOf(set.map(x => pos[x])).ratio >= 1.25; }) || hub;
  await waits(() => go('#v=graph&s=' + encodeURIComponent(outer)));
  await waits(() => go('#v=graph&s=' + encodeURIComponent(card)));
  await page.evaluate(() => document.activeElement?.blur());
  const c0 = await cam();
  const p0 = await page.evaluate(() => window.__oktPos);
  for (let k = 0; k < 4; k++) await waits(() => page.keyboard.press('ArrowRight'));
  const c1 = await cam();
  const h0 = await page.evaluate(() => history.length);
  await waits(() => page.keyboard.press('z'));
  const c2 = await cam();
  const p1 = await page.evaluate(() => window.__oktPos);
  const h1 = await page.evaluate(() => history.length);
  const turned = Math.max(Math.abs(dang(c1.theta, c2.theta)), Math.abs(c2.psi - c1.psi));
  check('z re-solves the camera and keeps the layout',
    Math.abs(Math.abs(dang(c0.theta, c1.theta)) - 60) <= 1 && turned >= 5 && c2.move !== c1.move && !!c0.layout && c2.layout === c0.layout && c2.posHash === c0.posHash && sameAt(p0, p1) && h1 === h0 && c2.sel === card,
    { card, orbit: +dang(c0.theta, c1.theta).toFixed(1), turned: +turned.toFixed(1), layout: c2.layout === c0.layout, posHash: c2.posHash === c0.posHash, positions: sameAt(p0, p1), history: [h0, h1], sel: c2.sel });
}
{
  await waits(() => go('#v=graph&s=' + encodeURIComponent(outer)));
  await waits(() => go('#v=graph&s=' + encodeURIComponent(hub)));
  await page.evaluate(() => document.activeElement?.blur());
  const box = await page.locator('#graph').boundingBox();
  const W = box.width, H = box.height;
  const view0 = JSON.parse((await gattr('data-view')) || '[]');
  const pos = await page.evaluate(() => window.__oktPos);
  const screenOf = c => c[2] > 0 ? [(c[0] / c[2] + 1) / 2 * W, (1 - c[1] / c[2]) / 2 * H] : null;
  const hubS = screenOf(clipOf(view0, pos[hub])) || [W / 2, H / 2];
  const portalOf = id => { const r = regionOf(id); return r && r !== regionKey(hub) ? r : null; };
  // where the walk sees a target, as a screen vector from the hub
  const seen = id => {
    const c = clipOf(view0, pos[id]);
    const s = screenOf(c);
    const on = !!s && s[0] >= 0 && s[1] >= 0 && s[0] <= W && s[1] <= H;
    const r = portalOf(id);
    if (r && !on) { const g = clipOf(view0, pos[r]); return [g[0] * W / 2, -g[1] * H / 2]; }
    if (s) return [s[0] - hubS[0], s[1] - hubS[1]];
    return [c[0] * W / 2, -c[1] * H / 2];
  };
  const shown = new Set(await page.evaluate(() => [...document.querySelectorAll('#ibody .nchip[data-go]')].map(c => c.dataset.go)));
  const walkState = () => page.evaluate(() => {
    const g = document.getElementById('graph');
    return { walk: g.getAttribute('data-walk'), i: +g.getAttribute('data-walk-i'), n: +g.getAttribute('data-walk-n'), chip: document.querySelector('#ibody .nchip.kb')?.dataset.go ?? null };
  });
  await key('j', 150);
  const m = (await state()).hint.match(/^1\/(\d+) · /);
  const N = m ? +m[1] : 0;
  const steps = [];
  let portal = null;
  for (let k = 1; k <= N; k++) {
    if (k > 1) await page.keyboard.press('j');
    await page.waitForFunction(k => document.getElementById('graph').getAttribute('data-walk-i') === String(k), k, { timeout: 3000 }).catch(() => {});
    const v = await walkState();
    steps.push(v);
    if (!portal && v.walk && portalOf(v.walk)) {
      const r = await restedLabels();
      portal = { target: v.walk, labels: r.labels.filter(l => l[4] === 'portal'), rim: +(await gattr('data-rim')) };
    }
  }
  const ids = steps.map(v => v.walk);
  const angles = ids.map(id => (pos[id] ? clockAngle(seen(id)) : NaN));
  // descents around the cycle (the last back to the first included); equal angles (two portals at
  // one glyph) are not descents
  const descents = angles.filter((a, k) => a < angles[k === 0 ? N - 1 : k - 1] - 1e-3).length;
  const once = new Set(ids).size === N && ids.every(Boolean) && steps.every((v, k) => v.i === k + 1 && v.n === N);
  const marks = steps.filter(v => (shown.has(v.walk) ? v.chip !== v.walk : v.chip !== null));
  check('j steps clockwise in screen order',
    N >= 3 && once && angles.every(Number.isFinite) && descents === 1 && marks.length === 0 && steps.some(v => v.chip === v.walk),
    { N, once, descents, firstAngles: angles.slice(0, 8).map(a => +(a * 180 / Math.PI).toFixed(1)), marks: marks.slice(0, 3), marked: steps.filter(v => v.chip === v.walk).length });
  check('a target in another region shows as a portal',
    !!portal && portal.labels.some(l => String(l[5]).includes(portal.target)) && portal.rim >= 1,
    portal || { error: 'no walk target in another region', N });
  // back to a target whose chip the inspector shows
  for (let k = 0; k < N && !shown.has((await walkState()).walk); k++) {
    const i0 = (await walkState()).i;
    await page.keyboard.press('k');
    await page.waitForFunction(i => document.getElementById('graph').getAttribute('data-walk-i') !== String(i), i0, { timeout: 3000 }).catch(() => {});
  }
  await page.waitForTimeout(200);
  const w1 = await walkState();
  const kb1 = await page.evaluate(() => [...document.querySelectorAll('#ibody .nchip.kb[data-go]')].map(c => c.dataset.go));
  check('the inspector marks the walk target',
    !!w1.walk && kb1.length === 1 && kb1[0] === w1.walk,
    { walk: w1.walk, marked: kb1 });
  await key('c', 800);
  await key('o', 300);
  await page.waitForFunction(() => !!document.querySelector('#ibody .nchip[data-go]'), null, { timeout: 5000 }).catch(() => {});
  await page.waitForTimeout(300);
  const w2 = await walkState();
  const kb2 = await page.evaluate(() => [...document.querySelectorAll('#ibody .nchip.kb[data-go]')].map(c => c.dataset.go));
  const tab = (await state()).sel === hub;
  check('the walk mark survives an inspector render',
    tab && w2.walk === w1.walk && kb2.length === 1 && kb2[0] === w2.walk,
    { walk: w2.walk, before: w1.walk, marked: kb2 });
  await key('Escape');
}
{
  // FIND a word with matches in at least two regions, then z in Results
  const words = [...new Set([word, 'pearc28', ...files.map(n => n.id).sort((x, y) => (degree[y] || 0) - (degree[x] || 0)).slice(0, 12).map(id => id.split('/').pop().replace(/\.md$/, '').split(/[_-]/)[0])])].filter(w => w.length >= 4);
  let pick = null;
  for (const w of words) {
    await page.evaluate(() => document.activeElement?.blur());
    await page.fill('#go', `FIND ${w}`);
    await page.press('#go', 'Enter');
    await page.waitForFunction(() => !document.getElementById('v-list').hidden && !!document.querySelector('#list [data-id], #list .empty'), null, { timeout: 15000 }).catch(() => {});
    await page.waitForTimeout(300);
    const got = await page.evaluate(() => [...new Set([...document.querySelectorAll('#list .lsimple[data-id], #list .lgh[data-id]')].map(e => e.dataset.id))]);
    const ms = got.filter(id => nodeIds.has(id) && kind[id] !== 'root');
    const regions = new Set(ms.map(regionOf).filter(Boolean));
    if (regions.size >= 2 && ms.length >= 3) { pick = { w, ms, regions: regions.size }; break; }
  }
  await page.evaluate(() => document.activeElement?.blur());
  if (!pick) {
    check('z frames the Results matches', false, { error: 'no FIND word with matches in two regions', words });
  } else {
    const view = (await state()).view;
    await waits(() => page.keyboard.press('z'));
    const f = await focusState();
    const box = await page.locator('#graph').boundingBox();
    const pos = await page.evaluate(() => window.__oktPos);
    const pts = pick.ms.map(id => pos[id]);
    const pr = projectAll(f.view, pts.filter(Boolean), box.width, box.height);
    const inside = pr.every(p => p.front && Math.abs(p.nx) <= 1 && Math.abs(p.ny) <= 1);
    const extent = Math.max(0, ...pr.map(p => Math.max(Math.abs(p.nx), Math.abs(p.ny))));
    const now = (await state()).view;
    check('z frames the Results matches',
      view === 'list' && now === 'graph' && f.scale === 'focus' && f.n === pick.ms.length && pts.every(Boolean) && inside && extent >= 0.6 && extent <= 0.8,
      { word: pick.w, matches: pick.ms.length, regions: pick.regions, from: view, to: now, scale: f.scale, focusN: f.n, inside, extent: +extent.toFixed(3) });
  }
}

// ---- WARP -----------------------------------------------------------------------------------
// Enter or space during a walk flies to the target: one move of about 600 ms that rises to at
// least twice both of its end distances, and a history entry. [ eases back to the camera the
// entry left had, ] forward to the flight's end; under reduced motion each is one write of the
// pose. Interleaved moves never trip a borrow (a page error or console "already borrowed").
// a flight in the timeline: from its first data-anim "1" to the "0" after it, and its data-dist
const flightOf = tl => {
  const on = tl.findIndex(e => e[1] === 'data-anim' && e[2] === '1');
  const off = on < 0 ? -1 : tl.findIndex((e, k) => k > on && e[1] === 'data-anim' && e[2] === '0');
  const dists = tl.slice(Math.max(on, 0), off < 0 ? tl.length : off + 1).filter(e => e[1] === 'data-dist').map(e => +e[2]);
  return { ms: on >= 0 && off > on ? +(tl[off][0] - tl[on][0]).toFixed(1) : 0, peak: Math.max(0, ...dists), writes: dists.length };
};
const near = (a, b) => Math.abs(dang(a.theta, b.theta)) <= 0.5 && Math.abs(a.psi - b.psi) <= 0.5 && Math.abs(a.dist - b.dist) <= 0.5;
const poseOf = c => ({ theta: c.theta, psi: c.psi, dist: c.dist });
// j (again, up to 12 times, until the walk is on a card the camera did not move out far to take
// in: a far re-fit puts twice its distance past the galaxy fit, where the flight is capped), then
// the key: what the walk was on, the history length and the camera before, and the flight
const fly = async (k, pg = page) => {
  await pg.evaluate(() => document.activeElement?.blur());
  for (let n = 0; n < 12; n++) {
    const i0 = await gattrOf(pg, 'data-walk-i');
    await pg.keyboard.press('j');
    await pg.waitForFunction(i => document.getElementById('graph').getAttribute('data-walk-i') !== i, i0, { timeout: 3000 }).catch(() => {});
    // j moves the camera only to re-fit a target near the edge: let any such move finish
    await twoFrames(pg);
    await pg.waitForFunction(() => document.getElementById('graph').getAttribute('data-anim') === '0', null, { timeout: 5000 }).catch(() => {});
    const c = await cam(pg);
    if (kind[await gattrOf(pg, 'data-walk')] === 'file' && 2.5 * c.dist <= c.galaxy) break;
  }
  const walk = await gattrOf(pg, 'data-walk');
  const c0 = await cam(pg);
  const h0 = await pg.evaluate(() => history.length);
  const i = await tlLen(pg);
  await waits(() => pg.keyboard.press(k), pg);
  await pg.waitForTimeout(100);
  const c1 = await cam(pg);
  const h1 = await pg.evaluate(() => history.length);
  return { walk, c0, c1, h0, h1, tl: await tlFrom(i, pg), focus: await gattrOf(pg, 'data-focus') };
};
const flew = f => {
  const m = flightOf(f.tl);
  return { ok: !!f.walk && f.c1.sel === f.walk && f.focus === f.walk && f.h1 === f.h0 + 1 && m.ms >= 500 && m.ms <= 800 && m.peak >= 2 * f.c0.dist && m.peak >= 2 * f.c1.dist,
    detail: { walk: f.walk, sel: f.c1.sel, history: [f.h0, f.h1], ms: m.ms, peak: m.peak, from: f.c0.dist, to: f.c1.dist } };
};
{
  // a fresh tab: Chromium caps history.length at 50, which this one has reached
  const wp = await ctx.newPage();
  wp.on('pageerror', e => errors.push(e.message));
  wp.on('console', m => { const t = m.text(); if (t.includes('panicked at') || t.includes('already borrowed')) errors.push('console: ' + t); });
  const page = wp;
  const go = async hash => { await page.evaluate(h => { location.hash = h; }, hash); await page.waitForTimeout(700); };
  const gattr = name => gattrOf(page, name);
  try {
    await page.goto(OKT + '#v=graph', { waitUntil: 'load' });
    await page.waitForFunction(() => document.getElementById('graph')?.getAttribute('data-dist'), null, { timeout: 30000 }).catch(() => {});
    await page.waitForTimeout(800);
    await waits(() => go('#v=graph&s=' + encodeURIComponent(walker)), page);
    await page.evaluate(() => document.activeElement?.blur());
    // orbit about 40 degrees by a drag (dθ = -dx / min(W, H) · 180°)
    const box = await page.locator('#graph').boundingBox();
    const cx = box.x + box.width / 2, cy = box.y + box.height / 2, dx = 40 / 180 * Math.min(box.width, box.height);
    const o0 = await cam(page);
    await page.mouse.move(cx - dx / 2, cy + 9);
    await page.mouse.down();
    await page.mouse.move(cx + dx / 2, cy + 9, { steps: 10 });
    await page.mouse.up();
    await twoFrames(page);
    const o1 = await cam(page);
    const space = await fly(' ', page);
    const s = flew(space);
    check('space flies to the walk target', s.ok, { ...s.detail, orbit: +dang(o0.theta, o1.theta).toFixed(1) });
    // [ back: the hash, the focus and the camera from before the flight, by an ease
    const i = await tlLen(page);
    await waits(() => page.keyboard.press('['), page);
    await page.waitForTimeout(100);
    const b = await cam(page);
    const bt = await tlFrom(i, page);
    const ease = ['data-theta', 'data-psi', 'data-dist'].map(a => eased(bt, a));
    const bf = await gattr('data-focus');
    check('[ eases back to the previous focus',
      b.sel === walker && bf === walker && near(b, space.c0) && ease.some(e => e.mid >= 3 && e.span >= 100) && Math.abs(dang(o0.theta, o1.theta)) >= 20,
      { sel: b.sel, focus: bf, before: poseOf(space.c0), back: poseOf(b), ease, orbit: +dang(o0.theta, o1.theta).toFixed(1) });
    await waits(() => page.keyboard.press(']'), page);
    await page.waitForTimeout(100);
    const f = await cam(page);
    check('] goes forward again', f.sel === space.walk && near(f, space.c1), { sel: f.sel, target: space.walk, end: poseOf(space.c1), now: poseOf(f) });
    const enter = await fly('Enter', page);
    const e = flew(enter);
    check('Enter flies to the walk target', e.ok, e.detail);
  } finally {
    await wp.close();
  }
}
{
  // reduced motion: the flight and [ each write the pose once, and nothing animates
  const rctx = await browser.newContext({ viewport: { width: 1600, height: 1000 }, reducedMotion: 'reduce' });
  try {
    await rctx.addInitScript(hooks);
    const rp = await rctx.newPage();
    rp.on('pageerror', e => errors.push(e.message));
    await rp.goto(OKT + '#v=graph&s=' + encodeURIComponent(walker), { waitUntil: 'load' });
    await rp.waitForFunction(() => document.getElementById('graph')?.getAttribute('data-dist'), null, { timeout: 30000 }).catch(() => {});
    await rp.waitForTimeout(800);
    // one write of each pose hook at most, at least one, all in one frame, each its end value
    const once = (tl, end) => {
      const ws = tl.filter(e => ['data-theta', 'data-psi', 'data-dist'].includes(e[1]));
      const per = a => ws.filter(e => e[1] === a).length;
      // one frame: every write within 4 ms of the first (frames are 7 ms or more apart)
      const span = ws.length ? Math.max(...ws.map(e => e[0])) - Math.min(...ws.map(e => e[0])) : 0;
      return { writes: ws.length, span: +span.toFixed(2),
        ok: ws.length >= 1 && per('data-theta') <= 1 && per('data-psi') <= 1 && per('data-dist') <= 1 && span <= 4
          && ws.every(e => +e[2] === end[{ 'data-theta': 'theta', 'data-psi': 'psi', 'data-dist': 'dist' }[e[1]]]) };
    };
    const f = await fly(' ', rp);
    const flight = once(f.tl, f.c1);
    const i = await tlLen(rp);
    await waits(() => rp.keyboard.press('['), rp);
    await rp.waitForTimeout(200);
    const b = await cam(rp);
    const back = once(await tlFrom(i, rp), b);
    const anim1 = (await rp.evaluate(() => window.__tl)).filter(e => e[1] === 'data-anim' && e[2] === '1').length;
    check('reduced motion cuts the flight',
      flight.ok && back.ok && anim1 === 0 && f.c1.sel === f.walk && !!f.walk && b.sel === walker,
      { flight, back, anim1, walk: f.walk, sel: [f.c1.sel, b.sel] });
  } finally {
    await rctx.close();
  }
}
{
  // interleaved moves: each starts while another is in flight
  const e0 = errors.length;
  const done = [];
  const flightStart = async () => {
    await page.evaluate(() => document.activeElement?.blur());
    await key('j', 150);
    await page.keyboard.press(' ');
    await page.waitForTimeout(200);
  };
  await waits(() => go('#v=graph&s=' + encodeURIComponent(walker)));
  await flightStart();
  await page.keyboard.press('[');
  await page.waitForTimeout(900);
  done.push('[ mid-flight');
  await page.evaluate(h => { location.hash = '#v=graph&s=' + encodeURIComponent(h); }, hub);
  await page.waitForTimeout(60);
  await page.keyboard.press('j');
  await page.keyboard.press('j');
  await page.waitForTimeout(700);
  done.push('j during a selection ease');
  await page.keyboard.press('Escape');
  await waits(() => go('#v=graph&s=' + encodeURIComponent(walker)));
  await flightStart();
  await page.keyboard.press('z');
  await page.waitForTimeout(700);
  done.push('z during a flight');
  const box = await page.locator('#graph').boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (let k = 0; k < 24; k++) { await page.mouse.wheel(0, 240); await page.waitForTimeout(16); }
  for (let k = 0; k < 24; k++) { await page.mouse.wheel(0, -240); await page.waitForTimeout(16); }
  await page.waitForTimeout(600);
  done.push('a wheel run across the scale bands');
  await waits(() => go('#v=graph&s=' + encodeURIComponent(walker)));
  await flightStart();
  await page.locator('#setbtn').click();
  await page.waitForTimeout(100);
  const swatches = page.locator('#settings [data-accent]');
  if (await swatches.count() > 1) { await swatches.nth(1).click(); await page.waitForTimeout(150); await page.locator('#settings [data-accent]').nth(0).click(); }
  done.push('an accent change mid-flight');
  await page.waitForTimeout(150);
  for (let k = 0; k < 2; k++) { await page.locator('#settings [data-showall]').click(); await page.waitForTimeout(250); }
  done.push('the SHOW ALL settings row, twice');
  await page.keyboard.press('Escape');
  await page.evaluate(() => document.activeElement?.blur());
  await waits(() => go('#v=graph&s=' + encodeURIComponent(walker)));
  await flightStart();
  const lost = await page.evaluate(async () => {
    const gl = document.getElementById('graph').getContext('webgl2');
    const ext = gl && gl.getExtension('WEBGL_lose_context');
    if (!ext) return 'no WEBGL_lose_context';
    ext.loseContext();
    await new Promise(r => setTimeout(r, 300));
    ext.restoreContext();
    await new Promise(r => setTimeout(r, 500));
    return gl.isContextLost() ? 'still lost' : 'ok';
  });
  done.push('context loss mid-flight');
  // and the camera still answers
  const m0 = await gattr('data-move');
  await waits(() => page.keyboard.press('ArrowRight'));
  const m1 = await gattr('data-move');
  const fresh = errors.slice(e0);
  check('no borrow panics under interleaved moves',
    fresh.length === 0 && lost === 'ok' && m1 !== m0 && done.length === 7,
    { done, lost, answers: m1 !== m0, errors: fresh.slice(0, 3) });
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
