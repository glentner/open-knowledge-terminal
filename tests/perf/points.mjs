// Screen points of a spread of file nodes, read from the reference's SVG at 1920x1080 (fit view).
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import fs from 'node:fs';
const browser = await chromium.launch();
const page = await (await browser.newContext({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 })).newPage();
await page.goto(process.argv[2] || 'http://127.0.0.1:8376/', { waitUntil: 'load' });
await page.waitForSelector('#graph .n');
await page.waitForTimeout(800);
const pts = await page.evaluate(() => [...document.querySelectorAll('#graph .n.file .shape')].filter((_, i) => i % 37 === 5).map(s => { const r = s.getBoundingClientRect(); return [Math.round(r.x + r.width / 2), Math.round(r.y + r.height / 2)]; }));
fs.writeFileSync(new URL('./points.json', import.meta.url), JSON.stringify(pts));
console.log(pts.length, 'points');
await browser.close();
