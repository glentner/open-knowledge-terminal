#!/bin/sh
# The phase gate for drag-to-dock. TECH.md's verify lines call it.
#
#   sh spec/drag-to-dock/verify/gate.sh P<n>                      every part the phase has, in order
#   sh spec/drag-to-dock/verify/gate.sh P<n> --part NAME          one part alone (build's targeted and
#                                                                 mutant runs; not a phase verdict)
#   sh spec/drag-to-dock/verify/gate.sh P<n> --part dock-a --groups "sweep-three-1600"
#                                                                 one part, the probe narrowed to the
#                                                                 named groups (prefixes allowed)
#   sh spec/drag-to-dock/verify/gate.sh P<n> --static             the static part alone (pre-commit)
#
# Parts, by phase, stopping at the first red:
#   static  P1-P4  static.sh: committed state, the frozen gate, the spec-id scan of added lines,
#                  untouched paths; from P2 preset-bar under the branch, keys.mjs and panes.mjs
#                  unchanged, README § Keys; P4 the suite copied into tests/ui, README § Tests, ledger
#   unit    P1-P4  the profile's cargo tests, then required.txt's tests by exact name, all passing
#   oracle  P1-P4  dock-oracle-test.mjs: the oracle on hand-worked cases (node, no browser)
#   suites  P2-P4  one tests/env.sh run: tests/ui/keys.mjs and tests/ui/panes.mjs, each exit 0, no
#                  FAIL line, no skip line
#   dock-a  P2-P4  one tests/env.sh run of dock.mjs: the four sweeps at 1600 px and sources
#   dock-b  P2-P4  the four sweeps at 900 px
#   dock-c  P2-P4  the cancels at both widths
#   dock-d  P2-P4  click (P3+: keyview)
#   dock-e  P2-P4  refuse (P3+: state)
# Each dock part is one env.sh run sized to stay under 10 minutes (TECH.md § Gate timings).
# A dock part passes when every checks.txt label of its groups for phases up to P<n> printed
# "ok   <group>: <label>", no structural FAIL printed (an unknown group, a group that ran no check,
# a group that didn't run to the end, page errors), and, from P3, the probe exited 0.
#
# Logs, kept: $REPO/.local/drag-to-dock/<worktree>-P<n>.log (this summary; a --part run writes
# <worktree>-P<n>-<part>-only.log instead) and <worktree>-P<n>-<part>.log (each part's full
# output, rewritten by every run of that part), where $REPO is the main checkout (the directory
# holding the common .git) and .local/ is git-excluded.
# Exit: 0 green; 1 red; 2 usage; 75 environment (the port busy for 5 min).
# Footprint: one part at a time. Peak: cargo (CARGO_BUILD_JOBS=4), or one okt server plus one
# Chromium (about 8 processes, about 1.5 GB). Wall time: TECH.md § Gate timings.
# Needs, exported by the caller: OKT_TEST_PORT (never 6580), WASM_BINDGEN, PLAYWRIGHT_BROWSERS_PATH,
# PLAYWRIGHT_MODULE, TMPDIR. Runs with the sandbox off: env.sh binds the port, and pgrep needs it.
main() {
  phase=${1:-}
  case "$phase" in P[1-4]) ;; *) echo "usage: gate.sh P<1-4> [--part NAME [--groups \"G …\"]|--static]" >&2; return 2 ;; esac
  shift
  only="" narrow=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --part) only=${2:-}; shift 2 || return 2 ;;
      --groups) narrow=${2:-}; shift 2 || return 2 ;;
      --static) only=static; shift ;;
      *) echo "gate: unknown argument $1" >&2; return 2 ;;
    esac
  done
  case "$only" in ""|static|unit|oracle|suites|dock-a|dock-b|dock-c|dock-d|dock-e) ;; *) echo "gate: unknown part $only" >&2; return 2 ;; esac
  [ -n "$narrow" ] && case "$only" in dock-*) ;; *) echo "gate: --groups needs --part dock-a|dock-b|dock-c|dock-d|dock-e" >&2; return 2 ;; esac
  n=${phase#P}
  here=$(cd "$(dirname "$0")" && pwd)
  root=$(git -C "$here" rev-parse --show-toplevel) || return 2
  cd "$root" || return 2
  port=${OKT_TEST_PORT:-}
  [ -n "$port" ] || { echo "gate: export OKT_TEST_PORT (this cycle's port)" >&2; return 2; }
  [ "$port" = 6580 ] && { echo "gate: never port 6580" >&2; return 2; }
  export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}
  common=$(git rev-parse --git-common-dir) || return 2
  logdir=$(cd "$common/.." && pwd)/.local/drag-to-dock
  mkdir -p "$logdir" || return 2
  tag=$(basename "$root")
  # a phase run's summary, or a part run's own, so a targeted run never overwrites the phase's
  sum="$logdir/$tag-$phase${only:+-$only-only}.log"
  : > "$sum"
  say "gate $phase on $(git rev-parse --short HEAD) in $root, $(date '+%Y-%m-%d %H:%M:%S')${only:+, part $only}${narrow:+, groups $narrow}"

  parts="static unit oracle"
  [ "$n" -ge 2 ] && parts="$parts suites dock-a dock-b dock-c dock-d dock-e"
  [ -n "$only" ] && parts=$only
  for part in $parts; do
    t0=$(date +%s)
    plog="$logdir/$tag-$phase-$part.log"
    case "$part" in
      static) sh "$here/static.sh" "$phase" > "$plog" 2>&1; st=$? ;;
      unit) unit > "$plog" 2>&1; st=$? ;;
      oracle) node "$here/dock-oracle-test.mjs" > "$plog" 2>&1; st=$? ;;
      suites) suites > "$plog" 2>&1; st=$? ;;
      dock-*) dock "$part" > "$plog" 2>&1; st=$? ;;
    esac
    grep -E '^(ok|FAIL|skip|noted|time)|^== ' "$plog" >> "$sum"
    grep -E '^(FAIL|noted)' "$plog" | head -40
    say "part $part: $([ "$st" -eq 0 ] && echo green || echo "red ($st)") in $(( $(date +%s) - t0 )) s; log $plog"
    [ "$st" -eq 75 ] && { say "gate $phase: ENVIRONMENT (port $port busy for 5 min)"; return 75; }
    [ "$st" -eq 0 ] || { say "gate $phase: FAIL (part $part)"; return 1; }
  done
  if [ -n "$only" ]; then say "gate $phase: part $only passed${narrow:+ (groups $narrow)}; not a phase verdict"
  else say "gate $phase: all passed"; fi
  return 0
}

say() { echo "$1"; echo "$1" >> "$sum"; }

# rows of a P<n>|… file up to this phase (comments skipped)
rows() {
  while IFS='|' read -r p rest; do
    case "$p" in P[1-4]) ;; *) continue ;; esac
    [ "${p#P}" -le "$n" ] && printf '%s\n' "$rest"
  done < "$1"
}

unit() {
  log=$(mktemp "${TMPDIR:-/tmp}/dd-unit.XXXXXX")
  cargo test -p okt-server -p okt-pygments -p okt-client > "$log" 2>&1; st=$?
  grep 'test result:' "$log"
  if [ "$st" -ne 0 ] || grep -q 'FAILED' "$log"; then tail -40 "$log"; echo "FAIL unit: cargo test exit $st"; del "$log" >/dev/null 2>&1; return 1; fi
  names=$(rows "$here/required.txt" | tr '\n' ' ')
  want=$(printf '%s\n' $names | grep -c .)
  # one run, exact names: every required test exists, is not ignored, and passes
  # shellcheck disable=SC2086
  cargo test -q -p okt-client -- --exact $names > "$log" 2>&1; st=$?
  got=$(sed -n 's/.*test result: ok\. \([0-9][0-9]*\) passed; 0 failed; 0 ignored.*/\1/p' "$log" | head -1)
  if [ "$st" -eq 0 ] && [ "${got:-0}" -eq "$want" ]; then echo "ok   required tests $got/$want"; r=0
  else tail -20 "$log"; echo "FAIL unit: required tests ${got:-0}/$want (names in required.txt)"; r=1; fi
  del "$log" >/dev/null 2>&1
  return $r
}

# One tests/env.sh run of "$@" under a private TMPDIR, so the throwaway server (its --kb is under
# it) and Chromium (its profile is under it) carry it in their argv; leftovers are matched by that
# path alone and killed, and fail the part. env.sh exits 2 "port … is busy" while another run holds
# the port: retry every 10 s for 5 min, then report the environment (75), never a test failure.
envrun() {
  run=$(mktemp -d "${TMPDIR:-/tmp}/dd-run.XXXXXX") || return 2
  # one slash between parts, as the paths the server and Chromium are given will read
  run=$(printf '%s' "$run" | sed 's#//*#/#g')
  w=0
  while :; do
    ( export TMPDIR="$run"; tests/env.sh "$@" ) > "$run.out" 2>&1; est=$?
    cat "$run.out"
    if [ "$est" -eq 2 ] && grep -q 'is busy' "$run.out"; then
      w=$((w + 10)); [ "$w" -gt 300 ] && { del "$run.out" >/dev/null 2>&1; del "$run" >/dev/null 2>&1; return 75; }
      sleep 10; continue
    fi
    break
  done
  del "$run.out" >/dev/null 2>&1
  i=0; new=""
  while :; do
    new=$(pgrep -f -- "$run" 2>/dev/null)
    [ -z "$new" ] && break
    i=$((i + 1)); [ "$i" -ge 10 ] && break
    sleep 0.5
  done
  if [ -n "$new" ]; then
    echo "$new" | while read -r p; do ps -o pid=,command= -p "$p"; kill "$p" 2>/dev/null; done
    echo "FAIL leftover processes, killed"
    est=1
  fi
  del "$run" >/dev/null 2>&1
  return $est
}

suites() {
  kref=$(git merge-base HEAD main 2>/dev/null) || { echo "FAIL suites: no merge base with main"; return 1; }
  if git show "$kref:okt-client/static/index.html" 2>/dev/null | grep -q 'id="presets"'; then
    echo "FAIL suites: preset-bar is not under this branch; rebase onto main once it lands"; return 1
  fi
  git diff --quiet "$kref" -- tests/ui/keys.mjs tests/ui/panes.mjs \
    || { echo "FAIL suites: keys.mjs or panes.mjs differs from main"; return 1; }
  out=$(mktemp "${TMPDIR:-/tmp}/dd-suites.XXXXXX")
  # shellcheck disable=SC2016
  envrun sh -c '
    r=0
    for s in tests/ui/keys.mjs tests/ui/panes.mjs; do
      echo "== $s"; node "$OKT_ROOT/$s"; c=$?; echo "== $s exit $c"; [ "$c" -eq 0 ] || r=1
    done
    exit $r' > "$out" 2>&1
  st=$?
  cat "$out"
  [ "$st" -eq 75 ] && { del "$out" >/dev/null 2>&1; return 75; }
  r=0
  [ "$st" -eq 0 ] || { echo "FAIL suites: exit $st"; r=1; }
  # each suite ran to its end and printed its own exit line, with no FAIL and no skip line
  for f in keys panes; do grep -q "^== tests/ui/$f.mjs exit 0" "$out" || { echo "FAIL suites: $f.mjs did not exit 0"; r=1; }; done
  if grep -E '^(FAIL|skip)' "$out" > /dev/null; then echo "FAIL suites: a FAIL or skip line (above)"; r=1; fi
  del "$out" >/dev/null 2>&1
  return $r
}

dock() {
  part=$1
  case "$part" in
    dock-a) groups="sweep-classic-1600 sweep-swapped-1600 sweep-stacked-1600 sweep-three-1600 sources" ;;
    dock-b) groups="sweep-classic-900 sweep-swapped-900 sweep-stacked-900 sweep-three-900" ;;
    dock-c) groups="cancel-1600 cancel-900" ;;
    dock-d) groups="click"; [ "$n" -ge 3 ] && groups="$groups keyview" ;;
    dock-e) groups="refuse"; [ "$n" -ge 3 ] && groups="$groups state" ;;
  esac
  if [ -n "$narrow" ]; then
    keep=""
    for g in $groups; do for a in $narrow; do case "$g" in "$a"*) keep="$keep $g"; break ;; esac; done; done
    groups=$keep
    [ -n "$groups" ] || { echo "FAIL dock: --groups $narrow names no group of $part"; return 2; }
  fi
  out=$(mktemp "${TMPDIR:-/tmp}/dd-dock.XXXXXX")
  # shellcheck disable=SC2016
  export DOCK_GROUPS="$groups"
  envrun sh -c 'node "$OKT_ROOT/spec/drag-to-dock/verify/dock.mjs"; c=$?; echo "== dock exit $c"; exit $c' > "$out" 2>&1
  st=$?
  cat "$out"
  [ "$st" -eq 75 ] && { del "$out" >/dev/null 2>&1; return 75; }
  r=0
  # structural failures are always red
  if grep -E '^FAIL (unknown group|settings key|leftover)|^FAIL [a-z0-9-]+: (no check ran|ran to the end)|^FAIL all: ' "$out"; then r=1; fi
  # every label of these groups up to this phase printed ok (a prefix at column 0)
  for g in $groups all; do
    rows "$here/checks.txt" | while IFS='|' read -r cg label; do
      [ "$cg" = "$g" ] || continue
      awk -v l="ok   $g: $label" 'index($0, l) == 1 { f = 1 } END { exit !f }' "$out" \
        || echo "FAIL missing check: $g: $label"
    done
  done > "$out.miss"
  if [ -s "$out.miss" ]; then cat "$out.miss"; r=1; fi
  # a FAIL on a later phase's label is noted, not red, before P3; from P3 the probe must exit 0
  if [ "$n" -ge 3 ]; then [ "$st" -eq 0 ] || { echo "FAIL dock: the probe exited $st"; r=1; }
  else grep '^FAIL' "$out" | sed 's/^FAIL/noted/'; fi
  del "$out" >/dev/null 2>&1; del "$out.miss" >/dev/null 2>&1
  [ "$r" -eq 0 ] && echo "ok   dock $part: $(echo $groups)"
  return $r
}

main "$@"; exit $?
