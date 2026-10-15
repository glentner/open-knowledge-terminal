---
status: unshaped
kind: tests
appetite: big
---

# End-to-end regression tests

> **Candidate, not a contract.** `code okt feature issues/regression-suite.md` promotes it;
> appetite, non-goals and R-IDs get negotiated there.

## Problem

okt's tests are the Rust unit tests, `tests/ui/keys.mjs` and `tests/ui/soak.mjs`. Nothing checks
the JSON API, raw files, the watcher or the rendered panes end to end, so a regression there
reaches the installed okt unseen, and the release gate is correspondingly thin.

## Evidence

Suites removed in `b8f4f3e` show the shape the coverage once had; they compared okt against
another implementation rather than against recorded output, so they are a map, not a starting
point:

| Suite | Covered |
|---|---|
| `tests/parity/api_diff.py` | `/api/index` and `/api/file` for every file and folder; commits, search, blame |
| `tests/parity/raw_diff.py` | `/raw/` bytes and headers |
| `tests/parity/watch_diff.py` | the watcher's events, driven by file and git changes in a scratch clone |
| `tests/ui/dom_diff.mjs` | the HTML of every pane across ~60 interactions |
| `tests/ui/shots.mjs` + `tests/ui/diff.py` | screenshots and pixel diffs |
| `tests/ui/live_diff.mjs` | live updates: toasts, status, inspector, ticker, tree |

## Possible acceptance (to negotiate)

- Recorded (golden) output from a small fixture KB kept in the repo, for `/api/index`,
  `/api/file` and `/raw/`, compared on every run.
- A watcher test that drives file and git changes in a scratch clone and checks the events.
- Pane HTML recorded across a fixed set of interactions, as `dom_diff.mjs` did.
- All of it runs inside `tests/env.sh` and joins the release recipe's `gate`.

## Non-goals (to negotiate)

- Pixel diffs.
