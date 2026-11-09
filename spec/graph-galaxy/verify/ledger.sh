#!/bin/sh
# The deferral ledger (code-plan Step 6.8): every deferral the cycle's records mention names an
# issues/ seed that exists, is unshaped (or later), and has a ROADMAP.md entry; and every seed
# the cycle added is indexed.
#
#   sh spec/graph-galaxy/verify/ledger.sh
#
# Reads prose and list items of spec/graph-galaxy/{TECH,PLAN,REVIEW}.md, never Markdown headings
# (REVIEW.md's "Out of scope (recorded, not blocking)" heading is not a deferral) and never the
# YAML frontmatter. A deviation-table row (a line starting "| D<n> |") records an accepted reading,
# not deferred work, and the checklist item naming the "deferral ledger" describes this check;
# both are skipped and nothing else is. The words "not deferred" are not a deferral.
# Footprint: one awk/grep at a time, < 1 s.
set -u
root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
base=${STATIC_BASE:-$(git log -1 --format=%H --grep='^\[feature\] Plan graph-galaxy' 2>/dev/null)}
fails=0

for f in spec/graph-galaxy/TECH.md spec/graph-galaxy/PLAN.md spec/graph-galaxy/REVIEW.md; do
  [ -f "$f" ] || continue
  # body units only: drop a leading frontmatter block and headings; join a paragraph or a list
  # item with its wrapped continuation lines, so a deferral and its seed may sit on different
  # lines of one item; each table row is its own unit
  out=$(awk 'NR==1 && $0=="---" {fm=1; next} fm && $0=="---" {fm=0; next} fm {next}
       function flush() { if (u != "") { gsub(/[[:space:]]+/, " ", u); print FILENAME ":" start ": " u } u = "" }
       /^#/ {flush(); next}
       /^[[:space:]]*$/ {flush(); next}
       /^[[:space:]]*([-*]|[0-9]+\.) / || /^\|/ {flush(); u = $0; start = NR; next}
       { if (u == "") start = NR; u = u " " $0 }
       END {flush()}' "$f" \
  | grep -Ei 'defer|follow-up|follow up|known limitation|do not fix here|not fixed (here|in this)|later cycle|next cycle|future work|left for later|out of scope' \
  | grep -v ': | D[0-9]* |' | grep -v 'deferral ledger' \
  | while IFS= read -r line; do
      # "not deferred" says the opposite (REVIEW.md's triage: "R5 is fixed, not deferred"): a
      # unit counts only when a deferral phrase is left once those words are taken out
      printf '%s\n' "$line" | sed -E 's/not deferred//g' \
        | grep -Eqi 'defer|follow-up|follow up|known limitation|do not fix here|not fixed (here|in this)|later cycle|next cycle|future work|left for later|out of scope' \
        || continue
      seeds=$(printf '%s\n' "$line" | grep -Eo 'issues/[a-z0-9-]+\.md' | sort -u)
      if [ -z "$seeds" ]; then echo "FAIL ledger: deferral names no issues/ seed: $line"; echo x; continue; fi
      for s in $seeds; do
        [ -f "$s" ] || { echo "FAIL ledger: $s missing ($line)"; echo x; continue; }
        grep -q "$s" ROADMAP.md || { echo "FAIL ledger: $s has no ROADMAP.md entry"; echo x; }
        grep -Eq '^status: (unshaped|shaped|adopted:|declined|accepted-behaviour)' "$s" \
          || { echo "FAIL ledger: $s has no valid status"; echo x; }
      done
    done)
  # the loop runs in a subshell: it reports through its output, one "x" line per failure
  fails=$((fails + $(printf '%s\n' "$out" | grep -c '^x$')))
  printf '%s\n' "$out" | grep -v '^x$' | grep . || true
done

# every seed this cycle added is indexed
if [ -n "$base" ]; then
  for s in $(git diff --name-only --diff-filter=A "$base" -- issues 2>/dev/null); do
    grep -q "$s" ROADMAP.md || { echo "FAIL ledger: added seed $s has no ROADMAP.md entry"; fails=$((fails + 1)); }
  done
fi

[ "$fails" -eq 0 ] && echo "ok   ledger" || echo "ledger: $fails failed"
[ "$fails" -eq 0 ]
