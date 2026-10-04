// Live updates: both servers watch the same scratch clone; change files and commit, and compare
// what the two pages show afterwards (toasts, status, inspector, ticker, tree).
//   node tests/ui/live_diff.mjs <kb-clone> [ref-url] [okt-url]
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import fs from 'node:fs';
import { execFileSync } from 'node:child_process';

const KB = process.argv[2];
const REF = process.argv[3] || 'http://127.0.0.1:8378/';
const OKT = process.argv[4] || 'http://127.0.0.1:6581/';
const git = (...a) => execFileSync('git', ['-C', KB, '-c', 'user.name=T', '-c', 'user.email=t@x', ...a]).toString();

const browser = await chromium.launch();
async function open(base) {
  const page = await (await browser.newContext({ viewport: { width: 1600, height: 1000 } })).newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  await page.goto(base + '#v=files&s=memory%2Finbox.md&tab=content', { waitUntil: 'load' });
  await page.waitForTimeout(2000);
  // record every toast (they expire; the servers notice changes at different speeds)
  await page.evaluate(() => {
    window.__toasts = [];
    new MutationObserver(ms => { for (const m of ms) for (const n of m.addedNodes) if (n.classList?.contains('toast')) window.__toasts.push(n.innerText.replace(/\n/g, ' | ')); })
      .observe(document.getElementById('toasts'), { childList: true });
  });
  return { page, errors };
}
const ref = await open(REF), okt = await open(OKT);
const norm = s => (s || '').replace(/<b>\d\d:\d\d:\d\d<\/b>/g, '').replace(/index <b>\d+<\/b> ms · \d\d:\d\d/g, 'index').replace(/LIVE · 2s|LIVE/g, 'LIVE');
const SELS = ['#status', '#ibody', '#ttrack', '#tree', '#ihead'];
async function compare(label) {
  const grab = p => p.evaluate(sels => sels.map(s => document.querySelector(s)?.innerHTML ?? null), SELS);
  const [a, b] = await Promise.all([grab(ref.page), grab(okt.page)]);
  const [ta, tb] = await Promise.all([ref.page.evaluate(() => window.__toasts.splice(0)), okt.page.evaluate(() => window.__toasts.splice(0))]);
  if (JSON.stringify(ta) !== JSON.stringify(tb)) console.log(`   toasts differ:\n     ref: ${JSON.stringify(ta)}\n     okt: ${JSON.stringify(tb)}`);
  else console.log(`   toasts: ${JSON.stringify(ta)}`);
  const bad = SELS.filter((s, i) => norm(a[i]) !== norm(b[i]));
  console.log(bad.length ? `DIFF ${label}: ${bad.join(', ')}` : `same ${label}`);
  for (const s of bad) {
    const i = SELS.indexOf(s), x = norm(a[i]), y = norm(b[i]);
    let k = 0; while (k < x.length && x[k] === y[k]) k++;
    console.log(`   ${s} @${k}\n     ref: ${JSON.stringify(x.slice(Math.max(0, k - 100), k + 160))}\n     okt: ${JSON.stringify(y.slice(Math.max(0, k - 100), k + 160))}`);
  }
}
const wait = ms => new Promise(r => setTimeout(r, ms));
const inbox = `${KB}/memory/inbox.md`;
const orig = fs.readFileSync(inbox, 'utf8');

fs.appendFileSync(inbox, '\n- live test line one\n');
await wait(4500); await compare('save selected file');
fs.writeFileSync(`${KB}/idea/zz-live.md`, '# Live\n\nA new idea linking [[inbox]].\n');
await wait(4500); await compare('new file');
git('add', '-A'); git('commit', '-q', '-m', '[memory] live test commit');
await wait(5000); await compare('commit');
fs.unlinkSync(`${KB}/idea/zz-live.md`); fs.writeFileSync(inbox, orig);
git('commit', '-qam', '[memory] undo live test');
await wait(5000); await compare('delete + commit');
git('reset', '-q', '--hard', 'HEAD~2');
await wait(5000); await compare('reset');
for (const [n, x] of [['ref', ref], ['okt', okt]]) if (x.errors.length) console.log(n, 'errors:', x.errors.slice(0, 5).join(' | '));
await browser.close();
