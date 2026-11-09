#!/bin/sh
# The deferral ledger (code-plan Step 6.8): every deferral the cycle's records mention names an
# issues/ seed that exists, has a valid status and a ROADMAP.md entry; every seed the cycle added is
# indexed.
#
#   sh spec/pane-arrangements/verify/ledger.sh               check the cycle's records
#   sh spec/pane-arrangements/verify/ledger.sh --self-test   check this script on fixtures
#
# Reads prose units (paragraphs, list items with their wrapped lines, table rows) of
# spec/pane-arrangements/{TECH,PLAN,REVIEW}.md. Never reads Markdown headings, the YAML
# frontmatter, fenced code or inline code spans, so quoting the vocabulary (`defer`) or a heading
# like "Out of scope (recorded, not blocking)" never counts. Negations ("not deferred", "nothing
# deferred", "no deferral", "no deferrals") and the name of this check ("deferral ledger") are removed
# before matching. A unit naming this script (verify/ledger.sh) describes the check and is skipped;
# nothing else is.
# Footprint: awk and grep, one at a time, under 1 s.
set -u
here=$(cd "$(dirname "$0")" && pwd)
words='defer|follow-up|follow up|known limitation|do not fix here|not fixed (here|in this)|later cycle|next cycle|future work|left for later|out of scope'

units() {
  awk 'NR==1 && $0=="---" {fm=1; next} fm && $0=="---" {fm=0; next} fm {next}
       /^[[:space:]]*```/ {flush(); code=!code; next} code {next}
       function flush() { if (u ~ /verify\/ledger\.sh/) u = ""; if (u != "") { gsub(/`[^`]*`/, "", u); gsub(/[[:space:]]+/, " ", u); print FILENAME ":" start ": " u } u = "" }
       /^#/ {flush(); next}
       /^[[:space:]]*$/ {flush(); next}
       /^[[:space:]]*([-*]|[0-9]+\.) / || /^\|/ {flush(); u = $0; start = NR; next}
       { if (u == "") start = NR; u = u " " $0 }
       END {flush()}' "$1"
}

# check FILES… against ROADMAP (in the current directory); prints failures, returns their count > 0
check() {
  out=$(for f in "$@"; do [ -f "$f" ] && units "$f"; done \
    | grep -Ei "$words" \
    | while IFS= read -r line; do
        printf '%s\n' "$line" | sed -E 's/(not|nothing|no) (deferred|deferrals?)//gI; s/deferral ledger//gI' | grep -Eqi "$words" || continue
        seeds=$(printf '%s\n' "$line" | grep -Eo 'issues/[a-z0-9-]+\.md' | sort -u)
        if [ -z "$seeds" ]; then echo "FAIL ledger: deferral names no issues/ seed: $line"; continue; fi
        for s in $seeds; do
          if [ ! -f "$s" ]; then echo "FAIL ledger: $s missing ($line)"; continue; fi
          grep -q "$s" ROADMAP.md || echo "FAIL ledger: $s has no ROADMAP.md entry"
          grep -Eq '^status: (unshaped|shaped|adopted:|declined|accepted-behaviour)' "$s" \
            || echo "FAIL ledger: $s has no valid status"
        done
      done)
  [ -n "$out" ] && printf '%s\n' "$out"
  [ -z "$out" ]
}

if [ "${1:-}" = "--self-test" ]; then
  lab=$(mktemp -d "${TMPDIR:-/tmp}/pa-ledger.XXXXXX") || exit 2
  (
    cd "$lab" || exit 2
    mkdir issues
    printf -- '---\nstatus: unshaped\n---\n# Seed\n' > issues/seeded.md
    printf -- '# Roadmap\n\n**Seed:** [x](issues/seeded.md)\n' > ROADMAP.md
    printf -- '## Out of scope (recorded, not blocking)\n\n- The last phase carries the deferral ledger.\n- Fixed in remediation 1, not deferred.\n- The amendment quotes `defer` and `follow-up`.\n- The last phase runs verify/ledger.sh, the deferral check.\n\n```\ndefer this\n```\n\nDrag-to-dock is deferred to issues/seeded.md.\n' > good.md
    printf -- 'Nothing else.\n\nThe column sweep is left as a follow-up.\n' > bad.md
    r=0
    check good.md > out.txt && echo "ok   ledger self-test: headings, negations, code, the ledger line and a seeded deferral pass" || { cat out.txt; echo "FAIL ledger self-test: the good fixture failed"; r=1; }
    if check bad.md > out.txt; then echo "FAIL ledger self-test: an unseeded deferral passed"; r=1
    else echo "ok   ledger self-test: an unseeded deferral fails"; fi
    exit $r
  ); st=$?
  del "$lab" >/dev/null 2>&1 || :
  exit $st
fi

root=$(git -C "$here" rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
base=${STATIC_BASE:-$(git log -1 --format=%H --grep='^\[feature\] Plan pane-arrangements' 2>/dev/null)}
fails=0
check spec/pane-arrangements/TECH.md spec/pane-arrangements/PLAN.md spec/pane-arrangements/REVIEW.md || fails=1
if [ -n "$base" ]; then
  for s in $(git diff --cached --name-only --diff-filter=A "$base" -- issues 2>/dev/null); do
    grep -q "$s" ROADMAP.md || { echo "FAIL ledger: added seed $s has no ROADMAP.md entry"; fails=1; }
  done
fi
[ "$fails" -eq 0 ] && echo "ok   ledger" || echo "ledger: failed"
[ "$fails" -eq 0 ]
