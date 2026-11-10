// Hand-worked cases for dock-oracle.mjs: each expected tree and rect is worked from GOAL.md's wording, not
// from the oracle. Node only, no browser: `node spec/drag-to-dock/verify/dock-oracle-test.mjs`.
// Prints "ok   <label>" / "FAIL <label>: <detail>"; exits 1 on any FAIL.
import { drop, aim, aimPoints, zoneAt, layout, leaves, broken, PRESETS, ZONES, AIMS, zoneOf } from './dock-oracle.mjs';

let fails = 0;
const check = (label, ok, detail) => { if (!ok) fails++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok ? '' : ': ' + JSON.stringify(detail)}`); };
const eq = (a, b) => JSON.stringify(a, (k, x) => typeof x === 'number' ? Math.round(x * 1e6) / 1e6 : x) === JSON.stringify(b, (k, x) => typeof x === 'number' ? Math.round(x * 1e6) / 1e6 : x);
const P = (views, shown, collapsed = false) => ({ views, shown, collapsed });
const V5 = ['graph', 'files', 'log', 'table', 'list'];
const W = 1600, H = 890;

// 1. classic: the Inspector, alone in its pane, onto the main pane's left edge. Its pane closes
// (R5), the main pane takes the whole root, and splits there (R4): the Inspector's half is 800 wide.
{
  const r = drop(PRESETS.classic, 'insp', 0, 'left', W, H);
  check('the last view onto a sibling edge: source closes, the grown target splits in half', r && eq(r.root, { split: 'row', f: 0.5, kids: [P(['insp'], 'insp'), P(V5, 'graph')] }) && r.pane === 0 && eq(r.rect, [0, 0, 800, 890]), r);
}
// 2. classic: the Inspector onto the main pane's centre: joins last and shown; the pane is the root.
{
  const r = drop(PRESETS.classic, 'insp', 0, 'centre', W, H);
  check('the last view onto a sibling centre joins it and the target takes the root', r && eq(r.root, P([...V5, 'insp'], 'insp')) && eq(r.rect, [0, 0, 1600, 890]), r);
}
// 3. classic: the Log (not shown) onto the Inspector's centre: the main pane keeps showing the Graph.
{
  const r = drop(PRESETS.classic, 'log', 1, 'centre', W, H);
  check('a tab that is not shown leaves its pane showing what it showed', r && eq(r.root, { split: 'row', f: 0.54, kids: [P(['graph', 'files', 'table', 'list'], 'graph'), P(['insp', 'log'], 'log')] }) && r.pane === 1 && eq(r.rect, [864, 0, 736, 890]), r);
}
// 4. classic: the Graph (shown) onto its own pane's right edge: the pane shows Files, the view
// after the Graph, and splits; the Graph takes the right half of 864.
{
  const r = drop(PRESETS.classic, 'graph', 0, 'right', W, H);
  const want = { split: 'row', f: 0.54, kids: [{ split: 'row', f: 0.5, kids: [P(['files', 'log', 'table', 'list'], 'files'), P(['graph'], 'graph')] }, P(['insp'], 'insp')] };
  check('its own pane\'s edge splits it when it holds other views', r && eq(r.root, want) && r.pane === 1 && eq(r.rect, [432, 0, 432, 890]), r);
}
// 5-6. no zone of a pane holding only the dragged view; not its own pane's centre.
check('no zone of its own pane is valid when it holds only the dragged view', ZONES.every(z => drop(PRESETS.classic, 'insp', 1, z, W, H) === null), {});
check('its own pane\'s centre is never valid', drop(PRESETS.classic, 'graph', 0, 'centre', W, H) === null, {});
// the last of a pane's views shown: the first shows after it leaves
{
  const t = { split: 'row', f: 0.5, kids: [P(['graph', 'files', 'list'], 'list'), P(['insp'], 'insp')] };
  const r = drop(t, 'list', 1, 'centre', W, H);
  check('the last tab shown leaves: the first shows', r && r.root.kids[0].shown === 'graph', r);
}
// 7. three: the Graph's bar is collapsed: its edges are not zones (all of it is centre), and a
// join expands it at its 40% share: 356 of 890.
{
  const ok = ['left', 'right', 'top', 'bottom'].every(z => drop(PRESETS.three, 'files', 0, z, W, H) === null);
  const r = drop(PRESETS.three, 'files', 0, 'centre', W, H);
  check('a collapsed pane offers only its centre, and a join expands it', ok && r && eq(r.root.kids[0], P(['graph', 'files'], 'files')) && eq(r.root.kids[1].kids[1], P(['log', 'table', 'list'], 'log')) && eq(r.rect, [0, 0, 1600, 356]), r);
}
// 8. three: the Graph (alone, collapsed) onto Files' top edge: its bar closes, the row takes the
// root, Files splits top and bottom: the Graph's half is [800, 0, 800, 445], expanded.
{
  const r = drop(PRESETS.three, 'graph', 2, 'top', W, H);
  const want = { split: 'row', f: 0.5, kids: [P(['insp'], 'insp'), { split: 'col', f: 0.5, kids: [P(['graph'], 'graph'), P(['files', 'log', 'table', 'list'], 'files')] }] };
  check('a collapsed pane\'s only view onto an edge: the bar closes, the new pane is expanded', r && eq(r.root, want) && eq(r.rect, [800, 0, 800, 445]), r);
}
// 9. the floors, at 900 wide: classic's main pane (486) splits into 243 + 243; a 243 px pane is
// too narrow to split side by side (121.5 < 200) and tall enough to stack (445 + 445).
{
  const a = drop(PRESETS.classic, 'list', 0, 'right', 900, H);
  const narrow = a && ['left', 'right'].every(z => drop(a.root, 'log', 1, z, 900, H) === null);
  const tall = a && ['top', 'bottom'].every(z => drop(a.root, 'log', 1, z, 900, H) !== null);
  check('a split narrower than 200 px is refused, a stack 120 px or taller is not', a && eq(a.rect, [243, 0, 243, 890]) && narrow && tall, a);
  // stacked at 890 high: 445 → 222.5 → 111.25 < 120
  const b = drop(PRESETS.stacked, 'list', 0, 'bottom', 1600, H);
  const c = b && drop(b.root, 'log', 1, 'top', 1600, H);
  check('a stack shorter than 120 px is refused', b && eq(b.rect, [0, 222.5, 1600, 222.5]) && c === null, { b, c });
  // the boundary: a pane exactly 400 wide splits into two 200s
  const t = { split: 'row', f: 0.25, kids: [P(['graph', 'files'], 'graph'), P(['insp'], 'insp')] };
  check('a pane exactly 400 px wide still splits', drop(t, 'insp', 0, 'right', 1600, H) !== null && drop(t, 'files', 0, 'right', 1599, H) === null, {});
}
// 10. the accordion: a source pane closing beside a collapsed pane, whose new sibling is another
// collapsed pane, would leave two collapsed panes in one split: refused.
{
  const t = { split: 'row', f: 0.5, kids: [{ split: 'col', f: 0.5, kids: [P(['graph'], 'graph', true), { split: 'row', f: 0.5, kids: [P(['files'], 'files'), P(['log'], 'log', true)] }] }, P(['table', 'list', 'insp'], 'table')] };
  check('the fixture keeps the rules', broken(t).length === 0, broken(t));
  check('a drop that leaves two collapsed panes in one split is refused', ZONES.every(z => drop(t, 'files', 3, z, W, H) === null), {});
  check('another drop from the same tree is not', drop(t, 'table', 1, 'centre', W, H) !== null, {});
}
// 11. stacked: the Inspector onto the main pane's top edge: the main pane takes the root and
// splits in half (Q12's 240 px rule read on the result, PLAN deviation D2).
{
  const r = drop(PRESETS.stacked, 'insp', 0, 'top', W, H);
  check('stacked: the Inspector on top of the main pane takes the top half of the root', r && eq(r.rect, [0, 0, 1600, 445]), r);
}
// the neighbour equivalence the keys define: w v from classic's main pane (Graph to the Inspector)
{
  const r = drop(PRESETS.classic, 'graph', 1, 'centre', W, H);
  check('a centre drop on the neighbour is w v', r && eq(r.root, { split: 'row', f: 0.54, kids: [P(['files', 'log', 'table', 'list'], 'files'), P(['insp', 'graph'], 'graph')] }), r);
}
// every result keeps the rules, and each view is held exactly once (a sweep over the presets)
{
  let n = 0, bad = [];
  for (const [name, t] of Object.entries(PRESETS)) for (const [w, h] of [[1600, 890], [900, 890], [1600, 918], [900, 918]]) {
    for (const v of ['graph', 'files', 'log', 'table', 'list', 'insp']) for (let i = 0; i < leaves(t).length; i++) for (const z of ZONES) {
      const r = drop(t, v, i, z, w, h);
      if (!r) continue;
      n++;
      const held = leaves(r.root).flatMap(l => l.node.views).sort().join();
      if (broken(r.root).length || held !== ['files', 'graph', 'insp', 'list', 'log', 'table'].join() || !eq(layout(r.root, w, h)[r.pane], r.rect)) bad.push([name, w, v, i, z]);
    }
  }
  check(`every valid preset drop keeps the rules (${n} drops)`, n > 100 && bad.length === 0, bad.slice(0, 5));
}
// the zones: each aim point lands in its own zone, in every pane of every preset at both widths
{
  const bad = [];
  for (const [name, t] of Object.entries(PRESETS)) for (const w of [1600, 900]) {
    const rs = layout(t, w, H), ls = leaves(t);
    rs.forEach((r, i) => {
      const pts = aimPoints(r, ls[i].node.collapsed);
      for (const z of AIMS) {
        const a = aim(t, w, H, ...pts[z]);
        const want = zoneOf(z, ls[i].node.collapsed);
        if (!a || a.pane !== i || a.zone !== want) bad.push([name, w, i, z, a]);
      }
    });
  }
  check('each aim point is in its pane and zone, the title bar\'s in the centre', bad.length === 0, bad.slice(0, 5));
  check('the title bar is the centre', zoneAt([0, 0, 864, 890], false, 5, 10) === 'centre', {});
  check('a point near a corner takes the nearer edge', zoneAt([864, 0, 736, 890], false, 1000, 860) === 'bottom' && zoneAt([0, 0, 864, 890], false, 50, 400) === 'left', {});
  check('outside the root is no pane', aim(PRESETS.classic, W, H, 1600, 10) === null && aim(PRESETS.classic, W, H, 10, -1) === null, {});
}
console.log(fails ? `oracle: ${fails} failed` : 'oracle: all passed');
process.exit(fails ? 1 : 0);
