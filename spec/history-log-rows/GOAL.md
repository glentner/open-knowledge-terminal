# GOAL — History: one line per commit, under the Log's day rules

> **Origin spec.** The *what* and *why* — the locked contract `code-review` grades against.
> The *how* lives in [`PLAN.md`](PLAN.md) and [`TECH.md`](TECH.md) (written by `code-plan`).
> Keep this at the right altitude: solved and bounded, but not over-specified — leave design
> freedom for the plan. Edit requirements here; do **not** silently drift them during build.

- **slug:** history-log-rows
- **kind:** feature
- **appetite:** small

## Problem

Geoffrey, 2026-11-08: "The History viewer looks weird because the 2nd column (after the hash) is
the date/time and it's stacked, which gives a kind of weird double-spaced look that clashes with
the full Log viewers more compact look. I like the Log look better. Feel free to use the horizontal
layers (Weekday Month Day Year · #) to separate commit days with the HH:MM inline."

Evidence, confirmed at `edcbed4` (the seed cited `8d2f001`; the anchors have not moved). This is
**pre-existing** behaviour, not a regression:

- `history_view` (`okt-client/src/insp.rs:1831`) renders the inspector's History tab. It has three
  listings: a file's commits ("Commits on this file", with a sparkline), a folder's ("Commits
  touching this folder", with a sparkline), and, with nothing or the root selected, "Repository
  history": the latest 300 commits and an OPEN THE FULL LOG button. Every row comes from `hrow`
  (`insp.rs:1917`).
- `.hrow` (`okt-client/static/app.css:356`) is `64px 88px 18px minmax(0, 1fr) auto`: hash, date and
  time, status, subject, then the `+added −deleted` counts and, on a file, an `@` button. The date
  and time is `fmt::dt` ("Nov 08 14:22", 12 characters, about 90px in the 12.5px monospace body
  font), which does not fit 88px and wraps, so every commit is two lines tall. The row has no fixed
  height.
- The Log (`okt-client/src/views/log.rs:205–230`) puts a `.dayrule` before the first commit of each
  local day, labelled `js::date_string(t) · N` (JavaScript `toDateString()`, e.g.
  "Sun Nov 08 2026 · 3", shown upper-case by the CSS), and shows each commit's time inline as
  `fmt::time` (local, 24-hour, zero-padded `HH:MM`). Its `.crow` rows are fixed at `height: 21px`
  (`app.css:256`). `.dayrule` (`app.css:267`) is `position: sticky; top: 0`.
- `.hrow` is also used outside History, by the Overview tab's "Last commit" row (`insp.rs:775`).

## Outcome / vision

History reads like the Log. Each commit is one line, the height of a Log row: hash, time, status,
subject, and the counts and `@` at the right. A long subject ends in "…" rather than wrapping. Days
are separated by the same day rule the Log uses, with the same label, and the current day's rule
stays pinned at the top while the list scrolls. Clicking still does exactly what it does today.

## Acceptance criteria (the contract)

Terms. *History* is the inspector's History tab, in all three listings of Problem. A *History row*
is one commit's line in it. A *day* is a local calendar day, as the Log groups them. *The Log* is
the Log view (`views/log.rs`).

- **R1** — WHEN History lists commits, it SHALL show a day rule before the first row of each day,
  keeping today's row order, so every row sits under the rule for its own day. The rule SHALL look
  like the Log's (its `.dayrule` style).
- **R2** — Each History day rule SHALL be labelled `js::date_string(t) · N`, the Log's format
  ("Sun Nov 08 2026 · 3"), where N is the number of rows History shows under that rule.
- **R3** — Each History row SHALL show, on one line and in this order: the short hash, the commit's
  time as `HH:MM` (local, 24-hour, the same text the Log shows for that commit), the status, the
  subject (with today's renamed-path note where there is one), and the actions (the
  `+added −deleted` counts and, where it appears today, the `@` button). The row SHALL carry no date.
  No part of a History row SHALL extend past the inspector's right edge, at any inspector width the
  split allows or with the panes stacked. WHERE the inspector is too narrow for the whole row, the
  subject SHALL yield first, down to nothing, and then the `+added −deleted` counts SHALL be hidden
  (graded on cells, with the counts in a fixed cell: Clarifications, "How wide is the counts
  cell…?", 2026-11-08); the short hash, the time, the status and, where it appears, the `@` button
  SHALL always show.
- **R4** — Each History row SHALL be 21px tall, the height of the Log's `.crow`, and SHALL never wrap
  to a second line at any inspector width the split allows (22% to 75% of the panes) or with the
  panes stacked. IF the subject does not fit, THEN it SHALL be cut at its end with "…", and the full
  subject SHALL stay available as hover text, as today.
- **R5** — WHILE History is scrolled, the rule of the topmost visible day SHALL stay pinned at the
  top of the History scroll area, as the Log's does, and SHALL never cover the inspector's header
  (the path and title block and the OVERVIEW / CONTENT / HISTORY tabs).
- **R6** — History's existing behaviour SHALL keep its meaning: clicking a row opens that commit's
  detail in the inspector; clicking `@` (shown on a file's rows unless the status is `D`) shows the
  file as of that commit and does not open the commit; OPEN THE FULL LOG opens the Log view; the
  `h` key opens the History tab; and the header line (with its count) and, for a file or folder, the
  sparkline still lead the tab, above the first day rule.
- **R7** — Outside History, nothing SHALL change: the Log view renders as it does at `edcbed4`, and
  so does the Overview tab's "Last commit" row, in content, layout and click target.
- **R8** — In the throwaway environment, on a file with commits on at least two days and at least
  two commits on one day, what Geoffrey sees in History SHALL be: every History row exactly one line
  tall (its rendered height equals the Log's `.crow`, 21px), at the inspector's default width and
  with the split dragged to its narrowest; a day rule above each day's rows; and each rule's date
  text identical to the date text of the Log's rule for the same day. The profile's `verify` SHALL
  pass, `tests/ui/keys.mjs` included.

## Non-goals (no-gos)

- Any change to the Log: its rows, its day rules, its label or its chart (ruling 1).
- The Overview tab's "Last commit" row. It shares the `.hrow` class today but is not History (R7).
- New History behaviour: no author column, no selected-row highlight, no keyboard row navigation,
  no filter, and no change to the 300-commit cap on Repository history.
- Changes to the commit detail view, the `@` time-travel view, or the sparkline itself.
- Day-rule label formats other than the Log's (for example a configurable or relative date).

## Clarifications

- **Q:** Does the History label change the Log's own day-rule text too? What is N? — **A:** One label
  format for both views, and it already exists: the Log's `js::date_string(t) · N`
  ("Sun Nov 08 2026 · 3") is Geoffrey's "Weekday Month Day Year · #". History uses the same function
  and the same `.dayrule` class. The Log is unchanged. N is the number of that file's commits on
  that day shown in History. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* History has three listings (Problem), and the ruling's "that file's
    commits" reads the same in each: for a folder, N counts the commits touching it that day; for
    Repository history, the commits of that day among the 300 it shows. R2 states it once as "the
    rows History shows under that rule". The Log counts every loaded commit of the day
    (`log.rs:83–86`), so the date text of the two labels matches, and N matches only where both list
    the same commits (R8 grades the date text).
- **Q:** What does a History row hold, and how tall is it? — **A:** One line per commit: hash,
  `HH:MM` (24-hour, local time, as the Log shows its time), status, subject, actions. The row height
  matches the Log's `.crow` and never wraps to two lines at any inspector width. The subject
  truncates with "…". (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* the Log formats its time with `fmt::time` (`fmt.rs:19`): local time,
    24-hour, zero-padded `HH:MM`. `.crow` sets `height: 21px` (`app.css:256`), so 21px is the
    height, not a measurement of a font. The split clamps the inspector to 22–75% of the panes
    (`keys.rs:658`), and below a 900px window the panes stack and the inspector spans the window
    (R4). "Actions" are what the row's right-hand cell holds today: the `+added −deleted` counts and,
    on a file's rows, the `@` button.
- **Q:** What must stay the same? — **A:** Nothing else changes: History's existing actions, its
  keyboard behaviour and its click targets keep their meaning, named in the GOAL so the review can
  check them. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* at `edcbed4` these are a row's click (`data-commit`, opens the commit
    detail, `keys.rs:118`), the `@` button (`data-at`, `data-atpath`, travels to the file at that
    commit, `keys.rs:99`), OPEN THE FULL LOG (`data-view="log"`), the subject's hover text (`title`),
    and the `h` key (`keys.rs:461`). History has no row-level keys; `,` and `.` step through a
    file's versions from any tab and are not History's. R6 names them. The Overview tab's "Last
    commit" row also uses `.hrow` (`insp.rs:775`); under "nothing else changes" it renders as today
    (R7), and the plan chooses how.
- **Q:** Is the History day rule sticky? — **A:** It follows the Log's: sticky within the History
  scroll area, so long as it doesn't cover the inspector's own header. If that conflict is real in
  the code, leave a marker. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* no conflict. The inspector's header (`#ihead`) and tabs (`#itabs`) are
    siblings of the scroll area `#ibody` (`index.html:81–87`; `.ibody { overflow: auto }`,
    `app.css:321`), not inside it, so a rule sticky at the top of `#ibody` cannot cover them.
    History renders no sticky bar of its own (`.ctool` and `.travel` belong to the Content tab), and
    its header line and sparkline are not sticky, so they scroll away beneath the pinned rule.
- **Q:** How is the visual result graded? — **A:** An R-ID states what Geoffrey sees, graded on the
  rendered page: every History row's height equals one line, day rules group the rows, and the
  label text matches the Log's for the same day. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* R8. "One line" is graded as the `.crow` height, 21px (ruling 2). The
    label's date text is compared, because N differs between the views by ruling 1.
- **Q:** What does a History row do in an inspector too narrow for all of it? (Asked at the PLAN/TECH
  sign-off: the plan's prototype let the counts and `@` run past the inspector's right edge.) —
  **A:** No part of a History row may extend past the inspector's right edge at any width the split
  allows. Below the width where the fixed cells fit, the subject yields first (down to 0), and then
  the `+added −deleted` counts cell hides, through a container query on `#ibody`, so the threshold
  follows the pane. Hash, `HH:MM`, status and `@` always stay. The row stays one line, 21px.
  (resolved 2026-11-08, foreman ruling under decision 2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* R3 carries the narrow-width clause. On the prototype a row's fixed
    parts took 234px with "+26 −0" counts and `@`. The split's narrowest in a 901px window gives a
    198px inspector, so there the counts must hide. `#ibody` is already
    `container-type: inline-size` (`app.css:448`), so a container query on its width sets the
    threshold by the pane, not the window. R4's one line and 21px are unchanged.
- **Q:** How wide is the counts cell, and how is "the subject yields first" graded? (Review cycle 2,
  N1: counts of 13–14 characters were cut at every width while their subject fitted whole.) —
  **A:** The counts are a fixed cell, after the Log's `.ad` column, so "the subject yields first"
  is graded on cells: the counts cell hides only once the subject cell is 0, not once the subject
  has no room beside the counts' own text. The counts cell holds 14 characters. Counts wider than
  that (100,000 or more lines on a side, or 10,000 or more on both sides) are cut with "…" and keep
  their full text in `title`. (resolved 2026-11-08, foreman ruling under decision
  2026-11-08-okt-dream-inbox-autonomous)
  - *Applied to the code:* 14 characters hold every commit in the KB (the widest is
    "+21,899 −7,071") and "+99,999 −9,999". At the History font's 7.5px advance the cell is 105px,
    where the Log's `.ad` is 96px (12 characters). The threshold below which the counts hide follows
    from the fixed cells, this one included.

## Related materials

- Seed: `issues/history-log-rows.md` (promoted; `status: adopted:history-log-rows`)
- ROADMAP: Part II, entry 3, "Compact History"
- Geoffrey's words: KB `workspace/open-knowledge-terminal/stories.md`, story 4 (2026-11-08)
- Source: `okt-client/src/insp.rs:1831` (`history_view`), `:1917` (`hrow`), `:775` (Overview "Last
  commit"); `okt-client/src/views/log.rs:83–86` (day counts), `:205–230` (day rule and `.crow`);
  `okt-client/src/fmt.rs:19` (`time`), `:28` (`dt`), `:89` (`day_key`); `okt-client/src/js.rs:486`
  (`date_string`); `okt-client/static/app.css:256` (`.crow`), `:267` (`.dayrule`), `:321`
  (`.ibody`), `:356–365` (`.hrow`); `okt-client/src/keys.rs:99`, `:118`, `:461`, `:658`
- Profile: `app.css` is coupled core with `okt-client/src/graph.rs` (AGENTS.md § Factory)
