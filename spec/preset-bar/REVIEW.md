# REVIEW — No always-on preset bar

> Adversarial QA by `code-review`, run in an isolated context (a fresh subagent). The correctness
> pass grades the branch diff against [`GOAL.md`](GOAL.md) and `AGENTS.md` § Invariants only. It
> does not see `PLAN.md` or `TECH.md`. Every finding cites an executed command.

- **Reviewed commit:** 4bb2a29  ·  **Base:** 50c9533  ·  **Date:** 2026-11-09
- **Verdict:** changes-requested
- **Cycle:** 1 of ≤3

Evidence: `$REPO/.local/preset-bar/review1/`.

## Verification run

- `cargo test -p okt-server -p okt-pygments -p okt-client`: 29 + 9 + 13 passed, 1 ignored. A wasm
  `cargo check` gives 0 warnings.
- `keys.mjs` 87/0, `history.mjs` 105/0 (live KB), `panes.mjs` 68/0.
- Probes:
  - 1 px width sweeps from 1600 to 400 px, with no selection and with a depth-8 selection, under
    the served KB name and under `~/KB`;
  - R1 graded in 208 states (8 widths × two selections × 4 presets, the cycle, collapsed states);
  - an emulation of main's static layer for comparison.
- **Disclosure:** a grep filter leaked about 100 `spec/` snippet lines into the reviewer's context.
  No spec file was opened, and every grade rests on measurement.

## Requirement → evidence matrix

| R-ID | Verified how | Status |
|------|--------------|--------|
| R1 | 208 states: the header bottom, the `#panes` top and the topmost pane are all at 38; nothing visible in the band; hit tests either side | ✅ |
| R2 | no preset control at 8 widths × every arrangement, collapsed, `?` open | ✅ |
| R3 | 1600–901 px: the crumbs are inside the header between GO and the chips; the header is 38 px at every width | ✅ |
| R4 | GO and the chips are identical for a deep selection and none at every width; the crumbs are 261 px wide at 1600 with `~/KB` (see F1) | ✅ |
| R5 | the crumbs, brand word and KB name are hidden at 900–400 | ✅ |
| R6 | the keys, the `p` cycle and the `w` leader work; `?` and README list them | ✅ |
| R7 | the re-apply-after-drag check can't pass on a no-op; no check reads a preset button | ✅ |

Unmapped changes: none that matter. `.go` goes to `flex: 0 1 900px` (R4), the dead
`Preset::name`/`parse` are removed, and the first-load height goes from 890 to 918 (R1).

## Findings

### [LOW/CONFIRMED] F2 · The new seed misstates where the double KB name came from
- **Where:** issues/crumbs-kb-name-twice.md, "Why it was deferred".
- **Failure scenario:** it says the repeat was "introduced by preset-bar's move… not present on
  main". In fact main at 1600 and 1000 px, with nothing selected, shows the KB name in the brand and
  again as the only crumb in the `nav.fkeys` row.
- **Evidence:** `mainemu.mjs`, `probe2.log`, `emu-main-1600.png`.

### [MEDIUM/PLAUSIBLE] F1 · R4's 1600 px clause holds only for a short KB display name
- **Where:** `app.css:110` (the brand is `flex: none`), `:113` (`.kbname` has no width bound),
  `:114` and `:131`.
- **Failure scenario:** a long KB display path widens the brand, so the crumbs get nothing at
  1600 px. Under the throwaway clone's temp path they are 5.78 px wide with nothing painted, and
  under a 71-character name 0 px. With `~/KB` they are 261 px, with 8 pieces painted.
- **Evidence:** `probe.log`, `deep-1600.png`, `probe3.log`.

### Out of scope
- **O1:** the `#offbar` (SERVER STOPPED, `app.css:712`, `top: 44px`) now covers the pane title bars
  while the event stream is down. It was placed for the old row.
- The ROADMAP entry for this seed is retired at landing (code-roadmap).

## Triage (foreman, 2026-11-09, under Geoffrey's word in session)

- **F2 (CONFIRMED) → build:**
  - correct issues/crumbs-kb-name-twice.md's "Why it was deferred": the repeat predates this cycle (main showed it in the
    `nav.fkeys` row), and this cycle moved it into the header;
  - add a note on long KB names (F1) to the same seed, since both concern what the crumbs show
    beside the brand.
- **F1 (PLAUSIBLE):** judged as designed. The GOAL-gate ruling gives the crumbs only the room GO
  leaves, and Geoffrey's KB is `~/KB`, where R4 holds with 261 px. A long display name is recorded
  in issues/crumbs-kb-name-twice.md, not fixed here.
- **O1:** seeded now as `issues/offbar-placement.md` plus a ROADMAP Part III entry: the banner
  should sit at the header's bottom edge without covering the pane title bars, or overlay them
  deliberately. It was accepted at the PLAN gate as a known effect.

## Human-gate triggers

- None. No CONFIRMED finding touches a high-blast-radius path.

## Review cycle 2 — changes-requested (2026-11-09)

- **Reviewed commit:** c3e65e4. **Mode:** scoped to the cycle-1 docs fix (`4bb2a29..c3e65e4`); one
  blind reviewer (xhigh). Evidence: `$REPO/.local/preset-bar/review2/`.
- **Verification run:**
  - The code is unchanged since 4bb2a29; only issues/ and ROADMAP.md changed.
  - `cargo test`: 29 + 9 + 13 passed, 1 ignored.
  - Probes at HEAD and on an emulation of main (`probe-r2.mjs`, `probe-r2b.mjs`).
- **Cycle 1's F2:** fixed. Every cite in issues/crumbs-kb-name-twice.md checks out, and its
  measurements reproduce: 261.08, 5.78 and 0 px; the long path is 74 characters.
- **ROADMAP:** entries 1–9 with no gaps, and every link resolves.

### [LOW/CONFIRMED] G1 · issues/offbar-placement.md overstates what this cycle introduced
- **Where:** issues/offbar-placement.md:37-38 ("Introduced by preset-bar … covers the title bars outright").
- **Failure scenario:** `.offbar` is fixed at `left: 50%`, so below 1196 px its message wraps to
  two lines (45.34 px tall). Main already hid 23.34 of the title bars' 24 px at 1195, 1000 and
  900 px. The branch hides 18 px at every width, and its top 6 px stay visible. A shaper who takes
  main's 6.7 px as the baseline to restore would miss the wrap.
- **Evidence:** `probe-r2b.log`.

### Non-blocking notes
- In issues/crumbs-kb-name-twice.md, "about 1339 px" is where the crumbs box opens. With `~/KB`
  the name first paints at 1350 px and is whole from 1377 px.
- Under the 74-character path, GO also shrinks to 678 px at 1600 px.

### Triage (foreman, 2026-11-09)
- **G1 → build:** correct issues/offbar-placement.md in place, since it is new on this branch:
  - at 1196 px and wider, this cycle raised the overlap from 6.7 to 18 px of the 24 px title bar;
  - below that, the banner wraps to two lines (`left: 50%` caps its width), and main already
    covered 23.3 px;
  - the R1 sketch must allow for a 45 px banner;
  - "outright" goes.
- **The notes:** add both to issues/crumbs-kb-name-twice.md as precision (1339 px opens the box;
  the name paints from 1350 px and is whole from 1377 px; a long name squeezes GO too).

## Human-gate triggers (cycle 2)

- None.

## Review cycle 3 — changes-requested, stopped for Geoffrey (2026-11-09)

- **Reviewed commit:** 09878c0. **Mode:** scoped to the cycle-2 docs fix; the cycle-2 reviewer,
  resumed (docs only). Evidence: `$REPO/.local/preset-bar/review3/` (`probe-r3.log`).
- The code is unchanged since 4bb2a29, which cycle 1 approved on every R-ID.
- G1 is correct for 618 px and wider. Both precision notes in issues/crumbs-kb-name-twice.md are
  accurate, and ROADMAP is consistent.

### [LOW/CONFIRMED] H1 · issues/offbar-placement.md gives the banner two heights where it has more
- **Where:** issues/offbar-placement.md:31-33, :50 and :67; ROADMAP.md:114-115.
- **Failure scenario:** the banner's width is capped at half the window, so it gains lines as the
  window narrows:
  - 2 lines (45.34 px) at 618–1195 px;
  - 3 lines (62.02 px) at 617 px and narrower;
  - 4 lines (78.69 px) at 430 px.

  Main hid the title bars completely from 617 px down; the branch hides 18 of 24 px at every width.
  The seed says "1195 px and narrower: two lines" and sketches R1 for two heights only. The
  mistake started in the cycle-2 reviewer's own wording.
- **Evidence:** `probe-r3.log`.

### Triage (foreman, 2026-11-09)
This is a CONFIRMED finding at review cycle 3, the bound. The foreman said in advance that it would
stop for Geoffrey here and not override the bound itself. **Stopped: no landing, no 0.4.1.**

Every finding in cycles 1–3 was prose in this cycle's own follow-up seeds; the fix itself (R1–R7)
was approved in cycle 1. The foreman recommends landing 0.4.1 by Geoffrey's override and then
correcting issues/offbar-placement.md and its ROADMAP entry on `main` in a `[docs]` commit, from
the table in `probe-r3.log`.

## Human-gate triggers (cycle 3)

- None. The stop is the review bound.

### Geoffrey's decision (in session, 2026-11-09): land 0.4.1 now

By Geoffrey's override of the review bound: preset-bar lands and releases 0.4.1 as reviewed. The
code was approved in cycle 1 on every R-ID and is unchanged since 4bb2a29. No fourth review ran.
H1 is corrected after landing, in a `[docs]` commit on `main` that fixes the banner heights and
widths in issues/offbar-placement.md and its ROADMAP entry, from `probe-r3.log`. The verdict below
is recorded as approved under that override.
