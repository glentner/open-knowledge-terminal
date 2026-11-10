#!/bin/sh
# Static checks for the drag-to-dock gate (gate.sh part "static"). Reads committed and staged state.
#
#   sh spec/drag-to-dock/verify/static.sh P<n>
#
#   1. committed state: nothing unstaged and nothing untracked (ignored files aside), so the gate
#      grades exactly what the phase commit holds
#   2. the gate is frozen: spec/drag-to-dock/verify/ is exactly as the plan commit left it
#   3. no spec ids in the lines this cycle adds under okt-client/ and tests/, nor in the names of
#      the files it adds there (an R, P or Q followed by 1-3 digits, standing alone; the
#      percentiles p50, p90, p95 and p99 are not ids)
#   4. the paths the plan leaves alone are untouched: okt-client/src/state.rs (high-blast-radius;
#      no change is planned) and okt-server/ (no server change, a non-goal)
#   5. from P2: preset-bar is under this branch (the merge base with main has no #presets row),
#      and tests/ui/keys.mjs and tests/ui/panes.mjs are byte-identical to that merge base
#   6. from P2: README § Keys tells of dragging a tab to dock its view
#   7. P4: tests/ui/dock.mjs and tests/ui/dock-oracle.mjs are byte-identical to this directory's
#      copies; README § Tests names dock.mjs; the deferral ledger (ledger.sh)
#
# The frozen check compares with the last "[feature] Plan drag-to-dock" commit (an approved plan
# amendment refreezes the gate); the scans of added lines and untouched paths start from the first
# one, so an amendment never hides earlier changes. STATIC_BASE overrides both (plan dry runs).
# The gate grades the index: build stages the phase's files by path, runs the gate, then commits.
# Footprint: git, awk and sh, one at a time, under 3 s.
main() {
  phase=${1:-}
  case "$phase" in P[1-4]) ;; *) echo "usage: static.sh P<1-4>" >&2; return 2 ;; esac
  n=${phase#P}
  here=$(cd "$(dirname "$0")" && pwd)
  root=$(git -C "$here" rev-parse --show-toplevel) || return 2
  cd "$root" || return 2
  base=${STATIC_BASE:-$(git log -1 --format=%H --grep='^\[feature\] Plan drag-to-dock' 2>/dev/null)}
  first=${STATIC_BASE:-$(git log --reverse --format=%H --grep='^\[feature\] Plan drag-to-dock' 2>/dev/null | head -1)}
  [ -n "$base" ] && [ -n "$first" ] || { echo "FAIL static: no plan commit (set STATIC_BASE)"; return 1; }
  fails=0

  # ---- 1. committed state
  dirty=$(git status --porcelain --untracked-files=all | grep -E '^(.[^ ]|\?\?)')
  [ -z "$dirty" ] || bad "unstaged or untracked files:
$dirty"

  # ---- 2. the gate is frozen
  v=spec/drag-to-dock/verify
  if ! git diff --quiet "$base" -- "$v" || ! git diff --cached --quiet "$base" -- "$v" \
     || [ -n "$(git ls-files --others --exclude-standard -- "$v")" ]; then
    bad "$v differs from the plan commit $(git rev-parse --short "$base"):
$(git diff --stat "$base" -- "$v"; git ls-files --others --exclude-standard -- "$v")"
  fi

  # ---- 3. spec ids in added lines and added file names
  ids='(^|[^A-Za-z0-9])[RrPpQq][0-9]{1,3}([^A-Za-z0-9]|$)'
  pct='s/(^|[^A-Za-z0-9])[Pp](50|90|95|99)([^A-Za-z0-9]|$)/\1\3/g'
  hits=$(git diff --cached -U0 "$first" -- okt-client tests | grep '^+' | grep -v '^+++' \
    | sed -E "$pct" | sed -E "$pct" | grep -E "$ids")
  [ -z "$hits" ] || bad "spec ids in added lines:
$hits"
  names=$(git diff --cached --name-only --diff-filter=A "$first" -- okt-client tests \
    | sed -E "$pct" | grep -E "$ids")
  [ -z "$names" ] || bad "spec ids in added file names: $names"

  # ---- 4. untouched paths
  for p in okt-client/src/state.rs okt-server; do
    git diff --cached --quiet "$first" -- "$p" || bad "$p changed; the plan changes none of it"
  done

  # ---- 5. on top of preset-bar, with the existing suites unchanged
  if [ "$n" -ge 2 ]; then
    kref=$(git merge-base HEAD main 2>/dev/null) || kref=""
    if [ -z "$kref" ]; then bad "no merge base with main"
    else
      if git show "$kref:okt-client/static/index.html" 2>/dev/null | grep -q 'id="presets"'; then
        bad "preset-bar is not under this branch (main's merge base still has #presets): rebase onto main once it lands"
      fi
      git diff --cached --quiet "$kref" -- tests/ui/keys.mjs tests/ui/panes.mjs \
        || bad "tests/ui/keys.mjs or tests/ui/panes.mjs differs from main ($(git rev-parse --short "$kref")): their checks stay as they are"
    fi
  fi

  # ---- 6. README § Keys tells of the drag
  if [ "$n" -ge 2 ]; then
    keys=$(git show :README.md | awk '/^## Keys/ {k=1; next} /^## / {k=0} k' | tr 'A-Z' 'a-z')
    case "$keys" in *drag*tab*|*tab*drag*) ;; *) bad "README § Keys does not tell of dragging a tab" ;; esac
  fi

  # ---- 7. the suite in the repo, its README line, the ledger
  if [ "$n" -ge 4 ]; then
    for f in dock.mjs dock-oracle.mjs; do
      if ! git show ":tests/ui/$f" > /dev/null 2>&1; then bad "tests/ui/$f is not committed"
      elif [ "$(git show ":tests/ui/$f" | git hash-object --stdin)" != "$(git show "$base:$v/$f" | git hash-object --stdin)" ]; then
        bad "tests/ui/$f differs from $v/$f"
      fi
    done
    git show :README.md | awk '/^## Tests/ {k=1; next} /^## / {k=0} k' | grep -q 'dock\.mjs' \
      || bad "README § Tests does not name dock.mjs"
    sh "$here/ledger.sh" || bad "ledger"
  fi

  [ "$fails" -eq 0 ] && echo "ok   static $phase" || echo "static: $fails failed"
  [ "$fails" -eq 0 ]
}
bad() { echo "FAIL static: $1"; fails=$((fails + 1)); }
main "$@"; exit $?
