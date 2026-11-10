// The drop oracle: what a drop does, written from the drag-to-dock GOAL's wording and PLAN
// § 2.2 (the zones), independently of the client's code. Pure: no DOM, no Playwright. dock.mjs
// grades the rendered page against it; dock-oracle-test.mjs checks it on hand-worked cases.
//
// A tree is the saved arrangement's root (PLAN § 2.1, `arrange::to_json`):
//   NODE = { split: 'row' | 'col', f, kids: [A, B] } | { views: [...], shown, collapsed }
// Panes are named by their index in reading order (depth first, first child first), which is
// also the order of `#panes .pane` in the document.

export const TITLE = 24;   // a collapsed pane's extent, and a pane's title bar height
export const MIN_W = 200;  // the side-by-side floor
export const MIN_H = 120;  // the stacked floor
export const BAND = 0.25;  // an edge zone's depth, as a share of the pane body's width or height
export const ZONES = ['centre', 'left', 'right', 'top', 'bottom'];
// the points dock.mjs aims at in each pane: the five zones, and a point in the title bar
export const AIMS = [...ZONES, 'title'];
// the zone an aim point is in: a collapsed pane is all centre, and so is a title bar
export const zoneOf = (point, collapsed) => (collapsed || point === 'title') ? 'centre' : point;

const isPane = n => !n.split;
const clone = n => JSON.parse(JSON.stringify(n));

// Every pane in reading order, with its parent split and its index there.
export function leaves(root) {
  const out = [];
  const walk = (n, parent, i) => {
    if (isPane(n)) { out.push({ node: n, parent, i }); return; }
    walk(n.kids[0], n, 0);
    walk(n.kids[1], n, 1);
  };
  walk(root, null, -1);
  return out;
}

// Each pane's [x, y, w, h] in a W x H root, in reading order: a split's children take f and 1 - f
// of its extent, or a collapsed child takes the title bar's 24 px and its sibling the rest.
export function layout(root, W, H) {
  const out = [];
  const folded = k => isPane(k) && k.collapsed;
  const tile = (n, x, y, w, h) => {
    if (isPane(n)) { out.push([x, y, w, h]); return; }
    const row = n.split === 'row';
    const e = row ? w : h;
    const lead = folded(n.kids[0]) ? TITLE : folded(n.kids[1]) ? e - TITLE : n.f * e;
    if (row) { tile(n.kids[0], x, y, lead, h); tile(n.kids[1], x + lead, y, w - lead, h); }
    else { tile(n.kids[0], x, y, w, lead); tile(n.kids[1], x, y + lead, w, h - lead); }
  };
  tile(root, 0, 0, W, H);
  return out;
}

// The zone of a pane at a point (PLAN § 2.2). A collapsed pane, and any pane's title bar, is all
// centre. Below the title bar, a point within a quarter of the body's width of its left or right
// side, or a quarter of its height of its top or bottom, is that edge's zone (the nearest edge,
// by share, when two bands meet; ties go left, right, top, bottom); anywhere else is the centre.
export function zoneAt([x, y, w, h], collapsed, px, py) {
  if (collapsed || py < y + TITLE || h - TITLE <= 0 || w <= 0) return 'centre';
  const by = y + TITLE, bh = h - TITLE;
  const share = { left: (px - x) / w, right: (x + w - px) / w, top: (py - by) / bh, bottom: (by + bh - py) / bh };
  let best = 'centre', least = BAND;
  for (const z of ['left', 'right', 'top', 'bottom']) if (share[z] < least) { least = share[z]; best = z; }
  return best;
}

// The pane and zone under a point given relative to the root, or null outside every pane.
export function aim(root, W, H, px, py) {
  const rs = layout(root, W, H);
  const ls = leaves(root);
  const i = rs.findIndex(([x, y, w, h]) => px >= x && px < x + w && py >= y && py < y + h);
  if (i < 0) return null;
  return { pane: i, zone: zoneAt(rs[i], ls[i].node.collapsed, px, py) };
}

// Where dock.mjs points for each zone of a pane: the body's centre, and a tenth of the body's width
// or height in from each side, on the body's middle lines; and `title`, 8 px from the title bar's
// right end, at its middle (the title bar is the centre zone). For a collapsed pane the same tenths
// of its bar or strip (all of it is the centre zone), and `title` its centre.
export function aimPoints([x, y, w, h], collapsed) {
  const [bx, by, bw, bh] = collapsed ? [x, y, w, h] : [x, y + TITLE, w, h - TITLE];
  const mx = bx + bw / 2, my = by + bh / 2;
  return {
    centre: [mx, my],
    left: [bx + bw / 10, my],
    right: [bx + bw * 9 / 10, my],
    top: [mx, by + bh / 10],
    bottom: [mx, by + bh * 9 / 10],
    title: collapsed ? [mx, my] : [x + w - 8, y + TITLE / 2],
  };
}

// The rules a tree must keep: no empty pane, no view twice, no split holding two collapsed panes,
// no collapsed root. Returns the problems found.
export function broken(root) {
  const bad = [];
  if (isPane(root) && root.collapsed) bad.push('collapsed root');
  const seen = new Set();
  const walk = n => {
    if (isPane(n)) {
      if (!n.views.length) bad.push('empty pane');
      for (const v of n.views) { if (seen.has(v)) bad.push(`${v} twice`); seen.add(v); }
      if (!n.views.includes(n.shown)) bad.push(`shows ${n.shown}, not held`);
      return;
    }
    if (isPane(n.kids[0]) && isPane(n.kids[1]) && n.kids[0].collapsed && n.kids[1].collapsed) bad.push('two collapsed panes in one split');
    walk(n.kids[0]); walk(n.kids[1]);
  };
  walk(root);
  return bad;
}

// The drop of view `v` on zone `zone` of pane `t` (a reading-order index) in a W x H root.
// Returns null when the zone is invalid (the release changes nothing), else
// { root, pane, rect }: the tree after the drop, the reading-order index of the pane holding v in
// it, and that pane's rect (what the overlay must show).
export function drop(before, v, t, zone, W, H) {
  const root = clone(before);
  const ls = leaves(root);
  const src = ls.find(l => l.node.views.includes(v));
  const tgt = ls[t];
  if (!src || !tgt || !ZONES.includes(zone)) return null;
  const own = src.node === tgt.node;
  if (zone === 'centre' && own) return null;                       // its own pane's centre
  if (zone !== 'centre' && tgt.node.collapsed) return null;        // a collapsed pane offers its centre only
  if (zone !== 'centre' && own && src.node.views.length === 1) return null; // nothing to split from
  // the view leaves its pane; if it was shown there, the view after it (or the first) shows
  const s = src.node;
  const at = s.views.indexOf(v);
  s.views.splice(at, 1);
  if (s.shown === v && s.views.length) s.shown = s.views[at < s.views.length ? at : 0];
  const made = { views: [v], shown: v, collapsed: false };
  if (zone === 'centre') {
    // joins as the last tab and the shown one, the pane expanded
    tgt.node.views.push(v); tgt.node.shown = v; tgt.node.collapsed = false;
  } else {
    // the target splits on that side, half and half; the new pane takes the side
    const row = zone === 'left' || zone === 'right';
    const first = zone === 'left' || zone === 'top';
    const old = tgt.node;
    const split = { split: row ? 'row' : 'col', f: 0.5, kids: first ? [made, old] : [old, made] };
    if (tgt.parent) tgt.parent.kids[tgt.i] = split;
    else return finish(split, null, v, zone, made, old, W, H);
    return finish(root, s.views.length ? null : src, v, zone, made, old, W, H);
  }
  return finish(root, s.views.length ? null : src, v, zone, null, null, W, H);
}

function finish(root, emptied, v, zone, made, old, W, H) {
  // an emptied source pane closes, its sibling taking its space
  if (emptied) {
    const p = emptied.parent;
    if (!p) return null; // the only pane can't be emptied by a valid drop
    const other = p.kids[1 - emptied.i];
    // replace p by `other` wherever p sits
    const up = findParent(root, p);
    if (!up) root = other;
    else up.node.kids[up.i] = other;
  }
  // the arrangement rules hold after the drop, or the zone is invalid
  if (broken(root).length) return null;
  const rs = layout(root, W, H);
  const ls = leaves(root);
  if (made) {
    const row = zone === 'left' || zone === 'right';
    const floor = row ? MIN_W : MIN_H;
    for (const n of [made, old]) {
      const r = rs[ls.findIndex(l => l.node === n)];
      if ((row ? r[2] : r[3]) < floor) return null;
    }
  }
  const pane = ls.findIndex(l => l.node.views.includes(v));
  return { root, pane, rect: rs[pane] };
}

function findParent(root, target) {
  let found = null;
  const walk = n => {
    if (isPane(n) || found) return;
    for (const i of [0, 1]) {
      if (n.kids[i] === target) { found = { node: n, i }; return; }
      walk(n.kids[i]);
    }
  };
  walk(root);
  return found;
}

// The panes as [views, shown, collapsed, x, y, w, h] in reading order: what the page must show.
export function picture(root, W, H) {
  const rs = layout(root, W, H);
  return leaves(root).map((l, i) => [l.node.views.slice(), l.node.shown, !!l.node.collapsed, ...rs[i]]);
}

// The presets as the keys apply them (arrange.rs `classic`, `swapped`, `stacked`, `three`), with
// classic at its default share.
const V5 = ['graph', 'files', 'log', 'table', 'list'];
export const PRESETS = {
  classic: { split: 'row', f: 0.54, kids: [{ views: V5, shown: 'graph', collapsed: false }, { views: ['insp'], shown: 'insp', collapsed: false }] },
  swapped: { split: 'row', f: 0.46, kids: [{ views: ['insp'], shown: 'insp', collapsed: false }, { views: V5, shown: 'graph', collapsed: false }] },
  stacked: { split: 'col', f: 0.5, kids: [{ views: V5, shown: 'graph', collapsed: false }, { views: ['insp'], shown: 'insp', collapsed: false }] },
  three: { split: 'col', f: 0.4, kids: [{ views: ['graph'], shown: 'graph', collapsed: true }, { split: 'row', f: 0.5, kids: [{ views: ['insp'], shown: 'insp', collapsed: false }, { views: ['files', 'log', 'table', 'list'], shown: 'files', collapsed: false }] }] },
};
