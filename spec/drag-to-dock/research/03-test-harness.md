# 03 — Test harness: what dock.mjs and the gate can reuse

Read-only research at `6f175fe` (drag-to-dock) and `fac7e93` (preset-bar, GOAL only). Nothing was run.

## 1. The existing suites

**Output.** All three suites share `check(label, ok, detail)`, which prints `ok   <label>` or
`FAIL <label>: <json>` (`tests/ui/panes.mjs:57`, `keys.mjs:64`, `history.mjs:39`) and ends
`process.exit(fails ? 1 : 0)`. The frozen probe prefixes the group: `ok   <group>: <label>`
(`spec/pane-arrangements/verify/rects.mjs:42-47`). It fails `unknown group` (exit 2) and
`<group>: no check ran` (`:22-23`, `:287-292`), and wraps each group in try/catch with a
`ran to the end` check.

**Group filters.** panes.mjs and keys.mjs have none. Research 06 planned groups for panes.mjs
(`spec/pane-arrangements/research/06-test-harness.md:129`), but build never added them. rects.mjs
takes its groups as argv. That is the model for dock.mjs; an env var works the same way.

**Timings.**
- panes.mjs: about 7 min (mutant logs in `.local/pane-arrangements/build5/`, about 7 min each;
  inbox friction 2026-11-09).
- keys.mjs: 30 s on the prototype (TECH.md:197), more with the FILES columns. Not re-measured.
- Gate P5 in full (env.sh, keys, history, panes, all probe groups): about 11 min with setup
  (TECH.md:469). That is over the 10-minute cap, so the new gate must split.
- Browser costs (research 06:18-20): a new context plus load plus ready ≈ 1.2 s; a reload to ready
  ≈ 0.5 s. panes.mjs's `open` adds 500 ms and its `press` 500–700 ms.

Resetting by preset key is cheapest: `apply_preset` replaces the whole tree, `*root = p.tree(c)`
(`okt-client/src/arrange.rs:592`). In rects.mjs's `key()` (`:85`) it costs 250 ms plus `settle`
(two stable frames plus 150 ms). Use one context per width, reset by key between zones, and
grade each reset against `expect()` so one bad drop can't cascade.

**Helpers to copy verbatim from rects.mjs.** This version is stricter than panes.mjs's.
- `open` (`:52`): seeds through `api/index` first, then asserts boot kb == basename(`OKT_KB`).
- `ready`, `settle`, `key` (blurs inputs first) and `leader`.
- `snap`, `audit`, `derived`, `same`, `shape`.
- `expect` (preset rects, `:170`) and `grade`.
- `clickTitle` (`:234`): avoids tabs and handles strips. panes.mjs's version clicks at `right-12`
  blind.
- `reloadSame` (`:201`).
- `watchToasts`.

`drag` (`:221`) won't do as it stands. It reads `mid()` at the halfway point and releases straight
after the last move. dock.mjs needs:
1. down on the tab centre, after checking `elementFromPoint` hits the `.ptab`;
2. a first move well past the threshold;
3. several stepped moves to the zone;
4. one rAF;
5. a read of the overlay rect and the count of visible overlays;
6. up.

**Tab selector.** Tabs are `button.ptab` in `header.ptitle` (`panes.rs:198-201`); the suites
select `.ptitle button[data-view]`. The other hooks are `data-pane`, `data-views`, `data-shown`,
`data-collapsed` and `data-focused` (`panes.rs:197`, `:264-269`).

**Checks that read the preset bar.** These change with preset-bar:
- `panes.mjs:725` (`marked`, `#presets [data-preset].on`);
- `:780-786` (the click, label "the window bar marks the current preset and a click applies it");
- `:20` (comment).
keys.mjs reads none.

Both of these assert `#panes` height 890 at first load: `panes.mjs:156` and rects.mjs's
`first-load` group. With the 28 px row gone it is 918, so dock.mjs must never hard-code H. Take
W and H from `#panes`'s rect and keep all rects relative to it.

## 2. The stored arrangement (the JS oracle)

The settings key is `okt:<boot.kb || 'kb'>` (`state.rs:104-112`). Under env.sh the clone is
`$site/kb`, so the key is `okt:kb`.

`arrangement` is written only once `arr_saved` is true (`state.rs:264`): a default first load
stores none, and a preset key stores one. `arrange::to_json` (`arrange.rs:413-430`) gives:

```
{"v":1,"root":NODE}
NODE = {"split":"row"|"col","f":<first kid's share>,"kids":[A,B]}   // A = left/top
     | {"views":["graph",…],"shown":"<view>","collapsed":bool}
```

View names are `graph files log table list insp`. The JSON holds no pane ids. Tile as PLAN § 2.3
does: exact `f·E`, with a collapsed child at 24 px. The 1 px rule is a `::before` overlay that
takes no space (TECH P1 build note).

The save is debounced 200 ms (`state.rs:287-297`), and `flush_settings` writes a pending save on
`pagehide` (`keys.rs:717`). So:
- **saved check:** before reloading, wait until the stored JSON differs from its pre-drop value
  (better, equals the oracle). `reloadSame` only waits for `includes('"arrangement"')`, which
  passes at once after a preset key. A reload's pagehide flush then hides a missing save; this is
  REVIEW F3, the no-flush mutant that passed.
- **unchanged check:** wait at least 400 ms past release, then compare the raw string and
  `location.hash`.

## 3. The prior gate, part by part

`spec/pane-arrangements/verify/gate.sh`:

**Modes (`:30-39`).**
- `P<n>`: cumulative.
- `--phase-only`
- `--static`: step 1, exits 0 after it.
- `--self-test STEPS`: needs `STATIC_BASE` and exits 3.

Exit codes are 0, 1, 2 (usage), 3, and 75 (environment). Adapt as is.

**Private TMPDIR.** Step 3 runs env.sh under `run=$(mktemp -d …)`, so the server's `--kb` and
Chromium's profile carry `$run` in their argv. `left()` (`:83-95`) polls `pgrep -f -- "$run"` ten
times at 0.5 s, then kills and fails. It needs the sandbox off. Keep it, once per env.sh run.

**Port busy.** `envrun` (`:98-105`) polls with `nc -z`, and our brief forbids that. Use what
env.sh already reports instead: it exits 2 with "env.sh: port N is busy" before building
(`tests/env.sh:20`). Retry env.sh every 10 s while it exits 2 with `is busy` in the log, and give
75 after 5 min (gate.sh:152 already maps this).

Caveat (inference from env.sh): the port is checked before the 15–70 s build. If another run takes
the port in that window, our okt fails to bind, and env.sh's `curl /api/index` loop (`:45`) succeeds
against the *other* server, so the suites grade someone else's build. Avoid it this way:
- Give the cycle its own port, not 6580, not 6581 (env.sh's default) and not 6584 (used by
  pane-arrangements), and not the one preset-bar uses if it runs at the same time.
- Optionally, have the gate confirm that an okt whose argv holds `$run` is alive after the run.

**kept.mjs.** Static mode checks that each `check(` label's longest literal segment (at least 4
characters, `${…}` split out) of the suites at `<ref>` is still in the file. With a log, it also
checks for an `ok…` line containing that segment and no `FAIL…` line. SUITES is keys and history
only (`kept.mjs:19`); panes labels went through `checks.txt` (prefix match `ok   <label>` at column
0, awk `index()==1`, gate.sh:158-161).

**Static (`static.sh`).**
- Nothing unstaged or untracked (it reads the index).
- The verify dir is frozen against the plan commit (`--grep '^\[feature\] Plan <slug>'`, or
  `STATIC_BASE`).
- The spec-id scan of added lines and added names under `okt-client tests`
  (`(^|[^A-Za-z0-9])[RrPp][0-9]{1,3}…`, with p50/90/95/99 exempted, `:48-56`).
- kept static.

Adapt with the slug changed. The README/`.trow` steps are cycle-specific; drop them.

**ledger.sh.** Generic apart from its paths, and has a `--self-test`. Adapt by changing the slug.

**Run log.** gate.sh writes the log to a mktemp file and deletes it at exit (`:49-51`). That forced
a `tee` in a remediation `verify:` line. Write it to `$root/.local/drag-to-dock/gate-<part>.log`
and keep it. `.local/` is excluded in the common `.git/info/exclude`, checked from this worktree.

**Budget.** keys plus panes is about 8 min warm and over 9 cold, which is too tight to share an
env.sh run with anything. Suggested parts, each its own env.sh run and its own gate invocation:
- (a) `kept`: keys.mjs plus panes.mjs plus kept.
- (b) `dock` by group: at 1600 and at 900, about 90 zone drops in all at about 3 s with reload.
  Roughly 4–5 min, plus extras, so split by width or preset.
- (c) history.mjs, pinned (§ 5), if kept at all.

## 4. Grading R11

Anchor on `kref=$(git merge-base HEAD main)`, as the prior gate did.

**Precondition.** preset-bar must be in the anchor. Today the branch's base `50c9533` predates it,
so the anchor's panes.mjs still holds the `#presets` checks. After preset-bar lands and its R7
replaces them, kept would report "lost the check … window bar". So require the rebase first, as
pane-arrangements P1 did. Add a static red when `git show $kref:tests/ui/panes.mjs` matches
`#presets`, or `$kref:okt-client/static/index.html` still has `nav.fkeys`. preset-bar's own edits
to panes.mjs (the 918 height, the new label for the bar's check) then sit inside the anchor and
need no allowance.

**Mechanism.**
1. **Strict:** `git diff --quiet $kref -- tests/ui/keys.mjs tests/ui/panes.mjs`. drag-to-dock has
   no reason to touch them: the tabs and title bars stay. That rules out a loosened condition,
   which kept.mjs can't see. A needed selector change becomes a plan amendment that names the
   lines.
2. kept.mjs with `SUITES = ['tests/ui/keys.mjs','tests/ui/panes.mjs']` (history optional), graded
   against **each suite's own log section**. The prior gate grepped one shared log, so a segment
   could be satisfied by another suite's or the probe's `ok` line.
3. Each suite exits 0, with no `skip` line (keys.mjs prints `skip FILES columns` outside env.sh,
   `keys.mjs:201`).

Before freezing, dry-run kept against main's real logs. In P1 a guard that printed only on failure
(`FILES columns: OKT_KB is not the real KB`) made kept unpassable even on main (TECH P1 build notes).

## 5. history.mjs fixture fragility

history.mjs scans the newest 400 commits of the live clone for a file with commits on two days,
two on one, and gives up after 80 files (`history.mjs:136-158`). A large KB commit batch turned
the 0.4.0 publish gate red with no code change. It passed 105/0 against a clone pinned before the
batch (foreman message `2026/11-09-pane-arrangements-0-4-0.md:30`; inbox friction 2026-11-09).

The prior gate did not deal with it: it ran history.mjs against the live `~/KB` through kept.mjs.
The pin was done by hand at publish, and its rev wasn't recorded.

R11 names only keys.mjs and panes.mjs. Two options:
- drop history.mjs from the gate;
- pin it: `git clone -q --no-hardlinks ~/KB "$run/kbsrc" && git -C "$run/kbsrc" reset -q --hard <rev>`,
  then `OKT_KB_SRC="$run/kbsrc"`, with `<rev>` chosen and recorded in TECH. env.sh clones the
  source's checked-out branch, so use reset rather than a detached checkout.

Pinning all parts makes keys and panes deterministic too: they pick `hub` and `deep` from the KB.

## 6. tests/env.sh

- On every run it builds the current tree: `make client` plus `cargo build --profile fast -p
  okt-server` (`:34-36`), 15–20 s warm and 73 s cold. Two gate runs in one worktree at once would
  collide on `dist/` and `target/`, so run them in sequence.
- It clones `OKT_KB_SRC` (default `~/KB`) to `$TMPDIR/okt-env.*/kb` and exports `OKT`, `OKT_BASE`,
  `OKT_KB` and `OKT_ROOT`.
- The port is `OKT_TEST_PORT` (default 6581). Its own `nc -z` sits at `:20`.
- It removes the site with `rm -rf` (in sh, outside zsh's block).
- Set `CARGO_BUILD_JOBS=4` (prior gate) and the exports in TECH.md § Conventions (`WASM_BINDGEN`,
  `PLAYWRIGHT_*`).

## Pitfalls the prior cycle hit (avoid in the new gate)

1. **The log was deleted at exit.** Keep it at `.local/drag-to-dock/`.
2. **No group filter on the browser suite.** Each mutant cost a full 7 min run. dock.mjs needs
   groups, and its `unknown group` and `no check ran` failures.
3. **The full gate took about 11 min,** over the cap. Split into parts, each one env.sh run.
4. **The save check was hidden by the pagehide flush** (REVIEW F3). Wait for the stored JSON to
   change before reloading.
5. **A check that printed only on failure** made kept unpassable on main. Dry-run kept on real
   logs before freezing.
6. **Spec-id-looking locals** (`r0`, `r1`, `p1`) turned static red (TECH P4). In zone and pane
   geometry, use `a`/`b`, `lo`/`hi`.
7. **The history fixture read the live KB** and went red after an unrelated KB commit. Pin it or
   drop it.
8. **Hard-coded `#panes` height 890** in the suites. preset-bar makes it 918; derive it.
9. **A mid-run edit of a long sh gate** made sh read new bytes at the old offset (inbox, another
   repo). Wrap the body in `main "$@"; exit $?`.
10. **The static base was the plan commit,** so dry runs needed a plan-first lab or `STATIC_BASE`.
    Give `--self-test` a recipe.
11. **kept matched label segments anywhere in a shared log.** Grade per suite section, and prefer
    an unchanged-file check for keys.mjs and panes.mjs.
12. **A grading gap let a mutant survive** (REVIEW G2: M3 survived; inbox: sweeps need additive
    mutants). Grade every pane × zone × preset × width (GOAL Q6). Add the extras R12 asks for:
    Inspector, emptying, cancel ×3, refusal, click-not-drag.
13. **Shared port.** Use a distinct cycle port, never 6580, and guard against the env.sh
    check-then-bind race described in § 3.
