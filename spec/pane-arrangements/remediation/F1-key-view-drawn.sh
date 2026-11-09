#!/bin/sh
# Remediation checker F1, for review cycle 4's G1 and G2 (the view the keys go to is always drawn).
#
#   sh spec/pane-arrangements/remediation/F1-key-view-drawn.sh LOG
#
# LOG is the output of one run of the frozen gate, `sh spec/pane-arrangements/verify/gate.sh P5`,
# teed to a file (P1's verify line does this). The frozen gate's checks.txt can't name the guards
# this remediation added, so this proves them from the same run: the gate ended green, and each
# label below opens exactly one line "ok   <label>" (a prefix at column 0, no regex) and never a
# FAIL line. Exit 0 green, 1 red, 2 usage.
set -u
log=${1:-}
[ -n "$log" ] && [ -f "$log" ] || { echo "F1-key-view-drawn: usage: $0 LOG (the gate's output)" >&2; exit 2; }

labels='the Log made the key view under the narrow floor is drawn, and its keys step as on screen
a Log bookmarked under the narrow floor is drawn at load, and its keys step as on screen
focus stepped onto a Log collapsed at load draws it, and its keys step as on screen'

red=0
grep -qx 'gate P5: all passed' "$log" || { echo "FAIL the gate run in $log did not end green"; red=1; }
n=0
while IFS= read -r label; do
  [ -n "$label" ] || continue
  n=$((n + 1))
  oks=$(awk -v l="ok   $label" 'index($0, l) == 1 { c++ } END { print c + 0 }' "$log")
  fails=$(awk -v l="FAIL $label" 'index($0, l) == 1 { c++ } END { print c + 0 }' "$log")
  if [ "$oks" -eq 1 ] && [ "$fails" -eq 0 ]; then echo "ok   printed ok: $label"
  else echo "FAIL $label: $oks ok lines, $fails FAIL lines in $log"; red=1; fi
done <<EOF
$labels
EOF
[ "$n" -eq 3 ] || { echo "FAIL expected 3 labels, read $n"; red=1; }
[ "$red" -eq 0 ] && echo "F1-key-view-drawn: all passed ($n labels)" || echo "F1-key-view-drawn: FAIL"
exit "$red"
