// The frozen rect probe for pane-arrangements. Planner-written, run by verify/gate.sh beside the
// cycle's own tests/ui/panes.mjs. It grades what renders: every pane, view and size comes from
// getBoundingClientRect, checkVisibility and elementFromPoint. The data-* attributes the client
// writes are used only to find elements, and each is cross-checked against geometry; a mismatch fails.
//
//   tests/env.sh sh -c 'node "$OKT_ROOT/spec/pane-arrangements/verify/rects.mjs" [group …]'
//
// Groups (gate.sh passes them by phase):
//   P1 first-load migration keys drag   P2 presets collapse three   P3 ops   P4 together fit
// Prints "ok   <group>: <label>" / "FAIL <group>: <label>: <detail>"; exits 1 on any FAIL, or when a
// requested group ran no check. Footprint: one Chromium (about 6 processes, under 1 GB), one context
// at a time.
// The DOM contract it reads (PLAN § 2.3): #panes; section.pane with header.ptitle and div.pbody;
// button[data-view] tabs in .ptitle (.on on the shown one); div.sash; the view roots #v-graph
// #v-files #v-log #v-table #v-list #insp; the keys ; ' \ ` and p; the leader w with d x r q v; the
// saved arrangement's JSON (PLAN § 2.1), seeded once for a view no pane holds.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');

const OKT = process.env.OKT || 'http://127.0.0.1:6584/';
const KBNAME = (process.env.OKT_KB || '/kb').split('/').filter(Boolean).pop() || 'kb';
const SKEY = 'okt:' + KBNAME;
const ALL = ['first-load', 'migration', 'keys', 'drag', 'presets', 'collapse', 'three', 'ops', 'together', 'fit'];
const want = process.argv.slice(2).length ? process.argv.slice(2) : ALL;
for (const g of want) if (!ALL.includes(g)) { console.log(`FAIL unknown group ${g}`); process.exit(2); }

const ROOT = { graph: '#v-graph', files: '#v-files', log: '#v-log', table: '#v-table', list: '#v-list', insp: '#insp' };
const FIVE = ['graph', 'files', 'log', 'table', 'list'];
const PRESET_KEY = { classic: ';', swapped: "'", stacked: '\\', three: '`' };
const TITLE = 24;
const sorted = a => a.slice().sort().join();

const idx = await (await fetch(OKT + 'api/index')).json();
const degree = {};
for (const e of idx.edges) { const [s, t] = Array.isArray(e) ? e : [e.s, e.t]; degree[s] = (degree[s] || 0) + 1; degree[t] = (degree[t] || 0) + 1; }
const mdFiles = idx.nodes.filter(n => n.t === 'file' && n.id.endsWith('.md'));
const hub = mdFiles.slice().sort((a, b) => (degree[b.id] || 0) - (degree[a.id] || 0))[0].id;
const stem = id => id.split('/').pop().replace(/\.md$/, '');

let fails = 0;
const ran = {};
let group = '';
const check = (label, ok, detail) => {
  ran[group] = (ran[group] || 0) + 1;
  if (!ok) fails++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${group}: ${label}${ok ? '' : ': ' + JSON.stringify(detail)}`);
};
const near = (a, b, tol) => Math.abs(a - b) <= tol;

const browser = await chromium.launch();
const errors = [];

async function open(w, h = 1000, { seed = null, dpr = 1, hash = '' } = {}) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: dpr });
  const page = await ctx.newPage();
  page.on('pageerror', e => errors.push(e.message));
  if (seed) {
    // seed only states real users have: the pre-cycle settings blob, or a saved arrangement
    await page.goto(OKT + 'api/index');
    await page.evaluate(([k, v]) => localStorage.setItem(k, v), [SKEY, JSON.stringify(seed)]);
  }
  await page.goto(OKT + hash, { waitUntil: 'load' });
  await ready(page);
  const kb = await page.evaluate(() => JSON.parse(document.getElementById('boot').textContent).kb || 'kb');
  if (kb !== KBNAME) { console.log(`FAIL settings key: boot kb ${kb} != ${KBNAME}`); fails++; }
  return { ctx, page };
}

// ready: panes exist, the index has loaded (the status bar has text), and pane rects are unchanged
// across two animation frames
async function ready(page) {
  await page.waitForFunction(() => document.querySelectorAll('#panes .pane').length > 0, null, { timeout: 15000 });
  await page.waitForFunction(() => (document.getElementById('status')?.innerText || '').trim().length > 0, null, { timeout: 8000 }).catch(() => {});
  await page.waitForTimeout(300);
  await settle(page);
}
async function settle(page) {
  await page.evaluate(() => new Promise(res => {
    const sig = () => [...document.querySelectorAll('#panes .pane')].map(p => { const r = p.getBoundingClientRect(); return [r.x, r.y, r.width, r.height].map(v => v.toFixed(2)).join(','); }).join('|');
    let last = sig(), same = 0, n = 0;
    const tick = () => { const s = sig(); same = s === last ? same + 1 : 0; last = s; if (same >= 2 || ++n > 120) res(); else requestAnimationFrame(tick); };
    requestAnimationFrame(tick);
  }));
  await page.waitForTimeout(150);
}
async function key(page, k, ms = 250) {
  await page.evaluate(() => { const a = document.activeElement; if (a && a !== document.body && a.matches('input, textarea')) a.blur(); });
  await page.keyboard.press(k);
  await page.waitForTimeout(ms);
  await settle(page);
}
const leader = async (page, k) => { await key(page, 'w', 60); await key(page, k); };
async function hashTo(page, h) { await page.evaluate(h => { location.hash = h; }, h); await page.waitForTimeout(700); await settle(page); }

// The arrangement as rendered, in reading order. For each pane: its rect relative to #panes; the
// views its title bar shows (visible tab buttons inside the title rect whose centre hits the tab);
// the view roots visible inside its body whose centre hits the root (nothing covers them); whether
// the shown root has content; the title bar's own overflow; how the title is drawn.
async function snap(page) {
  return page.evaluate(([ROOT]) => {
    const vis = e => !!e && e.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true }) && e.getBoundingClientRect().width > 0 && e.getBoundingClientRect().height > 0;
    const inside = (a, b, t = 1) => a.left >= b.left - t && a.top >= b.top - t && a.right <= b.right + t && a.bottom <= b.bottom + t;
    const hits = e => { const r = e.getBoundingClientRect(); const at = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2); return !!at && e.contains(at); };
    const content = {
      graph: () => true,
      files: () => document.querySelectorAll('#tree .trow:not(.thead)').length > 0,
      log: () => document.querySelectorAll('#ltable > *').length > 0,
      table: () => document.querySelectorAll('#tgrid tr').length > 0,
      list: () => document.getElementById('list').children.length > 0,
      insp: () => (document.getElementById('ibody').innerText || '').trim().length > 0,
    };
    const root = document.getElementById('panes').getBoundingClientRect();
    const panes = [...document.querySelectorAll('#panes .pane')].map(p => {
      const r = p.getBoundingClientRect();
      const t = p.querySelector(':scope > .ptitle');
      const b = p.querySelector(':scope > .pbody');
      const tr = t ? t.getBoundingClientRect() : null;
      const tabEls = t ? [...t.querySelectorAll('button[data-view]')].filter(x => vis(x) && inside(x.getBoundingClientRect(), tr)) : [];
      const roots = Object.entries(ROOT).filter(([, sel]) => { const e = document.querySelector(sel); return vis(e) && b && inside(e.getBoundingClientRect(), b.getBoundingClientRect()); }).map(([v]) => v);
      const held = Object.entries(ROOT).filter(([, sel]) => p.contains(document.querySelector(sel))).map(([v]) => v);
      const parent = p.parentElement?.closest('.psplit');
      const orient = parent ? (getComputedStyle(parent).flexDirection.startsWith('row') ? 'row' : 'col') : null;
      const ts = t ? getComputedStyle(t) : null;
      return {
        x: r.left - root.left, y: r.top - root.top, w: r.width, h: r.height,
        title: tr ? { x: tr.left - root.left, y: tr.top - root.top, w: tr.width, h: tr.height } : null,
        titleOver: t ? t.scrollWidth - t.clientWidth : 0,
        tabs: tabEls.map(x => x.dataset.view),
        tabsHit: tabEls.every(hits),
        onTab: t?.querySelector('button[data-view].on')?.dataset.view || null,
        roots, held, orient,
        rootHit: roots.every(v => hits(document.querySelector(ROOT[v]))),
        rootFull: roots.every(v => content[v]()),
        bodyShown: vis(b),
        look: ts ? [ts.backgroundColor, ts.borderTopColor, ts.boxShadow, ts.color].join(' ') : '',
        attr: { views: (p.dataset.views || '').split(/\s+/).filter(Boolean), shown: p.dataset.shown, collapsed: p.dataset.collapsed, focused: p.dataset.focused },
      };
    }).sort((a, b) => a.y - b.y || a.x - b.x);
    return { W: root.width, H: root.height, panes };
  }, [ROOT]);
}

// Derived facts, cross-checked against the attributes. Returns a list of problems.
function audit(s) {
  const out = [];
  const fi = s.panes.findIndex(p => p.attr.focused === '1');
  if (s.panes.filter(p => p.attr.focused === '1').length !== 1) out.push(`${s.panes.filter(p => p.attr.focused === '1').length} panes marked focused`);
  else if (s.panes.some((p, i) => i !== fi && p.look === s.panes[fi].look)) out.push('the focused title bar is drawn like another pane\'s');
  for (const [i, p] of s.panes.entries()) {
    const collapsed = !p.bodyShown && p.title && (p.orient === 'row' ? near(p.w, p.title.w, 1) && near(p.w, TITLE, 1) && p.title.h > p.title.w : near(p.h, p.title.h, 1) && near(p.h, TITLE, 1));
    const shown = p.roots.length === 1 ? p.roots[0] : null;
    if (sorted(p.tabs) !== sorted(p.attr.views)) out.push(`pane ${i}: tabs ${p.tabs} != data-views ${p.attr.views}`);
    if (sorted(p.held) !== sorted(p.tabs)) out.push(`pane ${i}: holds ${p.held} but shows tabs ${p.tabs}`);
    if (!p.tabsHit) out.push(`pane ${i}: a tab is covered`);
    if (p.titleOver > 1) out.push(`pane ${i}: title bar overflows by ${p.titleOver}`);
    if (!!collapsed !== (p.attr.collapsed === '1')) out.push(`pane ${i}: collapsed ${!!collapsed} != data-collapsed ${p.attr.collapsed}`);
    if (p.onTab !== p.attr.shown) out.push(`pane ${i}: tab marked ${p.onTab} != data-shown ${p.attr.shown}`);
    if (!collapsed && p.w >= 200) {
      if (!shown || shown !== p.attr.shown) out.push(`pane ${i}: visible roots ${p.roots} != data-shown ${p.attr.shown}`);
      if (!p.rootHit) out.push(`pane ${i}: its view is covered`);
      if (!p.rootFull) out.push(`pane ${i}: its view ${shown} is empty`);
    }
    if ((collapsed || p.w < 200) && p.roots.length) out.push(`pane ${i}: hidden body but shows ${p.roots}`);
    if (!p.title || !(p.title.w > 0 && p.title.h > 0)) out.push(`pane ${i}: no visible title bar`);
  }
  return out;
}
const derived = p => ({ views: sorted(p.tabs), shown: p.roots[0] || null, on: p.onTab, collapsed: !p.bodyShown });

// PLAN § 2.4: the presets' panes, in reading order, as [x, y, w, h, views, shown, collapsed]
function expect(preset, W, H, c = 0.54) {
  const V5 = FIVE, V4 = ['files', 'log', 'table', 'list'];
  switch (preset) {
    case 'classic': return [[0, 0, c * W, H, V5, 'graph', false], [c * W, 0, (1 - c) * W, H, ['insp'], 'insp', false]];
    case 'swapped': return [[0, 0, 0.46 * W, H, ['insp'], 'insp', false], [0.46 * W, 0, 0.54 * W, H, V5, 'graph', false]];
    case 'stacked': return [[0, 0, W, H / 2, V5, 'graph', false], [0, H / 2, W, H / 2, ['insp'], 'insp', false]];
    case 'three': return [[0, 0, W, TITLE, ['graph'], null, true], [0, TITLE, W / 2, H - TITLE, ['insp'], 'insp', false], [W / 2, TITLE, W / 2, H - TITLE, V4, 'files', false]];
  }
}
function grade(label, s, exp, tol = 2) {
  const bad = audit(s);
  if (s.panes.length !== exp.length) bad.push(`${s.panes.length} panes, want ${exp.length}`);
  exp.forEach(([x, y, w, h, views, shown, collapsed], i) => {
    const p = s.panes[i];
    if (!p) return;
    const d = derived(p);
    if (![[p.x, x], [p.y, y], [p.w, w], [p.h, h]].every(([a, b]) => near(a, b, tol))) bad.push(`pane ${i} rect ${[p.x, p.y, p.w, p.h].map(v => v.toFixed(1))} want ${[x, y, w, h].map(v => v.toFixed(1))}`);
    if (d.views !== sorted(views)) bad.push(`pane ${i} views ${d.views} want ${views}`);
    if (!collapsed && d.shown !== shown) bad.push(`pane ${i} shows ${d.shown} want ${shown}`);
    if (d.collapsed !== collapsed) bad.push(`pane ${i} collapsed ${d.collapsed} want ${collapsed}`);
  });
  check(label, bad.length === 0, bad);
}
const same = (a, b, tol = 1) => a.panes.length === b.panes.length && a.panes.every((p, i) => {
  const q = b.panes[i], d = derived(p), e = derived(q);
  return near(p.x, q.x, tol) && near(p.y, q.y, tol) && near(p.w, q.w, tol) && near(p.h, q.h, tol) && d.views === e.views && d.shown === e.shown && d.on === e.on && d.collapsed === e.collapsed;
});
const shape = s => s.panes.map(p => [derived(p).views, derived(p).on, derived(p).collapsed, p.orient, [p.x, p.y, p.w, p.h].map(v => v.toFixed(1)).join(',')].join('/')).join(' | ');

// Wait for the arrangement to reach localStorage (the save is debounced), reload, compare; then a
// new page at the bare URL (no history, no sessionStorage, no hash) must show the same.
async function reloadSame(page, ctx, label) {
  const stored = await page.waitForFunction(k => (localStorage.getItem(k) || '').includes('"arrangement"'), SKEY, { timeout: 3000 }).then(() => true, () => false);
  check(`${label}: the arrangement is stored`, stored, {});
  await page.waitForTimeout(300);
  const before = await snap(page);
  await page.reload({ waitUntil: 'load' });
  await ready(page);
  const after = await snap(page);
  check(label, same(before, after), { before: shape(before), after: shape(after) });
  const fresh = await ctx.newPage();
  fresh.on('pageerror', e => errors.push(e.message));
  await fresh.goto(OKT, { waitUntil: 'load' });
  await ready(fresh);
  const bare = await snap(fresh);
  check(`${label}: a new page at the bare URL shows the same`, same(before, bare), { before: shape(before), bare: shape(bare) });
  await fresh.close();
}
async function sash(page, i = 0) {
  return page.evaluate(i => { const s = [...document.querySelectorAll('#panes .sash')].filter(e => e.checkVisibility() && e.getBoundingClientRect().width > 0)[i]; if (!s) return null; const r = s.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; }, i);
}
async function drag(page, from, to, mid = null) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 4 });
  const during = mid ? await mid() : null;
  await page.mouse.move(to.x, to.y, { steps: 4 });
  await page.mouse.up();
  await settle(page);
  return during;
}
async function rootBox(page) { return page.evaluate(() => { const r = document.getElementById('panes').getBoundingClientRect(); return { left: r.left, top: r.top, right: r.right, bottom: r.bottom, W: r.width, H: r.height }; }); }
const PANES_IN_ORDER = `[...document.querySelectorAll('#panes .pane')].sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top || a.getBoundingClientRect().left - b.getBoundingClientRect().left)`;
// click the title bar's empty end (right, or bottom for a strip), never a tab
async function clickTitle(page, i) {
  const pt = await page.evaluate(([i, q]) => {
    const p = eval(q)[i]; const t = p.querySelector('.ptitle'); const r = t.getBoundingClientRect();
    const tabs = [...t.querySelectorAll('button[data-view]')].map(b => b.getBoundingClientRect());
    const strip = r.height > r.width;
    for (let k = 2; k < 40; k += 2) {
      const x = strip ? r.left + r.width / 2 : r.right - k, y = strip ? r.bottom - k : r.top + r.height / 2;
      if (!tabs.some(b => x >= b.left && x <= b.right && y >= b.top && y <= b.bottom)) return { x, y };
    }
    return { x: r.right - 2, y: r.bottom - 2 };
  }, [i, PANES_IN_ORDER]);
  await page.mouse.click(pt.x, pt.y);
  await settle(page);
}
const focusedIndex = s => s.panes.findIndex(p => p.attr.focused === '1');
// count toasts added from now on (they expire, so count additions, never the live list)
async function watchToasts(page) {
  await page.evaluate(() => {
    window.__paToasts = [];
    new MutationObserver(ms => { for (const m of ms) for (const n of m.addedNodes) if (n.classList?.contains('toast')) window.__paToasts.push(n.innerText); })
      .observe(document.getElementById('toasts'), { childList: true });
  });
}
const toasts = page => page.evaluate(() => (window.__paToasts || []).length);

// R15: nothing in the pane lies outside it, and no scroller in it scrolls sideways, except the
// content that is meant to: the Table's grid and the Inspector's body (rendered markdown, wide
// tables, History rows as the history cycle shipped them).
async function overflow(page, i) {
  return page.evaluate(([i, q]) => {
    const p = eval(q)[i];
    const pr = p.getBoundingClientRect();
    const bad = [];
    if (p.scrollWidth > p.clientWidth + 1) bad.push(`pane scrollWidth ${p.scrollWidth} > ${p.clientWidth}`);
    const allowed = e => e.id === 'tgrid' || e.id === 'ibody' || !!e.closest('#tgrid, #ibody');
    const scroller = e => { const o = getComputedStyle(e).overflowX; return o === 'auto' || o === 'scroll'; };
    const walk = (e, inScroll) => {
      for (const c of e.children) {
        if (!c.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true })) continue;
        const r = c.getBoundingClientRect();
        const name = `${c.tagName.toLowerCase()}${c.id ? '#' + c.id : ''}${c.classList.length ? '.' + [...c.classList].join('.') : ''}`;
        if (!inScroll && r.width > 0 && r.height > 0 && (r.left < pr.left - 1 || r.right > pr.right + 1)) bad.push(`${name} [${r.left.toFixed(0)}, ${r.right.toFixed(0)}] outside [${pr.left.toFixed(0)}, ${pr.right.toFixed(0)}]`);
        if (scroller(c) && !allowed(c) && c.scrollWidth > c.clientWidth + 1) bad.push(`${name} scrolls sideways (${c.scrollWidth} > ${c.clientWidth})`);
        if (bad.length > 8) return;
        walk(c, inScroll || scroller(c));
      }
    };
    walk(p, false);
    return bad;
  }, [i, PANES_IN_ORDER]);
}

// ---------------------------------------------------------------------------------------------
async function run(name, body) {
  if (!want.includes(name)) return;
  group = name;
  try { await body(); } catch (e) { check('ran to the end', false, String(e && e.stack || e).slice(0, 400)); }
  if (!ran[name]) { console.log(`FAIL ${name}: no check ran`); fails++; }
}

await run('first-load', async () => {
  for (const [w, preset] of [[899, 'stacked'], [900, 'classic'], [1600, 'classic']]) {
    const { ctx, page } = await open(w);
    const s = await snap(page);
    grade(`empty store at ${w} px starts ${preset}`, s, expect(preset, s.W, s.H));
    check(`${w} px: the root spans the window between the bars`, near(s.W, w, 1) && near(s.H, 890, 1), { W: s.W, H: s.H });
    // an unrelated setting is saved (the graph's all-labels toggle); the default must not be
    // saved as a choice
    await page.click('#v-graph .gtools button[data-z="labels"]');
    await page.waitForTimeout(600);
    const stored = await page.evaluate(k => localStorage.getItem(k) || '', SKEY);
    check(`${w} px: a settings save leaves the arrangement unsaved`, stored.includes('"labels"') && !stored.includes('"arrangement"'), stored.slice(0, 200));
    await ctx.close();
  }
  // a bookmarked view on a first load shows it without saving the arrangement as a choice
  const { ctx, page } = await open(1600, 1000, { hash: '#v=log' });
  const s = await snap(page);
  check('a first load at #v=log shows the Log in classic\'s left pane', s.panes.length === 2 && s.panes[0].roots.join() === 'log' && audit(s).length === 0, { roots: s.panes.map(p => p.roots), audit: audit(s) });
  const stored = await page.evaluate(k => localStorage.getItem(k) || '', SKEY);
  check('that first load saved no arrangement', !stored.includes('"arrangement"'), stored.slice(0, 200));
  await ctx.close();
});

await run('migration', async () => {
  for (const [w, preset] of [[1600, 'classic'], [900, 'classic'], [899, 'stacked']]) {
    const { ctx, page } = await open(w, 1000, { seed: { theme: 'terminal', insp: 30 } });
    const s = await snap(page);
    grade(`a pre-cycle insp of 30 at ${w} px starts ${preset}${preset === 'classic' ? ' with the Inspector at 30%' : ''}`, s, expect(preset, s.W, s.H, 0.70));
    await ctx.close();
  }
});

await run('keys', async () => {
  let { ctx, page } = await open(1600);
  for (const [k, v] of [['f', 'files'], ['l', 'log'], ['t', 'table'], ['g', 'graph']]) {
    await key(page, k, 600);
    const s = await snap(page);
    check(`${k} shows ${v} in the pane holding it and focuses that pane`, audit(s).length === 0 && focusedIndex(s) === 0 && s.panes[0].roots.join() === v, { f: focusedIndex(s), roots: s.panes[0].roots, audit: audit(s) });
  }
  for (const [k, tab] of [['o', 'overview'], ['c', 'content'], ['h', 'history']]) {
    await key(page, k, 500);
    const s = await snap(page);
    const on = await page.evaluate(t => !!document.querySelector(`#itabs button[data-tab=${t}].on`), tab);
    check(`${k} shows the Inspector on ${tab} and keeps focus on the Graph's pane`, audit(s).length === 0 && focusedIndex(s) === 0 && s.panes[0].roots.join() === 'graph' && s.panes[1].roots.join() === 'insp' && on, { f: focusedIndex(s), on, audit: audit(s) });
  }
  // c then j: the Inspector key leaves focus on the Graph, so j still steps the Graph walk
  await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
  await key(page, 'c', 500);
  await key(page, 'j', 400);
  {
    const s = await snap(page);
    const w = await page.evaluate(() => { const h = document.getElementById('ghint'); return { hint: h && !h.hidden ? h.textContent : '', content: !!document.querySelector('#itabs button[data-tab=content].on') }; });
    check('c then j steps the Graph walk and the Graph\'s pane keeps focus', /^1\/\d+ · /.test(w.hint) && w.content && focusedIndex(s) === 0 && s.panes[0].roots.join() === 'graph' && audit(s).length === 0, { w, f: focusedIndex(s), audit: audit(s) });
  }
  await key(page, 'Escape', 300);
  await clickTitle(page, 1);
  check('clicking a pane focuses it', focusedIndex(await snap(page)) === 1, {});
  await clickTitle(page, 0);
  check('clicking the other pane moves focus back', focusedIndex(await snap(page)) === 0, {});
  // a tab click shows its view
  const tab = await page.evaluate(q => { const t = eval(q)[0].querySelector('.ptitle button[data-view="log"]'); const r = t.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; }, PANES_IN_ORDER);
  await page.mouse.click(tab.x, tab.y); await page.waitForTimeout(600); await settle(page);
  let s = await snap(page);
  check('clicking the LOG tab shows the Log, with its rows', s.panes[0].roots.join() === 'log' && audit(s).length === 0, { roots: s.panes[0].roots, audit: audit(s) });
  // the hash and the selection: every shown view follows it
  const deep = mdFiles.slice().sort((a, b) => b.id.split('/').length - a.id.split('/').length)[0].id;
  await hashTo(page, '#v=files&s=' + encodeURIComponent(deep));
  s = await snap(page);
  const follow = await page.evaluate(() => ({ on: document.querySelector('#tree .trow.on')?.dataset.id || null, head: document.getElementById('ihead').innerText }));
  check('a hash with v=files shows Files, focuses its pane, and Files and the Inspector follow the selection', s.panes[0].roots.join() === 'files' && focusedIndex(s) === 0 && follow.on === deep && follow.head.includes(stem(deep)), { roots: s.panes[0].roots, follow });
  await page.keyboard.press('/'); await page.waitForTimeout(200);
  check('/ focuses the command line', await page.evaluate(() => document.activeElement?.id === 'go'), {});
  await page.keyboard.press('Escape');
  await ctx.close();
  // a view no pane holds is added to the focused pane
  ({ ctx, page } = await open(1600, 1000, { seed: { arrangement: { v: 1, root: { split: 'row', f: 0.54, kids: [{ views: ['graph', 'files', 'table', 'list'], shown: 'graph', collapsed: false }, { views: ['insp'], shown: 'insp', collapsed: false }] } } } }));
  s = await snap(page);
  check('a saved arrangement without the Log loads without it', s.panes.length === 2 && !s.panes.some(p => p.tabs.includes('log')) && audit(s).length === 0, { tabs: s.panes.map(p => p.tabs), audit: audit(s) });
  await clickTitle(page, 1);
  await key(page, 'l', 600);
  s = await snap(page);
  check('l adds the Log to the focused pane and shows it there', s.panes[1].tabs.includes('log') && s.panes[1].roots.join() === 'log' && focusedIndex(s) === 1 && audit(s).length === 0, { tabs: s.panes.map(p => p.tabs), roots: s.panes.map(p => p.roots), audit: audit(s) });
  await ctx.close();
});

await run('drag', async () => {
  let { ctx, page } = await open(1600);
  let R = await rootBox(page);
  let from = await sash(page);
  check('a split has a sash on the boundary', !!from && near(from.x - R.left, 0.54 * R.W, 3), from);
  if (from) {
    const during = await drag(page, from, { x: R.left + 1000, y: from.y }, () => snap(page));
    const mid = (from.x - R.left + 1000) / 2;
    check('mid-drag the panes follow the pointer', !!during && near(during.panes[0].w, mid, 3) && near(during.panes[1].w, R.W - mid, 3), during && during.panes.map(p => p.w));
    let s = await snap(page);
    check('after the drag the boundary is at the pointer', near(s.panes[0].w, 1000, 2) && near(s.panes[1].w, R.W - 1000, 2), s.panes.map(p => p.w));
    await drag(page, await sash(page), { x: R.left + 40, y: from.y });
    s = await snap(page);
    check('the drag stops the left side at 200 px', near(s.panes[0].w, 200, 2), s.panes.map(p => p.w));
    await drag(page, await sash(page), { x: R.right - 40, y: from.y });
    s = await snap(page);
    check('the drag stops the right side at 200 px', near(s.panes[1].w, 200, 2), s.panes.map(p => p.w));
    await drag(page, await sash(page), { x: R.left + 700, y: from.y });
    await reloadSame(page, ctx, 'a dragged split survives a reload within 1 px');
  }
  await ctx.close();
  ({ ctx, page } = await open(899));
  R = await rootBox(page);
  from = await sash(page);
  if (from) {
    await drag(page, from, { x: from.x, y: R.top + 20 });
    const s = await snap(page);
    check('a stacked drag stops the top at 120 px', near(s.panes[0].h, 120, 2), s.panes.map(p => p.h));
  } else check('a stacked split has a sash', false, {});
  await ctx.close();
});

await run('presets', async () => {
  for (const w of [1600, 900]) {
    const { ctx, page } = await open(w);
    for (const preset of ['classic', 'swapped', 'stacked', 'three']) {
      await key(page, PRESET_KEY[preset], 400);
      const s = await snap(page);
      grade(`${preset} at ${w}×1000`, s, expect(preset, s.W, s.H));
      await reloadSame(page, ctx, `${preset} at ${w}×1000 survives a reload within 1 px`);
    }
    // the cycle key: three -> classic -> swapped -> stacked -> three
    for (const preset of ['classic', 'swapped', 'stacked', 'three']) {
      await key(page, 'p', 400);
      const s = await snap(page);
      grade(`p cycles to ${preset} at ${w}`, s, expect(preset, s.W, s.H));
    }
    await ctx.close();
  }
  // a chosen arrangement survives any window width; the fractions hold
  {
    const { ctx, page } = await open(1600);
    for (const preset of ['swapped', 'three']) {
      await key(page, PRESET_KEY[preset], 400);
      for (const w of [880, 1300]) {
        await page.setViewportSize({ width: w, height: 1000 });
        await settle(page);
        const s = await snap(page);
        grade(`${preset} kept at a ${w} px window`, s, expect(preset, s.W, s.H));
      }
      await page.setViewportSize({ width: 1600, height: 1000 }); await settle(page);
    }
    await ctx.close();
  }
  // classic by key takes a pre-cycle insp width
  {
    const { ctx, page } = await open(1600, 1000, { seed: { insp: 30 } });
    await key(page, PRESET_KEY.swapped, 300);
    await key(page, PRESET_KEY.classic, 400);
    const s = await snap(page);
    grade('classic by key takes a pre-cycle insp of 30', s, expect('classic', s.W, s.H, 0.70));
    await ctx.close();
  }
  // a Graph moved without a resize (classic -> swapped) still hit-tests where it is drawn
  {
    const { ctx, page } = await open(1600);
    await key(page, PRESET_KEY.swapped, 600);
    const tip = await page.evaluate(() => { const r = document.getElementById('graph').getBoundingClientRect(); return { left: r.left, top: r.top, w: r.width, h: r.height }; });
    let found = null;
    for (let gy = 1; gy < 12 && !found; gy++) for (let gx = 1; gx < 16 && !found; gx++) {
      const x = tip.left + tip.w * gx / 16, y = tip.top + tip.h * gy / 12;
      await page.mouse.move(x, y); await page.waitForTimeout(25);
      const t = await page.evaluate(() => { const e = document.getElementById('gtip'); if (!e || e.hidden || !e.checkVisibility()) return null; const r = e.getBoundingClientRect(); return { l: r.left, t: r.top, r: r.right, b: r.bottom }; });
      if (t) found = { x, y, t };
    }
    check('after classic -> swapped a hover tip appears beside the pointer, inside the Graph pane', !!found && Math.abs(found.t.l - found.x) < 400 && Math.abs(found.t.t - found.y) < 400 && found.t.l >= tip.left - 2, found);
    await ctx.close();
  }
  // the help overlay pairs each preset key with its preset, and p with the cycle
  {
    const { ctx, page } = await open(1600);
    await key(page, '?', 300);
    const rows = await page.evaluate(() => [...document.querySelectorAll('#help dt')].map(dt => ({ k: dt.innerText, d: dt.nextElementSibling?.innerText || '' })));
    const has = (k, word) => rows.some(r => r.k.split(/\s+/).includes(k) && r.d.toLowerCase().includes(word));
    check('the help lists each preset key beside its preset, and p beside the cycle', has(';', 'classic') && has("'", 'swapped') && has('\\', 'stacked') && has('`', 'three') && rows.some(r => r.k.split(/\s+/).includes('p') && /cycle|next/.test(r.d.toLowerCase())), rows.slice(0, 40));
    await ctx.close();
  }
});

await run('collapse', async () => {
  const { ctx, page } = await open(1600);
  const R = await rootBox(page), W = R.W, H = R.H;
  await clickTitle(page, 0);
  await leader(page, 'd');
  let s = await snap(page);
  check('collapse beside: a 24 px strip with its title on its side; the neighbour takes the width; focus stays', near(s.panes[0].w, TITLE, 1) && s.panes[0].title?.h > s.panes[0].title?.w && !s.panes[0].bodyShown && near(s.panes[1].w, W - TITLE, 2) && focusedIndex(s) === 0 && audit(s).length === 0, { w: s.panes.map(p => p.w), f: focusedIndex(s), audit: audit(s) });
  await leader(page, 'd');
  s = await snap(page);
  grade('expand returns it to its size before', s, expect('classic', W, H));
  await key(page, PRESET_KEY.stacked, 400);
  await clickTitle(page, 0);
  await leader(page, 'd');
  s = await snap(page);
  check('collapse in a stack: its title bar; the neighbour takes the height', near(s.panes[0].h, TITLE, 1) && near(s.panes[1].h, H - TITLE, 2) && audit(s).length === 0, { h: s.panes.map(p => p.h), audit: audit(s) });
  await reloadSame(page, ctx, 'a collapsed pane survives a reload within 1 px');
  await clickTitle(page, 1);
  await leader(page, 'd');
  s = await snap(page);
  check('collapsing beside a collapsed pane expands that pane', near(s.panes[1].h, TITLE, 1) && near(s.panes[0].h, H - TITLE, 2) && audit(s).length === 0, { h: s.panes.map(p => p.h), audit: audit(s) });
  // g expands the collapsed Graph in three and focuses it
  await key(page, PRESET_KEY.three, 400);
  await key(page, 'g', 600);
  s = await snap(page);
  check('g expands three\'s collapsed Graph and focuses it', near(s.panes[0].h, 0.4 * H, 2) && s.panes[0].roots.join() === 'graph' && focusedIndex(s) === 0 && audit(s).length === 0, { h: s.panes.map(p => p.h), audit: audit(s) });
  await ctx.close();
});

await run('three', async () => {
  for (const w of [1600, 900]) {
    const { ctx, page } = await open(w);
    await key(page, PRESET_KEY.three, 400);
    await hashTo(page, '#v=files&s=' + encodeURIComponent(hub));
    const s = await snap(page);
    grade(`three at ${w} with ${hub} selected`, s, expect('three', s.W, s.H));
    const g = s.panes[0];
    check(`${w}: the Graph's title bar spans the top`, g.title && near(g.title.x, 0, 1) && near(g.title.y, 0, 1) && near(g.title.w, s.W, 2) && near(g.h, g.title.h, 1) && g.roots.length === 0 && g.tabs.join() === 'graph', g);
    const facts = await page.evaluate(([hub, q]) => {
      const vis = e => !!e && e.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true }) && e.getBoundingClientRect().width > 0;
      const panes = eval(q);
      const inside = (e, box, t = 1) => { const r = e.getBoundingClientRect(); return r.left >= box.left - t && r.right <= box.right + t && r.top >= box.top - t && r.bottom <= box.bottom + t; };
      const ip = panes[1].getBoundingClientRect(), fp = panes[2].getBoundingClientRect();
      const ihead = document.getElementById('ihead');
      const tabs = [...document.querySelectorAll('#itabs button[data-tab]')].filter(b => vis(b) && inside(b, ip)).map(b => b.dataset.tab);
      const tree = document.getElementById('tree');
      const tb = tree.getBoundingClientRect();
      const client = { left: tb.left, top: tb.top, right: tb.left + tree.clientWidth, bottom: tb.top + tree.clientHeight };
      const body = panes[2].querySelector('.pbody').getBoundingClientRect();
      const names = [...tree.querySelectorAll('.trow:not(.thead) .nm')].filter(n => vis(n) && n.innerText.trim() && inside(n, client) && inside(n, body) && n.getBoundingClientRect().width >= 40);
      return {
        insp: vis(document.getElementById('insp')) && inside(document.getElementById('insp'), ip),
        head: vis(ihead) && inside(ihead, ip) && ihead.innerText.includes(hub.split('/').pop().replace(/\.md$/, '')),
        tabs,
        filter: vis(document.getElementById('ffilter')) && inside(document.getElementById('ffilter'), fp),
        rows: names.length,
        filesRight: fp.left >= ip.right - 1,
      };
    }, [hub, PANES_IN_ORDER]);
    check(`${w}: the Inspector is on the left showing ${hub} with its O/C/H tabs`, facts.insp && facts.head && ['overview', 'content', 'history'].every(t => facts.tabs.includes(t)), facts);
    check(`${w}: Files is on the right with its filter and at least 10 named rows`, facts.filter && facts.rows >= 10 && facts.filesRight, facts);
    for (const i of [1, 2]) { const bad = await overflow(page, i); check(`${w}: no horizontal overflow in pane ${i}`, bad.length === 0, bad); }
    await ctx.close();
  }
});

await run('ops', async () => {
  const { ctx, page } = await open(1600);
  const R = await rootBox(page), W = R.W, H = R.H;
  await watchToasts(page);
  // swap
  await clickTitle(page, 0);
  await leader(page, 'x');
  let s = await snap(page);
  check('swap: the two exchange places, each keeping its views', near(s.panes[0].w, 0.54 * W, 2) && derived(s.panes[0]).views === 'insp' && derived(s.panes[1]).views === sorted(FIVE) && focusedIndex(s) === 1 && audit(s).length === 0, { v: s.panes.map(p => p.tabs), f: focusedIndex(s), audit: audit(s) });
  // flip
  await key(page, PRESET_KEY.classic, 400);
  await clickTitle(page, 0);
  await leader(page, 'r');
  s = await snap(page);
  check('flip: side by side becomes stacked, keeping order and share', s.panes.length === 2 && near(s.panes[0].w, W, 2) && near(s.panes[0].h, 0.54 * H, 2) && derived(s.panes[0]).views === sorted(FIVE) && derived(s.panes[1]).views === 'insp' && audit(s).length === 0, s.panes.map(p => [p.w, p.h, p.tabs]));
  // move the shown view to the neighbour
  await key(page, PRESET_KEY.classic, 400);
  await clickTitle(page, 0);
  await key(page, 'g', 400);
  await leader(page, 'v');
  s = await snap(page);
  check('move: the view appears in the neighbour as its shown tab and leaves the source', derived(s.panes[1]).views === sorted(['graph', 'insp']) && s.panes[1].roots.join() === 'graph' && !s.panes[0].tabs.includes('graph') && focusedIndex(s) === 1 && audit(s).length === 0, { v: s.panes.map(p => [p.tabs, p.roots]), f: focusedIndex(s), audit: audit(s) });
  // nested floors: the Graph beside a split of two panes can't squeeze them under 200 px each
  await key(page, PRESET_KEY.three, 400);
  await key(page, 'g', 500);
  await leader(page, 'r');
  s = await snap(page);
  const outer = await sash(page, 0);
  if (outer) {
    await drag(page, outer, { x: R.right - 10, y: outer.y });
    s = await snap(page);
    check('a drag stops where a nested split\'s panes reach 200 px', s.panes.length === 3 && s.panes.every(p => p.w >= 198) && audit(s).length === 0, s.panes.map(p => [p.x, p.w]));
  } else check('the flipped three has a sash', false, {});
  // close
  await key(page, PRESET_KEY.classic, 400);
  await clickTitle(page, 0);
  await key(page, 't', 500);
  await leader(page, 'q');
  s = await snap(page);
  check('close: the views join the neighbour, which takes the space and shows the closed pane\'s view', s.panes.length === 1 && near(s.panes[0].w, W, 2) && near(s.panes[0].h, H, 2) && derived(s.panes[0]).views === sorted([...FIVE, 'insp']) && s.panes[0].roots.join() === 'table' && audit(s).length === 0, { p: s.panes.map(p => [p.tabs, p.roots]), audit: audit(s) });
  let t0 = await toasts(page);
  await leader(page, 'q');
  let s2 = await snap(page);
  check('closing the only pane is refused and says so', same(s, s2) && (await toasts(page)) > t0, { t0, t1: await toasts(page) });
  t0 = await toasts(page);
  await leader(page, 'v');
  s2 = await snap(page);
  check('moving the only pane\'s view does nothing and says so', same(s, s2) && (await toasts(page)) > t0, {});
  t0 = await toasts(page);
  await leader(page, 'd');
  s2 = await snap(page);
  check('collapsing the only pane is refused and says so', same(s, s2) && (await toasts(page)) > t0, {});
  // moving a pane's last view closes it
  await key(page, PRESET_KEY.three, 400);
  await clickTitle(page, 1);
  await leader(page, 'v');
  s = await snap(page);
  check('moving the last view closes the source pane', s.panes.length === 2 && near(s.panes[0].h, TITLE, 1) && near(s.panes[1].w, W, 2) && near(s.panes[1].h, H - TITLE, 2) && derived(s.panes[1]).views === sorted(['files', 'insp', 'list', 'log', 'table']) && s.panes[1].roots.join() === 'insp' && audit(s).length === 0, { p: s.panes.map(p => [p.w, p.h, p.tabs, p.roots]), audit: audit(s) });
  await reloadSame(page, ctx, 'the arrangement after the operations survives a reload within 1 px');
  await ctx.close();
});

await run('together', async () => {
  const { ctx, page } = await open(1600, 1000, { dpr: 2 });
  await key(page, PRESET_KEY.three, 400);
  await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
  const graphOK = async label => {
    const r = await page.evaluate(() => {
      const c = document.getElementById('graph');
      const pane = c.closest('.pane'), body = pane && pane.querySelector('.pbody');
      const cr = c.getBoundingClientRect(), br = body ? body.getBoundingClientRect() : null;
      const vis = c.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true }) && cr.width > 0 && cr.height > 0;
      const pts = [[0.5, 0.5], [0.2, 0.8], [0.8, 0.3]].map(([a, b]) => document.elementFromPoint(cr.left + a * cr.width, cr.top + b * cr.height) === c);
      let drawn = 0;
      try { const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; for (let i = 3; i < d.length; i += 4) if (d[i] > 0) drawn++; } catch (e) { drawn = -1; }
      return { vis, fills: !!br && Math.abs(cr.width - br.width) <= 1 && Math.abs(cr.height - br.height) <= 1, back: [c.width, c.height], css: [cr.width, cr.height], dpr: devicePixelRatio, pts, frac: drawn / (c.width * c.height) };
    });
    check(label, r.vis && r.fills && near(r.back[0], r.css[0] * r.dpr, 2) && near(r.back[1], r.css[1] * r.dpr, 2) && r.pts.every(Boolean) && r.frac > 0.01, r);
  };
  let s = await snap(page);
  check('the Graph pane is expanded above the Inspector and Files', near(s.panes[0].h, 0.4 * s.H, 2) && s.panes[0].roots.join() === 'graph' && s.panes[2].roots.join() === 'files' && audit(s).length === 0, { p: s.panes.map(p => [p.h, p.roots]), audit: audit(s) });
  await graphOK('the Graph is drawn on screen, filling its pane');
  const hash = () => page.evaluate(() => { const c = document.getElementById('graph'); const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let h = 0; for (let i = 0; i < d.length; i += 97) h = (h * 31 + d[i]) | 0; return h; });
  const h0 = await hash();
  const gr = await page.evaluate(() => { const r = document.getElementById('graph').getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
  await page.mouse.move(gr.x, gr.y); await page.mouse.wheel(0, -240); await page.waitForTimeout(500);
  check('the Graph responds to the wheel', (await hash()) !== h0, {});
  // open the deepest file's folders so the tree has room to scroll, then come back to the hub
  const deep = mdFiles.slice().sort((a, b) => b.id.split('/').length - a.id.split('/').length)[0].id;
  await hashTo(page, '#v=files&s=' + encodeURIComponent(deep));
  await page.evaluate(() => { document.getElementById('tree').scrollTop = 0; });
  const tr = await page.evaluate(() => { const t = document.getElementById('tree'); const r = t.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2, top: t.scrollTop, room: t.scrollHeight - t.clientHeight }; });
  await page.mouse.move(tr.x, tr.y); await page.mouse.wheel(0, 400); await page.waitForTimeout(400);
  const top1 = await page.evaluate(() => document.getElementById('tree').scrollTop);
  check('Files scrolls beside it', tr.room > 0 && top1 > tr.top, { tr, top1 });
  await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
  // keys follow focus: j walks Files when Files is focused, the Graph when the Graph is
  await clickTitle(page, 2);
  const on0 = await page.evaluate(() => document.querySelector('#tree .trow.on')?.dataset.id || null);
  await key(page, 'j', 400);
  let k = await page.evaluate(() => ({ on: document.querySelector('#tree .trow.on')?.dataset.id || null, hint: (() => { const h = document.getElementById('ghint'); return h && !h.hidden ? h.textContent : ''; })() }));
  check('with Files focused, j moves the Files row and starts no Graph walk', k.on !== on0 && !/^1\/\d+ · /.test(k.hint), { on0, k });
  await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
  await clickTitle(page, 0);
  await key(page, 'j', 400);
  k = await page.evaluate(() => { const h = document.getElementById('ghint'); return h && !h.hidden ? h.textContent : ''; });
  check('with the Graph focused, j starts the Graph walk', /^1\/\d+ · /.test(k), k);
  await key(page, 'Escape', 300);
  // a selection in Files: the Inspector shows it, Files marks it, the Graph redraws
  const row = await page.evaluate(() => { const t = document.getElementById('tree').getBoundingClientRect(); const r = [...document.querySelectorAll('#tree .trow:not(.thead)')].find(e => { const b = e.getBoundingClientRect(); return b.height > 0 && e.dataset.id && /\.md$/.test(e.dataset.id) && b.top > t.top + 30 && b.bottom < t.bottom; }); if (!r) return null; const b = r.getBoundingClientRect(); return { id: r.dataset.id, x: b.left + 40, y: b.top + b.height / 2 }; });
  if (row) {
    const h1 = await hash();
    await page.mouse.click(row.x, row.y); await page.waitForTimeout(900); await settle(page);
    const sel = await page.evaluate(() => ({ s: decodeURIComponent((location.hash.match(/[#&]s=([^&]*)/) || [])[1] || ''), head: document.getElementById('ihead').innerText, on: document.querySelector('#tree .trow.on')?.dataset.id || null }));
    check('selecting in Files: the Inspector shows the card, Files marks it, the Graph follows', sel.s === row.id && sel.on === row.id && sel.head.includes(stem(row.id)) && (await hash()) !== h1, { row, sel });
  } else check('Files has a file row to select', false, {});
  await graphOK('the Graph is still drawn after Files was used');
  // the canvas fits its pane after a split drag, a neighbour's collapse and expand, a preset and a
  // window resize
  const R = await rootBox(page);
  const top = await sash(page, 0);
  if (top) { await drag(page, top, { x: top.x, y: R.top + 0.6 * R.H }); await graphOK('after a split drag the canvas fills its pane'); }
  else check('three has a sash under the Graph', false, {});
  await key(page, PRESET_KEY.stacked, 500); await graphOK('after a preset change the canvas fills its pane');
  await clickTitle(page, 1); await leader(page, 'd');
  s = await snap(page);
  check('the Graph pane took its collapsed neighbour\'s height', near(s.panes[0].h, s.H - TITLE, 2), s.panes.map(p => p.h));
  await graphOK('after a neighbour collapses the canvas fills its pane');
  await leader(page, 'd'); await graphOK('after the neighbour expands the canvas fills its pane');
  await page.setViewportSize({ width: 1200, height: 900 }); await settle(page); await graphOK('after a window resize the canvas fills its pane');
  await ctx.close();
});

await run('fit', async () => {
  // every view at a 200 px pane: drag the split to the floor on each side
  let { ctx, page } = await open(1600);
  await hashTo(page, '#v=graph&s=' + encodeURIComponent(hub));
  const R = await rootBox(page);
  await drag(page, await sash(page), { x: R.right - 10, y: R.top + 300 });
  let s = await snap(page);
  check('the Inspector pane is at the 200 px floor', near(s.panes[1].w, 200, 2), s.panes.map(p => p.w));
  for (const k of ['o', 'c', 'h']) {
    await key(page, k, 700);
    const bad = await overflow(page, 1);
    check(`a 200 px Inspector on ${k} does not overflow`, bad.length === 0 && audit(await snap(page)).length === 0, bad);
  }
  await drag(page, await sash(page), { x: R.left + 10, y: R.top + 300 });
  s = await snap(page);
  check('the views pane is at the 200 px floor', near(s.panes[0].w, 200, 2), s.panes.map(p => p.w));
  for (const [k, v] of [['g', 'graph'], ['f', 'files'], ['l', 'log'], ['t', 'table']]) {
    await key(page, k, 900);
    const bad = await overflow(page, 0);
    s = await snap(page);
    check(`a 200 px pane showing ${v} does not overflow`, s.panes[0].roots.join() === v && bad.length === 0 && audit(s).length === 0, { roots: s.panes[0].roots, bad, audit: audit(s) });
  }
  await page.keyboard.press('/'); await page.keyboard.type('FIND inbox'); await page.keyboard.press('Enter'); await page.waitForTimeout(1500); await settle(page);
  s = await snap(page);
  let bad = await overflow(page, 0);
  check('a 200 px pane showing Results does not overflow', s.panes[0].roots.join() === 'list' && bad.length === 0 && audit(s).length === 0, { roots: s.panes[0].roots, bad, audit: audit(s) });
  // the Log chart is redrawn for its pane's width after a drag
  await key(page, 'l', 700);
  await drag(page, await sash(page), { x: R.left + 900, y: R.top + 300 });
  await page.waitForTimeout(500);
  const chart = await page.evaluate(() => { const c = document.getElementById('lchart'); const svg = c.querySelector('svg'); const vb = svg?.viewBox?.baseVal; return { cw: c.clientWidth, vb: vb ? vb.width : null }; });
  check('the Log chart is drawn for its pane\'s new width', chart.vb !== null && near(chart.vb, chart.cw, 3), chart);
  // a window too small for the floor: the narrow pane hides its body behind its title bar
  await drag(page, await sash(page), { x: R.left + 10, y: R.top + 300 });
  await page.setViewportSize({ width: 440, height: 1000 }); await settle(page);
  s = await snap(page);
  const narrow = s.panes.filter(p => p.w < 200), wide = s.panes.filter(p => p.w >= 200);
  check('under 200 px a pane shows its title bar and hides its body', narrow.length >= 1 && narrow.every(p => !p.bodyShown && p.roots.length === 0 && p.title && p.title.w > 0 && p.title.h > 0) && wide.every(p => p.bodyShown && p.roots.length === 1) && audit(s).length === 0, { p: s.panes.map(p => [p.w, p.bodyShown, p.roots]), audit: audit(s) });
  const over = await Promise.all(s.panes.map((_, i) => overflow(page, i)));
  check('a 440 px window overflows in no pane', over.every(b => b.length === 0), over);
  await page.setViewportSize({ width: 1600, height: 1000 }); await settle(page);
  s = await snap(page);
  check('grown back over 200 px the pane shows its view again', s.panes.every(p => p.bodyShown && p.roots.length === 1) && audit(s).length === 0, { p: s.panes.map(p => [p.w, p.roots]), audit: audit(s) });
  await ctx.close();
  // the legend hides by the Graph pane's width, not the window's
  const legend = p => p.evaluate(() => { const l = document.getElementById('glegend'); return !!l && l.checkVisibility({ checkVisibilityCSS: true }) && l.getBoundingClientRect().width > 0; });
  ({ ctx, page } = await open(1600));
  check('legend shown in an 864 px Graph pane', await legend(page), {});
  const R2 = await rootBox(page);
  await drag(page, await sash(page), { x: R2.left + 470, y: R2.top + 300 });
  check('legend hidden in a 470 px Graph pane of a 1600 px window', !(await legend(page)), {});
  await drag(page, await sash(page), { x: R2.left + 490, y: R2.top + 300 });
  check('legend shown in a 490 px Graph pane', await legend(page), {});
  await ctx.close();
  ({ ctx, page } = await open(899));
  check('legend shown in an 899 px window whose Graph pane is 899 px wide', await legend(page), {});
  const chrome = await page.evaluate(() => [...document.querySelectorAll('.word, .kbname, .crumbs')].map(e => e.checkVisibility({ checkVisibilityCSS: true }) && e.getBoundingClientRect().width > 0));
  check('brand word, KB name and crumbs hide in an 899 px window', chrome.length > 0 && chrome.every(v => !v), chrome);
  await ctx.close();
});

group = 'all'; check('no page errors', errors.length === 0, errors.slice(0, 5));
await browser.close();
console.log(fails ? `rects: ${fails} failed` : 'rects: all passed');
process.exit(fails ? 1 : 0);
