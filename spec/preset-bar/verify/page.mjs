// The rendered page for preset-bar, graded against a running okt (normally tests/env.sh's).
// Every figure comes from getBoundingClientRect, checkVisibility, getComputedStyle and
// elementFromPoint; ids and classes only find elements. Frozen with the plan commit.
//
//   node page.mjs [okt-url] [--groups stack,sweep,help] [--mutant <id>]
//
// Groups (all by default):
//   stack  R1, R2: at 1600 and 900 px, each preset by its key (the pane shapes checked, so a key
//          that did nothing can't pass), then a dragged and a collapsed arrangement: the top
//          panes' edge and #panes' top meet the header's bottom within 0.5 px; across the window
//          every 2 px, the pixel row 1 px above the header's bottom hits the header, and ten rows
//          from 1 to 23 px below it hit a pane or a sash; no visible element but the panes, the
//          sashes and their ancestors overlaps the band from the header's bottom to 24 px below it
//          (the header's own descendants and #panes' other children included), and no out-of-flow
//          ::before or ::after on the header, #app or #panes; no preset button.
//   sweep  R3, R4, R5, R2: an empty selection and the deepest card side by side, at 3000, 2400 and
//          1800 px, then every width from 1600 down to 400 px (both pages resized together), with
//          the KB name the live okt shows for ~/KB ("~/KB") written into #kbname on both pages: the
//          throwaway clone's temp path would otherwise take about 280 px of a header a user never
//          sees (PLAN § 3, deviation D1). At each width: the header 38 px tall with every visible child inside
//          it; from 1600 to 901 px the crumbs have a box inside the header's box, after the GO form
//          and before the chips, nothing of theirs painting outside it unclipped and hit-tested
//          over GO or a chip; at every width GO's and each chip's width and left edge the same
//          (1 px) on both pages (a difference is measured again once after 600 ms, up to five
//          times a run, a live event having redrawn one page's status), and the widths the same with the crumbs taken out of
//          layout (the counterfactual: they take no width from GO or the chips, deviation D2); at
//          900 px and below the crumbs, the brand word and the KB name hidden; at 1600 px the deep
//          card's crumbs visible, non-zero, and hit-tested at their first segment, and at every
//          width where GO leaves room of 40 px or more (the header's content box less GO and the
//          chips), the deep card's crumbs visible, filling it but for the gap before the chips, their first segment hit-tested. Every 10 px (and at
//          901 and 900), no preset button.
//   help   R6: the help overlay lists ; ' \ ` p and the w leader's keys (w, w w, w W, w d, w x,
//          w r, w q, w v, w V), each with a description.
// A preset button: a visible, on-screen element (its centre hit-tested to itself) that is
// clickable (a button, a link, role=button, an input button, data-preset, or cursor: pointer) and
// names a preset (classic, swapped, stacked, three) in its text, title or aria-label, carries a
// kbd holding a preset key, or has data-preset. In the header, any clickable outside GO, the
// crumbs and the five chips (#live, #headchip, #refresh, #setbtn, #helpbtn) counts, whatever it
// says. In regions that render KB data (the Files tree, Log, Table, Results, the Inspector's head
// and body, the crumbs, the ticker, the legend and tip) the name test alone is skipped: a file or
// commit may be called "three".
//
// --mutant <id> (gate.sh --self-test only) injects one page change from mutants.mjs before load.
// Exit 0 all passed, 1 a check failed, 2 usage or fixture.
// Footprint: one Chromium with at most two contexts, about 600 MB; wall time in TECH.md.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');

const argv = process.argv.slice(2);
const opt = k => { const i = argv.indexOf(k); return i >= 0 ? argv[i + 1] : null; };
const OKT = argv.find(a => /^https?:/.test(a)) || process.env.OKT;
if (!OKT) { console.error('page.mjs: no okt url (arg or $OKT)'); process.exit(2); }
if (/:6580\b/.test(OKT)) { console.error('page.mjs: never port 6580'); process.exit(2); }
const GROUPS = (opt('--groups') || 'stack,sweep,help').split(',');
const mutantId = opt('--mutant');
let mutant = null;
if (mutantId) {
  const { MUTANTS } = await import(new URL('./mutants.mjs', import.meta.url));
  mutant = MUTANTS[mutantId];
  if (!mutant) { console.error(`page.mjs: no mutant ${mutantId}`); process.exit(2); }
  console.log(`mutant ${mutantId}: ${mutant.what}`);
}

const idx = await (await fetch(OKT + 'api/index')).json();
const md = idx.nodes.filter(n => n.t === 'file' && n.id.endsWith('.md'));
const depth = id => id.split('/').length;
const deep = md.slice().sort((a, b) => depth(b.id) - depth(a.id) || b.id.length - a.id.length)[0]?.id;
if (!deep || depth(deep) < 3) { console.error(`page.mjs: fixture: no card three folders deep (${deep})`); process.exit(2); }
const PRESET = { classic: ';', swapped: "'", stacked: '\\', three: '`' };

let fails = 0;
const check = (label, ok, detail) => { if (!ok) fails++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok ? '' : ': ' + JSON.stringify(detail).slice(0, 900)}`); };
const browser = await chromium.launch();
const errors = [];

async function open(width, hash = '') {
  const ctx = await browser.newContext({ viewport: { width, height: 1000 } });
  const page = await ctx.newPage();
  page.on('pageerror', e => errors.push(e.message));
  // the mutant's code is the init script's own text (never eval'd: okt's CSP forbids it)
  if (mutant) await page.addInitScript({ content: `(() => {
    const go = () => {
      const css = ${JSON.stringify(mutant.css || '')};
      if (css) { const s = document.createElement('style'); s.textContent = css; document.head.appendChild(s); }
      ${mutant.js || ''}
    };
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', go); else go();
  })();` });
  await page.goto(OKT + hash, { waitUntil: 'load' });
  await page.waitForFunction(() => document.querySelectorAll('#panes .pane').length > 0 && (document.getElementById('status')?.innerText || '').trim().length > 0, null, { timeout: 15000 });
  await page.waitForTimeout(600);
  return { ctx, page };
}
const press = async (page, k, ms = 700) => { await page.keyboard.press(k); await page.waitForTimeout(ms); };

// ---- in-page probes (serialised into the page) --------------------------------------------------
const PROBES = () => {
  const vis = e => !!e && e.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true }) && e.getBoundingClientRect().width > 0 && e.getBoundingClientRect().height > 0;
  const box = e => { const r = e.getBoundingClientRect(); return { l: r.left, r: r.right, t: r.top, b: r.bottom, w: r.width, h: r.height }; };
  const header = () => document.querySelector('#app > header') || document.querySelector('header');
  const DATA = ['#tree', '#ltable', '#tgrid', '#list', '#ibody', '#ihead', '#lchips', '#lchart', '#ttrack', '#gtip', '#glegend', '#crumbs'];
  const NAMES = /\b(classic|swapped|stacked|three)\b/i;
  const KEYS = [';', "'", '\\', '`'];
  const HEADER_OK = ['#goform', '#crumbs', '#live', '#headchip', '#refresh', '#setbtn', '#helpbtn'];
  window.__pb = {
    // preset buttons on screen
    buttons() {
      const out = [];
      const h = header();
      for (const e of document.querySelectorAll('body *')) {
        if (!vis(e)) continue;
        const clickable = e.matches('button, a[href], [role=button], input[type=button], input[type=submit], [data-preset]') || getComputedStyle(e).cursor === 'pointer';
        if (!clickable) continue;
        // the header's controls are exactly GO, the crumb links and the five chips: anything else
        // clickable there (an icon-only chip titled with a key, say) counts, whatever it says
        const extra = h.contains(e) && !HEADER_OK.some(s => e.closest(s));
        const text = `${e.innerText || ''} ${e.title || ''} ${e.getAttribute('aria-label') || ''}`;
        const kbd = [...e.querySelectorAll('kbd')].concat(e.matches('kbd') ? [e] : []).some(k => KEYS.includes((k.innerText || '').trim()));
        // inside KB data only the name test is skipped: a file may be called "three"
        const named = !DATA.some(s => e.closest(s)) && NAMES.test(text);
        if (!(extra || named || kbd || e.hasAttribute('data-preset'))) continue;
        const r = e.getBoundingClientRect();
        const cx = r.left + r.width / 2, cy = r.top + r.height / 2;
        if (cx < 0 || cy < 0 || cx > innerWidth || cy > innerHeight) continue;
        const at = document.elementFromPoint(cx, cy);
        if (!at || !(e.contains(at) || at.contains(e))) continue;
        out.push(`${e.tagName.toLowerCase()}${e.id ? '#' + e.id : ''}.${[...e.classList].join('.')} "${text.trim().slice(0, 40)}"`);
      }
      return out;
    },
    // the header against the panes
    stack() {
      const h = header(), hb = box(h), P = box(document.getElementById('panes'));
      const panes = [...document.querySelectorAll('#panes .pane')].filter(vis).map(box);
      const top = Math.min(...panes.map(p => p.t));
      const above = [], below = [];
      for (let x = 1; x < innerWidth - 1; x += 2) {
        const a = document.elementFromPoint(x, hb.b - 1);
        if (!a || !h.contains(a)) above.push([x, a && a.tagName + '.' + a.className]);
        for (const dy of [1, 2, 4, 6, 8, 10, 12, 16, 20, 23]) {
          const b = document.elementFromPoint(x, hb.b + dy);
          if (!b || !(b.closest('#panes .pane') || b.closest('#panes .sash'))) below.push([x, dy, b && b.tagName + '#' + b.id + '.' + b.className]);
        }
      }
      const band = [];
      const keep = new Set([document.documentElement, document.body, h.parentElement]);
      for (const e of document.querySelectorAll('body *')) {
        if (keep.has(e) || e === h || e.closest('#panes .pane') || e.closest('#panes .sash')) continue;
        if (e.contains(h) || e.querySelector('.pane')) continue;
        if (!vis(e)) continue;
        const r = box(e);
        if (r.b > hb.b + 0.25 && r.t < hb.b + 24) band.push(`${e.tagName.toLowerCase()}#${e.id}.${[...e.classList].join('.')} ${r.t.toFixed(1)}..${r.b.toFixed(1)}`);
      }
      // a pseudo-element hung off the header, #app or #panes, out of flow, can draw a bar that no
      // element enumeration or hit test sees
      for (const e of [h, h.parentElement, document.getElementById('panes')]) for (const pe of ['::before', '::after']) {
        const cs = getComputedStyle(e, pe);
        if (cs.content !== 'none' && cs.content !== 'normal' && ['absolute', 'fixed'].includes(cs.position) && cs.display !== 'none') band.push(`${e.tagName.toLowerCase()}#${e.id}${pe} ${cs.position}`);
      }
      return { hb: hb.b, panesTop: P.t, top, n: panes.length, tops: panes.map(p => Math.round(p.t)), lefts: panes.map(p => Math.round(p.l)), above: above.slice(0, 5), nAbove: above.length, below: below.slice(0, 5), nBelow: below.length, band: band.slice(0, 5) };
    },
    // the header at this width
    head() {
      const h = header(), hb = box(h);
      const go = document.getElementById('goform'), cr = document.getElementById('crumbs');
      const chips = h.querySelector('.chips');
      const chipW = () => [...chips.children].filter(vis).map(c => c.getBoundingClientRect().width);
      const kids = [...h.children].filter(vis).map(e => ({ e: e.className || e.tagName, ...box(e) }));
      const crBox = cr.getClientRects().length > 0 ? box(cr) : null;
      // anything of the crumbs drawn outside their box, unless the box clips it
      const clip = ['hidden', 'clip'].includes(getComputedStyle(cr).overflowX);
      const spill = clip || !crBox ? [] : [...cr.querySelectorAll('*')].filter(vis).map(box).filter(r => r.l < crBox.l - 0.5 || r.r > crBox.r + 0.5);
      // GO and each chip hit-tested where they are drawn: never the crumbs
      const pts = [go, ...[...chips.children].filter(vis)].map(e => { const r = e.getBoundingClientRect(); return [r.left + 4, r.right - 4, r.left + r.width / 2].map(x => [x, r.top + r.height / 2]); }).flat();
      const over = pts.filter(([x, y]) => { const a = document.elementFromPoint(x, y); return a && cr.contains(a); }).length;
      // the first segment of the crumbs, hit-tested inside their box
      let first = false;
      const f = [...cr.children].find(vis);
      if (f && crBox && crBox.w > 0) {
        const r = box(f); const l = Math.max(r.l, crBox.l), rr = Math.min(r.r, crBox.r);
        if (rr - l >= 1) { const a = document.elementFromPoint((l + rr) / 2, r.t + r.h / 2); first = !!a && cr.contains(a); }
      }
      const shown = { crumbs: vis(cr), word: vis(h.querySelector('.word')), kb: vis(document.getElementById('kbname')) };
      const goW = go.getBoundingClientRect().width, chipsW = chipW(), groupW = chips.getBoundingClientRect().width;
      // the counterfactual: the crumbs out of layout
      const was = cr.style.getPropertyValue('display'), pri = cr.style.getPropertyPriority('display');
      cr.style.setProperty('display', 'none', 'important');
      const without = { go: go.getBoundingClientRect().width, chips: chipW(), group: chips.getBoundingClientRect().width };
      if (was) cr.style.setProperty('display', was, pri); else cr.style.removeProperty('display');
      const chipsL = [...chips.children].filter(vis).map(c => c.getBoundingClientRect().left);
      // the room GO leaves: from GO's right edge to the header's content edge, less the chips
      const room = hb.r - parseFloat(getComputedStyle(h).paddingRight) - go.getBoundingClientRect().right - chips.getBoundingClientRect().width;
      return { hb, kids, go: box(go), chipsBox: box(chips), chipsL, room, crBox, spill: spill.length, over, first, shown, goW, chipsW, groupW, without, segs: cr.querySelectorAll('button').length };
    },
  };
};

// ---- stack: R1, R2 -------------------------------------------------------------------------------
if (GROUPS.includes('stack')) {
  const r1 = [], r2 = [], shapes = [], extra = [];
  const shapeOk = (name, s) => ({
    classic: s.n === 2 && s.tops[0] === s.tops[1],
    swapped: s.n === 2 && s.tops[0] === s.tops[1],
    stacked: s.n === 2 && s.tops[0] !== s.tops[1],
    three: s.n === 3,
  })[name];
  const graded = s => Math.abs(s.panesTop - s.hb) <= 0.5 && Math.abs(s.top - s.hb) <= 0.5 && s.nAbove === 0 && s.nBelow === 0 && s.band.length === 0;
  for (const w of [1600, 900]) {
    const { ctx, page } = await open(w, '#v=graph&s=' + encodeURIComponent(deep));
    await page.evaluate(PROBES);
    let last = null;
    for (const name of ['swapped', 'stacked', 'three', 'classic']) {
      await press(page, PRESET[name]);
      const s = await page.evaluate(() => window.__pb.stack());
      const sig = JSON.stringify([s.n, s.tops, s.lefts]);
      shapes.push({ w, name, ok: shapeOk(name, s) && sig !== last, n: s.n, tops: s.tops });
      last = sig;
      r1.push({ w, name, ok: graded(s), s });
      r2.push({ w, name, b: await page.evaluate(() => window.__pb.buttons()) });
    }
    // any arrangement: a dragged split, then the focused pane collapsed
    const sash = await page.evaluate(() => { const e = [...document.querySelectorAll('#panes .sash')].find(x => x.checkVisibility()); if (!e) return null; const r = e.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
    if (sash) { await page.mouse.move(sash.x, sash.y); await page.mouse.down(); await page.mouse.move(sash.x + 90, sash.y, { steps: 6 }); await page.mouse.up(); await page.waitForTimeout(500); }
    for (const [what, keys] of [['dragged', []], ['collapsed', ['w', 'd']]]) {
      for (const k of keys) await press(page, k, 300);
      await page.waitForTimeout(400);
      const s = await page.evaluate(() => window.__pb.stack());
      extra.push({ w, what, ok: !!sash && graded(s), s });
      r2.push({ w, name: what, b: await page.evaluate(() => window.__pb.buttons()) });
    }
    await ctx.close();
  }
  check('each preset key gives its preset\'s panes at 1600 and 900 px', shapes.length === 8 && shapes.every(x => x.ok), shapes.filter(x => !x.ok));
  check('the panes meet the header with nothing between, at every preset at 1600 and 900 px', r1.length === 8 && r1.every(x => x.ok), r1.filter(x => !x.ok));
  check('the panes meet the header in a dragged and a collapsed arrangement', extra.length === 4 && extra.every(x => x.ok), extra.filter(x => !x.ok));
  check('no preset button in any arrangement at 1600 and 900 px', r2.length === 12 && r2.every(x => x.b.length === 0), r2.filter(x => x.b.length));
}

// ---- sweep: R3, R4, R5, R2 -----------------------------------------------------------------------
if (GROUPS.includes('sweep')) {
  const widths = [3000, 2400, 1800];
  for (let w = 1600; w >= 400; w--) widths.push(w);
  const A = await open(1600, ''), B = await open(1600, '#v=graph&s=' + encodeURIComponent(deep));
  await A.page.evaluate(PROBES); await B.page.evaluate(PROBES);
  const KBNAME = '~/KB';
  const named = async p => p.evaluate(n => { const e = document.getElementById('kbname'); e.textContent = n; return e.textContent; }, KBNAME);
  await named(A.page); await named(B.page);
  const r3 = [], r3h = [], r4 = [], r4cf = [], r4s = [], r5 = [], r2 = [];
  let retries = 5;   // a live event is rare; a page that differs at every width is a defect
  let at1600 = null, fixture = null;
  const t0 = Date.now();
  for (const w of widths) {
    await Promise.all([A.page.setViewportSize({ width: w, height: 1000 }), B.page.setViewportSize({ width: w, height: 1000 })]);
    // widths and places alike on both pages: GO, each chip and the chip group. A live event can
    // redraw the status (the KB name, the LIVE chip) on one page, so a difference is measured
    // again once, 600 ms later, with the KB name written again
    const alike = (a, b) => [[a.goW, b.goW], [a.groupW, b.groupW], [a.go.l, b.go.l], [a.chipsBox.l, b.chipsBox.l]].every(([u, v]) => Math.abs(u - v) <= 1)
      && a.chipsW.length === b.chipsW.length && a.chipsW.every((x, i) => Math.abs(x - b.chipsW[i]) <= 1 && Math.abs(a.chipsL[i] - b.chipsL[i]) <= 1);
    let [a, b] = await Promise.all([A.page.evaluate(() => window.__pb.head()), B.page.evaluate(() => window.__pb.head())]);
    if (!alike(a, b) && retries-- > 0) {
      await Promise.all([A.page.waitForTimeout(600), named(A.page), named(B.page)]);
      [a, b] = await Promise.all([A.page.evaluate(() => window.__pb.head()), B.page.evaluate(() => window.__pb.head())]);
    }
    if (!alike(a, b)) r4.push({ w, go: [a.go.l, a.goW, b.go.l, b.goW], chips: [a.chipsBox.l, a.chipsW, b.chipsBox.l, b.chipsW] });
    if (!fixture) fixture = { emptySegs: a.segs, deepSegs: b.segs };
    for (const [sel, m] of [['empty', a], ['deep', b]]) {
      const hdr = m.hb;
      const inRow = Math.abs(hdr.h - 38) <= 0.5 && m.kids.every(k => k.t >= hdr.t - 0.5 && k.b <= hdr.b + 0.5);
      if (!inRow) r3h.push({ w, sel, h: hdr.h, kids: m.kids.filter(k => k.t < hdr.t - 0.5 || k.b > hdr.b + 0.5) });
      if (w > 900 && w <= 1600) {
        const c = m.crBox;
        const ok = !!c && c.t >= hdr.t - 0.5 && c.b <= hdr.b + 0.5 && c.l >= hdr.l - 0.5 && c.r <= hdr.r + 0.5
          && c.l >= m.go.r - 0.5 && c.r <= m.chipsBox.l + 0.5 && m.spill === 0 && m.over === 0;
        if (!ok) r3.push({ w, sel, cr: c, go: m.go.r, chips: m.chipsBox.l, spill: m.spill, over: m.over });
      }
      if (w <= 900 && (m.shown.crumbs || m.shown.word || m.shown.kb)) r5.push({ w, sel, shown: m.shown });
      const cf = Math.abs(m.goW - m.without.go) <= 1 && Math.abs(m.groupW - m.without.group) <= 1 && m.chipsW.length === m.without.chips.length && m.chipsW.every((x, i) => Math.abs(x - m.without.chips[i]) <= 1);
      if (!cf) r4cf.push({ w, sel, go: [m.goW, m.without.go], chips: [m.chipsW, m.without.chips] });
    }
    // the deep page's crumbs fill the room GO leaves them: visible, its width less the one gap
    // before the chips, their first segment hit-tested (wherever the room is 40 px or more)
    const slot = b.room;
    if (slot >= 40 && !(b.shown.crumbs && b.crBox && b.crBox.w >= slot - 11 && b.first)) r4s.push({ w, slot, cr: b.crBox && b.crBox.w, shown: b.shown.crumbs, first: b.first });
    if (w === 1600) at1600 = { shown: b.shown.crumbs, w: b.crBox && b.crBox.w, first: b.first, kbname: await B.page.evaluate(() => document.getElementById('kbname').textContent) };
    if (w % 10 === 0 || w === 901 || w === 900) {
      const [ba, bb] = await Promise.all([A.page.evaluate(() => window.__pb.buttons()), B.page.evaluate(() => window.__pb.buttons())]);
      if (ba.length || bb.length) r2.push({ w, empty: ba, deep: bb });
    }
  }
  const secs = ((Date.now() - t0) / 1000).toFixed(0);
  await A.ctx.close(); await B.ctx.close();
  console.log(`sweep: ${widths.length} widths in ${secs} s; empty selection ${fixture.emptySegs} crumb links, ${deep} ${fixture.deepSegs}`);
  const fx = fixture.emptySegs === 0 && fixture.deepSegs >= depth(deep);
  check('the sweep\'s two pages hold no selection and the deepest card', fx, { fixture, deep });
  check('the header stays one 38 px row at every width', fx && r3h.length === 0, r3h.slice(0, 6));
  check('from 1600 to 901 px the crumbs sit in the header between GO and the chips, overlapping neither', fx && r3.length === 0, r3.slice(0, 6));
  check('GO and the chips keep their widths and places whatever the crumbs hold, at every width', fx && r4.length === 0, r4.slice(0, 6));
  check('GO and the chips keep their widths with the crumbs out of layout, at every width', fx && r4cf.length === 0, r4cf.slice(0, 6));
  check('a card selected at 1600 px shows its crumbs', fx && !!at1600 && at1600.kbname === KBNAME && at1600.shown && at1600.w > 0 && at1600.first, at1600);
  check('a selected card\'s crumbs fill the room GO leaves, at every width', fx && r4s.length === 0, r4s.slice(0, 6));
  check('at 900 px and below the crumbs, the brand word and the KB name are hidden', fx && r5.length === 0, r5.slice(0, 6));
  check('no preset button at any width of the sweep', r2.length === 0, r2.slice(0, 4));
}

// ---- help: R6 --------------------------------------------------------------------------------------
if (GROUPS.includes('help')) {
  const { ctx, page } = await open(1600);
  await press(page, '?', 400);
  const rows = await page.evaluate(() => [...document.querySelectorAll('#help dt')].filter(dt => dt.checkVisibility()).map(dt => ({ k: dt.textContent.trim().split(/\s{2,}/), d: (dt.nextElementSibling?.innerText || '').trim() })));
  const want = [';', "'", '\\', '`', 'p', 'w', 'w w', 'w W', 'w d', 'w x', 'w r', 'w q', 'w v', 'w V'];
  const missing = want.filter(k => !rows.some(r => r.k.includes(k) && r.d.length > 0));
  check('the help lists every preset key, the cycle and the pane leader\'s keys', missing.length === 0, { missing, rows: rows.slice(0, 30) });
  await ctx.close();
}

check('no page errors', errors.length === 0, errors.slice(0, 5));
console.log(fails ? `${fails} failed` : 'all passed');
await browser.close();
process.exit(fails ? 1 : 0);
