// The inspector's History tab, checked against a running okt on a file, a
// folder and Repository history:
//   ROWS    one line per commit, in the API's order: short hash, HH:MM (the Log's time), status,
//           subject (cut with an ellipsis, full on hover), counts and, on a file, @; no date
//   RULES   the Log's day rule (date_string · N) before each run of one local day, N the rows under
//           it, styled as the Log's; scrolled, the topmost day's rule stays pinned under the tabs
//   WIDTHS  every row 21px, the Log's row height, at the default split, the narrowest split in a
//           1600, 1450 and 901px window, and stacked; nothing past the inspector's edge, and where
//           it is too narrow the counts hide while hash, time, status and @ stay; counts of up to 14
//           characters show whole wherever they show, wider ones end in an ellipsis, full on hover
//   LARGE   a commit with counts of 14 characters (+12,345 −6,789), made in the throwaway clone,
//           shows its counts whole in Repository history and its file's History, at the default
//           split and the widest
//   BAND    with the scrollbars the app draws, the split moved 2px at a time from its narrowest to
//           past the width where the counts hide, in a 901 and a 1280px window, on a file and on
//           Repository history: on every row at every step nothing lies past the inspector's edge
//           or its own box, the counts are hidden only where the subject would have no room beside
//           them, and shown counts of up to 14 characters are whole
//   CLICKS  a row opens its commit, @ shows the file as of that commit, h opens History, OPEN THE
//           FULL LOG opens the Log
//   node tests/ui/history.mjs [okt-url]
// Needs a KB with more than 300 commits and a recent file with commits on two days, two on one, and
// OKT_KB, the clone tests/env.sh serves: the suite commits its large-counts file there, and refuses
// to run against ~/KB itself.
import { execFileSync } from 'node:child_process';
import { mkdirSync, realpathSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');

const OKT = process.argv[2] || process.env.OKT || 'http://127.0.0.1:6580/';
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

// ---- a commit with counts of 14 characters ------------------------------------------------------
// The widest counts the actions cell holds whole: a file of 6,789 lines, then all of it replaced by
// 12,345 others ("+12,345 −6,789"). Both commits go into OKT_KB, the throwaway clone, never into the
// KB it was cloned from; the file has two commits on one day, so the file under test below is never
// this one.
const real = p => { try { return realpathSync(p); } catch { return null; } };
const KB = process.env.OKT_KB ? real(process.env.OKT_KB) : null;
const homeKB = real(join(homedir(), 'KB'));
const git = (...a) => execFileSync('git', ['-C', KB, '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null',
  '-c', 'user.name=okt tests', '-c', 'user.email=tests@okt.invalid', ...a], { encoding: 'utf8' }).trim();
const clone = KB && real(git('rev-parse', '--show-toplevel')) === KB
  && real(git('rev-parse', '--path-format=absolute', '--git-common-dir')) !== (homeKB && real(join(homeKB, '.git')))
  && KB !== homeKB && !(homeKB && KB.startsWith(homeKB + '/'));
check(`OKT_KB is a throwaway clone, not ~/KB (${process.env.OKT_KB ?? 'unset'})`, !!clone, { KB, homeKB });
if (!clone) process.exit(1);
const BIG = 'okt-tests/large-counts.md';
const BIGTEXT = '+12,345 −6,789';
const lines = (tag, n) => Array.from({ length: n }, (_, i) => `${tag} ${i + 1}`).join('\n') + '\n';
mkdirSync(join(KB, 'okt-tests'), { recursive: true });
writeFileSync(join(KB, BIG), lines('first', 6789));
git('add', '--', BIG);
git('commit', '-q', '-m', 'Large counts: a file of 6,789 lines', '--', BIG);
writeFileSync(join(KB, BIG), lines('second', 12345));
git('commit', '-q', '-m', 'Large counts: 12,345 lines in place of 6,789', '--', BIG);
const BIGH = git('rev-parse', 'HEAD');
// the server watches the clone: wait until its log leads with the commit and its index has the file
let seenBig = null;
for (let i = 0; i < 120 && !seenBig; i++) {
  const [l, x] = [await getJSON('api/log'), await getJSON('api/index')];
  if (l.commits[0]?.h === BIGH && x.nodes.some(n => n.id === BIG)) seenBig = l.commits[0];
  else await new Promise(r => setTimeout(r, 500));
}
check(`the server lists the large-counts commit (${BIGH.slice(0, 7)}, +${seenBig?.a} −${seenBig?.d})`, seenBig?.a === 12345 && seenBig?.d === 6789, seenBig);
if (!seenBig) process.exit(1);

const idx = await getJSON('api/index');
const files = new Set(idx.nodes.filter(n => n.t === 'file').map(n => n.id));
const log = await getJSON('api/log');
const logPos = new Map(log.commits.map((c, i) => [c.h, i]));

const browser = await chromium.launch();
const errors = [];
const open = async (hash, { viewport = VIEW, via = browser } = {}) => {
  const ctx = await via.newContext({ viewport, serviceWorkers: 'block' });
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
// the split dragged as far left as it goes: the inspector at its widest
const widest = async page => {
  const s = await page.locator('#split').boundingBox();
  const p = await page.locator('#panes').boundingBox();
  await page.mouse.move(s.x + s.width / 2, s.y + s.height / 2);
  await page.mouse.down();
  await page.mouse.move(p.x + 2, s.y + s.height / 2, { steps: 8 });
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
        // the box holding the counts: its text, its hover text, how it cuts, and whether the text
        // fits it (shown counts of up to 14 characters must)
        ad: (a => a && { text: a.textContent, title: a.getAttribute('title'), ws: getComputedStyle(a).whiteSpace, to: getComputedStyle(a).textOverflow, ov: getComputedStyle(a).overflowX,
          shown: shown(a), whole: a.scrollWidth <= a.clientWidth, sw: a.scrollWidth, cw: a.clientWidth })(e.querySelector('.rt .ad')),
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
  const adBad = rows.filter(r => !r.ad || r.ad.ws !== 'nowrap' || r.ad.to !== 'ellipsis' || r.ad.ov !== 'hidden' || r.ad.title !== r.ad.text);
  check(`${name} ${where}: counts cut with an ellipsis when wider than their box, full text on hover`, adBad.length === 0, adBad.slice(0, 2).map(r => r.ad));
  const cutBad = rows.filter(r => r.ad?.shown && r.ad.text.length <= 14 && !r.ad.whole);
  check(`${name} ${where}: shown counts of up to 14 characters are whole`, cutBad.length === 0, cutBad.slice(0, 2).map(r => ({ h: r.h, ...r.ad })));
  return rows;
};

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
// above the width where the counts hide: 22% of a 1450px window, every count still inside
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

// ---- counts of 14 characters, whole -------------------------------------------------------------
// The large-counts commit, in Repository history (it leads it) and in its file's History, at the
// default split (46%) and the widest (75%): its counts show, whole, with no ellipsis.
const bigRow = page => page.evaluate(h => {
  const r = document.querySelector(`#ibody [data-commit="${h}"]`);
  const a = r?.querySelector('.rt .ad');
  if (!a) return null;
  const q = document.createRange();
  q.selectNodeContents(a);
  return {
    text: a.textContent, title: a.getAttribute('title'),
    shown: [...a.querySelectorAll('.plus, .minus')].every(x => getComputedStyle(x).display !== 'none' && x.getBoundingClientRect().width > 0),
    sw: a.scrollWidth, cw: a.clientWidth, box: a.getBoundingClientRect().width, textW: q.getBoundingClientRect().width,
  };
}, BIGH);
for (const [name, hash, n] of [['repository', '#v=graph&tab=history', Rc.length], ['large-counts file', '#v=graph&s=' + encodeURIComponent(BIG) + '&tab=history', 2]]) {
  for (const [where, at, share] of [['at the default width', null, 0.46], ['at the widest', widest, 0.75]]) {
    p = await open(hash);
    await rowsReady(p, n);
    if (at) await at(p);
    w = await widths(p);
    const b = await bigRow(p);
    check(`${name} ${where} (${Math.round(w.insp)}px, ${Math.round(100 * w.insp / w.panes)}%): ${BIGTEXT} shows whole (text ${b?.textW?.toFixed(2)}px in a ${b?.box}px box)`,
      !w.stacked && Math.abs(w.insp / w.panes - share) < 0.01 && !!b && b.text === BIGTEXT && b.title === BIGTEXT && b.shown
      && b.sw <= b.cw && b.textW <= b.box + 0.01, { w, b });
    await close(p);
  }
}

// ---- the band where the counts hide -------------------------------------------------------------
// Headless Chromium hides scrollbars unless told otherwise; the app draws 10px ones, and they take
// their width from #ibody, so the band is swept with them drawn. One drag holds the split from its
// narrowest and moves it 2px at a time (so a fault 3px wide is never stepped over) until every row
// has shown its counts for 16 steps (32px).
// Each row's actions cell is measured first at the default split, with its counts shown: at a step
// where the counts are hidden, the subject's width less what the counts would add back is the room
// it would have beside them, and that must not be above 0.
const drawn = await chromium.launch({ ignoreDefaultArgs: ['--hide-scrollbars'] });
const sweep = async (name, hash, n, viewport) => {
  p = await open(hash, { viewport, via: drawn });
  await rowsReady(p, n);
  const wide = await p.evaluate(() => Object.fromEntries([...document.querySelectorAll('#ibody [data-commit]')]
    .map(r => [r.dataset.commit, r.querySelector('.rt').getBoundingClientRect().width])));
  const s = await p.locator('#split').boundingBox();
  const pn = await p.locator('#panes').boundingBox();
  const y = s.y + s.height / 2;
  await p.mouse.move(s.x + s.width / 2, y);
  await p.mouse.down();
  const steps = [];
  let shownFor = 0;
  // from just right of the 22% clamp (so the narrowest), leftwards, never past 75%
  for (let x = pn.x + pn.width * 0.78 + 2; x > pn.x + pn.width * 0.25 && shownFor < 16; x -= 2) {
    await p.mouse.move(x, y);
    await p.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
    const st = await p.evaluate(wide => {
      const body = document.getElementById('ibody');
      // the scroll area's content edge, left of a drawn scrollbar
      const edge = body.getBoundingClientRect().left + body.clientLeft + body.clientWidth;
      const shown = el => getComputedStyle(el).display !== 'none' && el.getBoundingClientRect().width > 0;
      const out = { w: body.clientWidth, bar: body.offsetWidth - body.clientWidth - body.clientLeft, rows: 0, hidden: 0, past: [], over: [], crowded: [], cut: [] };
      for (const r of body.querySelectorAll('[data-commit]')) {
        out.rows++;
        const parts = [r, ...r.querySelectorAll('*')].filter(shown);
        const right = Math.max(...parts.map(e => e.getBoundingClientRect().right));
        if (right > edge + 0.5) out.past.push({ h: r.dataset.commit, over: +(right - edge).toFixed(2) });
        // and every cell inside the row's own box, left of its right padding
        const inner = r.getBoundingClientRect().right - parseFloat(getComputedStyle(r).paddingRight);
        const cells = Math.max(...[...r.children].filter(shown).map(e => e.getBoundingClientRect().right));
        if (cells > inner + 0.5) out.over.push({ h: r.dataset.commit, over: +(cells - inner).toFixed(2) });
        const counts = [...r.querySelectorAll('.plus, .minus')];
        const ad = r.querySelector('.rt .ad');
        if (ad && shown(ad) && ad.textContent.length <= 14 && ad.scrollWidth > ad.clientWidth) out.cut.push({ h: r.dataset.commit, text: ad.textContent, sw: ad.scrollWidth, cw: ad.clientWidth });
        if (counts.length !== 2 || counts.every(shown)) continue;
        out.hidden++;
        const room = r.querySelector('.sj').getBoundingClientRect().width - (wide[r.dataset.commit] - r.querySelector('.rt').getBoundingClientRect().width);
        if (room > 0.5) out.crowded.push({ h: r.dataset.commit, room: +room.toFixed(2) });
      }
      return out;
    }, wide);
    steps.push(st);
    shownFor = st.rows > 0 && st.hidden === 0 ? shownFor + 1 : 0;
  }
  await p.mouse.up();
  await close(p);
  const at = `${name} in a ${viewport.width}px window, ${steps.length} steps from ${steps[0]?.w}px to ${steps.at(-1)?.w}px`;
  const past = steps.filter(x => x.past.length);
  const crowded = steps.filter(x => x.crowded.length);
  check(`${at}: the band holds the width where the counts hide, and every step has all ${n} rows`,
    steps.length >= 16 && steps[0].hidden === n && shownFor >= 16 && steps.every(x => x.rows === n),
    steps.map(x => [x.w, x.rows, x.hidden]));
  check(`${at}: no part of any row past the inspector's edge`, steps.length > 0 && past.length === 0, past.slice(0, 3).map(x => ({ w: x.w, n: x.past.length, rows: x.past.slice(0, 2) })));
  const over = steps.filter(x => x.over.length);
  check(`${at}: every row's cells inside its own box`, steps.length > 0 && over.length === 0, over.slice(0, 3).map(x => ({ w: x.w, n: x.over.length, rows: x.over.slice(0, 2) })));
  check(`${at}: counts hidden only where the subject has no room beside them`, steps.length > 0 && crowded.length === 0, crowded.slice(0, 3).map(x => ({ w: x.w, n: x.crowded.length, rows: x.crowded.slice(0, 2) })));
  const cut = steps.filter(x => x.cut.length);
  check(`${at}: shown counts of up to 14 characters are whole`, steps.length > 0 && cut.length === 0, cut.slice(0, 3).map(x => ({ w: x.w, n: x.cut.length, rows: x.cut.slice(0, 2) })));
  return steps;
};
const band = await sweep('repository', '#v=graph&tab=history', Rc.length, NARROW);
check('the band is swept with the scrollbars the app draws', band.length > 0 && band.every(x => x.bar === 10), band.map(x => x.bar));
await sweep('repository', '#v=graph&tab=history', Rc.length, { width: 1280, height: 1000 });
await sweep('file', '#v=graph&s=' + encodeURIComponent(F) + '&tab=history', Fc.length, NARROW);
await sweep('file', '#v=graph&s=' + encodeURIComponent(F) + '&tab=history', Fc.length, { width: 1280, height: 1000 });
await drawn.close();

if (errors.length) { fails++; console.log('page errors:', errors.slice(0, 5).join(' | ')); }
console.log(fails ? `${fails} failed` : 'all passed');
await browser.close();
process.exit(fails ? 1 : 0);
