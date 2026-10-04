# Open Knowledge Terminal

A live, read-only, Bloomberg-style terminal for a markdown knowledge base kept in git: a graph of
every file and link, a file tree, the commit log, a frontmatter screener, full-text search, and an
inspector that renders each file, its history, and its blame. It watches the repository and
updates itself as files are saved and commits land.

`okt` is a Rust rebuild of the KB's `bin/terminal.py` with the same interface: the same layout,
styling, keys and commands, the same JSON API, byte for byte, and pixel-for-pixel the same pages,
apart from a few deliberate improvements (see Known differences). What changed is underneath. The
server is one native binary. The client is Rust compiled to WebAssembly. It draws the graph on a
canvas instead of restyling an SVG of a thousand elements, and it updates lists in place instead of
re-rendering them, so panning, zooming, hovering and arrowing through lists stay at frame rate.

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
| `okt-client/` | The browser client (`cdylib` for `wasm32-unknown-unknown`): `src/` is a module-for-module port of the reference client; `static/` holds the page, the stylesheet and the loader. |
| `okt-pygments/` | Syntax highlighting that reproduces Pygments' HTML output exactly (lexer tables exported from Pygments by `tools/pygments/export.py`, run by a Rust port of Pygments' lexer engine). |
| `vendor/markdown-it/` | markdown-it 0.6.1 with one fix (emphasis source maps across stripped line prefixes); see `Cargo.toml` `[patch.crates-io]`. |
| `tests/parity/` | API parity against a running reference server: every file, folder, commit, search, blame, raw file, and the watcher's events. |
| `tests/ui/` | Browser parity: screenshots and pixel diffs, the HTML of every pane across ~60 interactions, live updates; a soak test for a page left open all day. |
| `tests/perf/` | Main-thread cost and frame pacing of the same interactions in both clients. |

## Parity

Until the reference retires, `bin/terminal.py` is the specification. With both running against
the same KB (reference on 8376, okt on 6580):

```sh
python3 tests/parity/api_diff.py          # JSON API, every file and folder
python3 tests/parity/raw_diff.py          # /raw/ bytes and headers
node tests/ui/dom_diff.mjs                # pane HTML across interactions
node tests/ui/shots.mjs /tmp/shots && python3 tests/ui/diff.py /tmp/shots
```

Watch-mode checks (`tests/parity/watch_diff.py`, `tests/ui/live_diff.mjs`) run against a scratch
clone of the KB. `tests/ui/keys.mjs` checks the keys okt adds on its own. The browser tests need
Playwright (`npm install` in `tests/`).

### Known differences

All deliberate, and marked in the code:

- The name: the app is Open Knowledge Terminal and its logo reads OK, the default port is 6580,
  settings live under `okt:` in local storage, and POSTs carry `X-OKT` (`X-KB-Terminal` is
  accepted too). The background log and the start hint name `okt`.
- The watcher reacts to file events in about 0.2 s instead of polling every 2 s.
- The background server: `okt --stop` without `--kb` stops the server for this directory's KB or,
  failing that, the only one running (the default KB depends on the directory, where the
  reference's was always its own repo). `okt` and `okt --detach` restart a server that runs an
  older build of the same binary instead of opening it; a server run from another binary, such as
  a development build, is left alone. `/api/hello` reports the binary's path and time for this.
- On macOS the installed web app is found by the address it was installed from, then by the
  `--app` name. The reference only tries the name, which misses when Safari named the Dock app
  after the manifest's short name or something typed in its dialog, and opens a tab instead.
- Keys the reference doesn't have (`?` lists them all, with your pins):
  - FILES: space opens or closes the selected folder, or shows a file in the graph; `u` goes up
    to the parent folder and folds everything under it (at the top, the whole tree); and closing
    a folder, by any key or click, folds its subfolders too, so it reopens one level deep.
  - GRAPH: `j`/`k` step through the selection's connections in the order the inspector lists
    them (a dashed ring, the card and the hint show where), Enter or space goes there, esc stops;
    with no walk, space opens or closes the selected folder and Enter shows a file's content;
    `z` zooms to the selection and its neighbors.
  - Anywhere: `J`/`K` scroll the inspector, `s` switches between source and rendered, `y` copies
    the path, `m` pins the selection and `1`–`9` go to the pins (kept per KB), `n`/`N` step
    through FIND matches and on into the next or previous result file, and `E`/`O`/`F` edit,
    open and reveal, as the buttons do.
- Markdown tables: a column is never narrower than its longest word (links may still break),
  short cells (a word, number or identifier up to 24 characters, any text up to 12) stay on one
  line, and a wide table may run past the prose measure to the edge of the pane. In the reference
  every column can shrink to one letter while the longest takes the width.
- Three bugs of the reference are fixed:
  - Hollow graph shapes (records, external systems) are outlined in their family color, as the
    legend draws them. In the reference a stylesheet rule overrides their stroke and they paint
    as background.
  - The GO bar suggests arguments for ACCENT, TYPE, SHOW and AT (accents, frontmatter types,
    recent commits; for SHOW and AT the rev as typed comes first, so Enter runs it). In the
    reference a dangling `else` means they only ever list the command itself.
  - A node that flashed after a change gets its selection ring back when the flash ends. In the
    reference the ring stays hidden until the graph is rebuilt.

`tests/ui/dom_diff.mjs` expects its GO-suggestion and help steps to differ and ignores
the class that marks short table cells; the graph screenshots differ where the hollow shapes
paint, and content screenshots where tables lay out differently.

## License

MIT
