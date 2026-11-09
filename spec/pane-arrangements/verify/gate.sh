#!/bin/sh
# The phase gate for pane-arrangements. TECH.md's verify lines call it.
#
#   sh spec/pane-arrangements/verify/gate.sh P<n>               everything up to P<n> (cumulative)
#   sh spec/pane-arrangements/verify/gate.sh P<n> --phase-only  P<n>'s own checks (build's fail-once mutation)
#   sh spec/pane-arrangements/verify/gate.sh P<n> --static      static checks alone (the pre-push part)
#   sh spec/pane-arrangements/verify/gate.sh P<n> --self-test STEPS   plan dry runs only: run only the
#        listed steps (e.g. 12), with STATIC_BASE naming the base commit; exits 3 when it skipped one
#
# Steps, in order, stopping at the first red:
#   1. static.sh: committed state, the spec-id scan of added lines, the window rules (from P4), the
#      deferral ledger (P5)
#   2. the profile's unit tests, then the phase's required native tests by exact name
#      (required.txt), all passing in one run
#   3. one tests/env.sh run on $OKT_TEST_PORT (default 6584), under a private TMPDIR:
#      tests/ui/keys.mjs, tests/ui/history.mjs, tests/ui/panes.mjs and the frozen probe
#      verify/rects.mjs (the phase's groups), each to its own exit code; then no process the run
#      started is left; every existing check of keys.mjs and history.mjs (as they stand at the merge
#      base with main) is still there and passed (kept.mjs); and panes.mjs printed every checks.txt
#      label up to the phase as "ok   <label>"
#
# Exit: 0 green; 1 red; 2 usage; 3 self-test skipped steps; 75 environment (the port busy for 5 min).
# Footprint: one step at a time. Peak: cargo (CARGO_BUILD_JOBS=4), or one okt server plus one
# Chromium (about 8 processes, about 1.5 GB). Wall time: see TECH.md § Gate timings (measured).
# Needs, exported by the caller: WASM_BINDGEN, PLAYWRIGHT_BROWSERS_PATH, PLAYWRIGHT_MODULE (unless
# tests/node_modules has playwright), TMPDIR. Runs with the sandbox off: env.sh binds the port, and
# pgrep needs it. Never port 6580.
set -u

phase=${1:-}
case "$phase" in P[1-5]) ;; *) echo "usage: gate.sh P<1-5> [--phase-only|--static|--self-test STEPS]" >&2; exit 2 ;; esac
mode=${2:-}
steps=1234
case "$mode" in
  ""|--phase-only|--static) ;;
  --self-test) steps=${3:-}; [ -n "$steps" ] && [ -n "${STATIC_BASE:-}" ] \
                 || { echo "gate: --self-test needs STEPS and STATIC_BASE" >&2; exit 2; } ;;
  *) echo "gate: unknown mode $mode" >&2; exit 2 ;;
esac
n=${phase#P}
here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
port=${OKT_TEST_PORT:-6584}
[ "$port" = 6580 ] && { echo "gate: never port 6580" >&2; exit 2; }
export OKT_TEST_PORT="$port"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}
run=""
log=$(mktemp "${TMPDIR:-/tmp}/pa-gate.XXXXXX")
cleanup() { del "$log" >/dev/null 2>&1 || :; [ -n "$run" ] && [ -d "$run" ] && del "$run" >/dev/null 2>&1; :; }
trap cleanup EXIT

on() { case "$steps" in *"$1"*) return 0 ;; esac; return 1; }
red() { echo "gate $phase: FAIL ($1)"; exit 1; }
env75() { echo "gate $phase: ENVIRONMENT ($1)"; exit 75; }
# rows of a P<n>|… file: cumulative, or this phase alone with --phase-only
rows() {
  while IFS='|' read -r p rest; do
    case "$p" in P[1-5]) ;; *) continue ;; esac
    q=${p#P}
    [ "$q" -le "$n" ] || continue
    [ "$mode" = "--phase-only" ] && [ "$q" -ne "$n" ] && continue
    printf '%s\n' "$rest"
  done < "$1"
}

# the probe's groups by phase
groups=""
for q in 1 2 3 4 5; do
  [ "$q" -le "$n" ] || break
  [ "$mode" = "--phase-only" ] && [ "$q" -ne "$n" ] && [ "$n" -ne 5 ] && continue
  case "$q" in
    1) groups="$groups first-load migration keys drag" ;;
    2) groups="$groups presets collapse three" ;;
    3) groups="$groups ops" ;;
    4) groups="$groups together fit" ;;
  esac
done

# Step 3 runs under a private TMPDIR, so the throwaway server (its --kb is under it) and Playwright's
# Chromium (its profile is under it) carry it in their argv. Leftovers are matched by that path
# alone; another session's servers and browsers never match, and are never killed.
ours() { [ -n "$run" ] && pgrep -f -- "$run" 2>/dev/null; }
left() {
  i=0
  while :; do
    new=$(ours)
    [ -z "$new" ] && return 0
    i=$((i + 1)); [ "$i" -ge 10 ] && break
    sleep 0.5
  done
  echo "$new" | while read -r p; do ps -o pid=,command= -p "$p"; kill "$p" 2>/dev/null; done
  echo "gate $phase: FAIL (leftover processes, killed)"
  return 1
}
# env.sh exits 2 "port … is busy" while another run holds the port: poll up to 5 min, then report
# the environment, never a test failure
envrun() {
  w=0
  while nc -z 127.0.0.1 "$port" 2>/dev/null; do
    w=$((w + 10)); [ "$w" -gt 300 ] && env75 "port $port busy for 5 min"
    sleep 10
  done
  tests/env.sh "$@"
}

# ---- 1. static ---------------------------------------------------------------------------------
if on 1; then
  sh "$here/static.sh" "$phase" ${mode:+"$mode"} || red static
fi
[ "$mode" = "--static" ] && { echo "gate $phase --static: all passed"; exit 0; }

# ---- 2. unit tests -----------------------------------------------------------------------------
if on 2; then
  cargo test -p okt-server -p okt-pygments -p okt-client > "$log" 2>&1; st=$?
  grep 'test result:' "$log"
  [ "$st" -eq 0 ] && ! grep -q 'FAILED' "$log" || { tail -30 "$log"; red "cargo test exit $st"; }
  names=$(rows "$here/required.txt" | tr '\n' ' ')
  want=$(printf '%s\n' $names | grep -c .)
  if [ "$want" -gt 0 ]; then
    # one run, exact names: every required test exists, is not ignored, and passes
    # shellcheck disable=SC2086
    cargo test -q -p okt-client -- --exact $names > "$log" 2>&1; st=$?
    got=$(sed -n 's/.*test result: ok\. \([0-9][0-9]*\) passed; 0 failed; 0 ignored.*/\1/p' "$log" | head -1)
    if [ "$st" -eq 0 ] && [ "${got:-0}" -eq "$want" ]; then echo "ok   required tests $got/$want"
    else tail -20 "$log"; red "required tests ${got:-0}/$want (names in required.txt)"; fi
  fi
fi

# ---- 3. the browser suites, one env.sh run -------------------------------------------------------
if on 3; then
  for f in tests/ui/keys.mjs tests/ui/history.mjs tests/ui/panes.mjs; do
    [ -f "$f" ] || red "missing $f"
  done
  kref=$(git merge-base HEAD main) || red "no merge base with main"
  run=$(mktemp -d "${TMPDIR:-/tmp}/pa-run.XXXXXX") || exit 2
  # the probe's groups go in as arguments ($groups is split on purpose)
  # shellcheck disable=SC2016,SC2086
  ( export TMPDIR="$run"; envrun sh -c '
    r=0
    for s in tests/ui/keys.mjs tests/ui/history.mjs tests/ui/panes.mjs; do
      echo "== $s"; node "$OKT_ROOT/$s"; c=$?; echo "== $s exit $c"; [ "$c" -eq 0 ] || r=1
    done
    echo "== rects $*"; node "$OKT_ROOT/spec/pane-arrangements/verify/rects.mjs" "$@"; c=$?
    echo "== rects exit $c"; [ "$c" -eq 0 ] || r=1
    exit $r' sh $groups ) > "$log" 2>&1
  st=$?
  grep -E '^(ok|FAIL|skip)|^== |failed|passed|busy' "$log"
  lst=0; left || lst=1
  del "$run" >/dev/null 2>&1 || :
  [ "$st" -eq 75 ] && env75 "port $port busy for 5 min"
  [ "$st" -eq 2 ] && grep -q 'is busy' "$log" && env75 "port $port busy"
  [ "$st" -eq 0 ] || red "browser suites exit $st"
  [ "$lst" -eq 0 ] || red "leftover processes"
  # the existing checks of keys.mjs and history.mjs: still in the files, and passed in this run
  node "$here/kept.mjs" "$kref" "$log" || red "an existing browser check was lost or failed"
  # each label must open a line "ok   <label>" (a prefix match at column 0, no regex)
  miss=$(rows "$here/checks.txt" | while IFS= read -r label; do
    awk -v l="ok   $label" 'index($0, l) == 1 { f = 1 } END { exit !f }' "$log" \
      || echo "FAIL missing check: $label"
  done)
  [ -n "$miss" ] && echo "$miss"
  miss=$(printf '%s' "$miss" | grep -c 'FAIL missing')
  [ "$miss" -eq 0 ] || red "$miss named checks missing"
fi

case "$steps" in 1234) ;; *) echo "gate $phase: SELF-TEST ran steps $steps only"; exit 3 ;; esac
echo "gate $phase: all passed"
