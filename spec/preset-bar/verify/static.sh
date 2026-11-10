#!/bin/sh
# Static checks for the preset-bar gate (gate.sh step 1). Reads committed and staged state.
# Frozen with the plan commit.
#
#   sh spec/preset-bar/verify/static.sh
#
#   1. committed state: nothing unstaged and nothing untracked anywhere in the repo (ignored files
#      aside), so the gate grades exactly what the phase commit will hold
#   2. the gate is frozen: spec/preset-bar/verify/ is exactly as the plan commit left it, in the
#      index and the working tree. An amendment is a new "[fix] Plan preset-bar" commit.
#   3. no spec ids in the lines this cycle adds under okt-client/ and tests/, nor in the names of the
#      files it adds there (an R or P followed by 1-3 digits, standing alone; p50/p90/p95/p99 aside)
#   4. R6: README § Keys still names every preset key, the cycle key and the pane leader's keys
#   5. R7: tests/ui/panes.mjs reads no preset button (#presets, data-preset, the .pre class), and
#      its PRESETS header block names no window bar and no button
#   6. R7: no tracked prose under README.md, AGENTS.md, tests/ and okt-client/ names a window bar,
#      preset buttons, a preset bar or the fkeys row (the cycle's records, spec/, issues/ and
#      ROADMAP.md, describe history and are not read)
#   7. every existing browser check is still in its suite, the replaced one gone (kept.mjs); and,
#      since a check can be loosened under its old label: tests/ui/keys.mjs and history.mjs are
#      unchanged since the plan commit, and every line the cycle removes from tests/ui/panes.mjs is
#      one panes-removed.txt lists (the lines PLAN § 2.3 replaces), no more often than it lists it
#   8. the deferral ledger (ledger.sh)
#   9. nothing of the bar is left in the client: no data-preset or id="presets" under okt-client/,
#      no .fkeys, .presets or .pre rule in app.css
#
# The base is the plan commit (the last "[fix] Plan preset-bar" commit), or STATIC_BASE.
# Footprint: git, awk, grep and one node process, one at a time, under 3 s.
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
# the plan commit, by its subject alone (a body line can't move the base)
base=${STATIC_BASE:-$(git log --format='%H%x09%s' 2>/dev/null | awk -F'\t' 'index($2, "[fix] Plan preset-bar:") == 1 { print $1; exit }')}
[ -n "$base" ] || { echo "FAIL static: no plan commit (set STATIC_BASE)"; exit 1; }
fails=0
bad() { echo "FAIL static: $1"; fails=$((fails + 1)); }

# ---- 1. committed state
dirty=$(git status --porcelain --untracked-files=all | grep -E '^(.[^ ]|\?\?)')
[ -z "$dirty" ] || bad "unstaged or untracked files:
$dirty"

# ---- 2. the gate is frozen
v=spec/preset-bar/verify
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

# ---- 4. README § Keys names every key (each as a code span)
keys=$(git show :README.md | awk '/^## Keys/ {k=1; next} /^## / {k=0} k')
for w in '`;`' "\`'\`" '`\`' '`` ` ``' '`p`' '`w`' '`w w`' '`w W`' '`w d`' '`w x`' '`w r`' '`w q`' '`w v`' '`w V`'; do
  case "$keys" in *"$w"*) ;; *) bad "README § Keys does not name $w" ;; esac
done

# ---- 5. panes.mjs reads no preset button; its PRESETS block names no window bar
pm=$(git show :tests/ui/panes.mjs)
reads=$(printf '%s\n' "$pm" | grep -n -E '#presets|data-preset|\.pre([^A-Za-z0-9_-]|$)')
[ -z "$reads" ] || bad "tests/ui/panes.mjs reads a preset button:
$reads"
block=$(printf '%s\n' "$pm" | awk '/^\/\/   PRESETS / {p=1; print; next} p && /^\/\/   [A-Z][A-Z ]+ / {p=0} p && !/^\/\// {p=0} p')
[ -n "$block" ] || bad "tests/ui/panes.mjs has no PRESETS header block"
printf '%s\n' "$block" | grep -q -i -E 'window bar|button' && bad "the PRESETS header block names a window bar or a button:
$block"

# ---- 6. prose naming the bar
words='window bar|preset buttons?|preset bar|presets bar|fkeys'
prose=$(git grep --cached -n -i -E "$words" -- README.md AGENTS.md tests okt-client ':!tests/node_modules' 2>/dev/null)
[ -z "$prose" ] || bad "prose names the window bar or preset buttons:
$prose"

# ---- 7. the existing browser checks are still there
kref=$(git merge-base HEAD main 2>/dev/null) || kref=""
if [ -n "$kref" ]; then node "$here/kept.mjs" "$kref" || bad "an existing browser check was removed"
else bad "no merge base with main"; fi

# only the planned lines leave panes.mjs, and the other suites are untouched
git diff --cached --quiet "$base" -- tests/ui/keys.mjs tests/ui/history.mjs \
  || bad "tests/ui/keys.mjs or history.mjs changed since the plan commit"
gone=$(git diff --cached -U0 "$base" -- tests/ui/panes.mjs | grep '^-' | grep -v '^---' \
  | awk -v allow="$here/panes-removed.txt" 'BEGIN { while ((getline l < allow) > 0) n[l]++ }
      { if (n[$0] > 0) n[$0]--; else print }')
[ -z "$gone" ] || bad "tests/ui/panes.mjs lost lines the plan does not replace:
$gone"

# ---- 8. the deferral ledger
sh "$here/ledger.sh" || bad "ledger"

# ---- 9. nothing of the bar left in the client
left=$(git grep --cached -n -E 'data-preset|id="presets"' -- okt-client; git show :okt-client/static/app.css \
  | grep -n -E '\.(fkeys|presets|pre)([^A-Za-z0-9_-]|$)')
[ -z "$left" ] || bad "the bar is left in the client:
$left"

[ "$fails" -eq 0 ] && echo "ok   static" || echo "static: $fails failed"
[ "$fails" -eq 0 ]
