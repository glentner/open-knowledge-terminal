#!/bin/sh
# Static checks for the pane-arrangements gate (gate.sh step 1). Reads committed and staged state.
#
#   sh spec/pane-arrangements/verify/static.sh P<n> [--phase-only|--static]
#
#   1. committed state: nothing unstaged and nothing untracked anywhere in the repo (ignored files
#      aside), so the gate grades exactly what the phase commit will hold
#   2. the gate is frozen: spec/pane-arrangements/verify/ is exactly as the plan commit left it, in
#      the index and the working tree. An amendment is a new "[feature] Plan pane-arrangements"
#      commit, which moves the base.
#   3. no spec ids in the lines this cycle adds under okt-client/ and tests/, nor in the names of the
#      files it adds there (an R or P followed by 1-3 digits, standing alone; percentiles p50, p90,
#      p95 and p99 are not ids)
#   4. every existing check label of tests/ui/keys.mjs and tests/ui/history.mjs is still in the file
#      (kept.mjs; the gate's step 3 also requires each to pass)
#   5. from P2: README § Keys names the four presets and the cycle key
#   6. from P4: no rule for the Files rows (.trow) inside any @media block of app.css, at any depth
#   7. P5: the deferral ledger (ledger.sh)
#
# The base is the plan commit (the last "[feature] Plan pane-arrangements" commit), or STATIC_BASE.
# Footprint: git, awk and one node process, one at a time, under 3 s.
set -u
phase=${1:-}
case "$phase" in P[1-5]) ;; *) echo "usage: static.sh P<1-5>" >&2; exit 2 ;; esac
n=${phase#P}
here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
base=${STATIC_BASE:-$(git log -1 --format=%H --grep='^\[feature\] Plan pane-arrangements' 2>/dev/null)}
[ -n "$base" ] || { echo "static: no plan commit (set STATIC_BASE)"; exit 1; }
fails=0
bad() { echo "FAIL static: $1"; fails=$((fails + 1)); }

# ---- 1. committed state
dirty=$(git status --porcelain --untracked-files=all | grep -E '^(.[^ ]|\?\?)')
[ -z "$dirty" ] || bad "unstaged or untracked files:
$dirty"

# ---- 2. the gate is frozen
v=spec/pane-arrangements/verify
if ! git diff --quiet "$base" -- "$v" || ! git diff --cached --quiet "$base" -- "$v" \
   || [ -n "$(git ls-files --others --exclude-standard -- "$v")" ]; then
  bad "$v differs from the plan commit $(git rev-parse --short "$base"):
$(git diff --stat "$base" -- "$v"; git ls-files --others --exclude-standard -- "$v")"
fi

# ---- 3. spec ids in added lines and added file names
ids='(^|[^A-Za-z0-9])[RrPp][0-9]{1,3}([^A-Za-z0-9]|$)'
pct='s/(^|[^A-Za-z0-9])[Pp](50|90|95|99)([^A-Za-z0-9]|$)/\1\3/g'
hits=$(git diff --cached -U0 "$base" -- okt-client tests | grep '^+' | grep -v '^+++' \
  | sed -E "$pct" | sed -E "$pct" | grep -E "$ids")
[ -z "$hits" ] || bad "spec ids in added lines:
$hits"
names=$(git diff --cached --name-only --diff-filter=A "$base" -- okt-client tests \
  | sed -E "$pct" | grep -E "$ids")
[ -z "$names" ] || bad "spec ids in added file names: $names"

# ---- 4. the existing browser checks are still there
kref=$(git merge-base HEAD main 2>/dev/null) || kref=""
if [ -n "$kref" ]; then node "$here/kept.mjs" "$kref" || bad "an existing browser check was removed"
else bad "no merge base with main"; fi

# ---- 5. README § Keys names the presets and the cycle key
if [ "$n" -ge 2 ]; then
  keys=$(awk '/^## Keys/ {k=1; next} /^## / {k=0} k' README.md | tr 'A-Z' 'a-z')
  for w in classic swapped stacked three '`p`'; do
    case "$keys" in *"$w"*) ;; *) bad "README § Keys does not name $w" ;; esac
  done
fi

# ---- 6. no window-scoped rule for the Files rows (comments stripped; @media nested in
# @container or @supports counts too)
if [ "$n" -ge 4 ]; then
  win=$(awk '
    BEGIN { depth = 0; mdepth = -1 }
    {
      line = $0
      while (1) {
        if (incomment) {
          if (match(line, /\*\//)) { line = substr(line, RSTART + 2); incomment = 0 } else { line = ""; break }
        } else if (match(line, /\/\*/)) {
          pre = substr(line, 1, RSTART - 1); rest = substr(line, RSTART + 2)
          if (match(rest, /\*\//)) line = pre " " substr(rest, RSTART + 2); else { line = pre; incomment = 1; break }
        } else break
      }
      if (mdepth < 0 && line ~ /@media/) mdepth = depth
      o = gsub(/\{/, "{", line); c = gsub(/\}/, "}", line)
      if (mdepth >= 0 && (depth > mdepth || o > 0) && line ~ /\.trow/) print FILENAME ":" NR ": " $0
      depth += o - c
      if (mdepth >= 0 && depth <= mdepth) mdepth = -1
    }' okt-client/static/app.css)
  [ -z "$win" ] || bad "a .trow rule inside @media:
$win"
fi

# ---- 7. the deferral ledger
if [ "$n" -eq 5 ]; then
  sh "$here/ledger.sh" || bad "ledger"
fi

[ "$fails" -eq 0 ] && echo "ok   static $phase" || echo "static: $fails failed"
[ "$fails" -eq 0 ]
