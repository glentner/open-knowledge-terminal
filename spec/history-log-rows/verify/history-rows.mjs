// History's commit rows, graded on the rendered page of a running okt.
//
//   node history-rows.mjs [okt-url]                   History: rows, day rules, widths, clicks
//   node history-rows.mjs --baseline DIST [okt-url]   the Overview's Last commit row and the Log,
//                                                     this build against the client in DIST
//
// --baseline loads the page twice from the same server and KB: once as served, once with the
// client assets (app.css, main.js, okt.js, okt_bg.wasm) answered from DIST, a client built from the
// base commit. Everything outside History must render the same in both.
//
// Footprint: one headless Chromium, one page open at a time; about 400 MB.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');

const args = process.argv.slice(2);
let baseline = null;
if (args[0] === '--baseline') { baseline = args[1]; args.splice(0, 2); }
const OKT = args[0] || process.env.OKT || 'http://127.0.0.1:6580/';
const VIEW = { width: 1600, height: 1000 };
const STACKED = { width: 860, height: 1000 };   // below 900px the panes stack
const NARROW = { width: 901, height: 1000 };

let fails = 0;
// the keys where two flat objects differ, for a readable failure
const diff = (a, b) => a && b ? Object.keys({ ...a, ...b }).filter(k => JSON.stringify(a[k]) !== JSON.stringify(b[k])).map(k => [k, a[k], b[k]]) : { a, b };
const check = (label, ok, detail) => {
  if (!ok) fails++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok ? '' : ': ' + JSON.stringify(detail).slice(0, 600)}`);
};
const getJSON = async p => (await fetch(OKT + p)).json();

const idx = await getJSON('api/index');
const files = new Set(idx.nodes.filter(n => n.t === 'file').map(n => n.id));
const log = await getJSON('api/log');
const logPos = new Map(log.commits.map((c, i) => [c.h, i]));

const browser = await chromium.launch();
const errors = [];
const open = async (hash, { viewport = VIEW, dist = null } = {}) => {
  const ctx = await browser.newContext({ viewport, serviceWorkers: 'block' });
  if (dist) {
    const types = { 'app.css': 'text/css', 'main.js': 'text/javascript', 'okt.js': 'text/javascript', 'okt_bg.wasm': 'application/wasm' };
    for (const [name, type] of Object.entries(types)) {
      await ctx.route(`**/${name}`, route => route.fulfill({ status: 200, contentType: type, body: readFileSync(join(dist, name)) }));
    }
  }
  const page = await ctx.newPage();
  page.on('pageerror', e => errors.push(e.message));
  await page.goto(OKT + hash, { waitUntil: 'load' });
  await page.waitForFunction(() => !!document.querySelector('#itabs button.on'), null, { timeout: 30000 });
  await page.waitForTimeout(300);
  return page;
};
// History has rendered n rows
const rowsReady = (page, n) => page.waitForFunction(n => document.querySelectorAll('#ibody [data-commit]').length === n, n, { timeout: 30000 }).then(() => page.waitForTimeout(200));
// the Log has rendered and stopped changing
const logReady = async page => {
  await page.waitForFunction(() => document.querySelectorAll('#ltable .crow').length > 0, null, { timeout: 30000 });
  let last = -1;
  for (let i = 0; i < 20; i++) {
    const n = await page.evaluate(() => document.getElementById('v-log').innerHTML.length);
    if (n === last) break;
    last = n;
    await page.waitForTimeout(250);
  }
};
const close = page => page.context().close();
const go = async (page, hash, ms = 1200) => { await page.evaluate(h => { location.hash = h; }, hash); await page.waitForTimeout(ms); };
// the split dragged as far right as it goes: the inspector at its narrowest
const narrowest = async page => {
  const s = await page.locator('#split').boundingBox();
  const p = await page.locator('#panes').boundingBox();
  await page.mouse.move(s.x + s.width / 2, s.y + s.height / 2);
  await page.mouse.down();
  await page.mouse.move(p.x + p.width - 2, s.y + s.height / 2, { steps: 8 });
  await page.mouse.up();
  await page.waitForTimeout(500);
};
const widths = page => page.evaluate(() => ({
  insp: document.getElementById('insp').getBoundingClientRect().width,
  panes: document.getElementById('panes').getBoundingClientRect().width,
  stacked: getComputedStyle(document.getElementById('panes')).gridTemplateRows.split(' ').length > 1,
}));

// ---- the file under test --------------------------------------------------------------------------
// page0 stays open to read local days with the browser's own Date.
// A recent file whose commits fall on at least two days, with at least two on one day, all of them
// among the Log's rows (the Log renders the first 1500), and a folder above it.
const page0 = await open('');
const dayOf = ts => page0.evaluate(ts => ts.map(t => new Date(t * 1000).toDateString()), ts);
let F = null, Fc = null;
const seen = new Set();
for (const c of log.commits.slice(0, 400)) {
  for (const f of c.f || []) {
    const p = f[1];
    if (!p || seen.has(p) || !files.has(p) || !p.includes('/')) continue;
    seen.add(p);
    if (seen.size > 80 || F) break;
    const h = await getJSON('api/log?path=' + encodeURIComponent(p));
    const cs = h.commits || [];
    if (cs.length < 3 || cs.length > 120 || !cs.every(x => (logPos.get(x.h) ?? 1e9) < 1500)) continue;
    const ds = await dayOf(cs.map(x => x.t));
    const per = {};
    for (const d of ds) per[d] = (per[d] || 0) + 1;
    if (Object.keys(per).length >= 2 && Object.values(per).some(v => v >= 2)) { F = p; Fc = cs; }
  }
  if (F || seen.size > 80) break;
}
check('a file with commits on two days, two on one day', !!F, { tried: seen.size });
if (!F) { await browser.close(); process.exit(1); }
const D = F.slice(0, F.lastIndexOf('/') + 1);  // a folder's id ends in /
console.log(`file ${F} (${Fc.length} commits), folder ${D}`);

// ---- what a page shows ----------------------------------------------------------------------------
// History's body in document order: each rule's text, each row's hash, cells and geometry.
const readHistory = page => page.evaluate(() => {
  const body = document.getElementById('ibody');
  const items = [...body.querySelectorAll('.dayrule, [data-commit]')];
  const firstRule = body.querySelector('.dayrule');
  const lead = [...body.querySelectorAll('.sh, svg.spark')]
    .map(e => ({ tag: e.tagName.toLowerCase(), text: e.textContent, before: !!firstRule && !!(e.compareDocumentPosition(firstRule) & Node.DOCUMENT_POSITION_FOLLOWING) }));
  const oneLine = el => {
    // every line box of the cell's content starts within half an em of the first: one line,
    // whatever the line-height
    const r = document.createRange();
    r.selectNodeContents(el);
    const tops = [...r.getClientRects()].filter(b => b.width > 0 && b.height > 0).map(b => b.top);
    if (!tops.length) return true;
    return Math.max(...tops) - Math.min(...tops) < 0.5 * parseFloat(getComputedStyle(el).fontSize);
  };
  const ib = body.getBoundingClientRect();
  const shown = el => { const r = el.getBoundingClientRect(); return getComputedStyle(el).display !== 'none' && r.width > 0 && r.height > 0; };
  return {
    lead,
    items: items.map(e => {
      if (e.classList.contains('dayrule')) return { rule: e.textContent };
      const b = e.getBoundingClientRect();
      const cells = [...e.children];
      const sj = e.querySelector('.sj');
      return {
        h: e.dataset.commit,
        cls: cells.map(c => c.className),
        text: Object.fromEntries(cells.map(c => [c.className, c.textContent])),
        title: sj?.getAttribute('title') ?? null,
        at: !!e.querySelector('[data-at]'),
        height: b.height,
        scrollH: e.scrollHeight, clientH: e.clientHeight, scrollW: e.scrollWidth, clientW: e.clientWidth,
        // a cell with nothing rendered in it (an empty status, counts hidden and no @) may have no
        // box; any other cell has height and sits inside the row
        cellsInV: cells.every(c => {
          const r = c.getBoundingClientRect();
          const q = document.createRange();
          q.selectNodeContents(c);
          const empty = ![...q.getClientRects()].some(x => x.width > 0 && x.height > 0);
          return (r.height > 0 || empty) && r.top >= b.top - 0.5 && r.bottom <= b.bottom + 0.5;
        }),
        cellsInH: cells.every(c => c.getBoundingClientRect().right <= b.right + 0.5),
        cellsOneLine: cells.every(oneLine),
        // shown left to right in the order of the markup, no cell over the next, and every cell
        // but the subject shows all of its text
        cellsInOrder: cells.every((c, i) => i === 0 || cells[i - 1].getBoundingClientRect().right <= c.getBoundingClientRect().left + 0.5),
        cellsWhole: cells.filter(c => !c.classList.contains('sj') && !c.classList.contains('rt')).every(c => c.scrollWidth <= c.clientWidth + 0.5),
        cellW: cells.map(c => [c.className, c.scrollWidth, c.clientWidth, Math.round(c.getBoundingClientRect().left - b.left)]),
        // every cell, and everything shown in it, inside the inspector's scroll area
        inInsp: b.left >= ib.left - 0.5 && b.right <= ib.right + 0.5 && cells.every(c => {
          const r = c.getBoundingClientRect();
          return r.left >= ib.left - 0.5 && r.right <= ib.right + 0.5
            && [...c.querySelectorAll('*')].filter(shown).every(x => x.getBoundingClientRect().right <= ib.right + 0.5);
        }),
        counts: [...e.querySelectorAll('.plus, .minus')].map(shown),
        atShown: e.querySelector('[data-at]') ? shown(e.querySelector('[data-at]')) : null,
        rtWhole: [...e.querySelectorAll('.rt')].every(c => c.scrollWidth <= c.clientWidth + 0.5),
        sj: sj && { ws: getComputedStyle(sj).whiteSpace, to: getComputedStyle(sj).textOverflow, ov: getComputedStyle(sj).overflowX, cut: sj.scrollWidth > sj.clientWidth, w: sj.clientWidth },
      };
    }),
  };
});
// every computed property of an element, but those that follow the container's width
const STYLE = `el => { const cs = getComputedStyle(el); const o = {}; for (const k of cs) if (!/^(width|inline-size|perspective-origin|transform-origin)$/.test(k)) o[k] = cs.getPropertyValue(k); return o; }`;
const ruleStyle = (page, scope) => page.evaluate(([scope, STYLE]) => {
  const r = document.querySelector(scope + ' .dayrule');
  if (!r) return null;
  return { ...(0, eval)(STYLE)(r), boxHeight: r.getBoundingClientRect().height };
}, [scope, STYLE]);
const crowHeight = page => page.evaluate(() => document.querySelector('#ltable .crow')?.getBoundingClientRect().height ?? null);
// the Log's rule date text for each commit hash
const logRuleOf = (page, hs) => page.evaluate(hs => Object.fromEntries(hs.map(h => {
  let e = document.querySelector(`#ltable .crow[data-h="${h}"]`);
  const dt = e?.querySelector('.dt')?.textContent ?? null;
  while (e && !e.classList.contains('dayrule')) e = e.previousElementSibling;
  return [h, { rule: e ? e.textContent.split(' · ')[0] : null, dt }];
})), hs);

const hhmm = (page, ts) => page.evaluate(ts => ts.map(t => { const d = new Date(t * 1000); return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`; }), ts);

// One listing's content: rules, counts, order, cells.
const gradeContent = async (page, name, expected, { isFile }) => {
  const hv = await readHistory(page);
  const rows = hv.items.filter(i => i.h);
  check(`${name}: rows in today's order`, JSON.stringify(rows.map(r => r.h)) === JSON.stringify(expected.map(c => c.h)), { got: rows.length, want: expected.length });
  check(`${name}: a day rule comes first`, hv.items.length > 0 && 'rule' in hv.items[0], hv.items[0]);
  const days = await dayOf(expected.map(c => c.t));
  const dayByHash = Object.fromEntries(expected.map((c, i) => [c.h, days[i]]));
  let rule = null, under = 0, bad = [], counts = [], prev = null, repeats = 0;
  const close1 = () => { if (rule) { const n = +rule.split(' · ')[1]; if (n !== under || under === 0) counts.push({ rule, under }); } };
  for (const it of hv.items) {
    if ('rule' in it) {
      close1();
      if (prev !== null && it.rule.split(' · ')[0] === prev) repeats++;
      prev = it.rule.split(' · ')[0];
      rule = it.rule; under = 0;
      if (!/^[A-Z][a-z]{2} [A-Z][a-z]{2} \d\d \d{4} · \d+$/.test(it.rule)) bad.push(it.rule);
    } else {
      under++;
      if (!rule || rule.split(' · ')[0] !== dayByHash[it.h]) bad.push({ h: it.h, rule, day: dayByHash[it.h] });
    }
  }
  close1();
  check(`${name}: every row under its own day's rule, labelled like the Log's`, bad.length === 0, bad.slice(0, 3));
  check(`${name}: each rule counts the rows under it`, counts.length === 0, counts.slice(0, 3));
  check(`${name}: no two rules in a row for one day`, repeats === 0, repeats);
  check(`${name}: rules for ${new Set(days).size} days`, hv.items.filter(i => 'rule' in i).length >= new Set(days).size, hv.items.length);
  const times = await hhmm(page, expected.map(c => c.t));
  const cellBad = [];
  rows.forEach((r, i) => {
    const c = expected[i];
    if (!c || r.h !== c.h) return;
    const order = r.cls.join(',') === 'h,dt,st,sj,rt';
    const okText = r.text.h === c.h.slice(0, 7) && r.text.dt === times[i] && r.text.st === (c.st || '')
      && r.title === c.s && r.text.sj === c.s + (isFile && c.path && c.path !== F ? ` (${c.path})` : '')
      && r.text.rt.includes('+') && r.text.rt.includes('−');
    const noDate = !/\b(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\b|\b(19|20)\d\d\b/.test(r.text.h + r.text.dt + r.text.st + r.text.rt);
    const at = isFile ? r.at === ((c.st || '') !== 'D') : !r.at;
    if (!(order && okText && noDate && at)) cellBad.push({ i, r, want: { h: c.h, t: times[i], st: c.st, s: c.s } });
  });
  check(`${name}: each row is hash, HH:MM, status, subject, actions, with no date`, cellBad.length === 0, cellBad.slice(0, 2));
  return hv;
};

// One listing's geometry at the width the page is at now.
// At every width nothing of a row lies past the inspector's edge. Where the inspector is too narrow
// for the whole row (narrow: true) the subject has yielded and the counts are hidden; elsewhere the
// counts show. The hash, time, status and @ always show, whole.
const gradeGeometry = async (page, name, where, crow, { narrow = false } = {}) => {
  const hv = await readHistory(page);
  const rows = hv.items.filter(i => i.h);
  const bad = rows.filter(r => Math.abs(r.height - crow) > 0.01 || r.scrollH > r.clientH || !r.cellsOneLine
    || !r.cellsInV || !r.cellsInOrder || !r.cellsWhole || !r.rtWhole
    || r.scrollW > r.clientW || !r.cellsInH || !r.inInsp || r.atShown === false
    || r.counts.length !== 2 || r.counts.some(c => c === narrow)
    || (!narrow && r.sj?.w <= 0));
  check(`${name} ${where}: all ${rows.length} rows one line, ${crow}px, inside the inspector, counts ${narrow ? 'hidden' : 'shown'}`, rows.length > 0 && bad.length === 0, bad.slice(0, 2).map(({ text, title, ...r }) => r));
  const sjBad = rows.filter(r => !r.sj || r.sj.ws !== 'nowrap' || r.sj.to !== 'ellipsis' || r.sj.ov !== 'hidden' || r.title === null);
  check(`${name} ${where}: subjects end in an ellipsis, full text on hover`, sjBad.length === 0, sjBad.slice(0, 2));
  return rows;
};

if (!baseline) {
  // ---- the Log, for reference -----------------------------------------------------------------
  const pl = await open('#v=log');
  await logReady(pl);
  const crow = await crowHeight(pl);
  const logRule = await ruleStyle(pl, '#ltable');
  const logOf = await logRuleOf(pl, Fc.map(c => c.h));
  await close(pl);
  check(`the Log's rows are 21px (${crow})`, crow === 21, crow);

  // ---- a file ---------------------------------------------------------------------------------
  let p = await open('#v=graph&s=' + encodeURIComponent(F) + '&tab=history');
  await rowsReady(p, Fc.length);
  const hv = await gradeContent(p, 'file', Fc, { isFile: true });
  check('file: header and sparkline lead, above the first rule',
    hv.lead.some(l => l.tag === 'div' && l.text.startsWith('Commits on this file') && l.text.endsWith(String(Fc.length)) && l.before)
    && hv.lead.some(l => l.tag === 'svg' && l.before), hv.lead);
  const histRule = await ruleStyle(p, '#ibody');
  const ruleDiff = diff(histRule, logRule);
  check("file: the rule looks like the Log's", Array.isArray(ruleDiff) && ruleDiff.length === 0, ruleDiff);
  const rules = hv.items.reduce((a, it) => { if ('rule' in it) a.cur = it.rule.split(' · ')[0]; else a.out.push([it.h, a.cur]); return a; }, { cur: null, out: [] }).out;
  const mism = rules.filter(([h, d]) => logOf[h]?.rule !== d);
  check("file: each rule's date text is the Log's for that day", mism.length === 0, mism.slice(0, 3).map(([h, d]) => ({ h, history: d, log: logOf[h] })));
  const tmism = hv.items.filter(i => i.h && (logOf[i.h]?.dt || '').split(' ')[0] !== i.text.dt).map(i => ({ h: i.h, history: i.text.dt, log: logOf[i.h]?.dt }));
  check("file: each row's time is the Log's", tmism.length === 0, tmism.slice(0, 3));
  let w = await widths(p);
  check(`default inspector (${Math.round(w.insp)}px of ${Math.round(w.panes)})`, !w.stacked && Math.abs(w.insp / w.panes - 0.46) < 0.01, w);
  await gradeGeometry(p, 'file', 'at the default width', crow);
  await narrowest(p);
  w = await widths(p);
  check(`narrowest inspector (${Math.round(w.insp)}px of ${Math.round(w.panes)})`, !w.stacked && Math.abs(w.insp / w.panes - 0.22) < 0.005, w);
  const nrows = await gradeGeometry(p, 'file', 'at the narrowest', crow);
  check('file at the narrowest: some subject is cut', nrows.some(r => r.sj?.cut), nrows.map(r => r.sj));
  await close(p);
  // the narrowest the split allows in the narrowest window that keeps the panes side by side
  p = await open('#v=graph&s=' + encodeURIComponent(F) + '&tab=history', { viewport: NARROW });
  await rowsReady(p, Fc.length);
  await narrowest(p);
  w = await widths(p);
  check(`narrowest inspector in a ${NARROW.width}px window (${Math.round(w.insp)}px)`, !w.stacked && Math.abs(w.insp / w.panes - 0.22) < 0.005, w);
  const nrw = await gradeGeometry(p, 'file', `at the narrowest in a ${NARROW.width}px window`, crow, { narrow: true });
  check(`file at the narrowest in a ${NARROW.width}px window: @ shows on every row that has one`, nrw.filter(r => r.atShown !== null).length >= 1 && nrw.every(r => r.atShown !== false), nrw.map(r => r.atShown));
  await close(p);
  p = await open('#v=graph&s=' + encodeURIComponent(F) + '&tab=history', { viewport: STACKED });
  await rowsReady(p, Fc.length);
  w = await widths(p);
  check(`stacked panes (${Math.round(w.insp)}px of ${Math.round(w.panes)})`, w.stacked && Math.abs(w.insp - w.panes) < 1, w);
  await gradeGeometry(p, 'file', 'stacked', crow);
  await close(p);

  // clicks, each from a fresh tab
  const Fh = Fc[Math.min(1, Fc.length - 1)].h;
  p = await open('#v=graph&s=' + encodeURIComponent(F) + '&tab=history');
  await rowsReady(p, Fc.length);
  await p.locator(`#ibody [data-commit="${Fh}"] .sj`).click();
  await p.waitForFunction(h => !!document.querySelector(`#ihead [data-copysha="${h}"]`), Fh, { timeout: 15000 }).catch(() => {});
  let st = await p.evaluate(() => ({ hash: decodeURIComponent(location.hash), copy: document.querySelector('#ihead [data-copysha]')?.dataset.copysha ?? null }));
  check('clicking a row opens its commit', st.hash.includes('c=' + Fh) && st.copy === Fh, st);
  await close(p);
  const Fa = Fc.find(c => (c.st || '') !== 'D');
  p = await open('#v=graph&s=' + encodeURIComponent(F) + '&tab=history');
  await rowsReady(p, Fc.length);
  await p.locator(`#ibody [data-commit="${Fa.h}"] [data-at]`).click();
  await p.waitForFunction(() => !!document.querySelector('#ibody .travel'), null, { timeout: 15000 }).catch(() => {});
  st = await p.evaluate(() => ({ hash: decodeURIComponent(location.hash), copy: document.querySelector('#ihead [data-copysha]')?.dataset.copysha ?? null, travel: !!document.querySelector('#ibody .travel') }));
  check('clicking @ shows the file as of that commit, not the commit', st.hash.includes('rev=' + Fa.h) && !/[#&]c=/.test(st.hash) && st.copy === null && st.travel, st);
  await close(p);
  p = await open('#v=graph&s=' + encodeURIComponent(F));
  await p.keyboard.press('h');
  await p.waitForFunction(n => document.querySelectorAll('#ibody [data-commit]').length === n, Fc.length, { timeout: 15000 }).catch(() => {});
  st = await p.evaluate(() => ({ hash: decodeURIComponent(location.hash), on: document.querySelector('#itabs button.on')?.dataset.tab, rows: document.querySelectorAll('#ibody [data-commit]').length, rules: document.querySelectorAll('#ibody .dayrule').length }));
  check('h opens the History tab, with its rules', st.on === 'history' && st.hash.includes('tab=history') && st.rows === Fc.length && st.rules > 1, st);
  await close(p);

  // ---- a folder -------------------------------------------------------------------------------
  const Dc = (await getJSON('api/log?path=' + encodeURIComponent(D))).commits;
  p = await open('#v=graph&s=' + encodeURIComponent(D) + '&tab=history');
  await rowsReady(p, Dc.length);
  const dv = await gradeContent(p, 'folder', Dc, { isFile: false });
  check('folder: header leads, above the first rule', dv.lead.some(l => l.tag === 'div' && l.text.startsWith('Commits touching this folder') && l.before), dv.lead);
  await gradeGeometry(p, 'folder', 'at the default width', crow);
  await close(p);

  // ---- the repository -------------------------------------------------------------------------
  const Rc = log.commits.slice(0, 300);
  check(`the KB has more than 300 commits (${log.total}), so OPEN THE FULL LOG shows`, log.total > 300, log.total);
  const total = String(log.total).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  p = await open('#v=graph&tab=history');
  await rowsReady(p, Rc.length);
  const rv = await gradeContent(p, 'repository', Rc, { isFile: false });
  check('repository: header with its count leads, above the first rule', rv.lead.some(l => l.tag === 'div' && l.text === `Repository history ${total} commits` && l.before), rv.lead);
  await gradeGeometry(p, 'repository', 'at the default width', crow);
  // the topmost visible day's rule stays pinned, below the inspector's header
  const pins = await p.evaluate(async () => {
    const body = document.getElementById('ibody');
    const tabs = document.getElementById('itabs').getBoundingClientRect();
    const rows = [...body.querySelectorAll('[data-commit]')];
    const ruleOf = el => { let e = el; while (e && !e.classList.contains('dayrule')) e = e.previousElementSibling; return e; };
    const out = [];
    // rows that are not the first of their day: scroll each to just below the pinned rule
    const early = rows.slice(0, Math.floor(rows.length * 2 / 3));
    const picks = early.filter(r => !r.previousElementSibling?.classList.contains('dayrule')).filter((_, i, a) => i % Math.max(1, Math.floor(a.length / 6)) === 0).slice(0, 6);
    for (const r of picks) {
      const rule = ruleOf(r);
      body.scrollTop += r.getBoundingClientRect().top - body.getBoundingClientRect().top - rule.getBoundingClientRect().height - 1;
      await new Promise(res => requestAnimationFrame(() => requestAnimationFrame(res)));
      const bb = body.getBoundingClientRect();
      const atTop = document.elementFromPoint(bb.left + 30, bb.top + 3)?.closest('.dayrule');
      const top = document.elementFromPoint(bb.left + 30, bb.top + rule.getBoundingClientRect().height + 4)?.closest('[data-commit]');
      out.push({
        scroll: body.scrollTop,
        pinned: !!atTop && Math.abs(atTop.getBoundingClientRect().top - bb.top) < 0.5,
        same: !!atTop && !!top && atTop === ruleOf(top),
        clear: !!atTop && atTop.getBoundingClientRect().top >= tabs.bottom - 0.5,
        tabs: Math.abs(document.getElementById('itabs').getBoundingClientRect().top - tabs.top) < 0.5,
      });
    }
    return out;
  });
  check('repository: scrolled, the topmost day\'s rule is pinned and clear of the header', pins.length >= 3 && pins.every(x => x.scroll > 0 && x.pinned && x.same && x.clear && x.tabs), pins);
  await narrowest(p);
  await gradeGeometry(p, 'repository', 'at the narrowest', crow);
  await close(p);
  // just above the width where the counts hide: 22% of a 1450px window, every count still inside
  p = await open('#v=graph&tab=history', { viewport: { width: 1450, height: 1000 } });
  await rowsReady(p, Rc.length);
  await narrowest(p);
  w = await widths(p);
  check(`narrowest inspector in a 1450px window (${Math.round(w.insp)}px)`, Math.abs(w.insp / w.panes - 0.22) < 0.005, w);
  await gradeGeometry(p, 'repository', 'at the narrowest in a 1450px window', crow);
  await close(p);
  // the repository at the narrowest of the narrowest window
  p = await open('#v=graph&tab=history', { viewport: NARROW });
  await rowsReady(p, Rc.length);
  await narrowest(p);
  await gradeGeometry(p, 'repository', `at the narrowest in a ${NARROW.width}px window`, crow, { narrow: true });
  await close(p);
  p = await open('#v=graph&tab=history');
  await rowsReady(p, Rc.length);
  await p.locator('#ibody [data-view="log"]').click();
  await p.waitForFunction(() => !document.getElementById('v-log').hidden, null, { timeout: 15000 }).catch(() => {});
  const v = await p.evaluate(() => !document.getElementById('v-log').hidden);
  check('OPEN THE FULL LOG opens the Log', v, v);
  await close(p);
} else {
  // ---- outside History: this build against the base build ---------------------------------------
  const snap = async dist => {
    const out = {};
    for (const [where, viewport, narrow] of [['default', VIEW, false], ['narrowest', VIEW, true], ['stacked', STACKED, false]]) {
      const p = await open('#v=graph&s=' + encodeURIComponent(F), { viewport, dist });
      await p.waitForFunction(() => [...document.querySelectorAll('#ibody .sh')].some(s => s.textContent === 'Last commit'), null, { timeout: 30000 }).catch(() => {});
      if (narrow) await narrowest(p);
      out['overview ' + where] = await p.evaluate(STYLE => {
        const style = (0, eval)(STYLE);
        const b = [...document.querySelectorAll('#ibody .sect')].find(s => s.querySelector('.sh')?.textContent === 'Last commit')?.querySelector('[data-commit]');
        if (!b) return null;
        const r = b.getBoundingClientRect();
        return {
          html: b.outerHTML, height: r.height, width: r.width, style: style(b),
          cells: [...b.children].map(c => { const q = c.getBoundingClientRect(); return [c.className, c.textContent, q.left - r.left, q.top - r.top, q.width, q.height, style(c)]; }),
        };
      }, STYLE);
      if (out['overview ' + where]) {
        await p.locator('#ibody .sect [data-commit]').first().hover();
        await p.waitForTimeout(150);
        out['overview ' + where].hover = await p.evaluate(() => getComputedStyle(document.querySelector('#ibody .sect [data-commit]')).backgroundColor);
      }
      await close(p);
    }
    const p = await open('#v=log', { dist });
    await logReady(p);
    out.css = await p.evaluate(() => fetch('/app.css').then(r => r.text()).then(t => t.length));
    out.log = await p.evaluate(STYLE => {
      const t = document.getElementById('ltable');
      const tr = t.getBoundingClientRect();
      const st = sel => { const e = t.querySelector(sel); return e ? (0, eval)(STYLE)(e) : null; };
      return {
        html: document.getElementById('v-log').innerHTML,
        rows: [...t.children].slice(0, 80).map(e => { const q = e.getBoundingClientRect(); return [e.className, e.textContent, q.left - tr.left, q.top - tr.top, q.width, q.height]; }),
        rule: st('.dayrule'), crow: st('.crow'), cells: [...(t.querySelector('.crow')?.children || [])].map(c => (0, eval)(STYLE)(c)),
      };
    }, STYLE);
    out.historyDt = await (async () => {
      await go(p, '#v=graph&s=' + encodeURIComponent(F) + '&tab=history', 300);
      await rowsReady(p, Fc.length).catch(() => {});
      return p.evaluate(() => document.querySelector('#ibody [data-commit] .dt')?.textContent ?? null);
    })();
    await close(p);
    return out;
  };
  const mine = await snap(null);
  const base = await snap(baseline);
  const baseCss = readFileSync(join(baseline, 'app.css'), 'utf8').length;
  check('the base stylesheet was served', base.css === baseCss, { base: base.css, file: baseCss, mine: mine.css });
  check('the base client was served (its History rows still carry a date)', /[A-Z][a-z]{2} \d\d \d\d:\d\d/.test(base.historyDt || '') && /^\d\d:\d\d$/.test(mine.historyDt || ''), { base: base.historyDt, mine: mine.historyDt });
  for (const k of Object.keys(base).filter(k => k !== 'historyDt' && k !== 'css')) {
    // compared leaf by leaf: computed styles list their properties in no fixed order
    const flat = (o, at = '') => o && typeof o === 'object' ? Object.assign({}, ...Object.entries(o).map(([a, v]) => flat(v, at + (at ? '.' : '') + a))) : { [at]: o };
    const d = diff(flat(mine[k]), flat(base[k]));
    check(`${k}: renders as the base build does`, base[k] !== null && Object.keys(flat(base[k])).length > 20 && Array.isArray(d) && d.length === 0, Array.isArray(d) ? d.slice(0, 8) : d);
  }
}

if (errors.length) { fails++; console.log('page errors:', errors.slice(0, 5).join(' | ')); }
console.log(fails ? `${fails} failed` : 'all passed');
await browser.close();
process.exit(fails ? 1 : 0);
