// Supersedes spec/drag-to-dock/verify/dock.mjs, which stays frozen as the record. There the state
// group scrolled whatever the root folder and README.md's History held, and on a KB where neither
// overflows its scroller no page could pass; and no keyview route had a key view that arrives off
// screen never drawn. Changed: state picks its scroll content by measurement and grades that
// precondition under its own label; keyview gains that route. Everything else is as frozen.
//
// Drag a view's tab to dock it in another pane, graded on what renders, against a running okt:
//   SWEEP     every shipped preset at 1600 x 1000 and 900 x 1000; every pane in it, collapsed ones
//             included; each of its five zones and its title bar; dragged there from every pane (its shown tab, and
//             one tab it doesn't show), the pane itself and the Inspector's included. For each
//             drop: the overlay is shown exactly where the drop is valid and shows the rect the
//             view lands in; the result is the tree dock-oracle.mjs gives, drawn and saved; a refused
//             zone changes nothing; nothing changes while the drag is under way; focus, the hash
//             and the history after the drop; every shown view filling its pane; a reload
//   CANCEL    Esc, another key, a release outside every pane, on its own pane's centre and title
//             bar, the window losing focus, and the pointer capture lost, from every pane of every
//             preset: no overlay where it is released, and the arrangement, tabs, saved settings,
//             hash and history as they were
//   CLICK     a press and release that moves less than the drag threshold acts as a click on that
//             tab; a drag that comes back to its tab, or one moved 5 px, acts as nothing
//   SOURCES   every view dragged from a collapsed side-by-side strip, and onto it
//   REFUSE    panes too narrow or too short to split, and a drop that would leave two collapsed
//             panes in one split: no overlay, and a release changes nothing
//   KEYVIEW   the Inspector dropped over the Log or Files, the Log dropped under the 200 px floor
//             (once after it was drawn, once never drawn before), the Inspector dropped while the
//             Log's pane is collapsed: the keys step as on screen
//   STATE     a moved view keeps its scroll, the selection and the Inspector's tab, from content
//             measured to overflow its scroller (a precondition graded on its own); back and
//             forward after a drop never undo it
// Every rect comes from getBoundingClientRect, visibility from checkVisibility, and every drag is
// real pointer input (Playwright mouse down, stepped moves, up). The data-* attributes the client
// writes only find elements, and each is cross-checked against geometry. Presets are applied by
// their keys. The overlay is the element #dock.
//   node tests/ui/dock.mjs [group …]          or DOCK_GROUPS="group …" (a prefix selects every
//                                               group it begins: "sweep" runs all eight sweeps)
// Prints "ok   <group>: <label>" or "FAIL <group>: <label>: <detail>"; exits 1 on any FAIL or a
// group that ran no check, 2 on an unknown group. Footprint: one Chromium (about 6 processes, under
// 1 GB), one context at a time.
import { drop, aim, aimPoints, layout, leaves, picture, PRESETS, ZONES, AIMS, zoneOf, TITLE } from './dock-oracle.mjs';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');

const OKT = process.env.OKT || 'http://127.0.0.1:6581/';
const KBNAME = (process.env.OKT_KB || '/kb').split('/').filter(Boolean).pop() || 'kb';
const SKEY = 'okt:' + KBNAME;
const PRESET_KEY = { classic: ';', swapped: "'", stacked: '\\', three: '`' };
const ROOT = { graph: '#v-graph', files: '#v-files', log: '#v-log', table: '#v-table', list: '#v-list', insp: '#insp' };
const MAIN = ['graph', 'files', 'log', 'table', 'list'];
const WIDTHS = [1600, 900];
const ALL = [
  ...WIDTHS.flatMap(w => Object.keys(PRESETS).map(p => `sweep-${p}-${w}`)),
  ...WIDTHS.map(w => `cancel-${w}`),
  'click', 'sources', 'refuse', 'keyview', 'state',
];
const asked = (process.argv.slice(2).length ? process.argv.slice(2) : (process.env.DOCK_GROUPS || '').split(/[\s,]+/)).filter(Boolean);
const want = asked.length ? ALL.filter(g => asked.some(a => g === a || g.startsWith(a))) : ALL;
for (const a of asked) if (!ALL.some(g => g === a || g.startsWith(a))) { console.log(`FAIL unknown group ${a}`); process.exit(2); }

let fails = 0;
const ran = {};
let group = '';
const check = (label, ok, detail) => {
  ran[group] = (ran[group] || 0) + 1;
  if (!ok) fails++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${group}: ${label}${ok ? '' : ': ' + JSON.stringify(detail).slice(0, 1500)}`);
};
const near = (a, b, tol) => Math.abs(a - b) <= tol;
const nearRect = (a, b, tol) => !!a && !!b && a.every((v, i) => near(v, b[i], tol));
const fix = r => r && r.map(v => +(+v).toFixed(1));
const browser = await chromium.launch();
const errors = [];

// ---- the page ---------------------------------------------------------------------------------

async function open(w, { seed = null } = {}) {
  const ctx = await browser.newContext({ viewport: { width: w, height: 1000 } });
  const page = await ctx.newPage();
  page.on('pageerror', e => errors.push(e.message));
  if (seed) {
    await page.goto(OKT + 'api/index');
    await page.evaluate(([k, v]) => localStorage.setItem(k, v), [SKEY, JSON.stringify({ arrangement: { v: 1, root: seed } })]);
  }
  await page.goto(OKT, { waitUntil: 'load' });
  await ready(page);
  const kb = await page.evaluate(() => JSON.parse(document.getElementById('boot').textContent).kb || 'kb');
  if (kb !== KBNAME) { console.log(`FAIL settings key: boot kb ${kb} != ${KBNAME}`); fails++; }
  return { ctx, page };
}
async function ready(page) {
  await page.waitForFunction(() => document.querySelectorAll('#panes .pane').length > 0, null, { timeout: 15000 });
  await page.waitForFunction(() => (document.getElementById('status')?.innerText || '').trim().length > 0, null, { timeout: 8000 }).catch(() => {});
  await settle(page);
}
// pane rects unchanged across two animation frames
async function settle(page) {
  await page.evaluate(() => new Promise(res => {
    const sig = () => [...document.querySelectorAll('#panes .pane')].map(p => { const r = p.getBoundingClientRect(); return [r.x, r.y, r.width, r.height].map(v => v.toFixed(2)).join(','); }).join('|');
    let last = sig(), same = 0, n = 0;
    const tick = () => { const s = sig(); same = s === last ? same + 1 : 0; last = s; if (same >= 2 || ++n > 120) res(); else requestAnimationFrame(tick); };
    requestAnimationFrame(tick);
  }));
  await page.waitForTimeout(100);
}
async function key(page, k, ms = 200) {
  await page.evaluate(() => { const a = document.activeElement; if (a && a !== document.body && a.matches('input, textarea')) a.blur(); });
  await page.keyboard.press(k);
  await page.waitForTimeout(ms);
  await settle(page);
}
const raw = page => page.evaluate(k => localStorage.getItem(k) || '', SKEY);
const stored = async page => { try { return JSON.parse(await raw(page)).arrangement?.root ?? null; } catch { return null; } };
const sameTree = (a, b) => JSON.stringify(a, (k, x) => typeof x === 'number' ? Math.round(x * 1e6) / 1e6 : x) === JSON.stringify(b, (k, x) => typeof x === 'number' ? Math.round(x * 1e6) / 1e6 : x);
// the stored arrangement reaches `tree` (the save is debounced), within `ms`
async function storedIs(page, tree, ms = 2500) {
  return page.waitForFunction(([k, t]) => {
    try {
      const r = (JSON.parse(localStorage.getItem(k) || '{}').arrangement || {}).root;
      const n = x => JSON.stringify(x, (q, v) => typeof v === 'number' ? Math.round(v * 1e6) / 1e6 : v);
      return n(r) === n(t);
    } catch { return false; }
  }, [SKEY, tree], { timeout: ms }).then(() => true, () => false);
}
const hashOf = page => page.evaluate(() => location.hash);
const hlen = page => page.evaluate(() => history.length);
const param = (h, k) => new URLSearchParams((h || '').replace(/^#/, '')).get(k);

// The panes in document order (the tree's reading order). For each: its rect relative to #panes,
// whether it is drawn collapsed (its body hidden and it is a 24 px bar or strip), the views its
// title bar shows as tabs and the one drawn as current, the view root visible in its body, whether
// its title bar is drawn focused, and how its shown view fits it.
async function snap(page) {
  return page.evaluate(([ROOT, TITLE]) => {
    const vis = e => !!e && e.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true }) && e.getBoundingClientRect().width > 0 && e.getBoundingClientRect().height > 0;
    const hits = e => { const r = e.getBoundingClientRect(); const at = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2); return !!at && e.contains(at); };
    const R = document.getElementById('panes').getBoundingClientRect();
    const rel = r => [r.left - R.left, r.top - R.top, r.width, r.height];
    const panes = [...document.querySelectorAll('#panes .pane')].map(p => {
      const r = p.getBoundingClientRect();
      const t = p.querySelector(':scope > .ptitle');
      const b = p.querySelector(':scope > .pbody');
      const tabs = t ? [...t.querySelectorAll('button[data-view]')].filter(vis) : [];
      const on = tabs.filter(x => { const c = getComputedStyle(x).borderBottomColor; return c !== 'rgba(0, 0, 0, 0)' && c !== 'transparent'; }).map(x => x.dataset.view);
      const roots = Object.entries(ROOT).filter(([, s]) => { const e = document.querySelector(s); return vis(e) && b && b.contains(e) && hits(e); }).map(([v]) => v);
      const bar = Math.min(r.width, r.height);
      const collapsed = !vis(b) && Math.abs(bar - TITLE) <= 1;
      const fit = roots.map(v => {
        const e = document.querySelector(ROOT[v]);
        const er = e.getBoundingClientRect(), br = b.getBoundingClientRect();
        const c = v === 'graph' ? document.getElementById('graph').getBoundingClientRect() : null;
        const full = (a, z) => Math.abs(a.left - z.left) <= 2 && Math.abs(a.top - z.top) <= 2 && Math.abs(a.width - z.width) <= 2 && Math.abs(a.height - z.height) <= 2;
        // drawn, not just placed: rows in the list views, text in the Inspector, a sized backing store
        const content = {
          graph: () => { const g = document.getElementById('graph'); return g.width > 0 && g.height > 0; },
          files: () => document.querySelectorAll('#tree .trow:not(.thead)').length > 0,
          log: () => document.querySelectorAll('#ltable > *').length > 0,
          table: () => document.querySelectorAll('#tgrid tr').length > 0,
          list: () => document.getElementById('list').children.length > 0,
          insp: () => (document.getElementById('ibody').innerText || '').trim().length > 0,
        }[v]();
        return { v, fills: full(er, br) && (!c || full(c, br)), sideways: e.scrollWidth > e.clientWidth + 1, content };
      });
      return {
        rect: rel(r), collapsed, tabs: tabs.map(x => x.dataset.view), on,
        roots, held: Object.entries(ROOT).filter(([, s]) => p.contains(document.querySelector(s))).map(([v]) => v),
        marked: p.dataset.focused === '1' && !!t && getComputedStyle(t).boxShadow !== 'none',
        attr: { focused: p.dataset.focused, collapsed: p.dataset.collapsed, shown: p.dataset.shown },
        fit, wide: r.width >= 200,
      };
    });
    const dock = document.getElementById('dock');
    const shown = !!dock && vis(dock);
    return { W: R.width, H: R.height, origin: [R.left, R.top], panes, dock: shown ? rel(dock.getBoundingClientRect()) : null, docks: document.querySelectorAll('#dock').length, sel: (getSelection() || '').toString() };
  }, [ROOT, TITLE]);
}
const focusOf = s => { const m = s.panes.map((p, i) => p.marked ? i : -1).filter(i => i >= 0); return m.length === 1 ? m[0] : -1; };

// The drawn panes against a tree: same count, each pane's views as tabs (and held), the shown one
// drawn current and visible unless collapsed or under the 200 px floor, collapsed alike, rect
// within `tol` px. Returns the problems.
function drawn(s, tree, tol = 2) {
  const want = picture(tree, s.W, s.H);
  const bad = [];
  if (s.panes.length !== want.length) bad.push(`${s.panes.length} panes drawn, want ${want.length}`);
  want.forEach(([views, shown, collapsed, ...rect], i) => {
    const p = s.panes[i];
    if (!p) return;
    const so = a => a.slice().sort().join();
    if (so(p.tabs) !== so(views) || so(p.held) !== so(views)) bad.push(`pane ${i} tabs ${p.tabs} holds ${p.held}, want ${views}`);
    if (p.on.join() !== shown) bad.push(`pane ${i} marks tab ${p.on}, want ${shown}`);
    if (p.collapsed !== collapsed) bad.push(`pane ${i} collapsed ${p.collapsed}, want ${collapsed}`);
    if (!collapsed && p.wide && p.roots.join() !== shown) bad.push(`pane ${i} shows ${p.roots}, want ${shown}`);
    if ((collapsed || !p.wide) && p.roots.length) bad.push(`pane ${i} hidden body shows ${p.roots}`);
    if (!nearRect(p.rect, rect, tol)) bad.push(`pane ${i} at ${fix(p.rect)}, want ${fix(rect)}`);
  });
  return bad;
}
// The page's state a cancelled or refused drag must leave as it was: the drawn panes (rects within
// 1 px, tabs, shown, collapsed), the stored settings string, the hash, the history length.
async function state(page) {
  return { snap: await snap(page), raw: await raw(page), hash: await hashOf(page), hlen: await hlen(page) };
}
function unchanged(a, b) {
  const bad = [];
  if (a.raw !== b.raw) bad.push('saved settings changed');
  if (a.hash !== b.hash) bad.push(`hash ${a.hash} -> ${b.hash}`);
  if (a.hlen !== b.hlen) bad.push(`history ${a.hlen} -> ${b.hlen}`);
  if (a.snap.panes.length !== b.snap.panes.length) bad.push(`${a.snap.panes.length} -> ${b.snap.panes.length} panes`);
  a.snap.panes.forEach((p, i) => {
    const q = b.snap.panes[i];
    if (!q) return;
    if (!nearRect(p.rect, q.rect, 1) || p.tabs.join() !== q.tabs.join() || p.on.join() !== q.on.join() || p.collapsed !== q.collapsed || p.roots.join() !== q.roots.join()) bad.push(`pane ${i}: ${fix(p.rect)} ${p.tabs}/${p.on} -> ${fix(q.rect)} ${q.tabs}/${q.on}`);
  });
  return bad;
}

// The centre of view v's tab in pane i (document order), in client px, and whether the tab is hit there.
async function tabAt(page, i, v) {
  return page.evaluate(([i, v]) => {
    const p = document.querySelectorAll('#panes .pane')[i];
    const b = p && p.querySelector(`:scope > .ptitle > [data-view="${v}"]`);
    if (!b) return null;
    const r = b.getBoundingClientRect();
    const x = r.left + r.width / 2, y = r.top + r.height / 2;
    const at = document.elementFromPoint(x, y);
    return { x, y, hit: !!at && b.contains(at) };
  }, [i, v]);
}
// a client point in pane i's title bar off every tab (its empty end, or a strip's bottom)
async function titleEnd(page, i) {
  return page.evaluate(i => {
    const p = document.querySelectorAll('#panes .pane')[i];
    const t = p.querySelector(':scope > .ptitle'); const r = t.getBoundingClientRect();
    const tabs = [...t.querySelectorAll('button[data-view]')].map(b => b.getBoundingClientRect());
    const strip = r.height > r.width;
    for (let k = 2; k < 60; k += 2) {
      const x = strip ? r.left + r.width / 2 : r.right - k, y = strip ? r.bottom - k : r.top + r.height / 2;
      if (!tabs.some(b => x >= b.left && x <= b.right && y >= b.top && y <= b.bottom)) return { x, y };
    }
    return null;
  }, i);
}
// Press view v's tab in pane i and move past the threshold (two stepped moves, 8 px along the
// title bar, inward), so the drag has begun. Returns the press point.
async function begin(page, i, v) {
  const t = await tabAt(page, i, v);
  if (!t || !t.hit) return null;
  await page.mouse.move(t.x, t.y);
  await page.mouse.down();
  const s = await snap(page);
  const strip = s.panes[i].collapsed && s.panes[i].rect[3] > s.panes[i].rect[2];
  await page.mouse.move(t.x + (strip ? 0 : 8), t.y + (strip ? 8 : 0), { steps: 2 });
  return t;
}
const to = async (page, pt, steps = 8) => page.mouse.move(pt[0], pt[1], { steps });
const client = (s, [x, y]) => [s.origin[0] + x, s.origin[1] + y];
async function reset(page, preset) {
  await key(page, PRESET_KEY[preset]);
  return storedIs(page, PRESETS[preset]);
}
async function reseed(page, tree) {
  await page.evaluate(([k, t]) => { const o = JSON.parse(localStorage.getItem(k) || '{}'); o.arrangement = { v: 1, root: t }; localStorage.setItem(k, JSON.stringify(o)); }, [SKEY, tree]);
  await page.reload({ waitUntil: 'load' });
  await ready(page);
  return storedIs(page, tree);
}

// ---- the one drop checker ----------------------------------------------------------------------
// Every drop in every group runs through here, and each property is tallied per group: a property
// passes only when it held for every case of the group, and the tally of valid and refused cases
// is printed with it, so an empty domain can't pass.
const PROPS = {
  prediction: 'the overlay shows exactly where each drop lands, and only where a drop is valid',
  refusal: 'a refused zone shows no overlay and its release changes nothing',
  result: 'each drop gives the defined tree, drawn and saved',
  midway: 'nothing changes while a drag is under way',
  focus: 'after a drop the dropped view\'s pane is focused, the hash replaced and no history pushed',
  fit: 'after a drop every shown view is drawn and fills its pane without sideways scroll',
  click: 'the first click after a drop acts as a click',
  persist: 'each drop is restored by a reload within 1 px',
};
let tally = null;
const startTally = () => { tally = Object.fromEntries(Object.keys(PROPS).map(k => [k, { n: 0, bad: [] }])); tally.valid = 0; tally.refused = 0; };
const note = (prop, ok, detail) => { tally[prop].n++; if (!ok) tally[prop].bad.push(detail); };
function endTally({ minValid = 1, minRefused = 0 } = {}) {
  for (const [k, label] of Object.entries(PROPS)) {
    const t = tally[k];
    const enough = k === 'refusal' ? tally.refused >= minRefused : t.n > 0 && tally.valid >= minValid;
    if (k === 'refusal' && minRefused === 0 && t.n === 0) continue;
    check(`${label} (${tally.valid} valid, ${tally.refused} refused)`, enough && t.bad.length === 0, { cases: t.n, bad: t.bad.slice(0, 4), more: Math.max(0, t.bad.length - 4) });
  }
}

// One drop: view v's tab in pane i onto aim point z (a zone, or 'title') of pane t. `tree` is the
// arrangement drawn now.
async function dropCase(page, tree, i, v, t, z, label) {
  const id = `${label} ${v}@${i} -> ${z}@${t}`;
  const s0 = await snap(page);
  const start = drawn(s0, tree, 1);
  if (start.length) { note('result', false, { id, start }); return; }
  const at = aimPoints(s0.panes[t].rect, s0.panes[t].collapsed)[z];
  const aimed = aim(tree, s0.W, s0.H, ...at);
  if (!aimed || aimed.pane !== t || aimed.zone !== zoneOf(z, s0.panes[t].collapsed)) { note('prediction', false, { id, aimed }); return; }
  // what the zone under the pointer does (a collapsed pane's and a title bar's points are its centre)
  const want = drop(tree, v, t, aimed.zone, s0.W, s0.H);
  const press = await begin(page, i, v);
  if (!press) { note('result', false, { id, why: 'tab not hit' }); return; }
  const b0 = await state(page); // the drag has begun
  await to(page, client(s0, at));
  const mid = await state(page);
  note('midway', unchanged(b0, mid).length === 0 && mid.snap.sel === '', { id, diff: unchanged(b0, mid), sel: mid.snap.sel });
  const overlay = mid.snap.dock;
  if (!want) {
    tally.refused++;
    await page.mouse.up();
    await page.waitForTimeout(450); // past the 200 ms save delay
    await settle(page);
    const after = await state(page);
    const diff = unchanged(b0, after);
    note('refusal', overlay === null && diff.length === 0 && after.snap.dock === null, { id, overlay: fix(overlay), diff });
    return;
  }
  tally.valid++;
  note('prediction', overlay !== null && mid.snap.docks === 1 && nearRect(overlay, want.rect, 2), { id, overlay: fix(overlay), want: fix(want.rect), docks: mid.snap.docks });
  await page.mouse.up();
  const saved = await storedIs(page, want.root);
  await settle(page);
  const s1 = await snap(page);
  const hash = await hashOf(page), n = await hlen(page);
  // the overlay's last rect is where the view now is (read from the page, not the oracle)
  const lands = s1.panes.findIndex(p => p.tabs.includes(v));
  note('prediction', s1.dock === null && lands >= 0 && overlay !== null && nearRect(s1.panes[lands].rect, overlay, 2), { id, lands, rect: fix(s1.panes[lands]?.rect), overlay: fix(overlay), after: fix(s1.dock) });
  const bad = drawn(s1, want.root, 2);
  note('result', saved && bad.length === 0, { id, saved, stored: saved ? undefined : await stored(page), bad });
  const fv = focusOf(s1);
  const hv = param(hash, 'v'), hv0 = param(b0.hash, 'v');
  note('focus', fv === want.pane && n === b0.hlen && (MAIN.includes(v) ? hv === v : hv === hv0), { id, focus: fv, want: want.pane, hlen: [b0.hlen, n], hash: [b0.hash, hash] });
  const misfit = s1.panes.flatMap((p, k) => p.fit.filter(f => !f.fills || f.sideways || !f.content).map(f => ({ pane: k, ...f })));
  note('fit', misfit.length === 0, { id, misfit });
  // the first click after the drop: a tab an expanded pane doesn't show, which it then shows
  let after = want.root;
  const k = s1.panes.findIndex(p => !p.collapsed && p.tabs.some(x => x !== p.on.join()));
  if (bad.length) note('click', false, { id, why: 'the drop did not give the defined tree' });
  else if (k >= 0) {
    const x = s1.panes[k].tabs.find(y => y !== s1.panes[k].on.join());
    const tb = await tabAt(page, k, x);
    await page.mouse.click(tb.x, tb.y);
    after = JSON.parse(JSON.stringify(want.root));
    leaves(after)[k].node.shown = x;
    const shown = await storedIs(page, after);
    await settle(page);
    const s3 = await snap(page);
    note('click', shown && s3.panes[k].on.join() === x, { id, pane: k, tab: x, on: s3.panes[k].on });
  }
  await page.reload({ waitUntil: 'load' });
  await ready(page);
  const s2 = await snap(page);
  const back = drawn(s2, after, 1);
  const moved = s1.panes.map((p, j) => nearRect(p.rect, s2.panes[j]?.rect, 1)).every(Boolean);
  note('persist', back.length === 0 && moved && sameTree(await stored(page), after), { id, back, moved });
}

// the sources a preset offers: each pane's shown tab, and its last tab it doesn't show
function sources(tree) {
  return leaves(tree).flatMap((l, i) => {
    const out = [[i, l.node.shown]];
    const other = l.node.views.filter(x => x !== l.node.shown).pop();
    if (other) out.push([i, other]);
    return out;
  });
}

// ---- groups -------------------------------------------------------------------------------------
async function run(name, body) {
  if (!want.includes(name)) return;
  group = name;
  const t0 = Date.now();
  try { await body(); } catch (e) { check('ran to the end', false, String(e && e.stack || e).slice(0, 600)); }
  if (!ran[name]) { console.log(`FAIL ${name}: no check ran`); fails++; }
  console.log(`time ${name}: ${((Date.now() - t0) / 1000).toFixed(0)} s`);
}

for (const w of WIDTHS) for (const preset of Object.keys(PRESETS)) {
  await run(`sweep-${preset}-${w}`, async () => {
    startTally();
    const { ctx, page } = await open(w);
    const tree = PRESETS[preset];
    const n = leaves(tree).length;
    let cases = 0, resets = true;
    for (const [i, v] of sources(tree)) for (let t = 0; t < n; t++) for (const z of AIMS) {
      resets = (await reset(page, preset)) && resets;
      await dropCase(page, tree, i, v, t, z, `${preset} ${w}`);
      cases++;
    }
    check(`each case starts from the preset its key applies (${cases} cases)`, resets && cases === sources(tree).length * n * AIMS.length, { cases });
    endTally({ minValid: 10, minRefused: 5 });
    await ctx.close();
  });
}

// The cancels, from every pane's tabs in every preset: each while an overlay shows over a valid
// zone of another pane (or of its own, when no other pane offers one).
for (const w of WIDTHS) {
  await run(`cancel-${w}`, async () => {
    const kinds = ['esc', 'key', 'outside', 'own-centre', 'own-title', 'blur', 'lost'];
    const out = Object.fromEntries(kinds.map(k => [k, { n: 0, bad: [] }]));
    for (const preset of Object.keys(PRESETS)) {
      const { ctx, page } = await open(w);
      const tree = PRESETS[preset];
      // a commit open in the Inspector: Esc would close it, were it handled as usual
      await reset(page, preset);
      await key(page, 'l', 1200); await key(page, 'j', 900);
      const commit = param(await hashOf(page), 'c');
      for (const [i, v] of sources(tree)) for (const kind of kinds) {
        await reset(page, preset);
        const s0 = await snap(page);
        const valid = leaves(tree).map((_, t) => ZONES.map(z => [t, z])).flat().find(([t, z]) => t !== i && drop(tree, v, t, z, s0.W, s0.H))
          || ZONES.map(z => [i, z]).find(([t, z]) => drop(tree, v, t, z, s0.W, s0.H));
        const id = `${preset} ${v}@${i} ${kind}`;
        if (!valid) { out[kind].bad.push({ id, why: 'no valid zone to hover' }); continue; }
        const shownBefore = s0.panes[i].on.join();
        if (!(await begin(page, i, v))) { out[kind].bad.push({ id, why: 'tab not hit' }); continue; }
        const b0 = await state(page);
        await to(page, client(s0, aimPoints(s0.panes[valid[0]].rect, s0.panes[valid[0]].collapsed)[valid[1]]));
        const over = (await snap(page)).dock !== null;
        let extra = true;
        if (kind === 'esc') { await page.keyboard.press('Escape'); }
        else if (kind === 'key') { await page.keyboard.press('?'); extra = await page.evaluate(() => document.getElementById('help').checkVisibility()); }
        else if (kind === 'blur') { await page.evaluate(() => window.dispatchEvent(new Event('blur'))); }
        else if (kind === 'lost') {
          // the pointer capture the drag holds on its tab is lost (as when the element goes away)
          extra = 1 === await page.evaluate(() => { let n = 0; for (const b of document.querySelectorAll('#panes .ptab')) if (b.hasPointerCapture(1)) { b.releasePointerCapture(1); n++; } return n; });
          await page.waitForTimeout(50);
        }
        const gone = (await snap(page)).dock === null || ['outside', 'own-centre', 'own-title'].includes(kind);
        // the release: outside every pane (over the header), on its own pane, or where the overlay was
        let rel;
        if (kind === 'outside') rel = [s0.origin[0] + s0.W / 2, s0.origin[1] - 12];
        else if (kind === 'own-centre') rel = client(s0, aimPoints(s0.panes[i].rect, s0.panes[i].collapsed).centre);
        else if (kind === 'own-title') { const e = await titleEnd(page, i); rel = e ? [e.x, e.y] : null; }
        else rel = client(s0, aimPoints(s0.panes[valid[0]].rect, s0.panes[valid[0]].collapsed)[valid[1]]);
        if (!rel) { out[kind].bad.push({ id, why: 'no title end' }); await page.mouse.up(); continue; }
        await to(page, rel, 4);
        const before = (await snap(page)).dock;
        await page.mouse.up();
        await page.waitForTimeout(450);
        await settle(page);
        if (kind === 'key') {
          // the help it opened covers the panes: close it (Esc on the help only closes it)
          extra = extra && await page.evaluate(() => document.getElementById('help').checkVisibility());
          await key(page, 'Escape');
        }
        const after = await state(page);
        const diff = unchanged(b0, after);
        const s1 = after.snap;
        const bad = [];
        if (!over) bad.push('no overlay over a valid zone before the cancel');
        if (!gone) bad.push('overlay still shown after the cancel');
        // no overlay where the release happens: over no pane, its own pane, or after a cancel
        if (before !== null) bad.push(`overlay shown at the release point (${kind})`);
        if (s1.dock !== null) bad.push('overlay after the release');
        if (diff.length) bad.push(...diff);
        if (focusOf(s1) !== i) bad.push(`focus on pane ${focusOf(s1)}, not the pressed pane ${i}`);
        if (s1.panes[i].on.join() !== shownBefore) bad.push(`pressed pane shows ${s1.panes[i].on}, showed ${shownBefore}`);
        if (s1.sel !== '') bad.push('text selected');
        if (kind === 'esc' && !commit) bad.push('no commit was open, so Esc had nothing else to do');
        if (kind === 'esc' && param(after.hash, 'c') !== commit) bad.push('Esc did something else: the open commit closed');
        if (kind === 'lost' && !extra) bad.push('the drag held no pointer capture on its tab');
        if (kind === 'key' && !extra) bad.push('the key was not handled as usual (no help shown)');
        out[kind].n++;
        if (bad.length) out[kind].bad.push({ id, bad });
      }
      await ctx.close();
    }
    const label = {
      esc: 'Esc cancels a drag, leaves everything as it was and does nothing else',
      key: 'another key cancels a drag, leaves the arrangement as it was and is handled as usual',
      outside: 'a release outside every pane changes nothing',
      'own-centre': 'a release on the dragged view\'s own pane\'s centre changes nothing',
      'own-title': 'a release on the dragged view\'s own title bar changes nothing',
      blur: 'the window losing focus cancels a drag, and the release then changes nothing',
      lost: 'losing the pointer capture cancels a drag, and the release then changes nothing',
    };
    for (const k of kinds) check(`${label[k]} (${out[k].n} drags)`, out[k].n >= 13 && out[k].bad.length === 0, out[k].bad.slice(0, 4));
  });
}

// A press and release under the threshold is a click on that tab: the same page state as a real
// click from the same start. A drag that returns to its tab is nothing.
await run('click', async () => {
  const res = { click: { n: 0, bad: [] }, back: { n: 0, bad: [] }, short: { n: 0, bad: [] } };
  for (const w of WIDTHS) for (const preset of Object.keys(PRESETS)) {
    const tree = PRESETS[preset];
    for (const [i, l] of leaves(tree).entries()) for (const v of l.node.views) {
      const id = `${preset} ${w} ${v}@${i}`;
      // a fresh page per tab (history.length stays far under Chromium's cap of 50)
      const { ctx, page } = await open(w);
      // a first click, so what the view loads on its first showing (the Table's folder) is loaded
      // before both runs below, which then start from the same state
      await reset(page, preset);
      let t = await tabAt(page, i, v);
      await page.mouse.click(t.x, t.y); await page.waitForTimeout(900); await settle(page);
      // a real click
      await reset(page, preset); await key(page, 'l', 300);
      t = await tabAt(page, i, v);
      const h0 = await hlen(page);
      await page.mouse.click(t.x, t.y);
      await page.waitForTimeout(450); await settle(page);
      const A = await state(page);
      // the same tab pressed, moved 3 px in three steps, released
      await reset(page, preset); await key(page, 'l', 300);
      t = await tabAt(page, i, v);
      const h1 = await hlen(page);
      await page.mouse.move(t.x, t.y); await page.mouse.down();
      await page.mouse.move(t.x + 3, t.y, { steps: 3 });
      const during = (await snap(page)).dock;
      await page.mouse.up();
      await page.waitForTimeout(450); await settle(page);
      const B = await state(page);
      const diff = unchanged({ ...A, hlen: A.hlen - h0 }, { ...B, hlen: B.hlen - h1 });
      if (focusOf(A.snap) !== focusOf(B.snap)) diff.push(`focus ${focusOf(A.snap)} vs ${focusOf(B.snap)}`);
      if (during !== null) diff.push('an overlay under the threshold');
      res.click.n++;
      if (diff.length) res.click.bad.push({ id, diff });
      // a drag out 10 px and back onto its own tab
      await reset(page, preset); await key(page, 'l', 300);
      let s0 = await snap(page);
      if (!(await begin(page, i, v))) { res.back.bad.push({ id, why: 'tab not hit' }); await ctx.close(); continue; }
      const b0 = await state(page);
      t = await tabAt(page, i, v);
      await page.mouse.move(t.x + 10, t.y + 6, { steps: 2 });
      await page.mouse.move(t.x, t.y, { steps: 3 });
      await page.mouse.up();
      await page.waitForTimeout(450); await settle(page);
      const C = await state(page);
      const d2 = unchanged(b0, C);
      if (C.snap.panes[i]?.on.join() !== s0.panes[i].on.join()) d2.push(`the pressed tab ${v} was shown`);
      res.back.n++;
      if (d2.length) res.back.bad.push({ id, d2 });
      // pressed, moved 5 px along its title bar (or down its strip) and released: past the
      // threshold, so a drag on its own title bar, which is nothing
      await reset(page, preset); await key(page, 'l', 300);
      s0 = await snap(page);
      t = await tabAt(page, i, v);
      const strip = s0.panes[i].collapsed && s0.panes[i].rect[3] > s0.panes[i].rect[2];
      await page.mouse.move(t.x, t.y); await page.mouse.down();
      const pressed = await state(page);
      await page.mouse.move(t.x + (strip ? 0 : 5), t.y + (strip ? 5 : 0), { steps: 2 });
      await page.mouse.up();
      await page.waitForTimeout(450); await settle(page);
      const D = await state(page);
      const d3 = unchanged(pressed, D);
      if (D.snap.panes[i]?.on.join() !== s0.panes[i].on.join()) d3.push(`the pressed tab ${v} was shown`);
      res.short.n++;
      if (d3.length) res.short.bad.push({ id, d3 });
      await ctx.close();
    }
  }
  check(`a press and release moving under the threshold acts as a click on that tab (${res.click.n} tabs)`, res.click.n >= 48 && res.click.bad.length === 0, res.click.bad.slice(0, 4));
  check(`a drag back onto its own tab acts as nothing (${res.back.n} tabs)`, res.back.n >= 48 && res.back.bad.length === 0, res.back.bad.slice(0, 4));
  check(`a press moved 5 px and released acts as nothing, not a click (${res.short.n} tabs)`, res.short.n >= 48 && res.short.bad.length === 0, res.short.bad.slice(0, 4));
});

// Every view dragged out of a collapsed side-by-side strip, and the Inspector and a view onto it.
await run('sources', async () => {
  startTally();
  const P = (views, shown, collapsed = false) => ({ views, shown, collapsed });
  const strip = { split: 'row', f: 0.54, kids: [P(MAIN, 'graph', true), P(['insp'], 'insp')] };
  const strip2 = { split: 'row', f: 0.54, kids: [P(MAIN, 'graph'), P(['insp'], 'insp', true)] };
  const { ctx, page } = await open(1600, { seed: strip });
  let ok = true;
  for (const v of MAIN) for (const z of ['centre', 'left', 'right']) {
    ok = (await reseed(page, strip)) && ok;
    await dropCase(page, strip, 0, v, 1, z, 'strip');
  }
  for (const z of ZONES) { ok = (await reseed(page, strip)) && ok; await dropCase(page, strip, 1, 'insp', 0, z, 'onto strip'); }
  for (const z of ['centre', 'left', 'right']) { ok = (await reseed(page, strip2)) && ok; await dropCase(page, strip2, 1, 'insp', 0, z, 'inspector strip'); }
  for (const z of ZONES) { ok = (await reseed(page, strip2)) && ok; await dropCase(page, strip2, 0, 'table', 1, z, 'onto inspector strip'); }
  check('each case starts from its seeded arrangement', ok, {});
  endTally({ minValid: 20 });
  await ctx.close();
});

// Panes too narrow or too short to split, and the accordion.
await run('refuse', async () => {
  startTally();
  const P = (views, shown, collapsed = false) => ({ views, shown, collapsed });
  const states = [
    // 900 wide: the main pane split in two 243 px halves
    [900, { split: 'row', f: 0.54, kids: [{ split: 'row', f: 0.5, kids: [P(['graph', 'files', 'log', 'table'], 'graph'), P(['list'], 'list')] }, P(['insp'], 'insp')] }],
    // stacked: the main pane in two quarter-height halves
    [1600, { split: 'col', f: 0.5, kids: [{ split: 'col', f: 0.5, kids: [P(['graph', 'files', 'log', 'table'], 'graph'), P(['list'], 'list')] }, P(['insp'], 'insp')] }],
    // Files alone between two collapsed panes: taking it out would leave them in one split
    [1600, { split: 'row', f: 0.5, kids: [{ split: 'col', f: 0.5, kids: [P(['graph'], 'graph', true), { split: 'row', f: 0.5, kids: [P(['files'], 'files'), P(['log'], 'log', true)] }] }, P(['table', 'list', 'insp'], 'table')] }],
  ];
  // the sources that reach each refusal: a view from another pane, and the pane-emptying view
  const from = [[[0, 'log'], [1, 'list']], [[0, 'log'], [1, 'list']], [[1, 'files'], [3, 'table']]];
  let ok = true;
  for (const [k, [w, tree]] of states.entries()) {
    const { ctx, page } = await open(w, { seed: tree });
    const n = leaves(tree).length;
    for (const [i, v] of from[k]) for (let t = 0; t < n; t++) for (const z of ZONES) {
      ok = (await reseed(page, tree)) && ok;
      await dropCase(page, tree, i, v, t, z, `refuse ${w}`);
    }
    await ctx.close();
  }
  check('each case starts from its seeded arrangement', ok, {});
  endTally({ minValid: 20, minRefused: 30 });
});

// The Inspector dropped over the Log (or Files) in its pane: the keys then go to a view that is off
// screen, and must step as on screen. Graded against the same keys with no drop.
// After a drop the view the keys act on is drawn, on screen or off, and its keys step exactly as
// with no drop. Routes: the Inspector dropped over the key view's slot (its pane's centre) and
// beside it (an edge), in every preset, for the Log and Files; the key view dropped into a pane
// under the 200 px floor (a 420 px window), once after it was drawn on screen and once never drawn
// before; the Inspector dropped while the key view's pane is collapsed. Each drop must land (saved
// and drawn as defined) before its keys are graded.
await run('keyview', async () => {
  const runs = [];
  const steps = async (page, v) => {
    const out = [];
    for (const k of ['j', 'j', 'j', 'k']) { await key(page, k, 600); out.push(v === 'log' ? param(await hashOf(page), 'c') : param(await hashOf(page), 's')); }
    return out.join(' ');
  };
  const prep = async (page, preset, keys) => { await reset(page, preset); for (const k of keys) await key(page, k, k === 'w' ? 60 : 1200); };
  const routes = [];
  for (const preset of Object.keys(PRESETS)) for (const [v, k] of [['log', 'l'], ['files', 'f']]) for (const z of ['centre', 'left']) {
    routes.push({ id: `${preset} ${v}: the Inspector onto its pane's ${z}`, w: 1600, preset, keys: [k], v, drag: 'insp', target: s => s.panes.findIndex(p => p.tabs.includes(v)), z });
  }
  routes.push({ id: 'classic 420: the Log onto the Inspector, a pane under the floor', w: 420, preset: 'classic', keys: ['l'], v: 'log', drag: 'log', target: s => s.panes.findIndex(p => p.tabs.includes('insp')), z: 'centre', under: true });
  // the same drop with the Log never drawn: its tab pressed in a pane showing the Graph, so it
  // arrives off screen with nothing drawn, and only the drop's own drawing of the key view can
  // make its keys step (graded against the Log on screen, as the route above)
  routes.push({ id: 'classic 420: the Log, never drawn, onto the Inspector, a pane under the floor', w: 420, preset: 'classic', keys: [], base: ['l'], v: 'log', drag: 'log', target: s => s.panes.findIndex(p => p.tabs.includes('insp')), z: 'centre', under: true, fresh: true });
  routes.push({ id: 'three: the Log\'s pane collapsed, the Inspector onto the Graph bar', w: 1600, preset: 'three', keys: ['l', 'w', 'd'], v: 'log', drag: 'insp', target: s => s.panes.findIndex(p => p.tabs.includes('graph')), z: 'centre', collapsedKey: true });
  const bases = {};
  for (const r of routes) {
    const bk = `${r.w} ${r.preset} ${r.v}`;
    if (!bases[bk]) { const { ctx, page } = await open(r.w); await prep(page, r.preset, r.base || r.keys.slice(0, 1)); bases[bk] = await steps(page, r.v); await ctx.close(); }
    const { ctx, page } = await open(r.w);
    await prep(page, r.preset, r.keys);
    const tree = await stored(page);
    const s0 = await snap(page);
    const t = r.target(s0);
    const i = s0.panes.findIndex(p => p.tabs.includes(r.drag));
    const want = t >= 0 && drop(tree, r.drag, t, r.z, s0.W, s0.H);
    if (!want) { runs.push({ id: r.id, why: 'not a valid drop' }); await ctx.close(); continue; }
    // a route whose key view must arrive never drawn: nothing of it drawn before the press
    const undrawn = !r.fresh || await page.evaluate(() => document.querySelectorAll('#ltable .crow').length === 0);
    const began = await begin(page, i, r.drag);
    await to(page, client(s0, aimPoints(s0.panes[t].rect, s0.panes[t].collapsed)[r.z]));
    await page.mouse.up();
    const landed = !!began && await storedIs(page, want.root);
    await settle(page);
    const s1 = await snap(page);
    const bad = drawn(s1, want.root, 2);
    const off = !s1.panes.some(p => p.roots.includes(r.v));
    const got = await steps(page, r.v);
    const base = bases[bk];
    await key(page, r.v === 'log' ? 'l' : 'f', 1200);
    const s2 = await snap(page);
    const back = r.under || s2.panes.some(p => p.roots.includes(r.v));
    runs.push({ id: r.id, off, ok: undrawn && landed && bad.length === 0 && got === base && got.split(' ').every(Boolean) && back && focusOf(s1) === want.pane, undrawn, landed, bad, base, got });
    await ctx.close();
  }
  const offRuns = runs.filter(r => r.off).length;
  check(`after a drop the view keys step as on screen, the key view on screen or not (${runs.length} drops, ${offRuns} off screen)`, runs.length === routes.length && offRuns >= 6 && runs.every(r => r.ok), runs.filter(r => !r.ok).slice(0, 4));
});

// A moved view keeps its scroll, the selection and the Inspector's tab; back and forward after the
// drop move between views, never undoing the arrangement. The scroll is graded on content measured
// to overflow its scroller by at least the distance the wheel scrolls it: for Files the first
// folder whose opening does that to the tree, for the Inspector the first row whose History does
// it, stepping on from where the case's two j presses leave the selection, and the Log's table as
// it is. That the scroller scrolled before the drop is a precondition with its own label, so a
// page that can't scroll fails there, never as a moved view losing its scroll; the keep-state
// label grades the cases where it held.
await run('state', async () => {
  const out = [], hist = [], pre = [];
  const wheel = 240;
  const held = t => leaves(t).map(l => l.node.views.slice().sort().join()).join(' | ');
  const over = (page, s) => page.evaluate(s => { const e = document.querySelector(s); return e ? e.scrollHeight - e.clientHeight : -1; }, s);
  // click a folder row's arrow, where it is hit; false when it isn't
  const arrow = async (page, d) => {
    const at = await page.evaluate(d => {
      const t = [...document.querySelectorAll('#tree .trow.dir')].find(r => r.dataset.id === d)?.querySelector('.tw');
      if (!t) return null;
      const r = t.getBoundingClientRect(); const x = r.left + r.width / 2, y = r.top + r.height / 2;
      const e = document.elementFromPoint(x, y);
      return e && t.contains(e) ? [x, y] : null;
    }, d);
    if (!at) return false;
    await page.mouse.click(at[0], at[1]); await page.waitForTimeout(300);
    return true;
  };
  for (const preset of ['classic', 'three']) for (const [v, z] of [['files', 'right'], ['insp', 'centre'], ['log', 'bottom']]) {
    const { ctx, page } = await open(1600);
    await reset(page, preset);
    await key(page, v === 'log' ? 'l' : 'f', 1200);
    const scroller = { files: '#tree', log: '#ltable', insp: '#ibody' }[v];
    let picked = v === 'log' ? 'the Log' : '';
    if (v === 'files') {
      // open the folders in turn, keeping the first that makes the tree overflow enough
      const dirs = await page.evaluate(() => [...document.querySelectorAll('#tree .trow.dir')].map(r => r.dataset.id));
      for (const d of dirs) {
        if (!await arrow(page, d)) continue;
        if (await over(page, scroller) >= wheel) { picked = d; break; }
        await arrow(page, d);
      }
    }
    await key(page, 'j', 600); await key(page, 'j', 600);
    await key(page, 'h', 900); // the Inspector on its History tab
    if (v === 'insp') {
      // step down the rows until the History shown overflows enough
      for (let k = 0; k < 30; k++) {
        if (await over(page, scroller) >= wheel) { picked = param(await hashOf(page), 's'); break; }
        await key(page, 'j', 900);
      }
    }
    const room = await over(page, scroller);
    const box = await page.evaluate(s => { const e = document.querySelector(s); const r = e.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; }, scroller);
    await page.mouse.move(box.x, box.y); await page.mouse.wheel(0, wheel); await page.waitForTimeout(400);
    const before = { top: await page.evaluate(s => document.querySelector(s).scrollTop, scroller), hash: await hashOf(page) };
    pre.push({ id: `${preset} ${v}`, ok: before.top > 0, picked, room, top: before.top });
    const tree = await stored(page);
    const s0 = await snap(page);
    const i = s0.panes.findIndex(p => p.tabs.includes(v));
    const t = s0.panes.findIndex((p, k) => k !== i && drop(tree, v, k, z, s0.W, s0.H));
    const id = `${preset} ${v} -> ${z}@${t}`;
    if (t < 0) { out.push({ id, why: 'no target' }); await ctx.close(); continue; }
    const want = drop(tree, v, t, z, s0.W, s0.H);
    const began = await begin(page, i, v);
    await to(page, client(s0, aimPoints(s0.panes[t].rect, s0.panes[t].collapsed)[z]));
    await page.mouse.up();
    const landed = !!began && await storedIs(page, want.root);
    await settle(page);
    const bad = drawn(await snap(page), want.root, 2);
    const after = { top: await page.evaluate(s => document.querySelector(s).scrollTop, scroller), hash: await hashOf(page) };
    const histTab = await page.evaluate(() => document.querySelector('#itabs [data-tab="history"]')?.classList.contains('on') ?? false);
    const ok = landed && bad.length === 0 && near(after.top, before.top, 1) && param(after.hash, 's') === param(before.hash, 's') && param(after.hash, 'tab') === 'history' && histTab;
    out.push({ id, ok, graded: before.top > 0, landed, bad, before, after, histTab });
    // back, then forward: the hash steps through the entries the keys pushed, and every pane
    // keeps the views the drop left it (a view shown again may change a pane's shown tab)
    const h0 = await hashOf(page);
    await page.goBack(); await page.waitForTimeout(800); await settle(page);
    const hb = await hashOf(page), tb = await stored(page);
    await page.goForward(); await page.waitForTimeout(800); await settle(page);
    const hf = await hashOf(page), tf = await stored(page);
    hist.push({ id, ok: landed && hb !== h0 && hf === h0 && held(tb) === held(want.root) && held(tf) === held(want.root), hb, hf, h0 });
    await ctx.close();
  }
  check(`the scroller scrolled before the drop (${pre.filter(r => r.ok).length} of ${pre.length} cases)`, pre.length === 6 && pre.every(r => r.ok), pre.filter(r => !r.ok));
  const graded = out.filter(r => r.graded);
  check(`a moved view keeps its scroll, the selection and the Inspector's tab (${out.length} drops, ${graded.length} with the scroller scrolled)`, out.length === 6 && graded.every(r => r.ok), graded.filter(r => !r.ok));
  check(`back and forward after a drop step through views and never undo the arrangement (${hist.length} drops)`, hist.length === 6 && hist.every(r => r.ok), hist.filter(r => !r.ok));
});

group = 'all'; check('no page errors', errors.length === 0, errors.slice(0, 5));
await browser.close();
console.log(fails ? `dock: ${fails} failed` : 'dock: all passed');
process.exit(fails ? 1 : 0);
