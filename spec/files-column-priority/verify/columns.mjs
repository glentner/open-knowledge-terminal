// The Files table's columns by priority, graded on what renders at real pane widths. A draft:
// build folds these checks into tests/ui/keys.mjs as its "FILES columns" section (TECH.md) and
// the copy there is the gate; this file is what the plan dry-ran on its prototype.
//   tests/env.sh sh -c 'node "$OKT_ROOT/spec/files-column-priority/verify/columns.mjs"'
// It writes three fixtures into the throwaway clone ($OKT_KB), never into a real KB.
// Footprint: one headless Chromium with one page (~300 MB); about a minute.
import { writeFileSync, mkdirSync, realpathSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { homedir } from 'node:os';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');

const OKT = process.argv[2] || process.env.OKT;
const KB = process.env.OKT_KB;
if (!OKT || !KB) { console.log('FAIL needs OKT and OKT_KB (run it inside tests/env.sh)'); process.exit(2); }
let real = ''; try { real = realpathSync(join(homedir(), 'KB')); } catch {}
if (realpathSync(KB) === real) { console.log('FAIL OKT_KB is the real KB'); process.exit(2); }

// Fixtures: a titled file and a folder whose Name texts are 60 characters, a title-less file,
// and a long titled file one level down (a nested row).
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
  if (i > 60) { console.log('FAIL the fixtures never reached the index'); process.exit(1); }
  await new Promise(r => setTimeout(r, 500));
}

let fails = 0;
const check = (label, ok, detail) => { if (!ok) fails++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok ? '' : ': ' + JSON.stringify(detail)}`); };
const DROP = ['ti', 'lines', 'size', 'age', 'ty'];   // the order the droppable columns give way
const ALL = ['nm', ...DROP, 'st'];

const browser = await chromium.launch();
try {
  const page = await (await browser.newContext({ viewport: { width: 1600, height: 1000 } })).newPage();
  const errors = [];
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
  const chPx = wide.rows.long.text.full / wide.rows.long.text.chars;
  const capPx = wide.rows.long.text.left + wide.rows.long.text.full - wide.rows.long.nmLeft;
  check('the fixture rows render', ['long', 'bare', 'dir', 'nest'].every(k => wide.rows[k]), Object.keys(wide.rows).filter(k => !wide.rows[k]));
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

  // ---- the sweep: narrow the pane 1300 -> 380 and widen it back, 2px at a time -----------------
  const sweep = async () => page.evaluate(() => { const o = []; for (let x = 1300; x >= 380; x -= 2) o.push(window.__cols.pane(x)); for (let x = 380; x <= 1300; x += 2) o.push(window.__cols.pane(x)); return o; });
  // the pane width (and the table's inner width) at which each column first gives way
const transitions = (Ls, key = 'pane') => { const t = {}; let prev = null; for (const L of Ls) { const s = shownSet(L); if (prev) for (const c of DROP) if (prev.includes(c) && !s.includes(c)) (t[c] ??= L[key]); prev = s; } return t; };
  const down = await sweep();
  const half = down.length / 2;
  const graded = down.map((L, i) => ({ i, pane: L.pane, ...grade(L) }));
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
  // TECH.md's thresholds: a column shows while the table's inner width is at least the Name
  // width at its cap (60 characters plus 35px) plus what it and the columns after it take.
  const OFF = { ti: 496, lines: 366, size: 300, age: 226, ty: 156 };
  const inner = transitions(down.slice(0, half), 'content');
  const stated = DROP.map(c => [c, inner[c], +(60 * chPx + 35 + OFF[c]).toFixed(1)]);
  console.log('     thresholds (inner width at which each gives way, stated):', JSON.stringify(stated));
  check('each column gives way at the threshold TECH.md states', stated.every(([, got, at]) => got < at && got + 2 >= at), stated);

  // ---- the cap is one value: change it and every threshold moves by the same characters --------
  // Every style rule, nested ones too (@container, @media, @supports, @layer), that applies to
  // the Files table and writes a `ch` length.
  const cap = await page.evaluate(() => {
    const tree = document.getElementById('tree');
    const els = [tree, ...tree.querySelectorAll('*')];
    const found = [];
    const walk = (rules, path) => { for (const [i, r] of [...rules].entries()) {
      if (r instanceof CSSStyleRule) for (const p of r.style) { const v = r.style.getPropertyValue(p); if (/\b\d+(\.\d+)?ch\b/.test(v) && els.some(e => e.matches(r.selectorText))) found.push({ at: [...path, i], sel: r.selectorText, p, v }); }
      if (r.cssRules) walk(r.cssRules, [...path, i]);
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

  // ---- the three pane widths, set with the splitter in a 1600px window -------------------------
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
    if (w === 760) check('760px: Title and Lines hidden, Type shown, the 60-character file and folder names whole', !g.set.includes('ti') && !g.set.includes('lines') && g.set.includes('ty') && whole, { set: g.set, t, td });
    if (w === 760) {
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

  // ---- the same pane widths in a window narrow enough to stack the panes -----------------------
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

  // ---- hover: the full path on every Name cell, cut or not --------------------------------------
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
      check(`${x}px: hovering ${id.length > 40 ? id.slice(0, 40) + '…' : id} shows its full path`, tips.every(t => t === id), tips);
    }
  }

  // ---- a wide pane is as today: Name and Title split 1 : 0.9; the header stays on top -----------
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

  // ---- the other views still render ------------------------------------------------------------
  for (const [v, sel, hash] of [['log', '#ltable .crow', '#v=log'], ['table', '#tgrid table tbody tr', '#v=table'], ['list', '#list .lrow, #list .lsimple', '#v=list&q=' + encodeURIComponent('plain')]]) {
    await page.evaluate(h => { location.hash = h; }, hash);
    await page.waitForTimeout(1500);
    const got = await page.evaluate(([v, sel]) => { const els = document.querySelectorAll(sel); const b = els[0]?.getBoundingClientRect(); return { shown: !document.getElementById('v-' + v).hidden, n: els.length, w: b?.width ?? 0, h: b?.height ?? 0 }; }, [v, sel]);
    check(`the ${v} view renders`, got.shown && got.n > 0 && got.w > 0 && got.h > 0, got);
  }
  check('no page errors', errors.length === 0, errors.slice(0, 5));
} finally {
  await browser.close();
}
console.log(fails ? `${fails} failed` : 'all passed');
process.exit(fails ? 1 : 0);
