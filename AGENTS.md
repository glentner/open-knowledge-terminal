# Open Knowledge Terminal: agent notes

`okt` is the KB's terminal, written in Rust; it is its own specification. Read `README.md` first.

## Build and test

```sh
make                      # client (wasm) + server; `make install` puts okt in ~/.local/bin
cargo build --profile fast -p okt-server   # optimized, serves okt-client/dist from disk (no rebuild per client change)
make client               # rebuild only the client into okt-client/dist
cargo test -p okt-server -p okt-pygments -p okt-client
```

Browser suites run against a server, normally the throwaway one (`tests/env.sh`, § Factory):
`tests/ui/keys.mjs` covers the navigation keys (README § Keys); `tests/ui/soak.mjs` checks that
memory and listeners plateau on a page left open; `tests/perf/profile.mjs` measures main-thread
cost and frame pacing, and `tests/perf/compare.py` compares two runs. `tests/perf/profile.mjs
--graph` is the graph's frame-time gate: headed, against `$OKT` only (never 6580), it orbits,
orbits with show all on, and warps, and fails a scenario whose p95 frame interval passes 16.7 ms
(it needs a display of 120 Hz or faster to judge). They need Playwright (`npm install` in `tests/`).

## Map

- `okt-server/src`: `index.rs` (scan, parse, edges, history; `snapshot()` is `/api/index`),
  `parse.rs` (one file), `yaml.rs` (PyYAML-compatible loading), `git.rs`, `render/` (markdown,
  linkify, punycode, highlighting via okt-pygments), `http.rs` (routes, CSP, gzip), `watch.rs`,
  `pwa.rs`, `launch.rs` (editor/open/reveal), `main.rs` (CLI, detach/stop).
- `okt-client/src`: one module per pane or concern (`graph`, `views/*`,
  `insp`, `go`, `chrome`, `live`, `nav`, `keys`, `actions`); `graph/` holds the graph's
  submodules: the pure ones, natively tested, are `space` (the deterministic 3D layout), `cam`
  (camera math: projection, the view solve, easing, the warp path), `vis` (regions, visibility
  tiers, the walk's angular order) and `labels` (where the flat labels go, capped and never
  overlapping); `gl` is the WebGL2 renderer and `overlay` the flat 2D canvas over it (labels,
  rings, the compass); `js.rs` holds the JavaScript
  semantics the output depends on (number formatting, `toFixed`, `localeCompare`, object key
  order, UTF-16 lengths); `state.rs` holds `S`, settings (localStorage `okt:<kb>`) and the boot data.
- `okt-client/static`: the page, `app.css` (the stylesheet; its graph tokens and rules are
  the spec `graph.rs` and `okt-client/src/graph/` paint from, so change them together), `main.js`
  (loader).
- `okt-pygments`: the highlighter; its `data/pygments.json` is generated (§ Invariants 2).

## Factory

This repo is built by the spec-driven factory: GOAL → PLAN/TECH → build → blind review → publish →
release. The method and the skills live in Geoffrey's KB (`method/agentic-engineering.md`,
`skill/code-*.md`; run as `code okt …`). The cycle's records live here: `spec/{slug}/` for work in
flight, `issues/` and `ROADMAP.md` for deferred work. Inside this repo, its own rules win.

### Profile

| Key | Value |
|---|---|
| base | `main` |
| branches | `feat/{slug}` for kind `feature`, `fix/{slug}` for `fix`, `{kind}/{slug}` otherwise |
| commit categories | the areas in § Conventions. Branch commits take the GOAL's `kind` verbatim; the landing subject takes the area where most of the change lands |
| branch commits | factory house style, `[{kind}] Build {slug} P<n>`, no trailers; they are squashed away |
| landing | `squash-local`: squash onto `origin/main` in the cycle worktree, one `[area] what changed` |
| ci | none |
| setup | `make client && cargo build --profile fast -p okt-server`; the browser suites also need `npm install` in `tests/` once |
| verify | `cargo test -p okt-server -p okt-pygments -p okt-client`, plus `tests/ui/keys.mjs` inside the throwaway environment when the change touches the client; the release recipe's `gate` before a release |
| throwaway environment | `tests/env.sh <cmd…>` (contract below) |
| coupled core | `okt-client/static/app.css` with `okt-client/src/graph.rs` and `okt-client/src/graph/`; `okt-client/src/state.rs`; `okt-server/src/index.rs` |
| same-commit rules | see § Conventions |
| high-blast-radius paths | see § Invariants |
| security lane | off |
| release | see Release recipe (`release.trigger`) |

### Release recipe

```yaml
release:
  trigger: every-cycle
  version:
    source: Cargo.toml workspace.package.version   # the three crates inherit it
    scheme: semver
    tag_format: "{v}"
    prerelease_syntax: none
    propose_next: by-kind       # minor for kind feature, else patch; Geoffrey confirms
  bump:
    lock: cargo update -w
    lockstep: []
    files: [Cargo.toml, Cargo.lock]
    diff_rule: the version string in Cargo.toml and the three workspace crates' entries in Cargo.lock
    subject: "[build] Version {v}"
  gate:
    - cargo test -p okt-server -p okt-pygments -p okt-client
    - make
    - tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"'
  tag:
    message: "Release {v}"
    signed: true
  notes: none
  modes:
    release:
      base: main
      prerelease: forbidden
      final: true
      work_branch: in-place
      inputs: []
      branch_ops: []
      merge_into: none
      tag_on: bump
      back_merge_into: none
      push: [HEAD:main, "{v}"]
      publish: none
  post_verify:
    - git ls-remote origin refs/tags/{v}
  caveats:
    release: "The installed okt is not updated; `make install` is Geoffrey's call."
  clean_paths: [target/, okt-client/dist/, tests/node_modules/]
```

### Throwaway environment

`tests/env.sh <cmd…>` runs `<cmd…>` against a disposable okt server on 6581 (never 6580, where the
real one runs), serving a fresh clone of the KB. It:

1. runs from the repo or worktree root;
2. creates a fresh temp directory and removes it, and stops the server, on every exit path;
3. clones the KB into it (`$OKT_KB_SRC`, default `~/KB`), so the real KB is never touched;
4. runs the command inside it, so relative writes can't reach the worktree; `$OKT_ROOT` names the
   repo for scripts;
5. builds the current tree first (`make client`, then the `fast` server), never a stale binary;
6. exports `OKT` (with a trailing slash, for the browser suites), `OKT_BASE` (without) and
   `OKT_KB`;
7. writes its own output to stderr and passes the command's exit status through.

### Where deferred work goes

A pass that decides not to fix something records it, and the destination is fixed:

| Home | Holds | Written by |
|---|---|---|
| `issues/{slug}.md` | deferred code work, from the factory's `ISSUE.md` template, `status: unshaped` | whoever defers it |
| `ROADMAP.md` | the ordered index: one entry per issue, its `**Seed:**` pointing at the `issues/` file | whoever defers it |
| `spec/{slug}/` | work in flight | the lifecycle skills |
| the KB inbox | harness friction, as `friction` lines; never code follow-ups | the lifecycle skills |

An `issues/` seed is a candidate, not a contract. The feature step promotes it into
`spec/{slug}/GOAL.md`, and that is where appetite, non-goals and R-IDs get negotiated.

## Invariants

The gate for every plan and the footgun list for every review. A plan walks only the sections its
change touches. A review grades every violation of a numbered section, lettered subsections
included, as CRITICAL; a violation of § Conventions is HIGH.

### High-blast-radius paths (human gate)

A CONFIRMED finding in any of these files needs Geoffrey's decision before the cycle lands; a plan
touching one always runs research. The list only widens: when code moves, add the new home and
never remove the old entry. Skills and templates cite this list and never copy it.

- `okt-server/src/http.rs`: routes, CSP, the link token, POST checks
- `okt-server/src/launch.rs`: runs the editor, open and reveal commands
- `okt-server/src/index.rs`: `snapshot()` is the `/api/index` contract
- `okt-client/src/state.rs`: `S`, settings, and the borrow rule (§ 1)

### 1. Client re-entrancy (`okt-client/src`) — read before touching event handlers or async code

- The client keeps no borrow of `S`, settings or the graph state across a call that can re-enter
  (focus, blur, dispatching events) or an `await`.

### 2. Pygments data (`okt-pygments/data/pygments.json`)

- Regenerate it with `tools/pygments/export.py`, run with the Pygments and Python versions its
  docstring names, when okt moves to a new Pygments version; `tools/pygments/check.py` verifies.
  Never hand-edit it.

### Conventions (same-commit rules)

- Commit subjects are `[area] what changed` (areas: `server`, `client`, `pygments`, `tests`,
  `build`, `docs`, `vendor`). No trailers. Never push without being asked.
- `okt-client/static/app.css` changes together with `okt-client/src/graph.rs` and
  `okt-client/src/graph/`: the graph tokens and rules in the stylesheet are the spec they paint
  from.
- The version lives only at the release recipe's `version.source`; the recipe's `gate` is the full pre-release check (there is no CI).
- No spec R#/P# ids in source code.
- Point to a rule; don't restate it. A rule restated in N places is a same-commit obligation in
  all N.
