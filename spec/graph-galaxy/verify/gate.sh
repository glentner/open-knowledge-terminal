#!/bin/sh
# The phase gate for graph-galaxy. TECH.md's verify lines call it.
#
#   sh spec/graph-galaxy/verify/gate.sh P<n>               everything up to P<n> (cumulative)
#   sh spec/graph-galaxy/verify/gate.sh P<n> --phase-only  P<n>'s own checks (build's fail-once mutation)
#   sh spec/graph-galaxy/verify/gate.sh P<n> --static      static checks alone (the pre-push part)
#   sh spec/graph-galaxy/verify/gate.sh P<n> --self-test STEPS   plan dry runs only: run only the
#        listed steps (e.g. 3), against STATIC_BASE; always exits 3 when it skipped a step
#
# Steps, in order, stopping at the first red:
#   1. static.sh: committed-state facts (staged, no untracked or unstaged source), spec-id scan,
#      per-phase app.css-with-paint, pure-module rules, the ledger (P7)
#   2. the profile's unit tests, then the phase's required native tests by exact name
#      (required.txt), all passing in one run
#   3. tests/ui/keys.mjs in tests/env.sh: exit 0 AND every row of checks.txt up to the phase
#      printed as "ok   <label>" at the start of a line
#   4. from P2: tests/perf/profile.mjs --graph in tests/env.sh, headed: the gate re-derives the
#      verdict from the JSON lines (target 6581, headed, GPU renderer, refresh, frame counts,
#      draws per frame, p95 <= 16.7) for the phase's scenarios
#   5. no process this run started is left
#
# Exit: 0 green; 1 red; 2 usage; 3 self-test skipped steps; 75 environment (port 6581 busy after
# waiting, or a display slower than 120 Hz, where a 16.7 ms p95 can't be judged).
# Footprint: one step at a time. Peak: cargo (CARGO_BUILD_JOBS=4) or one okt server plus one
# Chromium (about 8 processes, about 2 GB). Measured wall on the prototype dry run (2026-11-08):
# step 2 34 s warm, step 3 91 s with a cold client build; a warm phase gate is about 2-4 min.
# Needs, exported by the caller: WASM_BINDGEN (the Cargo.lock pin), PLAYWRIGHT_BROWSERS_PATH, and
# PLAYWRIGHT_MODULE unless tests/node_modules has playwright. Runs with the sandbox off: env.sh
# binds 6581, never 6580. Steps 3-4 open Chromium on the host's display (headed in step 4).
set -u

phase=${1:-}
case "$phase" in P[1-7]) ;; *) echo "usage: gate.sh P<1-7> [--phase-only|--static|--self-test STEPS]" >&2; exit 2 ;; esac
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
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}
log=$(mktemp "${TMPDIR:-/tmp}/okt-gate.XXXXXX")
before=$(mktemp "${TMPDIR:-/tmp}/okt-gate-pids.XXXXXX")
cleanup() { del "$log" "$before" >/dev/null 2>&1 || :; }
trap cleanup EXIT

on() { case "$steps" in *"$1"*) return 0 ;; esac; return 1; }
red() { echo "gate $phase: FAIL ($1)"; exit 1; }
env75() { echo "gate $phase: ENVIRONMENT ($1)"; exit 75; }
# phase rows of a P<n>|… file: cumulative, or this phase alone with --phase-only
rows() {
  while IFS='|' read -r p rest; do
    case "$p" in P[1-7]) ;; *) continue ;; esac
    q=${p#P}
    [ "$q" -le "$n" ] || continue
    [ "$mode" = "--phase-only" ] && [ "$q" -ne "$n" ] && continue
    printf '%s\n' "$rest"
  done < "$1"
}

# The processes a run starts: the throwaway server (its --kb is env.sh's temp clone) and
# Playwright's Chromium. Matched by argv, then diffed against what ran before this step, so a
# caller whose own command line holds the env exports, another session's browser, or a dev server
# never counts.
ours() { { pgrep -f -- "--kb ${TMPDIR:-/tmp}/okt-env\." ; pgrep -f 'ms-playwright/chromium'; } 2>/dev/null | sort -u; }
mark() { ours > "$before"; }
left() {
  i=0
  while :; do
    new=$(ours | comm -13 "$before" -)
    [ -z "$new" ] && return 0
    i=$((i + 1)); [ "$i" -ge 10 ] && break
    sleep 0.5
  done
  echo "$new" | while read -r p; do ps -o pid=,command= -p "$p"; kill "$p" 2>/dev/null; done
  red "leftover processes (killed)"
}

# env.sh exits 2 with "port 6581 is busy" while another worktree's run holds the port: poll for
# up to 5 minutes, then report the environment, never a test failure.
envrun() {
  w=0
  while nc -z 127.0.0.1 6581 2>/dev/null; do
    w=$((w + 10)); [ "$w" -gt 300 ] && env75 "port 6581 busy for 5 min"
    sleep 10
  done
  tests/env.sh "$@"
}

# ---- 1. static ---------------------------------------------------------------------------------
if on 1; then
  if [ "$mode" = "--phase-only" ]; then sh "$here/static.sh" "$phase" --phase-only || red static
  else sh "$here/static.sh" "$phase" || red static; fi
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

# ---- 3. keys.mjs -------------------------------------------------------------------------------
if on 3; then
  mark
  envrun sh -c 'node "$OKT_ROOT/tests/ui/keys.mjs"' > "$log" 2>&1; st=$?
  grep -E '^(ok|FAIL)|failed|passed|busy' "$log"
  [ "$st" -eq 2 ] && grep -q 'is busy' "$log" && env75 "port 6581 busy"
  [ "$st" -eq 0 ] || red "keys.mjs exit $st"
  # each label must open a line "ok   <label>" (a prefix match at column 0, no regex)
  miss=$(rows "$here/checks.txt" | while IFS='|' read -r label pred; do
    awk -v l="ok   $label" 'index($0, l) == 1 { f = 1 } END { exit !f }' "$log" \
      || echo "FAIL missing check: $label"
  done)
  [ -n "$miss" ] && echo "$miss"
  miss=$(printf '%s' "$miss" | grep -c 'FAIL missing')
  [ "$miss" -eq 0 ] || red "$miss named checks missing"
  left
fi

# ---- 4. frame time (headed), every phase from P2 ------------------------------------------------
case "$n" in
  1) scen="" ;;
  7) scen="orbit orbit_all warp" ;;
  *) scen="orbit orbit_all" ;;
esac
[ "$mode" = "--phase-only" ] && [ "$n" -ne 2 ] && [ "$n" -ne 7 ] && scen=""
if [ -n "$scen" ] && on 4; then
  mark
  envrun sh -c 'node "$OKT_ROOT/tests/perf/profile.mjs" --graph' > "$log" 2>&1; st=$?
  grep -E '^(PASS|FAIL|WARN|target=|mode=|renderer=|refresh_ms=|\{)' "$log"
  [ "$st" -eq 2 ] && grep -q 'is busy' "$log" && env75 "port 6581 busy"
  left
  # re-derived, never trusted: where it ran, how, and on what
  grep -q '^target=http://127\.0\.0\.1:6581/$' "$log" || red "frame-time run did not target 6581"
  grep -q '^mode=headed$' "$log" || red "frame-time run was not headed"
  grep '^renderer=' "$log" | grep -q 'Apple' || red "frame-time run had no GPU renderer"
  ! grep '^renderer=' "$log" | grep -qi 'swiftshader' || red "frame-time run used SwiftShader"
  r=$(sed -n 's/^refresh_ms=\([0-9.]*\)$/\1/p' "$log" | head -1)
  [ -n "$r" ] || red "no refresh_ms line"
  awk -v r="$r" 'BEGIN { exit !(r + 0 <= 8.5) }' || env75 "display refresh ${r} ms: needs 120 Hz or faster"
  [ "$st" -eq 0 ] || red "profile.mjs --graph exit $st"
  for s in $scen; do
    # {"scenario":"orbit","frames":N,"draws":D,"p50":x,"p95":x,"max":x,...}, one line per scenario
    j=$(grep "^{\"scenario\":\"$s\"" "$log" | head -1)
    [ -n "$j" ] || red "no JSON line for $s"
    printf '%s\n' "$j" | awk -v s="$s" '
      function num(k,   m) { if (match($0, "\"" k "\":[0-9.]+")) { m = substr($0, RSTART, RLENGTH); sub(/.*:/, "", m); return m + 0 } return -1 }
      { f = num("frames"); d = num("draws"); p = num("p95")
        min = (s == "warp") ? 900 : 600
        if (f < min) { print "FAIL " s ": " f " frames < " min; exit 1 }
        if (d < 0.9 * f) { print "FAIL " s ": draws " d " < 0.9 x frames " f; exit 1 }
        if (p < 0 || p > 16.7) { print "FAIL " s ": p95 " p " > 16.7"; exit 1 }
        print "ok   frame time " s " p95=" p " ms over " f " frames" }' || red "frame time $s"
    grep -Eq "^PASS $s p95=[0-9.]+ <= 16\.7$" "$log" || red "no PASS line for $s"
  done
fi

on 1 && on 2 && on 3 && on 4 || { echo "gate $phase: self-test ran steps $steps only"; exit 3; }
echo "gate $phase${mode:+ $mode}: all passed"
