#!/bin/sh
# The history-log-rows gate, from the worktree root:
#
#   OKT_TEST_PORT=6583 sh spec/history-log-rows/verify/gate.sh [unit|browser]
#
#   unit     the profile's unit tests
#   browser  builds the base commit's client, then, in one throwaway environment: keys.mjs, the
#            repo's History suite, this cycle's History checks and the outside-History comparison
#   (none)   both, unit first
#
# Footprint: one cargo or wasm build at a time (about 2 GB peak), then one okt server and one
# headless Chromium with one page open at a time (about 600 MB). Measured on the prototype (mac): the
# whole gate 83 s warm, the browser half alone about 80 s; a first run also builds the base client
# from scratch (about 40 s more).
# The base client is built from the commit this branch forked from origin/main at (override:
# OKT_BASELINE), into target/history-log-rows-base/<commit>/ (gitignored, kept between runs so the
# build stays warm; the wasm build directory is shared).
set -eu
root=$(git rev-parse --show-toplevel)
cd "$root"
port=${OKT_TEST_PORT:?set OKT_TEST_PORT: a free port that is not 6580}
[ "$port" != 6580 ] || { echo "gate: never 6580" >&2; exit 2; }
mode=${1:-all}

if [ "$mode" = all ] || [ "$mode" = unit ]; then
  cargo test -p okt-server -p okt-pygments -p okt-client
fi

if [ "$mode" = all ] || [ "$mode" = browser ]; then
  test -f tests/ui/history.mjs || { echo "gate: tests/ui/history.mjs is missing" >&2; exit 1; }
  rev=${OKT_BASELINE:-$(git merge-base HEAD origin/main)}
  echo "gate: base $rev" >&2
  # The comparison below swaps in the base client only; the page and the server come from this
  # tree on both sides, so they must not differ from the base, and neither may the Log.
  if ! git diff --quiet "$rev" -- okt-client/static/index.html okt-client/static/main.js okt-server okt-client/src/views/log.rs; then
    echo "gate: index.html, main.js, okt-server/ or views/log.rs differ from $rev" >&2
    exit 1
  fi
  base=target/history-log-rows-base/$rev
  if [ ! -f "$base/.extracted" ]; then
    mkdir -p "$base"
    git archive -o "$base.tar" "$rev"
    tar -xf "$base.tar" -C "$base"
    : > "$base/.extracted"
  fi
  make -C "$base" client WASM_TARGET_DIR="$root/target/history-log-rows-base/wasm" >&2
  OKT_TEST_PORT=$port OKT_BASE_DIST="$root/$base/okt-client/dist" tests/env.sh sh -c '
    node "$OKT_ROOT/tests/ui/keys.mjs" &&
    node "$OKT_ROOT/tests/ui/history.mjs" &&
    node "$OKT_ROOT/spec/history-log-rows/verify/history-rows.mjs" &&
    node "$OKT_ROOT/spec/history-log-rows/verify/history-rows.mjs" --baseline "$OKT_BASE_DIST"'
fi
