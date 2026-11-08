# 07 — Labels, picking and visibility

Research brief for the plan step (R5, R8, R9, outcome items: depth cues, size, compass). Read at
`8c07189`+; `file:line` refers to the worktree as of 2026-11-08.

## What exists today

- **Labels:** text is `p.label` (folder chain) or `idx.name_of(id)` = file name with `.md`
  (`graph.rs:774`, `model.rs:603`); the title (`label_of`) is used only in the tooltip
  (`graph.rs:2040`). Width is estimated, never measured: `len16 × fs × CW`, `CW = 0.61` for the mono
  font (`graph.rs:1166`, `:815`). `declutter()` (`:1218`) is a greedy priority pass over rotated
  label *segments* with a 56 px uniform grid (`:1287`), throttled to 140 ms plus a trailing 90 ms
  re-run while zooming (`declutter_soon`, `:1198`). Labels have a knockout stroke in `--graph-bg`
  (`app.css:183`).
- **Hit-testing:** `hit()` (`:1987`) walks the nodes in reverse paint order in world space,
  `reach = max(r, ring_r) + 5`. Hover shows the DOM tooltip `#gtip` (`set_hover`, `:2046`).
- **Colours:** `Colors::read()` (`:580`) reads custom properties with `theme::css_var`
  (computed style on `:root`, `theme.rs:209`). It runs once per `build()` (`:864`); a theme click
  rebuilds (`chrome.rs:362`). Values mix `#hex` and `rgba()` strings (`app.css:40–43`).
- **Size:** file radius `2.9 + min(0.55·√deg, 3.4)` (`:719`), where `deg = in + out` over
  `/api/index` edges (`model.rs:582`). `Node.lines`, `size` and `words` exist (`model.rs:61–63`).
- **Fixed pane chrome:** `.gtools` top-left (`app.css:172`), `.glegend` bottom-left (`:175`),
  `#ghint` (the walk hint) top-right (`graph.rs:1155`). The compass therefore goes **bottom-right**.

## 1. Visibility (computed on focus change, not per frame)

Precompute once per index load, at `build()` time:

- `ix: id → u32` and an adjacency list `adj: Vec<Vec<u32>>` over **link edges** (all `/api/index`
  edge kinds, both directions).
- `region: Vec<u16>`: the top-level ancestor (`ancestors().last()`). Root-level cards get a
  pseudo-region `"."`; where it sits is the layout's call.
- `pair: HashMap<(u16,u16), u32>`: cross-region link counts, keyed `min,max`, summing `Edge.n`.

The **focus F** and the **focused regions R_F**:

| selection | F (tier 0) | R_F |
|---|---|---|
| none (galaxy) | ∅ | ∅: glyphs and bundles only |
| card | {card} | {region(card)} |
| folder | {folder}; its direct children seed tier 1 | {region(folder)} |
| results `z` (R4) | the matches | regions of the matches |

**BFS** from F to depth 2 over `adj`, plus the hop-1 parent edge of a card focus. Folder nodes are
terminal: hops never pass *through* a folder, or every sibling would become tier 2. The cost is
O(V+E) on about 1.3k nodes, well under 0.1 ms. The result is `tier: Vec<u8>`, where 255 means not
drawn:

```
t = hop(c)                 // 0, 1, 2, else ∞
if c ∈ F or (t == 1)       -> t                 // focus set: drawn in ANY region (Clarifications)
elif region(c) ∈ R_F       -> min(t, 3)         // tier 2 only inside the focused region(s)
else                       -> 255               // folded into its region's glyph
```

- A 2-hop card in another region is **not drawn** (R8: outside the focused region and outside the
  focus set).
- If `|R_F| > 1` (a results focus), suppress tier 3. R8's tier 3 is "the rest of *the* focused
  region", and drawing several whole regions would bring back the busy picture.
- In show-all, every card's tier is `min(t, 3)`, and nothing is 255.

**Edges** (tiered mode). An edge draws only if both ends are drawn or it is a portal. Anything else
is counted in the bundles:

- tier0–tier1: full weight, the `--edge-hot` colour (out-links in the accent, as `:1494`); the walk
  target's edge is thicker, in the accent.
- tier1–tier1: faint (`--edge`). R8 allows it because it touches the focus set.
- tier1–tier2: dim, the "path to the focus".
- Nothing else: no tier2–tier2 edges and no tier-3 edges.
- **Portal (R5):** a tier-1 card in a region not in R_F. Its edge runs from the focus to that
  region's glyph *as drawn* (the rim position comes from the camera/layout brief). The target
  point is drawn too if it is on the pane.
- **Bundles:** at galaxy scale, every pair; at system and focus scale, pairs that touch R_F. Width
  is `w0 + w1·ln(1+n)` and alpha is `a0 + a1·ln(1+n)/ln(1+n_max)`. The shape is a quadratic Bézier
  with its control point at the midpoint pulled about 50% toward the galaxy centre (the root-bundling
  look, and it keeps antipodal chords off the core). Tessellate to 16 segments once per load, as
  static world-space geometry.
- **Show-all:** every link edge, plus faint tree edges.

## 2. Size as importance

Use **link degree**. Line count rewards long logs such as `memory/inbox.md` and says nothing about
centrality; degree is already in the client.

```
imp(c)   = ln(1 + deg) + 1.0·[name == "index.md"]
r_world  = r0 · (1 + 0.35·ln(1+deg)) · (1.4 if index.md)       clamp [r0, 3·r0]
folder   = r0 · (1.6 + 0.3·ln(1+files))                        (sub-cluster glyph)
r_px     = clamp(r_world · focal / z_view, 1.5, 14) · dpr      (perspective)
```

`imp` also orders label fill. Tiers change only alpha and saturation, never size, so importance
reads the same at every tier.

## 3. Labels (R9, the hard cap of 24)

- **Candidates:** tier 0, then tier 1 (portals included). Tiers 2 and 3 are never labelled, per the
  tier table. At galaxy scale there are no card candidates (R1). In show-all, every card is a
  candidate, still capped at 24.
- **Order:** `(tier, walk-target-first, −imp, path)`, so the result is deterministic.
- **Portals count against the 24.** R9 exempts only region names, and a portal label names a
  *card* (R5: "labelled with the target's path"). The walk target is sorted first in tier 1, so its
  portal always gets a slot.
- Hover keeps the DOM `#gtip` tooltip. It is pane chrome, like `#ghint`, not an overlay label, so it
  is not counted.

**Text:**

- Card label: `name_of` without `.md` (today it keeps `.md`; dropping it saves about 3 characters
  across 24 labels).
- Portal label: the KB-relative path, middle-ellipsised to at most 34 characters, keeping the
  region and the file name (`workspace/…/stories.md`).
- Measure with `measureText` once per `(text, font-class)` in a `HashMap` cache, cleared on
  `build()` (theme or font change). The current `CW` estimate is wrong for non-ASCII text.

**Placement:** greedy, in screen space, axis-aligned rects (labels are flat now, not rotated as
today's `--rot`).

```
obstacles = [compass rect, #gtools/#glegend/#ghint rects, tier-0 discs]
place region names first (not counted): anchors [centre-below, above, right, left];
  the focused region's name always; others by card count; a region name that collides is
  dropped for this frame (never overlap)
for c in candidates, while placed < 24:
    (x, y, z) = proj(c); skip if behind the near plane or outside the pane
    for a in [last_anchor[c]] ++ [right, left, above, below]:   // portals: outward-radial first
        rect = anchor_rect(a, x, y, r_px, w[c], h) padded 2px
        if inside pane && no AABB overlap with obstacles ∪ placed: accept, last_anchor[c]=a; break
```

A brute-force O(n²) check is fine (n ≤ 24 + about 25 region names + 6 obstacles, ×4 anchors, which
is under 10k AABB tests); the grid can go. Keep the knockout stroke.

**Stability:**

- The **label set** (which ≤ 24 cards) is solved on a focus change and at rest: 150 ms after the
  last camera change, the same idea as `declutter_soon`.
- During orbit, zoom and warp, the set is **frozen** and only anchors are re-picked each frame,
  starting with last frame's anchor (hysteresis). A frozen label that fits no anchor is hidden
  *immediately* for that frame, and no other label is swapped in until rest. So labels never
  overlap and nothing pops in mid-motion.
- New labels fade in over 120 ms (no fade under reduced motion). There are no fade-outs, since a
  fading label could overlap another.
- Mid-warp, above system altitude, hide card labels entirely.

**Test hook:** publish the placed rects each frame (for example `canvas.dataset.labels =
JSON[[x,y,w,h,kind]]`) so `tests/ui/keys.mjs` can assert R9: count of `kind=card` ≤ 24 and no
pairwise overlap.

## 4. Picking (CPU, not GPU)

The frame loop already projects every drawn point for labels and sorting. Keep
`scr: Vec<(x, y, z_view, r_px, item)>` from the last frame and reuse it on `pointermove`, coalesced
to one per rAF:

```
1. a label rect containing the pointer      -> its card (labels are on top; new: clickable)
2. drawn points with d² ≤ (r_px + 4)²       -> smallest z_view (front-most); ties → smaller d
3. cluster glyphs, d ≤ glyph halo radius    -> region (click = select the region's folder → R2)
4. nothing
```

- Only drawn items are pickable: tier ≤ 3, glyphs, portals (the portal's glyph end picks its
  target card). Bundles are not pickable.
- Cost: about 1.3k comparisons, a few µs.
- **Reject GPU colour-ID picking.** `readPixels` forces a pipeline sync, which stalls a frame on
  every move. It also needs an extra FBO pass and has antialiasing and DPR edge cases. That is all
  for a problem the CPU solves in microseconds.

## 5. Depth cues and glow (WebGL2)

**Geometry:** draw **instanced quads** (`drawArraysInstanced`, a unit quad), not `gl.POINTS`. There
is no `ALIASED_POINT_SIZE_RANGE` cap, so halos and glyphs can be large. ANGLE back ends behave the
same. One pipeline serves points, halos and glyphs.

Per instance: `pos vec3, r f32, rgb vec3, tier_alpha f32, kind u8`. Uniforms: `viewProj, focal,
dpr, zNear_set, zFar_set` (the drawn set's depth range, recomputed per frame) and `bg`.

```
t      = clamp((z_view - zNear_set)/(zFar_set - zNear_set), 0, 1)
rgb    = mix(rgb, vec3(luma(rgb)), 0.6·t)        // saturation falls with depth
rgb    = mix(rgb, bg, 0.35·t)                    // aerial fade toward --graph-bg (works light+dark)
alpha  = tier_alpha · mix(1.0, 0.35, t)
frag:  d = length(uv*2-1); a = alpha·(1 - smoothstep(0.75, 1.0, d))   // soft disc, no blur
out    = vec4(rgb·a, a)                          // premultiplied
```

**Passes** (depth test off, `blendFunc(ONE, ONE_MINUS_SRC_ALPHA)`):

1. Bundles and edges, as line quads with alpha by tier.
2. Points, **CPU-sorted back to front** by `z_view` each frame. At about 1.3k indices that is
   under 0.1 ms in WASM. Upload the sorted instance order. This makes blending correct with no
   depth buffer.
3. Halos and glyph halos. Use additive blending `(ONE, ONE)` only when `--graph-bg` is dark
   (luma < 0.5); otherwise "over" with the accent at low alpha. Additive glow disappears on the
   paper theme (`#fbfaf7`).

Keep the focus halo **static**. A pulse forces continuous rAF, costs power, and pollutes the R11
idle profile. If one is added, gate it off under `prefers-reduced-motion` (`app.css:513`).

**Colours:**

- Keep `Colors::read()` and parse into `[f32; 4]`. To canonicalise, assign the string to the
  overlay context's `fillStyle` and read it back (`#rrggbb` or `rgba(...)`), then parse.
- Re-read on `build()` as today.
- Add tokens to `app.css` with the graph rules, which remain the spec: `--graph-halo`,
  `--graph-bundle`, `--graph-fog` (0.35), `--graph-t2-alpha` (about 0.45), `--graph-t3-alpha`
  (about 0.18).
- Rewrite the `.n .lab` rules as overlay label rules: `--glab-fs: 10.5px`, the t0 weight 700 in
  `--accent-text`, the region name at 12px weight 600 in `--ink`, the portal in `--accent-text`.

## 6. Compass (overlay, bottom-right, 56 px, 10 px inset; a label obstacle)

- Three short axis lines: the galaxy's world X, Y (the Fibonacci pole, "up") and Z, rotated by the
  camera's *view rotation only*. The axis pointing away from the viewer is drawn dimmer and under
  the others, which gives the same depth cue as the scene. Lines use `--ink-2`; letters are
  9px mono in `--dim`.
- A ring for the galaxy sphere's horizon, with a dot for the **focused region's direction** (galaxy
  centre → R_F centroid, in view rotation) in that region's family colour.
- A one-word scale readout underneath (`galaxy · system · focus`).
- This encodes the GOAL's "the galaxy's orientation and where the camera is in it" with no space
  styling.
- Optional: clicking the compass resets θ and ψ (map-style "north up"). It is pointer-only, so R7's
  key rule does not apply.
