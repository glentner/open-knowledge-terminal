# PLAN — Drag a view's tab to dock it in another pane

> **Status:** Draft for review · **Last updated:** 2026-11-09
> **Authoritative technical design.** The *how*. Vision/contract is [`GOAL.md`](GOAL.md);
> the phased executable roadmap is [`TECH.md`](TECH.md). Backing detail is in
> [`research/`](research/). Every design element traces to a GOAL R-ID.

## 1. Summary

A drop is one pure function on the arrangement tree in `okt-client/src/arrange.rs`: it builds the
whole result on a copy, or says the zone is invalid. The overlay shows the dropped view's pane in
that result, laid out by the same pure tiling the page draws, and the release hands that result to
`nav::operate`, as the `w` keys do. So the overlay is the prediction by construction, a refusal
changes nothing by construction, and there is one model. The pointer side in `panes.rs` is a small
state machine beside the sash drag: Pending, Dragging, Cancelled. It captures the pointer at a
4 px threshold, swallows the click that follows every drag, and cancels on Esc, any other key,
`blur`, `pointercancel` or lost capture. The gate grades every property with one checker per
property, swept over its whole domain: every preset at both widths, every pane, every zone, every
source pane (its own included), collapsed panes and the Inspector. An independent JS oracle
(`verify/dock-oracle.mjs`) is written from the GOAL's words, and a frozen probe (`verify/dock.mjs`)
drives real pointer drags against it. Four phases: the pure model, the interaction and overlay,
focus and persistence, then the suite.

**Built on preset-bar.** `preset-bar` lands on `main` first. It removes the `nav.fkeys` row,
`#presets` and the preset marking in `panes::apply` (`panes.rs:125-129`, the `#presets
[data-preset]` loop), and moves `#crumbs` into the header, so `#panes` starts 28 px higher and is
918 px tall at 1000 px. This plan is written for the code it leaves. Build's P1 begins by rebasing
`feat/drag-to-dock` onto `main` once preset-bar has landed (TECH § Conventions). Nothing here reads
the preset row, and no measurement hard-codes `#panes`'s height.

## 2. Design

### 2.1 The arrangement (unchanged) and its JSON

`arrange::Node` / `Pane` / `Outcome` / `PaneId` as they stand. The saved form is
`arrange::to_json`'s: `{"v":1,"root":NODE}`, where `NODE` is
`{"split":"row"|"col","f":…,"kids":[A,B]}` or `{"views":[…],"shown":…,"collapsed":…}`. It is
stored under `arrangement` in localStorage `okt:<kb>` once `arr_saved` is set. Panes in reading
order (depth first, first child first) are also the document order of `#panes .pane`
(`panes::build`). The probe and the oracle name panes by that index.

### 2.2 Zones, aim and layout (pure, `arrange.rs`)

New public items, all pure:

- `pub struct Rect { x, y, w, h }` (f64), and `pub fn layout(root: &Node, w: f64, h: f64) ->
  Vec<(PaneId, Rect)>`. This is the test helper `rects` promoted (`arrange.rs` tests module,
  `fn rects`/`fn layout`): a split's first child takes `f·E` and the second `E − f·E`; a collapsed
  child takes `TITLE` (24 px) and its sibling the rest. Reading order. Measured equal to the drawn
  rects to 0.000 px in every preset (research digest). The old tests switch to it.
- `pub enum Zone { Centre, Left, Right, Top, Bottom }`.
- `pub fn zone_at(r: Rect, collapsed: bool, x: f64, y: f64) -> Zone`. **The zones' geometry**
  (GOAL Terms leaves it to the plan):
  - a collapsed pane (a 24 px bar or strip) is all `Centre` (Q12);
  - a pane's title bar (`y < r.y + TITLE`) is `Centre` (Q11);
  - below it, in the body `(r.x, r.y + 24, r.w, r.h − 24)`, take the point's share of the body from
    each side: left `(x − bx)/bw`, right `(bx + bw − x)/bw`, top `(y − by)/bh`, bottom
    `(by + bh − y)/bh`. The smallest share under **1/4** names the edge (ties go left, right, top,
    bottom). If every share is 1/4 or more, it is `Centre`.
- `pub fn aim(root: &Node, w: f64, h: f64, x: f64, y: f64) -> Option<(PaneId, Zone)>`: the pane
  whose layout rect holds the point (half-open: `x ≤ px < x + w`), and its zone. `None` outside
  `#panes`. Coordinates are relative to `#panes`'s rect.

`verify/dock-oracle.mjs` implements the same geometry from this section's words, and the probe aims at
fixed points: a tenth of the body in from each side on the body's mid-lines, the body's centre,
and a point 8 px from the title bar's right end (the title bar is the centre, Q11).

### 2.3 The drop (pure, `arrange.rs`)

- `pub fn docked(root: &Node, v: View, target: PaneId, zone: Zone, w: f64, h: f64) ->
  Option<(Node, PaneId)>`: the tree after the drop and the id of the pane holding `v`, or `None`
  when the zone is invalid. It works on a clone, in this order:
  1. `src = holder(root, v)`. `None` if no pane holds `v` or `target` doesn't exist.
  2. **Invalid outright:**
     - `Centre` on `src` (R6, Q2);
     - an edge of a collapsed target (Q12);
     - any zone of `src` when `src` holds only `v` (Q10).
  3. **Take `v` out of `src`** (R3). It removes `v`. If `src` showed `v`, `src` shows the view after
     it, or the first after the last, as `move_view` does. `src` keeps its collapsed state.
  4. **`Centre`** (R3): `target` appends `v` as its last tab, shows it, and is expanded.
     **An edge** (R4): the target node is replaced by
     `Split { orient: Row (left/right) | Col (top/bottom), f: 0.5, kids }`. The kids are
     `[new, target]` for left/top and `[target, new]` for right/bottom. `new` is
     `Pane { id: max id + 1, views: [v], shown: v, collapsed: false }`. Ops never renumber, so
     `len + 1` could collide (research 02 § 4).
  5. **If `src` is now empty** (R5), `lift` its path, recomputed by id after step 4: its sibling
     takes its space. The order doesn't change the result; the oracle checks it.
  6. **Validate the whole result** (R5, R7), else `None`:
     - no empty pane, and each view held exactly once, the same set as before;
     - no split whose two children are both collapsed panes, and no collapsed root (the accordion
       rule, checked, never repaired: R7 makes such a zone invalid);
     - for an edge, `layout(result, w, h)` gives the new pane **and** the target at least `MIN_W`
       (200) across, for left/right, or `MIN_H` (120) down, for top/bottom (R7). The floors are read
       on the result at the current `#panes` size (deviation D2).
- `pub fn dock(root: &mut Node, v, target, zone, w, h) -> Outcome`. With `Some((t, id))` it sets
  `*root = t` and returns `Outcome { changed: true, focus: Some(id), toast: None }`. With `None` it
  returns `Outcome::default()` and leaves `root` untouched. A refused drop shows no toast: the
  missing overlay was its warning (R7), and Esc-like silence matches R6.
- `pub fn preview(root: &Node, v, target, zone, w, h) -> Option<Rect>`: `docked(…)` laid out, and
  the rect of the pane holding `v`. That is the overlay (R2). Same function, so they can't drift.

**R8's equality.** A centre drop of `src`'s shown view onto `neighbour(src, dir)` gives the same
tree as `move_view(src, dir)`, ids included, because steps 3–5 are `move_view`'s steps in its
order. `accordion` there is a no-op, since the sibling subtree moves up whole (02 § 4). It is
proven exhaustively over small trees (§ 2.6). `move_view` itself is not changed.

### 2.4 The pointer and the overlay (`panes.rs`, `keys.rs`, `index.html`, `app.css`)

**Overlay.** One element, `<div class="dock" id="dock" hidden></div>`. It sits at the end of
`<body>` (not inside `#panes`, whose first non-`#parking` child is the skeleton,
`panes::skeleton`). It is `position: fixed; pointer-events: none`, above the panes and sashes
(`z-index` over `.sash`'s 30, under `#help`). It is a translucent accent fill with an accent
border, `box-sizing: border-box`, and **no CSS transition or animation**. It is positioned in
client px at `#panes`'s origin plus `preview`'s rect, **synchronously inside the `pointermove`
handler** (no rAF), and is `hidden` whenever no valid zone is under the pointer. `#dock`, its
synchronous update, and capture held on the pressed `.ptab` element are the probe's contract
(`verify/dock.mjs` reads the overlay right after each move and checks `hasPointerCapture` on the
tab).

**State** (`thread_local! TAB: RefCell<Tab>` in `panes.rs`, separate from the sash's `DRAG`):

```
Idle
Pending  { pid, view, pane, at: (x, y), tab: Element }
Dragging { pid, view, pane, tab, tree: Node /* the arrangement when the drag began */,
           aim: Option<(PaneId, Zone)> }
Cancelled{ pid }            // the button is still down; swallows the rest of the press
```

- **`pointerdown`** (the existing `panes::on_down`, after its `focus(id)`; Q14): with
  `button() == 0`, `is_primary()`, the target's closest `.ptab[data-view]`, and state `Idle`, set
  `Pending`. No `preventDefault` (01 row 3b: it would keep a focused input focused). **Any**
  primary press first resets a state left over from a lost gesture (`Cancelled` whose release
  never arrived, after a blur and a release outside the window) to `Idle` and removes a stale
  click swallower, so the next press is never eaten.
- **`pointermove`** (on `document`, since captured moves bubble there; coordinates as f64 through
  `Reflect`, `panes::client`). A move with `buttons() == 0` in `Cancelled` or `Pending` returns
  to `Idle` (the release happened where we couldn't see it):
  - `Pending` moved at least **4 px** (Euclidean) from `at` becomes `Dragging`. Take the state out,
    `tab.set_pointer_capture(pid)`, snapshot `cfg().arr.clone()` into `tree`, and put the state back.
    Under 4 px nothing happens (R1).
  - `Dragging`: read `#panes`'s rect, `aim` the pointer, and compute `preview` from the
    *snapshot*. Show the overlay at the rect, or hide it when `None`. Store `aim`.
- **`pointerup`** (`document`):
  - `Pending` → `Idle`. The browser's click follows and does today's work.
  - `Dragging`: process the up point as a move first (same aim as the last overlay), then take the
    state out. Hide the overlay and arm the click swallower. If the stored aim is a valid zone and
    `cfg().arr == tree` (a short borrow; else cancel: the tree changed under the drag, a defensive
    guard with no route the probe can drive, since every key cancels first), call
    `nav::operate(move |n, _| arrange::dock(n, v, target, zone, w, h))`, with `w`, `h` read before.
  - `Cancelled` → arm the swallower → `Idle`.
- **Cancel** (Esc and any other key, `blur`, `pointercancel`, `lostpointercapture` while
  `Dragging`): take the state out, hide the overlay, release capture if held, then `Cancelled`.
  `pointercancel` ends with `Idle`, since no click follows it. `lostpointercapture` is listened for
  on `document`, where Chromium delivers it after a removal (01 row 5). Ours, after a drop, finds
  `Idle`. The sash's `on_up` stays bound to `#panes`'s `lostpointercapture` and finds no sash
  `DRAG`.
- **The click swallower**: a one-shot capture-phase `click` listener on `window`. It runs
  `stop_immediate_propagation` + `prevent_default` on the first click with `detail() > 0`, then
  removes itself. It is armed only from a `pointerup` that ends a `Dragging` or `Cancelled` state. A `setTimeout(0)` also removes it, since the click arrives in the same task as
  `pointerup` (01 row 2). Keyboard clicks (`detail == 0`, Space or Enter after a key cancel) pass, as
  "handled as usual".
- **Keys** (`keys.rs`): a capture-phase `keydown` on `window`, registered in `keys::wire`, ahead
  of `#go`'s own listener (02 § 8). Only while `Dragging` (a key during `Pending` is handled as
  usual and leaves the press armed, so a release under the threshold stays exactly a click, R1):
  - a modifier alone (`Shift`, `Control`, `Alt`, `Meta`, `CapsLock`) does nothing (the leader's
    precedent, `keys::on_key`);
  - `Escape` cancels, then `prevent_default` + `stop_immediate_propagation` (R6: Esc does nothing
    else);
  - any other key cancels and propagates as usual (Q13).

  `blur` cancels beside `LEADER.set(-1.0)`.
- **No selection, no pan, no resize** (R1): capture keeps moves off the Graph canvas and the sashes
  (01 row 4; the Graph pans only from a canvas `pointerdown`, 02 § 9). A drag from a button selects
  no text (01 row 3a). `.ptitle { user-select: none }` is a backstop. `.ptab { touch-action: none }`
  costs one declaration and lets a touch drag reach the handler; it is not required or tested (GOAL
  Non-goals).
- **§ Invariants 1.** No `TAB`, `DRAG`, `cfg()` or `s()` borrow is held across
  `set_pointer_capture`, `release_pointer_capture`, `panes::focus`, `nav::operate`,
  `chrome::toast`, showing or hiding `#dock`, or adding or removing a listener. Each handler takes
  its state into a local (`TAB.with(|t| t.replace(Tab::Idle))`), acts, and stores the next state with
  one short `borrow_mut`. The `operate` closure captures only `View`, `PaneId`, `Zone` and two
  `f64`s.

### 2.5 After the drop (R8–R10): through `nav::operate`, unchanged

`operate` runs the op under one short `cfg_mut()` borrow, then focus, `sync_view`, `panes::apply`
(a shape change: the skeleton is rebuilt and the view roots are moved with `moveBefore`, keeping
scroll), `save_settings`, `show` for each view that came on screen, and `nav(false)` (a
`replaceState`). So:
- R9: the drop is saved as a key change is (`arr_saved |= changed`). A reload's `sanitise` keeps
  every valid result up to ids (02 § 5). The hash names `S.view`, and nothing is pushed.
- R10: focus goes to the dropped view's pane (`Outcome.focus`). For the Inspector, `sync_view`
  leaves `S.view` on the last main view (Q20), and `draw_key_view` draws that view if the drop left
  it off screen. That is the route pane-arrangements' review rounds kept finding, so the `keyview`
  group grades it from four presets. View state moves with its root (02 § 3).
- No change to `operate`, `state.rs`, `sanitise` or the load path is planned. If build finds one
  needed, that is a STOP: `state.rs` is high-blast-radius.

### 2.6 Native tests (`arrange.rs` tests module; names in `verify/required.txt`)

The tests sweep a **small-tree domain** exhaustively:
- every tree of 1–4 panes: shapes × `Row`/`Col` per split × `f ∈ {0.5, 0.3}`;
- views dealt to panes in order (each pane 1 or 2 views, from the six), each pane's shown tab each
  of its views;
- `collapsed` on any pane where the accordion and the no-collapsed-root rule allow it;
- × every view × every target pane × every zone × sizes `{1600×918, 900×918, 640×300, 430×250}`.

Each sweep test asserts its own domain size (at least 100 000 drops), so it can't pass empty.
- `layout_tiles_the_root`: the rects tile `w × h` exactly (no gap, no overlap, collapsed children
  24 px), and the presets' rects are the ones `preset_sizes_match_the_plan` pins.
- `aim_finds_the_pane_and_zone_under_a_point`: over a 7 px grid of every layout in the domain, the
  pane found holds the point, and the zone is § 2.2's (title bar and collapsed panes `Centre`,
  quarter bands, nearest edge). Outside the root is `None`.
- `dock_matches_the_hand_worked_cases`: the cases in `verify/dock-oracle-test.mjs`, with the same
  expected trees and rects.
- `dock_sweep_matches_the_written_rules`: a second, independent reading in the test (written from
  GOAL R3–R7, not from `docked`) gives validity and the expected tree for every drop. `docked`
  agrees on both, for every drop.
- `refused_dock_changes_nothing`: for every invalid drop, `dock` returns `Outcome::default()` and
  the tree is bit-identical (`==`) to before.
- `dock_preview_is_the_rect_the_view_lands_in`: `preview` is `Some` exactly when `docked` is, and
  equals `layout(result)`'s rect for the pane holding `v`.
- `centre_dock_on_the_neighbour_equals_the_move_keys`: for every pane with a neighbour and both
  `Dir`s, `dock` of its shown view at `Centre` of `neighbour(id, dir)` equals `move_view(id, dir)`:
  tree and `Outcome.focus`.
- `docked_trees_survive_save_and_load`: `sanitise(from_json(to_json(t)))` equals `t` renumbered,
  for every valid result.

### 2.7 Files changed

`okt-client/src/arrange.rs` (P1); `okt-client/src/panes.rs`, `okt-client/src/keys.rs`,
`okt-client/static/index.html`, `okt-client/static/app.css`, `README.md` § Keys (P2; same-commit
rule); fixes from P3's groups if any; `tests/ui/dock.mjs`, `tests/ui/dock-oracle.mjs` (copies),
`README.md` § Tests and § Layout's `tests/ui/` row (P4). Not changed: `state.rs`, `nav.rs`
(unless P3 finds a key-view route, which goes in `panes.rs` first), `okt-server/`,
`tests/ui/keys.mjs`, `tests/ui/panes.mjs`. `app.css` is coupled with `graph.rs`, but no graph paint
rule changes, so nothing pairs.

### Requirement → design map

| R-ID | Design element(s) that satisfy it |
|------|-----------------------------------|
| R1   | `Pending` → `Dragging` at 4 px; every `.ptab` (strips and Inspector included); under 4 px the browser's click is untouched; the swallower after every drag; capture keeps moves off the Graph and sashes; no selection (§ 2.4) |
| R2   | `preview` = the dropped view's pane in `layout(docked(…))`; `#dock` shown only for `Some`; one element (§ 2.3–2.4; D1) |
| R3   | `docked` steps 3–4 (`Centre`): append, show, expand; source shows the next or the first |
| R4   | `docked` step 4 (edge): `Split{orient, 0.5}`, new pane on that side; own pane allowed with other views (Q10) |
| R5   | `docked` step 5 (`lift`) and step 6's check (no empty pane, each view once) |
| R6   | invalid zones (`None`) change nothing; cancels by Esc/keys/blur/`pointercancel`/lost capture; the swallower; the tree snapshot guard (§ 2.4) |
| R7   | `docked` step 6: floors on the result, the accordion and root rules, all-or-nothing replacement (D2) |
| R8   | `dock` is an `arrange.rs` pure function run by `nav::operate`; neighbour equality proven (§ 2.3, § 2.6) |
| R9   | `operate`'s save, `replaceState` and no push; `sanitise` keeps results (§ 2.5) |
| R10  | `Outcome.focus` = the dropped view's pane; `sync_view` and `draw_key_view` (Q20, the key view); `moveBefore` keeps state (§ 2.5) |
| R11  | no key code path changes; `keys.mjs` and `panes.mjs` unchanged and green (§ 6, D3) |
| R12  | `tests/ui/dock.mjs`, the probe copied in P4, with `tests/ui/dock-oracle.mjs` (§ 6) |

## 3. Invariant gate (AGENTS.md constitution check)

Checked before research (touched: § 1 client re-entrancy; the high-blast list's `state.rs`; §
Conventions) and again against this design.

- **§ 1 Client re-entrancy.** Every new handler takes its state out of its `thread_local` before
  calling anything that can re-enter: `set_pointer_capture`, `panes::focus` (it runs `apply` and
  `nav`), `nav::operate` (`appendChild` fallback `blur`, the async `lostpointercapture`), a toast,
  showing or hiding `#dock`, adding or removing a listener. The `operate` closure captures values
  only. The one `cfg()` read in `pointerup` (the tree-unchanged guard) is a `let` bound and dropped
  before `operate` (§ 2.4). Reviewed per handler in P2's checklist.
- **High-blast-radius `okt-client/src/state.rs`.** Not changed (02 § 5). `static.sh` fails if it
  is. Research trigger 2 did not fire for that reason; the appetite (big) ran the fan-out anyway.
- **§ Conventions.** Commit subjects are the factory's on the branch. There are no spec ids in
  source or test names; `static.sh` scans added lines for `R`, `P` or `Q` and digits, and the
  probe and oracle are clean, since they become `tests/ui/` files in P4. `app.css` changes
  without `graph.rs`, since no graph paint rule changes. README § Keys changes in the same commit
  as the drag (P2). `.trow` and the `VIEWS` row tracks are not touched.
- **§ 2 Pygments data.** Not touched.

### Deviation justifications

| # | Deviation | Why needed | Simpler alternative rejected because |
|---|-----------|-----------|--------------------------------------|
| D1 | **R2's overlay is the rect the dropped view's pane takes in the result.** That is the target's whole rect (centre) or its half (edge) except when the drop closes a source pane that the target then grows into, or expands a collapsed target. Examples: classic at 1600 × 918, the Inspector onto the main pane's left edge shows `[0, 0, 800, 918]`, not half of the 864 px pane. A join onto three's Graph bar shows the expanded 40%. | R2 says both "the target pane's whole rect … the half of the target pane on that side" and "the overlay SHALL predict the result … within 2 px". In those cases one rect can't satisfy both. The prediction sentence is the one R12 grades. | Drawing the literal half of the current target would fail R2's prediction check on every drop that empties its source next to the target (most Inspector drags). Refusing such drops would break R3/R5 (they must work). **Question 1 for the foreman.** |
| D2 | **R7's floors (and Q12's 400/240 px) are read on the result's layout,** at the current `#panes` size: the new pane and the target each ≥ 200 px across (left/right) or ≥ 120 px down (top/bottom). | R7 words it as what the drop "would leave"; Q12's 400/240 px rule is the same rule read on the target's current size. They differ only when a closing source grows the target, e.g. a 380 px target beside a source pane being emptied. | Reading the current rect would refuse drops whose result keeps every floor, and contradict R7's own wording. **Question 2.** |
| D3 | **R11 graded stricter than worded:** `keys.mjs` and `panes.mjs` must be byte-identical to `main` after preset-bar, not merely keep their assertions. | It is the only check a loosened assertion can't slip past (03 § 4). The tabs and title bars don't move, so no selector should need to. | kept.mjs's label matching lets a changed condition through. If a selector must change, the plan is amended to name the lines. |
| D4 | **The accordion rule refuses, never repairs:** a drop whose result would put two collapsed panes in one split is invalid, though `move_view` repairs the same state with `accordion`. | R7 says such a zone "SHALL be invalid". For the `w v` neighbour the repair is a no-op (§ 2.3), so R8's equality is unaffected. | Repairing would apply a drop "part-way" in a different shape than the overlay predicted. |
| D5 | **The press focuses before the drag begins, and the press's `nav(false)` may replace the hash.** R6's "unchanged" is graded from the moment the drag begins (past the threshold). | Q14 accepted this: focus stays where the press put it. | — (recorded so the probe's baseline is understood: `begin()` snapshots after the threshold). |
| D6 | **A modifier key alone doesn't cancel a drag.** | Q13 says "follow the pane leader's precedent". The leader ignores modifier-only keys (`keys::on_key`). | Cancelling on Shift would make a held modifier abort drags. |

Each row was read against the wording of R1, R2, R6, R7, R8, R11 and Q12–Q14. D1 and D2 change
how an R-ID is *shown* (which rect), so they go to the foreman as questions 1 and 2. D3–D6 carry a
reading the design can hold. A draft row D7 (a key during `Pending` disarming the press) was
withdrawn after the adversarial review: it bent R1's "SHALL act exactly as a click" and, for Esc,
R11; keys now act only on a drag under way.

## 4. Rabbit holes (resolved)

- **Click after a drag** → with capture, Chromium always fires `click` on the captured tab, even
  off-window. A one-shot capture-phase window swallower is armed in `pointerup` and cleared by
  `setTimeout(0)`; `detail > 0` only ([`01`](research/01-pointer-events.md) rows 1, 2, 2b).
- **Selection and focus** → `preventDefault` on `pointerdown` would keep a focused input focused
  and redirect typed keys, so it is not used. Button drags select nothing ([`01`](research/01-pointer-events.md) row 3).
- **The tab removed mid-drag** → `lostpointercapture` arrives later, on `document` only. The state
  is taken before `operate`, so ours is a no-op ([`01`](research/01-pointer-events.md) row 5, [`02`](research/02-after-drop-path.md) § 1).
- **Esc with `#go` focused** → a capture-phase window `keydown` runs before `#go`'s handler
  ([`02`](research/02-after-drop-path.md) § 8).
- **Overlay geometry vs. lift and expand** → predicted from the result (D1), with the planner's
  measurement that the pure tiling matches the DOM to 0.000 px ([`00-digest`](research/00-digest.md)).
- **Pane ids** → `max + 1`, never renumber, so R8's equality holds with ids ([`02`](research/02-after-drop-path.md) § 4).
- **Persistence** → no `state.rs` or load change ([`02`](research/02-after-drop-path.md) § 5).
- **Gate time** → panes.mjs alone takes about 7 min, so the gate runs in parts, each one env.sh run
  ([`03`](research/03-test-harness.md) § 1, § 3).
- **Adversarial review** → see § 7.

## 5. Risks & open questions

**Questions for the foreman (the gate):**
1. **D1:** may the overlay show the rect the view will occupy in the result (R2's prediction),
   where that differs from the target's current rect or half? *Recommendation: yes.* The
   alternative fails R2's own check, or refuses drops R3/R5 require.
2. **D2:** are R7's floors read on the result (R7's wording) rather than the target's current
   width (Q12's 400/240 px)? *Recommendation: yes.* They differ only when a source closes beside
   the target.
3. **Gate run time.** The full gate from P2 is about 40 min. That is an estimate from the dry run's
   measured 1.5–1.8 s per drop; build's first P2 run records the real figure. It runs as nine
   parts, each one env.sh run under 10 min, through the background runner, with `--part` and
   `--groups` for targeted runs. Is that acceptable? *Recommendation: yes.* It is what sweeping
   the whole domain costs, and shrinking it would reopen pane-arrangements' failure mode.

**Risks:**
- **The probe is frozen before the feature exists.** Its plumbing was dry-run on today's tree
  (§ 6), and the oracle passes its hand-worked cases. Its drop branch can't go green until P2. A
  probe defect found in build is a stop-and-report to the foreman, and the amendment is a new
  `[feature] Plan drag-to-dock: …` commit (TECH § Conventions).
- **The key view after an Inspector drop** is the route pane-arrangements' reviews kept finding.
  P3 grades it behaviourally (`j j j k` against the same keys with no drop) in all four presets.
- **Timing flake** in the probe (drags at about 16 ms a step, the 200 ms save delay). Saves are
  awaited by polling the stored JSON, never a fixed wait, except the 450 ms "nothing was saved"
  wait for refusals, which is more than twice the delay.
- **preset-bar slips.** P1 is pure and doesn't need it. P2's static part is red until the rebase,
  by design.

## 6. Verification strategy

**The gate** is `verify/gate.sh P<n>`. Every part's log is kept under
`$REPO/.local/drag-to-dock/`, where `$REPO` is the main checkout (git-excluded):
- **static:** clean index, frozen `verify/`, the spec-id scan, untouched `state.rs` and server
  (scans from the first plan commit). From P2: preset-bar under the branch, `keys.mjs` and
  `panes.mjs` unchanged, README § Keys. At P4: the copies, README § Tests, the ledger.
- **unit:** the profile's `cargo test`, then `required.txt`'s tests by exact name, all passing.
- **oracle:** `dock-oracle-test.mjs`.
- **suites** (from P2): `keys.mjs` and `panes.mjs` in one env.sh run.
- **dock-a … dock-e** (from P2): the probe in five env.sh runs, by group, each sized under 10 min
  (TECH § Gate timings).

`--part` and `--groups` run one part, narrowed to probe groups, for build's mutants.

**Superseded from P3** (2026-11-10, the foreman's P3 stop). `verify/` stays frozen as the record.
The gate is `remediation/gate.sh P<n>`, and P4 copies `remediation/dock.mjs` into `tests/ui/`. The
superseding probe changes two groups and nothing else. `state` opens the first Files folder that
makes `#tree` overflow by the 240 px the wheel scrolls, and steps the Inspector on to the first
History that does the same for `#ibody`. It grades "the scroller scrolled before the drop" under
its own label, and the keep-state label grades only the cases where that held. `keyview` adds the
Log dropped, never drawn, onto the Inspector's pane under the floor at 420 px, so the key view
arrives off screen with nothing drawn. `keyview` runs in `dock-e` with `refuse` and `state`, and
`dock-d` keeps `click`. `remediation/static.sh` compares the copy with the new probe and freezes
the three files as it freezes `verify/`.

**One checker per property, each swept over its domain** (`verify/dock.mjs`):
- **Drops.** `dropCase` grades every drop of every group:
  - prediction: the overlay shows iff the zone is valid, and its rect is both the oracle's and
    where the view lands, as read from the page;
  - refusal: nothing changes;
  - mid-drag: nothing changes while the drag is under way;
  - the result tree drawn and saved;
  - the first click after the drop works (the swallower is gone);
  - focus, the hash and history;
  - fit: every shown view drawn (content, not only a box) and filling its pane;
  - a reload.

  It tallies per property, and a property's label prints the counts of valid and refused cases,
  so an empty domain can't pass. The zone under the pointer, as the oracle aims it, decides the
  expectation: a collapsed pane's points and a title bar's point are its centre. Its domain:
  - the eight sweeps: each preset × {1600, 900} × every pane × six aim points (five zones and the
    title bar) × every source (each pane's shown tab, and one tab it doesn't show). The own pane
    is included, so the collapsed bar, the Inspector and pane-emptying drops all fall in. 180
    drops per width.
  - `sources`: a collapsed side-by-side strip holding the five main views, dragged out and onto,
    and the Inspector as a strip.
  - `refuse`: seeded states too narrow (243 px at 900) or too short (222 px stacked) to split,
    and the accordion state, each swept over every pane and zone from the views that reach a
    refusal.
- **Cancel.** One checker over each width × preset × source × {Esc, `?`, release outside every
  pane, own centre, own title bar, window `blur`, lost pointer capture}, each after an overlay
  showed over a valid zone. It checks:
  - no overlay at the release point;
  - the rects, tabs, saved string, hash and history unchanged;
  - focus on the pressed pane, and the pressed tab not shown;
  - no text selected;
  - an open commit (asserted open) left open by Esc;
  - the help shown by `?`;
  - capture held on the tab before it was lost.
- **Click.** One checker over every tab of every preset at both widths, each on a fresh page:
  - a 3 px press and release leaves the same page state as a real click from the same start (a
    warm-up click first, so lazily loaded views start alike);
  - a drag back onto its own tab is nothing;
  - a 5 px press and release is nothing, which bounds the threshold.
- **Key view** (`keyview`): the Inspector dropped over the key view's pane centre and edge (Log
  and Files, every preset); the key view dropped into a pane under the 200 px floor (a 420 px
  window); the Inspector dropped while the key view's pane is collapsed. Each drop must land, and
  then `j j j k` steps exactly as with no drop.
- **State** (`state`): Files, the Log and the Inspector moved by an edge split and a join keep
  scroll, selection and the Inspector's History tab. Back and forward after the drop step through
  views and leave every pane's views as the drop left them.

**Dry runs on the plan's tree** (`6f175fe` plus the plan's `verify/`, today's page with no drag
feature):
- the oracle passes its hand-worked checks;
- the ledger self-test passes;
- `static.sh` gives the expected reds before the commit;
- the probe's groups run to the end in every group, red where today's page lacks the feature:
  - every valid drop's prediction, result, focus and reload;
  - the cancels (no overlay before the cancel, and Esc closing the open commit);
  - the drag back onto its own tab (today it clicks);

  and green where today's page already behaves: refusals, mid-drag, the 3 px click equivalence;
- the gate P1 red at "required tests 0/8", since build writes them;
- the gate P2 red at static (preset-bar not under the branch, README § Keys).

Measured figures are in TECH § Gate timings. Not dry-run: the drop branch green (it needs the
feature), the P4 copies, and the suites part, which needs preset-bar.

**Mutants build runs** (decision lists; each must turn its named part red):
- **P1, native:**
  - edge split `f: 0.5` → `0.54`;
  - new pane on the wrong side;
  - `len + 1` ids;
  - no `lift` on an emptied source;
  - floors read on the current target rect;
  - floor `>=` → `>`;
  - the accordion repaired instead of refused;
  - own-pane centre allowed;
  - a collapsed target's edges allowed;
  - the title bar as the top zone;
  - the band 1/4 → 1/3;
  - the source's shown tab not advanced;
  - `preview` taken from the target's current rect.
- **P2, browser** (`--part dock-… --groups …`):
  - the overlay from the target's current rect (`sweep-classic-1600`);
  - the overlay left up when the pointer leaves every pane (`cancel-1600`);
  - no click swallower (`click`);
  - a swallower never removed (`sweep-stacked-1600`, the first click after a drop);
  - Esc propagated (`cancel-1600`);
  - another key not cancelling (`cancel-1600`);
  - `blur` not cancelling (`cancel-1600`);
  - lost capture not cancelling (`cancel-1600`);
  - the threshold at 0 px (`click`) and at 8 px (`click`);
  - the overlay left visible after a drop (`sweep-stacked-1600`).
- **P3:**
  - no `draw_key_view` at the end of `panes::apply` (`dock-e`, `keyview`: the never-drawn Log
    route);
  - no `save_settings` in the drop path (`sweep-stacked-1600`);
  - focus left on the source pane (`sweep-stacked-1600`).

## 7. Adversarial review

A fresh reviewer read GOAL, this design and every drafted gate script (2026-11-09). It confirmed
the oracle against R3–R7 and Q10–Q12, and R8's equality with `move_view`. It found 16 problems,
and each is folded in:
- **HIGH, folded:**
  1. the probe expected a refusal on a collapsed pane's edge points, which the zone geometry
     makes centre joins, so a correct build would have gone red. The expectation now follows the
     zone under the pointer;
  2. nothing checked that the overlay hides outside every pane, which is now asserted at every
     cancel's release point;
  3. a click swallower never removed couldn't be caught, so every drop now checks that the first
     click after it works.
- **MEDIUM, folded:**
  - `keyview` and `state` now require the drop to land;
  - drops onto a title bar are in the sweep;
  - key-view routes under the floor and in a collapsed pane were added;
  - `fit` now requires drawn content;
  - a `Cancelled` state left over from a lost gesture is reset by the next press, or a buttonless
    move;
  - draft row D7 and Esc during `Pending` were withdrawn, so keys act only on a drag under way;
  - a lost-capture cancel kind was added, and the unreachable "live tree" mutant was dropped;
  - the static scans start from the first plan commit;
  - the Esc check asserts a commit was open.
- **LOW, folded:**
  - back and forward after a drop are checked;
  - the threshold is bounded at 5 px;
  - the overlay has no transition and updates synchronously (pinned in § 2.4);
  - `pgrep`'s path is normalised;
  - the static header now says the gate grades the index.

---

*Backing research: [`research/00-digest.md`](research/00-digest.md).*

## PLAN gate (foreman, 2026-11-09, under Geoffrey's word in session)

Signed off. Rulings:
- **D1, D2:** accepted, and written into the GOAL as Q15 and Q16 (they touch R2, R4 and R7's
  wording).
- **D3–D6:** accepted as read.
- **Gate run time (about 40 minutes from P2, in nine parts):** accepted. A part over 9 minutes is
  split by a plan amendment.
- **history.mjs left out of the gate:** accepted (R11 names keys and panes).
- **Order:** P1 is pure and builds now, while preset-bar finishes. P2 starts with the rebase onto
  `main` after preset-bar lands.

## P1 gate (foreman, 2026-11-09)

Accepted.
- **The survivor "a collapsed target's edges allowed":** recorded, not a stop. It is provably
  equivalent: a collapsed pane's 24 px fails both floors, so the floor check refuses the drop
  anyway. The realistic variant (allow the edge and expand the pane as it splits) is killed by two
  required tests, and the explicit check stays as § 2.3 orders.
- **The cut sweep domain** (8,943 arrangements, 3,643,560 drops per sweep, ids 1, 3, 5, 7):
  accepted.
- **The `aim` bounds fix:** accepted.
- **The P1 time of about 165 s:** accepted.

## P2 gate (foreman, 2026-11-10)

Accepted: the full `gate.sh P2` is green (2,026 s, nine parts), all twelve mutants went red, and
the rebase onto 0.4.1 merged cleanly, with every commit signed.
- **The per-frame lost-capture watch:** accepted. R6 needs the outline gone when capture is lost,
  and Chromium sends `lostpointercapture` only with the next pointer event.
- **Three cancel mutants killed by the probe crashing rather than by their own label:** accepted
  as is. The gate still goes red, through "ran to the end" and the missing labels. `verify/` stays
  frozen, and P4 copies the probe byte for byte.
- **`suites` at 535 s:** not split. Neither P3 nor P4 adds to it. If a P3 or P4 run takes it over
  9 minutes, the builder splits it one suite per part by a TECH amendment and says so.
- **The `Zone::ALL` dead-code warning in the wasm build:** P3 puts `#[cfg(test)]` on it.

## P3 stop (foreman, 2026-11-10)

P3 attempt 1 stopped red at `state`, "a moved view keeps its scroll, the selection and the
Inspector's tab", on 4 of 6 cases. The defect is in the probe. On the test KB neither `#tree` nor
`#ibody` overflows (scrollHeight equals clientHeight), so `before.top > 0` can't hold whatever the
page does. Rulings:
- **Supersede, don't edit.** `verify/` stays as the record, unchanged. A planner writes a
  superseding gate in `spec/drag-to-dock/remediation/`, as `code-build` orders for a frozen gate
  that can no longer pass for a legitimate reason. It names the files it supersedes, and TECH's P3
  and P4 `verify:` lines run it. P4 copies the superseding probe into `tests/ui/`.
- **The `state` precondition.** The probe picks scroll content by measurement (a Files folder and
  an Inspector body that overflow their scrollers by at least the distance it scrolls), at the
  same viewports. It grades the precondition under its own label ("the scroller scrolled before
  the drop"), so a page that can't scroll fails as a precondition, never as a product failure.
- **The `draw_key_view` survivor.** `keyview` gains a route where the key view arrives off screen
  and never drawn, so the group PLAN names kills the mutant itself. The planner shows that route
  red with the mutant planted and green without it, on the P3 tree. `suites` killing it at present
  is recorded, and doesn't stand in.
- **Part times.** `keyview` moves from `dock-d` to `dock-e`, which gives about 382 s and 350 s.
  Correction to my P2 ruling: the groups each part runs are set in the frozen `gate.sh`, so any
  split is made in the superseding gate, never by a TECH amendment alone.
- **Then P3 resumes:** the `state` and `keyview` groups, the mutant re-run, then the full
  superseding gate for P3.
