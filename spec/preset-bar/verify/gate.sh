#!/bin/sh
# The phase gate for preset-bar. TECH.md's verify line calls it. Frozen with the plan commit.
#
#   sh spec/preset-bar/verify/gate.sh                 everything: steps 1, 2, 3
#   sh spec/preset-bar/verify/gate.sh --static        step 1 alone (the pre-push part)
#   sh spec/preset-bar/verify/gate.sh --probe         step 3 with page.mjs alone (build's fail-once
#                                                     mutation of the page's own checks)
#   sh spec/preset-bar/verify/gate.sh --part keys     step 3 with keys.mjs and history.mjs alone
#   sh spec/preset-bar/verify/gate.sh --part panes    step 3 with panes.mjs and page.mjs alone
#        The two parts split the full run's step 3 under the 10-minute command cap; each grades
#        the kept and named checks of the suites it ran. Green on both parts plus --static and the
#        unit tests is not the phase's verify: that is the full run, one tree, one log.
#   sh spec/preset-bar/verify/gate.sh --self-test STEPS   plan dry runs and review only, with
#        STATIC_BASE naming the base commit: run only the listed steps (any of 1 2 3 m), exit 3.
#        Step m runs page.mjs clean (it must pass) and then under each page mutant of mutants.mjs
#        (each must fail, on its expected line), in one env.sh run.
#
# Steps, stopping at the first red:
#   1. static.sh: committed state, the frozen gate, spec ids in added lines, README § Keys, no
#      preset button read by panes.mjs, no prose naming a window bar or preset buttons, every
#      existing browser check still there (kept.mjs), the deferral ledger (ledger.sh)
#   2. the profile's unit tests (cargo test of the three crates), passing in one run
#   3. one tests/env.sh run on $OKT_TEST_PORT (default 6586), under a private TMPDIR:
#      tests/ui/keys.mjs, tests/ui/history.mjs, tests/ui/panes.mjs and verify/page.mjs, each to its
#      own exit code; then: the client build printed no warning; no process the run started is
#      left; every existing check of the three suites (as they stand at the merge base with main)
#      is still there and passed, the replaced window-bar check is gone (kept.mjs); and panes.mjs
#      printed every checks.txt label as "ok   <label>"
#
# The run log is kept, never deleted: $REPO/.local/preset-bar/gate[-<mode>].log, where $REPO is the
# main checkout (.local/ is git-excluded there), and one line per run is appended to runs.txt
# beside it (UTC time, HEAD, worktree, mode, exit). Remediation checkers read them there.
# Exit: 0 green; 1 red; 2 usage; 3 self-test ran a subset; 75 environment (the port busy 5 min).
# Footprint: one step at a time. Peak: cargo (CARGO_BUILD_JOBS=4), or one okt server plus one
# Chromium with up to two contexts (about 8 processes, about 1.5 GB). Wall time: TECH.md § Gate
# timings (measured). Needs, exported by the caller: WASM_BINDGEN, PLAYWRIGHT_BROWSERS_PATH,
# PLAYWRIGHT_MODULE (unless tests/node_modules has playwright), TMPDIR. Runs with the sandbox off:
# env.sh binds the port, and pgrep needs it. Never port 6580.
set -u
slice=""
# Playwright normalises paths, so a TMPDIR ending in / would hide our Chromium from pgrep
tmp=${TMPDIR:-/tmp}; tmp=${tmp%/}

mode=${1:-}
steps=123
case "$mode" in
  "") ;;
  --static) steps=1 ;;
  --probe) steps=3 ;;
  --part) steps=3; slice=${2:-}; case "$slice" in keys|panes) ;; *) echo "gate: --part keys|panes" >&2; exit 2 ;; esac ;;
  --self-test) steps=${2:-}; [ -n "$steps" ] && [ -n "${STATIC_BASE:-}" ] \
                 || { echo "gate: --self-test needs STEPS and STATIC_BASE" >&2; exit 2; } ;;
  *) echo "usage: gate.sh [--static|--probe|--self-test STEPS]" >&2; exit 2 ;;
esac
here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
repo=$(cd "$(git rev-parse --git-common-dir)/.." && pwd) || exit 2
port=${OKT_TEST_PORT:-6586}
[ "$port" = 6580 ] && { echo "gate: never port 6580" >&2; exit 2; }
export OKT_TEST_PORT="$port"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}
logdir="$repo/.local/preset-bar"
mkdir -p "$logdir" || exit 2
tag=${mode#--}${slice:+-$slice}; tag=${tag:+-$tag}
log="$logdir/gate$tag.log"
: > "$log"
run=""
st_all=1
finish() {
  [ -n "$run" ] && [ -d "$run" ] && del "$run" >/dev/null 2>&1
  printf '%s %s %s %s exit %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(git rev-parse --short HEAD)" \
    "$root" "${mode:-full}${slice:+ $slice}${STATIC_BASE:+ base=$STATIC_BASE}" "$st_all" >> "$logdir/runs.txt"
}
trap finish EXIT

on() { case "$steps" in *"$1"*) return 0 ;; esac; return 1; }
say() { echo "$*" | tee -a "$log"; }
red() { say "gate: FAIL ($1)"; st_all=1; exit 1; }
env75() { say "gate: ENVIRONMENT ($1)"; st_all=75; exit 75; }

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
  say "gate: leftover processes, killed"
  return 1
}
# one env.sh run of `sh -c SCRIPT sh ARGS…`, its output appended to the log. env.sh exits 2 "port …
# is busy" while another run holds the port: retry for up to 5 min, then report the environment.
envrun() {
  w=0
  while :; do
    out=$(mktemp "$run/env.XXXXXX") || exit 2
    ( export TMPDIR="$run"; tests/env.sh "$@" ) > "$out" 2>&1
    st=$?
    cat "$out" >> "$log"
    if [ "$st" -eq 2 ] && grep -q 'is busy' "$out"; then
      w=$((w + 15)); [ "$w" -gt 300 ] && env75 "port $port busy for 5 min"
      sleep 15; continue
    fi
    return "$st"
  done
}
show() { grep -E '^(ok|FAIL|skip|mutant|sweep)|^== |failed|passed|busy|^warning' "$1"; }

# ---- 1. static ---------------------------------------------------------------------------------
if on 1; then
  sh "$here/static.sh" >> "$log" 2>&1; st=$?
  grep -E '^(ok|FAIL)|static' "$log"
  [ "$st" -eq 0 ] || red static
fi
[ "$mode" = "--static" ] && { say "gate --static: all passed"; st_all=0; exit 0; }

# ---- 2. unit tests -----------------------------------------------------------------------------
if on 2; then
  u=$(mktemp "$tmp/pb-unit.XXXXXX") || exit 2
  cargo test -p okt-server -p okt-pygments -p okt-client > "$u" 2>&1; st=$?
  cat "$u" >> "$log"
  grep 'test result:' "$u"
  bad=$(grep -c 'FAILED' "$u")
  del "$u" >/dev/null 2>&1
  [ "$st" -eq 0 ] && [ "$bad" -eq 0 ] || red "cargo test exit $st"
  say "ok   unit tests"
fi

# ---- 3. the browser suites, one env.sh run ---------------------------------------------------------
if on 3; then
  run=$(mktemp -d "$tmp/pb-run.XXXXXX") || exit 2
  page=1
  case "$mode:$slice" in
    --probe:*) suites="" ;;
    --part:keys) suites="tests/ui/keys.mjs tests/ui/history.mjs"; page=0 ;;
    --part:panes) suites="tests/ui/panes.mjs" ;;
    *) suites="tests/ui/keys.mjs tests/ui/history.mjs tests/ui/panes.mjs" ;;
  esac
  for f in $suites; do [ -f "$f" ] || red "missing $f"; done
  mark=$(wc -l < "$log")
  # shellcheck disable=SC2016,SC2086
  envrun sh -c '
    r=0; page=$1; shift
    for s in "$@"; do
      t=$(date +%s); echo "== $s"; node "$OKT_ROOT/$s"; c=$?
      echo "== $s exit $c in $(( $(date +%s) - t )) s"; [ "$c" -eq 0 ] || r=1
    done
    if [ "$page" = 1 ]; then
      t=$(date +%s); echo "== page"; node "$OKT_ROOT/spec/preset-bar/verify/page.mjs"; c=$?
      echo "== page exit $c in $(( $(date +%s) - t )) s"; [ "$c" -eq 0 ] || r=1
    fi
    exit $r' sh "$page" $suites
  st=$?
  part=$(mktemp "$run/part.XXXXXX") || exit 2
  tail -n +"$((mark + 1))" "$log" > "$part"
  show "$part"
  lst=0; left || lst=1
  [ "$st" -eq 0 ] || red "browser suites exit $st"
  [ "$lst" -eq 0 ] || red "leftover processes"
  # the client build (env.sh's make client) printed no warning
  if grep -q '^warning' "$part"; then grep -A4 '^warning' "$part" | head -20; red "the client build printed a warning"; fi
  say "ok   the client build printed no warning"
  if [ "$mode" != "--probe" ]; then
    kref=$(git merge-base HEAD main) || red "no merge base with main"
    k=$(mktemp "$run/kept.XXXXXX") || exit 2
    # shellcheck disable=SC2086
    node "$here/kept.mjs" "$kref" "$part" $suites > "$k" 2>&1; kst=$?
    cat "$k" >> "$log"; cat "$k"
    [ "$kst" -eq 0 ] || red "an existing browser check was lost or failed"
    miss=0
    case " $suites " in *" tests/ui/panes.mjs "*) ;; *) : > "$run/none.txt" ;; esac
    while IFS= read -r label; do
      [ -f "$run/none.txt" ] && break
      case "$label" in ""|"#"*) continue ;; esac
      awk -v l="ok   $label" 'index($0, l) == 1 { f = 1 } END { exit !f }' "$part" \
        || { say "FAIL missing check: $label"; miss=$((miss + 1)); }
    done < "$here/checks.txt"
    [ "$miss" -eq 0 ] || red "$miss named checks missing"
    say "ok   named checks"
  fi
fi

# ---- m. the page mutants (self-test only) ----------------------------------------------------------
if [ "$mode" = "--self-test" ] && on m; then
  [ -n "$run" ] && [ -d "$run" ] || run=$(mktemp -d "$tmp/pb-run.XXXXXX") || exit 2
  ids=$(node -e 'import(process.argv[1]).then(m => console.log(Object.keys(m.MUTANTS).join(" ")))' "$here/mutants.mjs")
  [ -n "$ids" ] || red "no mutants"
  mark=$(wc -l < "$log")
  # shellcheck disable=SC2016,SC2086
  envrun sh -c '
    v="$OKT_ROOT/spec/preset-bar/verify"
    echo "== clean"; node "$v/page.mjs"; echo "== clean exit $?"
    for id in "$@"; do
      g=$(node -e "import(process.argv[1]).then(m => console.log(m.MUTANTS[process.argv[2]].groups))" "$v/mutants.mjs" "$id")
      echo "== mutant $id"; node "$v/page.mjs" --groups "$g" --mutant "$id"; echo "== mutant $id exit $?"
    done
    exit 0' sh $ids
  part=$(mktemp "$run/part.XXXXXX") || exit 2
  tail -n +"$((mark + 1))" "$log" > "$part"
  show "$part"
  lst=0; left || lst=1
  [ "$lst" -eq 0 ] || red "leftover processes"
  grep -q '^== clean exit 0$' "$part" || red "page.mjs is red on the clean page"
  survived=0
  for id in $ids; do
    exp=$(node -e 'import(process.argv[1]).then(m => console.log(m.MUTANTS[process.argv[2]].expect))' "$here/mutants.mjs" "$id")
    sect=$(awk -v s="== mutant $id" -v e="== mutant $id exit" 'index($0, e) == 1 { exit } f { print } index($0, s) == 1 && length($0) == length(s) { f = 1 }' "$part")
    ex=$(grep "^== mutant $id exit " "$part" | sed 's/.* exit //')
    if [ "$ex" = 1 ] && printf '%s\n' "$sect" | awk -v l="FAIL $exp" 'index($0, l) == 1 { f = 1 } END { exit !f }'; then
      say "ok   mutant $id killed"
    else
      say "FAIL mutant $id survived (exit ${ex:-none}; want a FAIL line \"$exp\")"; survived=$((survived + 1))
    fi
  done
  [ "$survived" -eq 0 ] || red "$survived page mutants survived"
fi

case "$mode" in --self-test) say "gate: SELF-TEST ran steps $steps only"; st_all=3; exit 3 ;; esac
say "gate${mode:+ $mode}: all passed"
st_all=0
