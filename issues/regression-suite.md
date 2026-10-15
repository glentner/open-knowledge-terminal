---
status: unshaped
kind: tests
appetite: big
---

# Regression tests for what the parity suites covered

> **Candidate, not a contract.** `code okt feature issues/regression-suite.md` promotes it;
> appetite, non-goals and R-IDs get negotiated there.

## Problem

Until 2026-10-14 the KB's `bin/terminal.py` was okt's specification, and the parity suites checked
okt against it end to end. The reference retired from the KB that day, and the suites went with it.
What remains is the Rust unit tests, `tests/ui/keys.mjs` and `tests/ui/soak.mjs`. Nothing now
checks the JSON API, raw files, the watcher or the rendered panes, so a regression there reaches
the installed okt unseen. The release gate is correspondingly thinner.

## Evidence

The retired suites are in the history (last at `64605fb`) and show what the coverage was:

| Suite | Covered |
|---|---|
| `tests/parity/api_diff.py` | `/api/index` and `/api/file` for every file and folder; commits, search, blame |
| `tests/parity/raw_diff.py` | `/raw/` bytes and headers |
| `tests/parity/watch_diff.py` | the watcher's events, driven by file and git changes in a scratch clone |
| `tests/ui/dom_diff.mjs` | the HTML of every pane across ~60 interactions |
| `tests/ui/shots.mjs` + `tests/ui/diff.py` | screenshots and pixel diffs |
| `tests/ui/live_diff.mjs` | live updates: toasts, status, inspector, ticker, tree |

The last parity run, on 2026-10-09 inside the throwaway environment, found 537 of 538 files
identical through `api_diff.py`; the one difference is the declined
`issues/tight-list-fence-newline.md`.

## Possible acceptance (to negotiate)

- Recorded (golden) output from a small fixture KB kept in the repo, for `/api/index`,
  `/api/file` and `/raw/`, compared on every run.
- A watcher test that drives file and git changes in a scratch clone and checks the events.
- Pane HTML recorded across a fixed set of interactions, as `dom_diff.mjs` did.
- All of it runs inside `tests/env.sh` and joins the release recipe's `gate`.

## Non-goals (to negotiate)

- Pixel diffs.
- Byte-compatibility with `bin/terminal.py`.
