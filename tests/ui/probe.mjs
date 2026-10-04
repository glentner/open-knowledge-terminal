// Load a URL, run a few steps, print console output and an expression's value.
//   node probe.mjs <url> <js-expression-to-print> [wait-ms]
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const [url, expr, wait] = [process.argv[2], process.argv[3] || 'document.title', +(process.argv[4] || 2000)];
const browser = await chromium.launch();
const page = await (await browser.newContext({ viewport: { width: 1600, height: 1000 } })).newPage();
page.on('console', m => console.log('console.' + m.type() + ':', m.text().slice(0, 500)));
page.on('pageerror', e => console.log('pageerror:', e.message.slice(0, 500)));
await page.goto(url, { waitUntil: 'load' });
await page.waitForTimeout(wait);
const steps = process.env.STEPS ? JSON.parse(process.env.STEPS) : [];
for (const s of steps) {
  if (s.key) await page.keyboard.press(s.key);
  if (s.type) await page.keyboard.type(s.type);
  if (s.eval) await page.evaluate(s.eval);
  if (s.click) await page.mouse.click(s.click[0], s.click[1]);
  await page.waitForTimeout(s.wait || 500);
}
console.log('RESULT:', JSON.stringify(await page.evaluate(expr)).slice(0, 3000));
await browser.close();
