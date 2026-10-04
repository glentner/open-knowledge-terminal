// Keys okt adds that have no counterpart in the reference (README, Known differences), checked on
// their own: in FILES, `u` walks up to the parent folder and folds everything under it, space
// opens or closes a folder (closing folds everything inside) and shows a file in the graph.
//   node tests/ui/keys.mjs [okt-url]
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');

const OKT = process.argv[2] || process.env.OKT || 'http://127.0.0.1:6580/';
const idx = await (await fetch(OKT + 'api/index')).json();
const dirs = new Set(idx.nodes.filter(n => n.t === 'dir').map(n => n.id));
const parent = Object.fromEntries(idx.nodes.map(n => [n.id, n.parent]));
const ancestors = id => { const out = []; for (let p = parent[id]; p && p !== '.'; p = parent[p]) out.push(p); return out; };
// the deepest file, to walk up from; a folder with a subfolder, to fold
const deep = idx.nodes.filter(n => n.t === 'file').sort((a, b) => ancestors(b.id).length - ancestors(a.id).length)[0].id;
const outer = [...dirs].find(d => [...dirs].some(s => parent[s] === d && [...dirs].some(t => parent[t] === s)));
const inner = [...dirs].find(s => parent[s] === outer && [...dirs].some(t => parent[t] === s));

const browser = await chromium.launch();
const page = await (await browser.newContext({ viewport: { width: 1600, height: 1000 } })).newPage();
const errors = [];
page.on('pageerror', e => errors.push(e.message));
const go = async hash => { await page.evaluate(h => { location.hash = h; }, hash); await page.waitForTimeout(600); };
const key = async k => { await page.keyboard.press(k); await page.waitForTimeout(250); };
const state = () => page.evaluate(() => {
  const rows = [...document.querySelectorAll('#tree .trow:not(.thead)')];
  return {
    open: rows.filter(r => r.querySelector('.tw')?.textContent === '▾').map(r => r.dataset.id),
    on: rows.find(r => r.classList.contains('on'))?.dataset.id ?? null,
    view: ['graph', 'files', 'log', 'table', 'list'].find(v => !document.getElementById('v-' + v).hidden),
  };
});
let fails = 0;
const check = (label, ok, detail) => { if (!ok) fails++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok ? '' : ': ' + JSON.stringify(detail)}`); };

await page.goto(OKT, { waitUntil: 'load' });
await page.waitForTimeout(2000);
await go('#v=files&s=' + encodeURIComponent(deep));
let st = await state();
check(`${deep}: its folders open`, ancestors(deep).every(a => st.open.includes(a)), st.open);

// u: up one folder at a time, folding everything under the new selection
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

// space folds a folder and everything inside it
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

// space on a file shows it in the graph
await go('#v=files&s=' + encodeURIComponent(deep));
await key(' ');
st = await state();
const sel = await page.evaluate(() => decodeURIComponent((location.hash.match(/[#&]s=([^&]*)/) || [])[1] || ''));
check('space on a file shows the graph', st.view === 'graph' && sel === deep, { view: st.view, sel });

if (errors.length) { fails++; console.log('page errors:', errors.slice(0, 5).join(' | ')); }
console.log(fails ? `${fails} failed` : 'all passed');
await browser.close();
process.exit(fails ? 1 : 0);
