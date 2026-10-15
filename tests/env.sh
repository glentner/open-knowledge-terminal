#!/bin/sh
# Run one command against a throwaway okt server on 6581, serving a fresh clone of the KB. Nothing
# outside a temp directory is touched, and the real server on 6580 keeps running.
#
#   tests/env.sh <cmd…>
#   tests/env.sh sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"'
#
# Exports OKT (with a trailing slash, for the browser suites), OKT_BASE (without), OKT_KB (the
# clone) and OKT_ROOT (this repo). The command runs inside the temp directory; its exit status is
# passed through. Set OKT_KB_SRC to clone another KB (default ~/KB), and OKT_TEST_PORT to move the
# port.
set -eu

[ $# -gt 0 ] || { echo "usage: tests/env.sh <cmd…>" >&2; exit 2; }
root=$(git rev-parse --show-toplevel)
cd "$root"
src=${OKT_KB_SRC:-$HOME/KB}
port=${OKT_TEST_PORT:-6581}

if nc -z 127.0.0.1 "$port" 2>/dev/null; then echo "env.sh: port $port is busy" >&2; exit 2; fi

site=$(mktemp -d "${TMPDIR:-/tmp}/okt-env.XXXXXX")
pid=""
cleanup() {
  if [ -n "$pid" ]; then
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
  rm -rf "$site"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

echo "env.sh: building the current tree" >&2
make client >&2
cargo build --profile fast -p okt-server >&2

echo "env.sh: cloning $src" >&2
git clone --quiet --no-hardlinks "$src" "$site/kb"

"$root/target/fast/okt" --kb "$site/kb" --port "$port" --open none > "$site/okt.log" 2>&1 &
pid=$!

n=0
until curl -fsS -o /dev/null "http://127.0.0.1:$port/api/index"; do
  n=$((n + 1))
  if [ "$n" -ge 120 ]; then
    echo "env.sh: okt on $port did not come up; log:" >&2
    cat "$site/okt.log" >&2
    exit 1
  fi
  sleep 0.5
done
echo "env.sh: okt on $port" >&2

export OKT="http://127.0.0.1:$port/" OKT_BASE="http://127.0.0.1:$port" OKT_KB="$site/kb" OKT_ROOT="$root"
mkdir "$site/run"
cd "$site/run"
set +e
"$@"
status=$?
set -e
exit "$status"
