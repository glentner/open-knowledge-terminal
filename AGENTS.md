# Open Knowledge Terminal: agent notes

`okt` reimplements the KB's `bin/terminal.py` (the reference) in Rust. Read `README.md` first.

## The rule

Until Geoffrey retires the reference, its behavior is the specification. A change to what a user
sees or what the API returns must either match the reference or be a deliberate, named deviation
(listed in README "Known differences" and marked in the code). Improve the internals freely; keep
the interface identical. When the reference changes, port the change and re-run the parity tests.

## Build and test

```sh
make                      # client (wasm) + server; `make install` puts okt in ~/.local/bin
cargo build --profile fast -p okt-server   # optimized, serves okt-client/dist from disk (no rebuild per client change)
make client               # rebuild only the client into okt-client/dist
cargo test -p okt-server -p okt-pygments -p okt-client
```

Parity (reference on 8376, okt on 6580, same KB; see README): `tests/parity/api_diff.py`
(JSON API), `tests/parity/raw_diff.py`, `tests/ui/dom_diff.mjs` (pane HTML across interactions),
`tests/ui/shots.mjs` + `tests/ui/diff.py` (pixels), and against a scratch clone in watch mode
`tests/parity/watch_diff.py` and `tests/ui/live_diff.mjs`. Run what covers your change; run all
before a release. `tests/perf/profile.mjs` compares main-thread cost and frame pacing;
`tests/ui/soak.mjs` checks that memory and listeners plateau on a page left open.

## Map

- `okt-server/src`: `index.rs` (scan, parse, edges, history; `snapshot()` is `/api/index`),
  `parse.rs` (one file), `yaml.rs` (PyYAML-compatible loading), `git.rs`, `render/` (markdown,
  linkify, punycode, highlighting via okt-pygments), `http.rs` (routes, CSP, gzip), `watch.rs`,
  `pwa.rs`, `launch.rs` (editor/open/reveal), `main.rs` (CLI, detach/stop).
- `okt-client/src`: one module per section of the reference's APP_JS (`graph`, `views/*`,
  `insp`, `go`, `chrome`, `live`, `nav`, `keys`, `actions`); `js.rs` holds the JavaScript
  semantics the output depends on (number formatting, `toFixed`, `localeCompare`, object key
  order, UTF-16 lengths); `state.rs` holds `S`, settings (localStorage `okt:<kb>`) and the boot data.
- `okt-client/static`: the page, `app.css` (the reference stylesheet; the graph rules in it are
  the spec `graph.rs` paints from, so change both together), `main.js` (loader).
- `okt-pygments`: regenerate `data/pygments.json` with `tools/pygments/export.py` (run with the
  reference's Python, which has Pygments) when the reference's Pygments version changes;
  `tools/pygments/check.py` verifies.

## Conventions

- Commit subjects are `[area] what changed` (areas: `server`, `client`, `pygments`, `tests`,
  `build`, `docs`, `vendor`). No trailers. Never push without being asked.
- The client keeps no borrow of `S`, settings or the graph state across a call that can re-enter
  (focus, blur, dispatching events) or an `await`.
