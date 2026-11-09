# Open Knowledge Terminal

A live, read-only, Bloomberg-style terminal for a markdown knowledge base kept in git: a graph of
every file and link, a file tree, the commit log, a frontmatter screener, full-text search, and an
inspector that renders each file, its history, and its blame. It watches the repository and
updates itself as files are saved and commits land.

The server is one native binary that indexes the repository, serves a JSON API and server-sent
events, and embeds the client. The client is Rust compiled to WebAssembly. It draws the graph in
3D with WebGL and updates lists in place instead of re-rendering them, so orbiting, zooming,
hovering and arrowing through lists stay at frame rate.

## Install

Needs a Rust toolchain with the WebAssembly target, and `wasm-bindgen-cli` at the version in
`Cargo.lock`:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129
make install            # builds the client, embeds it in the server, installs ~/.local/bin/okt
```

## Use

```sh
okt                     # index the KB (the git repo you're in, or ~/KB), serve it, open it
okt --kb ~/KB --detach  # run in the background (log: ~/Library/Logs/okt.log or ~/.local/state/okt.log)
okt --stop              # stop the background server (this KB's, else the only one running)
okt --check             # index once and list broken links (exit 1 if any)
okt --help
```

It serves `http://127.0.0.1:6580/` by default (6580 spells OKT on a keypad). The page installs as
an app (Chrome: install from the address bar; Safari: File › Add to Dock). Once it is installed,
`okt` and `okt --detach` bring that app to the front instead of opening a tab, whatever it was
named, which makes `okt --detach` a good target for a launcher hotkey. After `make install`, the
next `okt` or `okt --detach` replaces a server still running the previous build, and the open app
reloads itself. Inside, `?` lists every key and command; `/` or `:` focuses the GO bar.

Useful flags: `--theme terminal|dark|light`, `--accent '#ffa028'`, `--font "Berkeley Mono"`,
`--editor 'code --goto'`, `--collapse N` (folders with more direct files start collapsed),
`--no-watch`, `--open app|browser|none`, `--host` (anything beyond loopback requires a token).

## Layout

| Path | What |
|---|---|
| `okt-server/` | The `okt` binary: indexer (files, frontmatter, links, mentions, git history), JSON API, server-sent events, file watcher, markdown rendering, PWA assets. Embeds the client. |
| `okt-client/` | The browser client (`cdylib` for `wasm32-unknown-unknown`): `src/` has one module per pane or concern; `static/` holds the page, the stylesheet and the loader. |
| `okt-pygments/` | Syntax highlighting that reproduces Pygments' HTML output exactly (lexer tables exported from Pygments by `tools/pygments/export.py`, run by a Rust port of Pygments' lexer engine). |
| `vendor/markdown-it/` | markdown-it 0.6.1 with one fix (emphasis source maps across stripped line prefixes); see `Cargo.toml` `[patch.crates-io]`. |
| `tests/env.sh` | A throwaway okt server on 6581 over a fresh clone of the KB, to run the browser tests against. |
| `tests/ui/` | Browser tests: the navigation keys (see Keys), and a soak test for a page left open all day. |
| `tests/perf/` | Main-thread cost and frame pacing of a fixed set of interactions, compared across two builds. |

## Tests

```sh
make test                                                   # unit tests
tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"'     # the navigation keys
tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/soak.mjs" "$OKT_KB" "$OKT"'
```

`tests/env.sh` builds the current tree and serves a fresh clone of the KB on 6581, so the real KB
and the okt on 6580 are never touched. The browser tests need Playwright (`npm install` in
`tests/`).

End-to-end tests of the JSON API, raw files, the watcher and the rendered panes are not written
yet; `ROADMAP.md` tracks them.

## Keys

`?` lists every key and command, with your pins; `/` or `:` focuses the GO bar. Some worth knowing:

- FILES: space opens or closes the selected folder, or shows a file in the graph; `u` goes up to
  the parent folder and folds everything under it (at the top, the whole tree); and closing a
  folder, by any key or click, folds its subfolders too, so it reopens one level deep.
- GRAPH: `j`/`k` step through the selection's connections clockwise around the selection, in
  screen order like a clock hand from 12 o'clock (a dashed ring, a thicker edge, the inspector's
  marked card and the hint show where; a connection in another region is a portal, its edge out
  to that region on the rim, labelled with its path), Enter or space goes there, esc stops; with
  no walk, space opens or closes the selected folder and Enter shows a file's content; `z` turns
  and fits the camera to the selection and its links, or, from the Results view, to the list's
  matches wherever they are, moving no card. The camera: ←/→ orbit it around what it looks at and
  PageUp/PageDown tilt it, while focus is on the page rather than the inspector; `+`/`=` and `-`
  move it closer and farther, and `0` fits the whole graph, keeping the selection; dragging
  orbits, and the wheel or a pinch moves it closer or farther.
- Anywhere: `J`/`K` scroll the inspector, `s` switches between source and rendered, `y` copies the
  path, `m` pins the selection and `1`–`9` go to the pins (kept per KB), `n`/`N` step through FIND
  matches and on into the next or previous result file, and `E`/`O`/`F` edit, open and reveal, as
  the buttons do.
- GO bar: ACCENT, TYPE, SHOW and AT suggest their arguments (accents, frontmatter types, recent
  commits; for SHOW and AT the rev as typed comes first, so Enter runs it).

## Details

- The watcher reacts to file events in about 0.2 s; a slow safety poll catches anything they miss.
- The background server: `okt --stop` without `--kb` stops the server for this directory's KB or,
  failing that, the only one running. `okt` and `okt --detach` restart a server that runs an
  older build of the same binary instead of opening it; a server run from another binary, such as
  a development build, is left alone. `/api/hello` reports the binary's path and time for this.
- On macOS the installed web app is found by the address it was installed from, then by the
  `--app` name. The name alone misses when Safari named the Dock app after the manifest's short
  name or something typed in its dialog.
- Markdown tables: a column is never narrower than its longest word (links may still break),
  short cells (a word, number or identifier up to 24 characters, any text up to 12) stay on one
  line, and a wide table may run past the prose measure to the edge of the pane.

## License

MIT
