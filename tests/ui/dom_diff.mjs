// Drive the reference (terminal.py) and okt through the same interactions and compare the HTML
// of the panes after each step (the graph itself is compared by screenshots, not here). Steps in
// DEVIATIONS cover okt's deliberate fixes (README, Known differences): those must differ.
//   node tests/ui/dom_diff.mjs [--only name,...] [--show N]
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');

const REF = process.env.REF || 'http://127.0.0.1:8376/';
const OKT = process.env.OKT || 'http://127.0.0.1:6580/';
const args = process.argv.slice(2);
const only = args.includes('--only') ? args[args.indexOf('--only') + 1].split(',') : null;
const SHOW = args.includes('--show') ? +args[args.indexOf('--show') + 1] : 3;

const hash = h => async p => { await p.evaluate(x => { location.hash = x; }, h); };
const key = (...ks) => async p => { for (const k of ks) { await p.keyboard.press(k); await p.waitForTimeout(60); } };
const go = text => async p => { await p.keyboard.press('/'); await p.keyboard.type(text); await p.keyboard.press('Enter'); };
const typeIn = (sel, text) => async p => { await p.fill(sel, text); };
const click = sel => async p => { await p.click(sel); };

// [name, action, selectors to compare, wait ms]
const STEPS = [
  ['start', async () => {}, ['#ihead', '#ibody', '#crumbs', '#glegend', '#itabs', '#headchip', '#kbname'], 600],
  ['select', hash('#v=graph&s=contact%2Findex.md'), ['#ihead', '#ibody', '#crumbs', '#itabs', '#ghint'], 900],
  ['content', key('c'), ['#ibody'], 900],
  ['history', key('h'), ['#ibody'], 900],
  ['folder', hash('#v=graph&s=contact%2F'), ['#ihead', '#ibody', '#crumbs'], 900],
  ['folder-content', key('c'), ['#ibody'], 900],
  ['folder-history', key('h'), ['#ibody'], 900],
  ['root-history', hash('#v=graph&tab=history'), ['#ihead', '#ibody'], 900],
  ['surface', hash('#v=graph&s=%40web'), ['#ihead', '#ibody', '#crumbs'], 900],
  ['files', async p => { await p.evaluate(() => { location.hash = '#v=files'; }); }, ['#tree', '#fmeta'], 900],
  ['files-select', hash('#v=files&s=memory%2Finbox.md'), ['#tree', '#ihead'], 900],
  ['files-jk', key('j', 'j', 'j', 'k'), ['#tree', '#ihead', '#crumbs'], 900],
  ['files-left', key('ArrowLeft'), ['#tree', '#ihead'], 700],
  ['files-enter', key('Enter'), ['#tree'], 700],
  ['files-filter', typeIn('#ffilter', 'pearc site'), ['#tree', '#fmeta'], 700],
  ['log', async p => { await p.fill('#ffilter', ''); await p.evaluate(() => { location.hash = '#v=log'; }); }, ['#lchips', '#ltable', '#lmeta'], 1200],
  ['log-chip', click('#lchips .pchip:nth-child(2)'), ['#lchips', '#ltable', '#lmeta'], 700],
  ['log-filter', async p => { await p.click('#lchips .pchip.on'); await p.fill('#lfilter', 'pearc'); }, ['#ltable', '#lmeta'], 900],
  ['log-jk', async p => { await p.fill('#lfilter', ''); await p.waitForTimeout(300); await p.click('#ltable'); await p.keyboard.press('j'); await p.waitForTimeout(100); await p.keyboard.press('j'); }, ['#ltable', '#ihead', '#ibody'], 1200],
  ['table', hash('#v=table&t=contact%2F'), ['#tgrid', '#tmeta', '#tfold', '#tcolpick'], 1200],
  ['table-sort', click('#tgrid th[data-k="team"]'), ['#tgrid'], 700],
  ['table-sort2', click('#tgrid th[data-k="team"]'), ['#tgrid'], 700],
  ['table-sort-name', click('#tgrid th[data-k="_name"]'), ['#tgrid'], 700],
  ['table-sort-lt', click('#tgrid th[data-k="_lt"]'), ['#tgrid'], 700],
  ['table-filter', typeIn('#tfilter', 'team:research'), ['#tgrid', '#tmeta'], 800],
  ['table-cols', async p => { await p.fill('#tfilter', ''); await p.click('#tcols'); await p.click('#tcolpick .pchip[data-col="email"]'); }, ['#tgrid', '#tcolpick'], 900],
  ['table-rec', hash('#v=table&t=workspace%2F'), ['#tgrid', '#tmeta', '#tfold'], 1500],
  ['find', go('FIND pearc'), ['#list', '#listtitle', '#listmeta'], 1500],
  ['find-re', go('FIND /rcac-[a-z]+/'), ['#list', '#listtitle', '#listmeta'], 1500],
  ['find-click', click('#list .lrow[data-ln]'), ['#ihead', '#ibody'], 1500],
  ['broken', go('BROKEN'), ['#list', '#listmeta'], 700],
  ['dirty', go('DIRTY'), ['#list', '#listmeta'], 700],
  ['recent', go('RECENT'), ['#list', '#listmeta'], 700],
  ['orphans', go('ORPHANS'), ['#list', '#listmeta'], 700],
  ['hubs', go('HUBS'), ['#list', '#listmeta'], 700],
  ['type', go('TYPE person'), ['#list', '#listmeta', '#listtitle'], 700],
  ['show-head', go('SHOW HEAD'), ['#ihead', '#ibody'], 1500],
  ['commit-older', go('SHOW HEAD~7'), ['#ihead', '#ibody'], 1500],
  ['code', hash('#v=files&s=bin%2Fkb-lint&tab=content'), ['#ihead', '#ibody'], 1500],
  ['blame', key('b'), ['#ibody'], 2500],
  ['md-blame', hash('#v=files&s=skill%2Fcheckin.md&tab=content'), ['#ibody'], 2500],
  ['md-source', click('[data-mode="source"]'), ['#ibody'], 1200],
  ['at', go('AT HEAD~20'), ['#ihead', '#ibody'], 2000],
  ['at-older', key(','), ['#ihead', '#ibody'], 2000],
  ['live', key('Escape'), ['#ihead', '#ibody'], 1500],
  ['csv', hash('#v=files&s=workspace%2Fpearc28%2Fresearch%2Flocation%2Fscores.csv&tab=content'), ['#ibody'], 1500],
  ['csv-sort', click('.dtable th[data-dk="1"]'), ['#ibody'], 900],
  ['outline', async p => { await p.evaluate(() => { location.hash = '#v=graph&s=contact%2Findex.md'; }); await p.waitForTimeout(900); await p.click('.outline button:nth-of-type(2)'); }, ['#ibody'], 1500],
  ['go-sug', async p => { await p.keyboard.press('/'); await p.keyboard.type('mem doc'); }, ['#gosug'], 700],
  ['go-sug-cmd', async p => { await p.fill('#go', 'TABLE con'); await p.dispatchEvent('#go', 'input'); }, ['#gosug'], 700],
  ['go-sug-rev', async p => { await p.fill('#go', 'SHOW head'); await p.dispatchEvent('#go', 'input'); }, ['#gosug'], 700],
  ['go-sug-type', async p => { await p.fill('#go', 'TYPE pe'); await p.dispatchEvent('#go', 'input'); }, ['#gosug'], 700],
  ['go-sug-accent', async p => { await p.fill('#go', 'ACCENT g'); await p.dispatchEvent('#go', 'input'); }, ['#gosug'], 700],
  ['go-sug-at', async p => { await p.fill('#go', 'AT HEAD'); await p.dispatchEvent('#go', 'input'); }, ['#gosug'], 700],
  ['go-sug-theme', async p => { await p.fill('#go', 'THEME d'); await p.dispatchEvent('#go', 'input'); }, ['#gosug'], 700],
  ['go-sug-path', async p => { await p.fill('#go', 'OPEN inbox'); await p.dispatchEvent('#go', 'input'); }, ['#gosug'], 700],
  ['go-sug-hash', async p => { await p.fill('#go', 'eb46'); await p.dispatchEvent('#go', 'input'); }, ['#gosug'], 700],
  ['go-sug-empty', async p => { await p.fill('#go', ''); await p.dispatchEvent('#go', 'input'); }, ['#gosug'], 700],
  ['help', async p => { await p.keyboard.press('Escape'); await p.keyboard.press('?'); }, ['#help'], 600],
  ['settings', async p => { await p.keyboard.press('Escape'); await p.click('#setbtn'); }, ['#settings'], 600],
  ['theme-dark', async p => { await p.click('#settings [data-theme="dark"]'); }, ['#settings'], 900],
];

// the reference lists only the command itself for these arguments (a dangling else); okt suggests
const DEVIATIONS = {
  'go-sug-rev': 'SHOW: the rev as typed, then matching commits',
  'go-sug-type': 'TYPE: matching frontmatter types',
  'go-sug-accent': 'ACCENT: matching accent names',
  'go-sug-at': 'AT: the rev as typed, then matching commits',
  help: 'the keys okt adds (space, u in files)',
};

function norm(s, sel) {
  if (s == null) return s;
  s = s.replace(/ class="nw"/g, '');  // okt marks short markdown table cells (README, Known differences)
  s = s.replace(/\b(index <b>)\d+(<\/b> ms · )\d\d:\d\d/g, '$1N$2HH:MM');
  s = s.replace(/<b>\d\d:\d\d:\d\d<\/b>/g, '<b>HH:MM:SS</b>');
  s = s.replace(/KB Terminal|Open Knowledge Terminal/g, 'APP');
  s = s.replace(/OK TERMINAL|KB TERMINAL/g, 'APP');
  s = s.replace(/bin\/terminal\.py --detach|okt --detach/g, 'START');
  s = s.replace(/(APP) [0-9][0-9.]* · editor/g, '$1 V · editor');
  s = s.replace(/the script opens it|okt opens it/g, 'X opens it');
  return s;
}

function firstDiff(a, b) {
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  return i;
}

const browser = await chromium.launch();
async function open(base) {
  const ctx = await browser.newContext({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 1, acceptDownloads: false });
  const page = await ctx.newPage();
  const errors = [];
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('pageerror', e => errors.push('pageerror: ' + e.message));
  await page.goto(base, { waitUntil: 'load' });
  await page.waitForTimeout(1500);
  return { page, errors };
}
const ref = await open(REF), okt = await open(OKT);
let fails = 0, steps = 0, devs = 0;
for (const [name, act, sels, wait] of STEPS) {
  if (only && !only.includes(name)) {
    // still perform the step so later steps see the same state
  }
  steps++;
  try {
    await Promise.all([act(ref.page), act(okt.page)]);
  } catch (e) {
    console.log(`STEP ${name}: action failed: ${e.message.split('\n')[0]}`);
  }
  await Promise.all([ref.page.waitForTimeout(wait), okt.page.waitForTimeout(wait)]);
  if (only && !only.includes(name)) continue;
  const grab = p => p.evaluate(sels => sels.map(s => { const el = document.querySelector(s); return el ? (el.hidden ? '[hidden]' : '') + el.innerHTML : null; }), sels);
  const [ra, oa] = await Promise.all([grab(ref.page), grab(okt.page)]);
  const bad = [];
  sels.forEach((s, i) => {
    const a = norm(ra[i], s), b = norm(oa[i], s);
    if (a !== b) bad.push([s, a, b]);
  });
  if (name in DEVIATIONS) {
    if (bad.length) {
      devs++;
      const rows = name.startsWith('go-sug') ? await okt.page.evaluate(() => [...document.querySelectorAll('#gosug .gs')].slice(0, 3).map(e => e.innerText.replace(/\n/g, ' '))) : [];
      console.log(`deviation ${name} (${DEVIATIONS[name]})${rows.length ? ': okt ' + JSON.stringify(rows) : ''}`);
    } else {
      fails++;
      console.log(`NO DEVIATION ${name}: expected ${DEVIATIONS[name]}`);
    }
  } else if (bad.length) {
    fails++;
    console.log(`DIFF ${name}: ${bad.map(x => x[0]).join(', ')}`);
    for (const [s, a, b] of bad.slice(0, SHOW)) {
      if (a == null || b == null) { console.log(`   ${s}: ref ${a == null ? 'missing' : 'present'} okt ${b == null ? 'missing' : 'present'}`); continue; }
      const i = firstDiff(a, b);
      console.log(`   ${s} @${i} (len ${a.length} vs ${b.length})`);
      console.log(`     ref: ${JSON.stringify(a.slice(Math.max(0, i - 120), i + 180))}`);
      console.log(`     okt: ${JSON.stringify(b.slice(Math.max(0, i - 120), i + 180))}`);
    }
  } else {
    console.log(`same ${name}`);
  }
}
for (const [n, x] of [['ref', ref], ['okt', okt]]) if (x.errors.length) console.log(`${n} console errors:`, x.errors.slice(0, 8).join(' | '));
console.log(`steps ${steps}, differing ${fails}, deliberate deviations ${devs}/${Object.keys(DEVIATIONS).length}`);
await browser.close();
