// Long-running stability, for a page left open all day: the server watches a scratch clone of the
// KB; keep saving files, committing and moving around the views, and sample memory after a forced
// GC. JS heap, DOM size, listeners and WebAssembly memory should plateau, not climb.
//   node tests/ui/soak.mjs <kb-clone> [url] [rounds]
// The clone is reset to where it started when the run ends.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import fs from 'node:fs';
import { execFileSync } from 'node:child_process';

const KB = process.argv[2];
const BASE = process.argv[3] || 'http://127.0.0.1:6581/';
const ROUNDS = +(process.argv[4] || 400);
if (!KB) { console.log('usage: node tests/ui/soak.mjs <kb-clone> [url] [rounds]'); process.exit(2); }
const git = (...a) => execFileSync('git', ['-C', KB, '-c', 'user.name=T', '-c', 'user.email=t@x', ...a]).toString();
const start = git('rev-parse', 'HEAD').trim();
const files = ['memory/inbox.md', 'README.md', 'note/soak.md', 'idea/soak.md'];

const browser = await chromium.launch({ args: ['--enable-precise-memory-info'] });
const page = await (await browser.newContext({ viewport: { width: 1600, height: 1000 } })).newPage();
const errors = [];
page.on('pageerror', e => errors.push(e.message));
page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
const cdp = await page.context().newCDPSession(page);
await page.goto(BASE + '#v=graph', { waitUntil: 'load' });
await page.waitForTimeout(2500);

async function sample(round) {
  await cdp.send('HeapProfiler.collectGarbage');
  const r = await page.evaluate(async () => {
    let wasm = null; // the client's WebAssembly memory, once main.js has loaded it
    if (document.querySelector('script[src="/main.js"]')) wasm = (await (await import('/okt.js')).default()).memory.buffer.byteLength;
    return { heap: performance.memory.usedJSHeapSize, wasm };
  });
  const c = await cdp.send('Memory.getDOMCounters');
  const s = { round, heap_kb: Math.round(r.heap / 1024), wasm_kb: r.wasm == null ? null : Math.round(r.wasm / 1024), nodes: c.nodes, listeners: c.jsEventListeners };
  console.log(JSON.stringify(s));
  return s;
}

const samples = [await sample(0)];
try {
  for (let i = 1; i <= ROUNDS; i++) {
    const f = files[i % files.length];
    fs.mkdirSync(`${KB}/${f.split('/').slice(0, -1).join('/') || '.'}`, { recursive: true });
    fs.appendFileSync(`${KB}/${f}`, `\n- soak ${i}: see [[README]] and \`memory/inbox.md\`\n`);
    if (i % 10 === 0) { git('add', '-A'); git('commit', '-q', '-m', `soak ${i}`); }
    if (i % 5 === 0) {
      for (const k of ['f', 'j', 'j', 'l', 'j', 't', 'g']) { await page.keyboard.press(k); await page.waitForTimeout(60); }
    }
    if (i % 7 === 0) {
      await page.evaluate(f => { location.hash = '#v=graph&s=' + encodeURIComponent(f) + '&tab=content'; }, f);
    }
    await page.waitForTimeout(350);
    if (i % 50 === 0) samples.push(await sample(i));
  }
} finally {
  git('reset', '-q', '--hard', start);
  git('clean', '-qfd');
}
await page.waitForTimeout(1500);
const a = samples.find(x => x.round >= ROUNDS / 4) || samples[0], b = samples[samples.length - 1]; // after warm-up
console.log(`growth from round ${a.round} to ${b.round}: heap ${b.heap_kb - a.heap_kb} KB, wasm ${b.wasm_kb == null ? '-' : b.wasm_kb - a.wasm_kb + ' KB'}, nodes ${b.nodes - a.nodes}, listeners ${b.listeners - a.listeners}`);
if (errors.length) console.log('errors:', errors.slice(0, 5).join(' | '));
await browser.close();
