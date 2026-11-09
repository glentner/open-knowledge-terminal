// The navigation keys (README § Keys), checked on their own against a running okt:
//   FILES  u walks up and folds what is under the new parent; space opens or closes a folder
//          (closing folds everything inside) and shows a file in the graph
//   GRAPH  j/k step through the selection's connections, Enter/space go there, esc stops;
//          space with no walk opens or closes the selected folder; z frames the selection
//   ANY    J/K scroll the inspector, s source/rendered, y copies the path, m and 1-9 pins,
//          n/N FIND matches, E/O/F edit/open/reveal (the request is intercepted, nothing opens)
//   FILES columns  the Files table gives up Title, Lines, Size, Changed and Type in that order as
//          its pane narrows, Name holding its 60-character cap while any of them shows; graded
//          across pane widths, the splitter and stacked windows; every Name cell hovers its
//          full path; the Log, Table, Results and Paths (RECENT) rows keep the geometry their
//          app.css tracks give them. It writes fixtures into $OKT_KB, so it runs only under
//          tests/env.sh and prints a skip line elsewhere
//   node tests/ui/keys.mjs [okt-url]
import { writeFileSync, mkdirSync, realpathSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { homedir } from 'node:os';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');

const OKT = process.argv[2] || process.env.OKT || 'http://127.0.0.1:6580/';
const idx = await (await fetch(OKT + 'api/index')).json();
const dirs = new Set(idx.nodes.filter(n => n.t === 'dir').map(n => n.id));
const parent = Object.fromEntries(idx.nodes.map(n => [n.id, n.parent]));
const ancestors = id => { const out = []; for (let p = parent[id]; p && p !== '.'; p = parent[p]) out.push(p); return out; };
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
const page = await ctx.newPage();
const errors = [];
page.on('pageerror', e => errors.push(e.message));
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

// ---- FILES columns --------------------------------------------------------------------------
// The Files table gives up its columns by priority as its pane narrows, graded on what renders.
// It writes fixtures into the throwaway clone ($OKT_KB), so it runs only under tests/env.sh, on a
// fresh page, after every check above has made its picks.
const filesColumns = async () => {
  const KB = process.env.OKT_KB;
  if (!KB) { console.log('skip FILES columns: needs OKT_KB (run it inside tests/env.sh)'); return; }
  let realKB = ''; try { realKB = realpathSync(join(homedir(), 'KB')); } catch {}
  if (realpathSync(KB) === realKB) { check('FILES columns: OKT_KB is not the real KB', false, KB); return; }
  // Fixtures: a titled file and a folder whose Name texts are 60 characters, a title-less file,
  // and a long titled file one level down (a nested row). Untracked, so they show as uncommitted.
  const pad = (stem, n, tail = '') => (stem + '-abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz').slice(0, n - tail.length) + tail;
  const LONG = pad('okt-fixture-long-titled-file', 60, '.md');
  const BARE = 'okt-fixture-untitled.md';
  const DIR = pad('okt-fixture-long-folder', 59);              // its Name text is DIR + "/"
  const NEST = pad('okt-fixture-nested-titled-file', 60, '.md');
  writeFileSync(join(KB, LONG), '---\ntitle: A fixture with a title\n---\nbody\n');
  writeFileSync(join(KB, BARE), 'plain text and no heading, so the row has no title\n');
  mkdirSync(join(KB, DIR));
  writeFileSync(join(KB, DIR, NEST), '---\ntitle: A nested fixture\n---\nbody\n');
  const want = [LONG, BARE, `${DIR}/${NEST}`];
  let DIR_ID = null;                                            // the folder's id, as the index has it
  for (let i = 0; ; i++) {
    const nodes = (await (await fetch(OKT + 'api/index')).json()).nodes;
    const ids = new Set(nodes.map(n => n.id));
    DIR_ID = nodes.find(n => n.t === 'dir' && n.parent === '.' && n.name === DIR)?.id ?? null;
    if (DIR_ID && want.every(w => ids.has(w))) break;
    if (i > 60) break;
    await new Promise(r => setTimeout(r, 500));
  }
  check('FILES columns: the fixtures reach the index', !!DIR_ID, want);
  if (!DIR_ID) return;

  const DROP = ['ti', 'lines', 'size', 'age', 'ty'];   // the order the droppable columns give way
  const ALL = ['nm', ...DROP, 'st'];
  const page = await (await browser.newContext({ viewport: { width: 1600, height: 1000 } })).newPage();
  const before = errors.length;               // this page's errors join the suite's
  page.on('pageerror', e => errors.push(e.message));
  await page.goto(OKT + '#v=files&s=' + encodeURIComponent(`${DIR}/${NEST}`), { waitUntil: 'load' });
  await page.waitForTimeout(2000);

  // Everything the checks read, in one synchronous pass over the live layout.
  await page.evaluate(([LONG, BARE, DIR, NEST, ALL, DIR_ID]) => {
    const tree = document.getElementById('tree');
    const row = id => [...tree.querySelectorAll('.trow')].find(r => r.dataset.id === id);
    const textWidth = (el, from = 0, to) => { const r = document.createRange(); const t = el.firstChild; r.setStart(t, from); r.setEnd(t, to ?? t.length); return r.getBoundingClientRect().width; };
    window.__cols = {
      textWidth,
      read() {
        const tr = tree.getBoundingClientRect();
        const inner = tr.left + tree.clientLeft + tree.clientWidth;
        const rows = { head: tree.querySelector('.trow.thead'), long: row(LONG), bare: row(BARE), dir: row(DIR_ID), nest: row(`${DIR}/${NEST}`) };
        const out = { pane: tr.width, content: tree.clientWidth, overflow: tree.scrollWidth - tree.clientWidth, rows: {} };
        for (const [k, r] of Object.entries(rows)) {
          if (!r) { out.rows[k] = null; continue; }
          const rr = r.getBoundingClientRect();
          const cells = {};
          for (const c of ALL) {
            const el = r.querySelector(':scope > .' + c);
            if (!el) { cells[c] = { state: 'missing' }; continue; }
            const b = el.getBoundingClientRect();
            const none = getComputedStyle(el).display === 'none';
            const state = none || b.width === 0 ? 'hidden'
              : b.width > 0.5 && b.left >= rr.left - 0.5 && b.right <= Math.min(rr.right, inner) + 0.5 ? 'shown' : 'ambiguous';
            cells[c] = { state, l: b.left, r: b.right, w: b.width, none: el.classList.contains('none') };
          }
          const t = r.querySelector('.nm .t');
          const nm = r.querySelector('.nm');
          out.rows[k] = {
            l: rr.left, r: rr.right, cells,
            gap: parseFloat(getComputedStyle(r).columnGap), padL: parseFloat(getComputedStyle(r).paddingLeft),
            padR: parseFloat(getComputedStyle(r).paddingRight), border: parseFloat(getComputedStyle(r).borderLeftWidth),
            text: t && { full: textWidth(t), box: t.getBoundingClientRect().width, sw: t.scrollWidth, cw: t.clientWidth, left: t.getBoundingClientRect().left, chars: t.textContent.length },
            nmLeft: nm.getBoundingClientRect().left,
          };
        }
        return out;
      },
      // the Files pane at an exact width, through the same custom property the splitter sets
      pane(x) { document.getElementById('panes').style.setProperty('--insp-w', `calc(100% - 5px - ${x}px)`); return this.read(); },
    };
  }, [LONG, BARE, DIR, NEST, ALL, DIR_ID]);

  const shownSet = L => DROP.filter(c => L.rows.head.cells[c].state === 'shown');
  // The 60-character Name text and the width it needs, measured at a wide pane.
  const wide = await page.evaluate(() => window.__cols.pane(1300));
  const rendered = ['long', 'bare', 'dir', 'nest'].every(k => wide.rows[k]);
  check('FILES columns: the fixture rows render', rendered, Object.keys(wide.rows).filter(k => !wide.rows[k]));
  if (!rendered) return;
  const chPx = wide.rows.long.text.full / wide.rows.long.text.chars;
  const capPx = wide.rows.long.text.left + wide.rows.long.text.full - wide.rows.long.nmLeft;
  check('the long fixture names are 60 characters', wide.rows.long.text.chars === 60 && wide.rows.dir.text.chars === 60, [wide.rows.long.text.chars, wide.rows.dir.text.chars]);

  // One layout, graded: every cell is clearly shown or hidden, each column agrees with its header,
  // shown cells pack with one gap between them and none at the edges, and Name holds its cap
  // while any droppable column is shown.
  // `cap` is the Name width to hold; `whole` also asks that the 60-character names are not cut
  // (off for the moved-cap sweep, where 60 characters no longer fit by design).
  const grade = (L, { cap = capPx, whole = true } = {}) => {
    const bad = [];
    const head = L.rows.head;
    for (const [k, r] of Object.entries(L.rows)) {
      for (const c of ALL) {
        const cell = r.cells[c];
        if (cell.state === 'missing' || cell.state === 'ambiguous') bad.push(`${k}.${c} ${cell.state}`);
        if (c === 'ti' && cell.none) continue;     // a title-less row's empty Title cell
        if (c !== 'nm' && cell.state !== head.cells[c].state) bad.push(`${k}.${c} ${cell.state}, header ${head.cells[c].state}`);
        if (cell.state === 'shown' && c !== 'nm' && Math.abs(cell.l - head.cells[c].l) > 0.5) bad.push(`${k}.${c} not under its header`);
      }
      const vis = ALL.filter(c => r.cells[c].state === 'shown').map(c => r.cells[c]).sort((a, b) => a.l - b.l);
      if (Math.abs(vis[0].l - (r.l + r.border + r.padL)) > 0.5) bad.push(`${k} starts off the edge`);
      if (Math.abs(vis.at(-1).r - (r.r - r.padR)) > 0.5) bad.push(`${k} leaves an empty track at the end`);
      for (let i = 1; i < vis.length; i++) if (Math.abs(vis[i].l - vis[i - 1].r - r.gap) > 0.5) bad.push(`${k} has a gap that is not one column gap`);
      if (r.cells.st.state !== 'shown') bad.push(`${k} status column hidden`);
    }
    const set = shownSet(L);
    const bareEnd = head.cells.ti.state === 'shown' ? head.cells.ti.r : head.cells.nm.r;
    if (Math.abs(L.rows.bare.cells.nm.r - bareEnd) > 0.5) bad.push(`the title-less Name ends at ${L.rows.bare.cells.nm.r}, not ${bareEnd}`);
    if (set.length) {
      if (head.cells.nm.w < cap - 0.5) bad.push(`Name ${head.cells.nm.w} narrower than its cap ${cap} with ${set} shown`);
      if (whole) for (const k of ['long', 'dir']) {
        const t = L.rows[k].text;
        if (t.full > t.box + 0.05 || t.sw > t.cw) bad.push(`the 60-character ${k === 'dir' ? 'folder' : 'file'} name is cut with ${set} shown`);
      }
    }
    if (L.overflow > 0) bad.push(`the table scrolls sideways by ${L.overflow}`);
    return { ok: bad.length === 0, bad, set };
  };
  const ordered = set => DROP.every((c, i) => !set.includes(c) || DROP.slice(i + 1).every(d => set.includes(d)));

  // ---- the sweep: narrow the pane 1300 -> 380 and widen it back, 2px at a time
  const sweep = async () => page.evaluate(() => { const o = []; for (let x = 1300; x >= 380; x -= 2) o.push(window.__cols.pane(x)); for (let x = 380; x <= 1300; x += 2) o.push(window.__cols.pane(x)); return o; });
  // the pane width (or the table's inner width) at which each column first gives way
  const transitions = (Ls, k = 'pane') => { const t = {}; let prev = null; for (const L of Ls) { const s = shownSet(L); if (prev) for (const c of DROP) if (prev.includes(c) && !s.includes(c)) (t[c] ??= L[k]); prev = s; } return t; };
  const down = await sweep();
  const half = down.length / 2;
  const graded = down.map(L => ({ pane: L.pane, ...grade(L) }));
  const firstBad = graded.find(g => !g.ok);
  check('every sweep width lays out cleanly (clear cells, under their headers, no gaps, Name at its cap)', !firstBad, firstBad && { pane: firstBad.pane, bad: firstBad.bad.slice(0, 6) });
  const unordered = graded.find(g => !ordered(g.set));
  check('columns give way in order: Title, Lines, Size, Changed, Type', !unordered, unordered && { pane: unordered.pane, shown: unordered.set });
  check('all five shown at 1300px, none at 380px', shownSet(down[0]).length === 5 && shownSet(down[half - 1]).length === 0, [shownSet(down[0]), shownSet(down[half - 1])]);
  const back = down.slice(half).reverse();
  const hyst = down.slice(0, half).findIndex((L, i) => shownSet(L).join() !== shownSet(back[i]).join());
  check('widening brings them back in reverse, at the same widths', hyst < 0, hyst >= 0 && { pane: down[hyst].pane, down: shownSet(down[hyst]), up: shownSet(back[hyst]) });
  const t0 = transitions(down.slice(0, half));
  check('each column gives way exactly once on the way down', DROP.every(c => t0[c]) && DROP.every((c, i) => i === 0 || t0[c] < t0[DROP[i - 1]]), t0);
  // The stated thresholds: a column shows while the table's inner width is at least the Name
  // width at its cap (60 characters plus 35px) plus what it and the columns after it take. The
  // offsets restate app.css's column widths; they change together.
  const OFF = { ti: 496, lines: 366, size: 300, age: 226, ty: 156 };
  const innerAt = transitions(down.slice(0, half), 'content');
  const stated = DROP.map(c => [c, innerAt[c], +(60 * chPx + 35 + OFF[c]).toFixed(1)]);
  console.log('     thresholds (inner width at which each gives way, stated):', JSON.stringify(stated));
  check('each column gives way at its stated threshold', stated.every(([, got, at]) => got < at && got + 2 >= at), stated);

  // ---- the cap is one value: change it and every threshold moves by the same characters
  // Every style rule, nested ones too (@container, @media, @supports, @layer), that applies to
  // the Files table and writes a `ch` length.
  const cap = await page.evaluate(() => {
    const tree = document.getElementById('tree');
    const els = [tree, ...tree.querySelectorAll('*')];
    const found = [];
    const walk = (rules, at) => { for (const [i, r] of [...rules].entries()) {
      if (r instanceof CSSStyleRule) for (const p of r.style) { const v = r.style.getPropertyValue(p); if (/\b\d+(\.\d+)?ch\b/.test(v) && els.some(e => e.matches(r.selectorText))) found.push({ at: [...at, i], sel: r.selectorText, p, v }); }
      if (r.cssRules) walk(r.cssRules, [...at, i]);
    } };
    for (const [i, sh] of [...document.styleSheets].entries()) walk(sh.cssRules, [i]);
    return found;
  });
  check('the cap is written once in the stylesheet (one `ch` length on the Files table)', cap.length === 1, cap.length ? cap : 'cap not found');
  // ... and once in the source: the cap's literal across app.css and the client's Rust
  const ROOT = process.env.OKT_ROOT;
  if (ROOT && cap.length) {
    const lit = cap[0].v.match(/\b\d+(?:\.\d+)?ch\b/)[0];
    const rs = d => readdirSync(d, { withFileTypes: true }).flatMap(e => e.isDirectory() ? rs(join(d, e.name)) : e.name.endsWith('.rs') ? [join(d, e.name)] : []);
    const hits = [join(ROOT, 'okt-client/static/app.css'), ...rs(join(ROOT, 'okt-client/src'))].flatMap(f => (readFileSync(f, 'utf8').match(new RegExp(`\\b${lit}\\b`, 'g')) || []).map(() => f.slice(ROOT.length + 1)));
    check(`the cap's literal ${lit} appears once in the client source`, hits.length === 1, hits);
  }
  if (cap.length) {
    const n = +cap[0].v.match(/(\d+(?:\.\d+)?)ch/)[1];
    check('the cap is 60 characters', n === 60, cap[0]);
    const set = v => page.evaluate(([c, v]) => { let r = document.styleSheets[c.at[0]]; for (const i of c.at.slice(1)) r = r.cssRules[i]; r.style.setProperty(c.p, v); }, [cap[0], v]);
    await set(cap[0].v.replace(`${n}ch`, `${n - 20}ch`));
    const movedSweep = await sweep();
    const movedBad = movedSweep.map(L => ({ pane: L.pane, ...grade(L, { cap: capPx - 20 * chPx, whole: false }) })).find(g => !g.ok);
    check('with the cap 20 characters smaller, every width still lays out cleanly', !movedBad, movedBad && { pane: movedBad.pane, bad: movedBad.bad.slice(0, 6) });
    const t1 = transitions(movedSweep.slice(0, half));
    await set(cap[0].v);
    const moved = DROP.map(c => [c, t0[c] - t1[c]]);
    check(`20 characters off the cap moves every threshold by ~${(20 * chPx).toFixed(0)}px`, moved.every(([, d]) => Math.abs(d - 20 * chPx) <= 2.5), moved);
    const t2 = transitions((await sweep()).slice(0, half));
    check('restoring the cap restores the thresholds', DROP.every(c => t2[c] === t0[c]), { t0, t2 });
  }

  // ---- the three pane widths, set with the splitter in a 1600px window
  await page.evaluate(() => document.getElementById('panes').style.removeProperty('--insp-w'));
  const drag = async target => {
    const sp = await page.locator('#split').boundingBox();
    const pr = await page.evaluate(() => { const r = document.getElementById('panes').getBoundingClientRect(); return { right: r.right, width: r.width }; });
    const x = pr.right - (pr.width - 5 - target);
    await page.mouse.move(sp.x + sp.width / 2, sp.y + 200);
    await page.mouse.down();
    await page.mouse.move(x, sp.y + 200, { steps: 8 });
    await page.mouse.up();
    await page.waitForTimeout(150);
    return page.evaluate(() => window.__cols.read());
  };
  const R = {};
  for (const w of [1200, 760, 480]) {
    const L = await drag(w);
    const g = grade(L);
    R[w] = g.set;
    check(`splitter to ${w}px: the pane is ${w}px`, Math.abs(L.pane - w) <= 2, L.pane);
    check(`splitter to ${w}px: a clean layout`, g.ok, g.bad.slice(0, 6));
    const t = L.rows.long.text;
    const td = L.rows.dir.text;
    const whole = [t, td].every(x => x.full <= x.box + 0.05 && x.sw <= x.cw);
    if (w === 1200) check('1200px: every column, and the 60-character file and folder names whole', g.set.length === 5 && whole, { set: g.set, t, td });
    if (w === 760) {
      check('760px: Title and Lines hidden, Type shown, the 60-character file and folder names whole', !g.set.includes('ti') && !g.set.includes('lines') && g.set.includes('ty') && whole, { set: g.set, t, td });
      const expect = DROP.filter(c => L.content >= 60 * chPx + 35 + OFF[c]);
      check(`760px: Size and Changed as the thresholds say (${expect})`, g.set.join() === expect.join(), { set: g.set, expect, content: L.content });
    }
    if (w === 480) {
      const ell = await page.evaluate(() => { const s = document.createElement('span'); s.textContent = '…'; document.querySelector('#tree .trow:not(.thead) .nm .t').append(s); const w = s.getBoundingClientRect().width; s.remove(); return w; });
      const first40 = await page.evaluate(LONG => { const r = [...document.querySelectorAll('#tree .trow')].find(r => r.dataset.id === LONG); return window.__cols.textWidth(r.querySelector('.t'), 0, 40); }, LONG);
      const cs = await page.evaluate(LONG => { const t = [...document.querySelectorAll('#tree .trow')].find(r => r.dataset.id === LONG).querySelector('.t'); const s = getComputedStyle(t); return [s.textOverflow, s.overflowX, s.whiteSpace]; }, LONG);
      check('480px: only Name and the status column', g.set.length === 0, g.set);
      check('480px: Name fills the row', Math.abs(L.rows.head.cells.nm.r + L.rows.head.gap - L.rows.head.cells.st.l) <= 0.5, L.rows.head.cells);
      check('480px: the long name is cut at its end with "…" after at least 40 characters', t.full > t.box && cs.join() === 'ellipsis,hidden,nowrap' && t.box >= first40 + ell - 0.05, { t, first40, ell, cs });
      for (const k of ['dir', 'nest']) {
        const parts = await page.evaluate(id => { const r = [...document.querySelectorAll('#tree .trow')].find(r => r.dataset.id === id); const nm = r.querySelector('.nm').getBoundingClientRect(); return [...r.querySelector('.nm').children].map(c => { const b = c.getBoundingClientRect(); return { cls: c.getAttribute('class') || '', w: b.width, r: b.right, nmR: nm.right }; }); }, k === 'dir' ? DIR_ID : `${DIR}/${NEST}`);
        const depth = k === 'dir' ? 0 : 1;
        check(`480px: the ${k === 'dir' ? 'folder' : 'nested'} row keeps its indentation, twisty and glyph whole`, Math.abs(parts[0].w - 14 * depth) < 0.5 && parts[1].cls === 'tw' && Math.abs(parts[1].w - 10) < 0.5 && parts[2].cls === 'gl' && Math.abs(parts[2].w - 10) < 0.5 && parts.every(p => p.r <= p.nmR + 0.5), parts);
      }
    }
  }

  // ---- the same pane widths in a window narrow enough to stack the panes
  // Every stacked window from 900px (the widest that stacks) down to 380, 4px a step, against the
  // side-by-side sweep at the same inner width; and 760 and 480 against the splitter's.
  const side = new Map(down.slice(0, half).map(L => [L.content, shownSet(L).join()]));
  const stackBad = [];
  for (let w = 900; w >= 380; w -= 4) {
    await page.setViewportSize({ width: w, height: 1000 });
    const L = await page.evaluate(() => window.__cols.read());
    const stacked = await page.evaluate(() => document.getElementById('insp').getBoundingClientRect().top >= document.getElementById('view').getBoundingClientRect().bottom - 1);
    const g = grade(L);
    if (!stacked || Math.abs(L.pane - w) > 1) stackBad.push({ w, stacked, pane: L.pane });
    else if (!g.ok) stackBad.push({ w, bad: g.bad.slice(0, 4) });
    else if (side.has(L.content) && side.get(L.content) !== g.set.join()) stackBad.push({ w, stacked: g.set, side: side.get(L.content) });
    if (w === 760 || w === 480) check(`a ${w}px stacked pane shows what a ${w}px pane beside the inspector shows`, g.ok && g.set.join() === R[w].join(), { stacked: g.set, side: R[w], bad: g.bad.slice(0, 4) });
  }
  check('stacked windows 900 -> 380px lay out cleanly and match a side-by-side pane of the same width', stackBad.length === 0, stackBad.slice(0, 4));
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.waitForTimeout(300);

  // ---- hover: the full path on every Name cell, cut or not
  for (const x of [1200, 480]) {
    await page.evaluate(x => window.__cols.pane(x), x);
    for (const id of [LONG, BARE, DIR_ID, `${DIR}/${NEST}`]) {
      const tips = await page.evaluate(id => {
        const r = [...document.querySelectorAll('#tree .trow')].find(r => r.dataset.id === id);
        r.scrollIntoView({ block: 'center' });
        // the text, the glyph, the twisty, the cell's far end, and a nested row's indentation
        const nm = r.querySelector('.nm');
        const at = el => { const b = el.getBoundingClientRect(); return [b.left + Math.min(b.width / 2, 8), b.top + b.height / 2]; };
        const nb = nm.getBoundingClientRect();
        const pts = [at(nm.querySelector('.t')), at(nm.querySelector('.gl')), at(nm.querySelector('.tw')), [nb.right - 3, nb.top + nb.height / 2]];
        if (nm.firstElementChild.getBoundingClientRect().width > 4) pts.push(at(nm.firstElementChild));
        return pts.map(([x, y]) => document.elementFromPoint(x, y)?.closest('[title]')?.getAttribute('title') ?? null);
      }, id);
      check(`${x}px: hovering ${id.length > 40 ? '…' + id.slice(-40) : id} shows its full path`, tips.every(t => t === id), tips);
    }
  }

  // ---- a wide pane is as today: Name and Title split 1 : 0.9; the header stays on top
  const W = await page.evaluate(() => window.__cols.pane(1540));
  const ratio = W.rows.head.cells.nm.w / W.rows.head.cells.ti.w;
  check('a 1540px pane splits Name and Title as today (1 : 0.9)', Math.abs(ratio - 1 / 0.9) < 0.01, { nm: W.rows.head.cells.nm.w, ti: W.rows.head.cells.ti.w });
  const sticky = await page.evaluate(() => { const t = document.getElementById('tree'); t.scrollTop = 400; const d = t.querySelector('.thead').getBoundingClientRect().top - t.getBoundingClientRect().top; t.scrollTop = 0; return d; });
  check('the header stays at the top while the table scrolls', Math.abs(sticky) < 0.5, sticky);
  for (const x of [1200, 480]) {
    await page.evaluate(x => window.__cols.pane(x), x);
    const st = await page.evaluate(LONG => { const s = [...document.querySelectorAll('#tree .trow')].find(r => r.dataset.id === LONG).querySelector('.st'); return { text: s.textContent, title: s.title, w: s.getBoundingClientRect().width }; }, LONG);
    check(`${x}px: an uncommitted file shows its marker and "uncommitted"`, st.text.length > 0 && st.title === 'uncommitted' && st.w > 0, st);
  }

  // ---- the other views keep their row geometry
  // Each view's rows are graded against the tracks app.css gives them, pinned here: `.crow` (Log),
  // `.lrow` (Results), `.lsimple` (a path list such as RECENT) and the `.tgrid` cells (Table). A
  // number is a fixed track's width, `null` a track sized by its content, 'fr' one that takes the
  // rest; cells sit one gap apart, inside the row's left border and side padding, and the last one
  // ends at the padding. The Table is an HTML table: every row's cells line up under the header,
  // the table spans the pane, and each cell's text sits `inset` from its edge. These values restate
  // app.css: change a track there and its row here in the same commit (AGENTS.md § Conventions).
  const VIEWS = [
    { name: 'Log', view: 'log', open: '#v=log', rows: '#ltable .crow', tracks: [64, null, null, 'fr', 40, 96], gap: 10, pad: 10, border: 2, height: 21 },
    { name: 'Results', view: 'list', open: '#v=list&q=plain', title: 'FIND plain', rows: '#list .lrow', tracks: [48, 'fr'], gap: 10, pad: 10, border: 0 },
    { name: 'Paths (RECENT)', view: 'list', open: 'RECENT', title: 'RECENT', rows: '#list .lsimple', tracks: [null, 'fr'], gap: 14, pad: 10, border: 0 },
    { name: 'Table', view: 'table', open: '#v=table', rows: '#tgrid table tr', inset: 10 },
  ];
  // Read every row of the shown view and grade it against its pinned tracks, in the page.
  const gradeView = V => page.evaluate(V => {
    const bad = [];
    const near = (a, b) => Math.abs(a - b) <= 0.5;
    const rows = [...document.querySelectorAll(V.rows)];
    const pane = rows[0]?.closest('.ltable, .tgrid, .list');
    const out = { shown: !document.getElementById('v-' + V.view).hidden, n: rows.length, pane: pane?.clientWidth ?? 0, title: document.getElementById('listtitle').textContent };
    const box = el => el.getBoundingClientRect();
    if (V.tracks) for (const [i, r] of rows.entries()) {
      const rb = box(r);
      const cells = [...r.children].map(box);
      const at = `row ${i}`;
      if (cells.length !== V.tracks.length) { bad.push(`${at}: ${cells.length} cells, not ${V.tracks.length}`); continue; }
      if (!near(cells[0].left, rb.left + V.border + V.pad)) bad.push(`${at}: first cell at ${cells[0].left - rb.left}, not ${V.border + V.pad}`);
      if (!near(cells.at(-1).right, rb.right - V.pad)) bad.push(`${at}: last cell ends ${rb.right - cells.at(-1).right} from the edge, not ${V.pad}`);
      for (let j = 1; j < cells.length; j++) if (!near(cells[j].left - cells[j - 1].right, V.gap)) bad.push(`${at}: gap ${j} is ${cells[j].left - cells[j - 1].right}, not ${V.gap}`);
      V.tracks.forEach((t, j) => { if (typeof t === 'number' && !near(cells[j].width, t)) bad.push(`${at}: track ${j} is ${cells[j].width}, not ${t}`); });
      if (V.height && !near(rb.height, V.height)) bad.push(`${at}: height ${rb.height}, not ${V.height}`);
    }
    if (V.inset) {
      const head = rows[0] ? [...rows[0].children].map(box) : [];
      const tb = box(rows[0].closest('table'));
      if (!near(tb.left, box(pane).left + pane.clientLeft)) bad.push(`the table starts at ${tb.left - box(pane).left}`);
      if (tb.width < pane.clientWidth - 0.5) bad.push(`the table is ${tb.width} wide in a ${pane.clientWidth} pane`);
      for (const [i, r] of rows.entries()) {
        const cs = [...r.children];
        if (cs.length !== head.length) { bad.push(`row ${i}: ${cs.length} cells under ${head.length} headers`); continue; }
        cs.forEach((c, j) => {
          const b = box(c);
          if (!near(b.left, head[j].left) || !near(b.width, head[j].width)) bad.push(`row ${i} cell ${j} is not under its header`);
          if (!c.textContent.trim()) return;
          const rg = document.createRange(); rg.selectNodeContents(c); const t = rg.getBoundingClientRect();
          const inset = getComputedStyle(c).textAlign === 'right' ? b.right - t.right : t.left - b.left;
          if (!near(inset, V.inset)) bad.push(`row ${i} cell ${j}: text inset ${inset}, not ${V.inset}`);
        });
      }
    }
    return { ...out, bad };
  }, V);
  for (const x of [1200, 480]) {
    await page.evaluate(x => document.getElementById('panes').style.setProperty('--insp-w', `calc(100% - 5px - ${x}px)`), x);
    for (const V of VIEWS) {
      if (V.open.startsWith('#')) await page.evaluate(h => { location.hash = h; }, V.open);
      else { await page.fill('#go', V.open); await page.press('#go', 'Enter'); }
      await page.waitForTimeout(1500);
      const got = await gradeView(V);
      const there = got.shown && got.n > 0 && (!V.title || got.title === V.title);
      check(`${x}px: the ${V.name} view keeps its row geometry (${got.n} rows)`, there && Math.abs(got.pane - x) <= 1 && got.bad.length === 0, { shown: got.shown, n: got.n, pane: got.pane, title: got.title, bad: got.bad.slice(0, 4) });
      await page.evaluate(() => { location.hash = '#v=files'; });
      await page.waitForTimeout(300);
    }
  }
  check('FILES columns: no page errors', errors.length === before, errors.slice(before, before + 5));
};
await filesColumns();

if (errors.length) { fails++; console.log('page errors:', errors.slice(0, 5).join(' | ')); }
console.log(fails ? `${fails} failed` : 'all passed');
await browser.close();
process.exit(fails ? 1 : 0);
