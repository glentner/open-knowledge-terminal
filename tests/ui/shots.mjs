// Side-by-side screenshots of the reference (terminal.py) and okt for a list of scenarios.
//   node tests/ui/shots.mjs [outdir] [scenario...]
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const OUT = process.argv[2] || '/tmp/okt-shots';
const only = process.argv.slice(3);
const REF = process.env.REF || 'http://127.0.0.1:8376/';
const OKT = process.env.OKT || 'http://127.0.0.1:6580/';
import fs from 'node:fs';
fs.mkdirSync(OUT, { recursive: true });

const scenarios = {
  graph: async p => {},
  sel: async p => { await p.evaluate(() => { location.hash = '#v=graph&s=contact%2Findex.md'; }); await p.waitForTimeout(1200); },
  content: async p => { await p.evaluate(() => { location.hash = '#v=graph&s=contact%2Findex.md&tab=content'; }); await p.waitForTimeout(1500); },
  files: async p => { await p.keyboard.press('f'); await p.waitForTimeout(800); },
  log: async p => { await p.keyboard.press('l'); await p.waitForTimeout(1200); },
  table: async p => { await p.evaluate(() => { location.hash = '#v=table&t=contact%2F'; }); await p.waitForTimeout(1500); },
  history: async p => { await p.evaluate(() => { location.hash = '#v=files&s=AGENTS.md&tab=history'; }); await p.waitForTimeout(1500); },
  code: async p => { await p.evaluate(() => { location.hash = '#v=files&s=bin%2Fkb-lint&tab=content'; }); await p.waitForTimeout(1500); },
  help: async p => { await p.keyboard.press('?'); await p.waitForTimeout(500); },
  go: async p => { await p.keyboard.press('/'); await p.keyboard.type('pearc'); await p.waitForTimeout(600); },
};

const browser = await chromium.launch();
for (const [name, base] of [['ref', REF], ['okt', OKT]]) {
  for (const [sc, fn] of Object.entries(scenarios)) {
    if (only.length && !only.includes(sc)) continue;
    const ctx = await browser.newContext({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 1 });
    const page = await ctx.newPage();
    const errors = [];
    page.on('console', m => { if (m.type() === 'error' || m.type() === 'warning') errors.push(m.text()); });
    page.on('pageerror', e => errors.push('pageerror: ' + e.message));
    await page.goto(base, { waitUntil: 'load' });
    await page.waitForTimeout(1500);
    await page.mouse.move(5, 995);
    await fn(page);
    await page.screenshot({ path: `${OUT}/${sc}-${name}.png` });
    if (errors.length) console.log(name, sc, 'errors:', errors.slice(0, 5).join(' | '));
    await ctx.close();
  }
}
await browser.close();
console.log('done');
