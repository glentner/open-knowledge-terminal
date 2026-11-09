#!/bin/sh
# Static checks for one graph-galaxy phase, graded on the index (what the commit will hold).
#
#   sh spec/graph-galaxy/verify/static.sh P<n> [--phase-only]
#
# Cumulative by default (every phase's facts up to P<n>); --phase-only checks P<n>'s alone.
# The build stages its work (git add -A) before verify. Untracked or unstaged source fails: the
# compiler would build a file the commit lacks.
# Footprint: one process at a time (git, grep, awk), no network, < 1 s.
set -u

phase=${1:-}
case "$phase" in P[1-7]) ;; *) echo "usage: static.sh P<1-7> [--phase-only]" >&2; exit 2 ;; esac
only=${2:-}
case "$only" in ""|--phase-only) ;; *) echo "static: unknown mode $only" >&2; exit 2 ;; esac
n=${phase#P}
root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2

# The base is the OLDEST plan commit, so a re-plan mid-build never hides earlier phases' work.
# STATIC_BASE overrides it, for the plan's dry runs only.
base=${STATIC_BASE:-$(git log --reverse --format=%H --grep='^\[feature\] Plan graph-galaxy' 2>/dev/null | head -1)}
[ -n "$base" ] || { echo "static: no plan commit found (set STATIC_BASE for a dry run)" >&2; exit 2; }

fails=0
ok() { echo "ok   static $1"; }
no() { echo "FAIL static $1"; fails=$((fails + 1)); }
want() { if eval "$2"; then ok "$1"; else no "$1"; fi; }
has() { git show ":$1" 2>/dev/null | grep -Eq -- "$2"; }      # staged content, extended regex
lacks() { ! has "$1" "$2"; }
staged() { git cat-file -e ":$1" 2>/dev/null; }
changed() { ! git diff --cached --quiet "$base" -- "$@" 2>/dev/null; }      # since the plan commit
this_phase() { ! git diff --cached --quiet HEAD -- "$@" 2>/dev/null; }     # in this phase's commit
okstatus='^status: (unshaped|shaped|adopted:|declined|accepted-behaviour)'
issue() { staged "issues/$1.md" && has ROADMAP.md "issues/$1\.md" && has "issues/$1.md" "$okstatus"; }
runs() { [ -z "$only" ] && [ "$1" -le "$n" ] || [ "$1" -eq "$n" ]; }
# a pure module: std only, no hashed collections, no crate imports beyond its graph/ peers
pure() {
  staged "$1" && lacks "$1" 'web_sys|js_sys|wasm_bindgen|RandomState|Hash(Map|Set)|hash_(map|set)' \
    && ! git show ":$1" | grep -E '^[[:space:]]*use crate::' | grep -vq 'use crate::graph::\(space\|cam\|vis\|labels\)'
}

# ---- every phase ------------------------------------------------------------------------------
# Committed state: nothing untracked or unstaged under the source and test trees.
dirty=$(git status --porcelain --untracked-files=all -- okt-client okt-server okt-pygments tests README.md AGENTS.md issues ROADMAP.md \
  | grep -Ev '^(M|A|D|R)  ' )
if [ -z "$dirty" ]; then ok "work is staged, nothing untracked or unstaged"; else no "work is staged, nothing untracked or unstaged"; echo "$dirty" | head -5; fi
# No spec ids in source or tests (AGENTS.md § Conventions): added lines only, in any case form
# and as any part of a name (R10, r10, r10_property, test_p7, GLYPH_R0). An underscore is a
# boundary, so a name built on an id fails too. Not ids: paths such as
# WebGl2RenderingContext::R8, and the percentile keys p50, p90, p95 and p99.
ids=$(git diff --cached -U0 "$base" -- okt-client okt-server okt-pygments tests 2>/dev/null \
  | grep '^+' | grep -v '^+++' | sed -E 's/::[A-Z][A-Z0-9_]*//g; s/(^|[^A-Za-z0-9])p(50|90|95|99)([^A-Za-z0-9]|$)/\1\3/g; s/(^|[^A-Za-z0-9])p(50|90|95|99)([^A-Za-z0-9]|$)/\1\3/g' \
  | grep -E '(^|[^A-Za-z0-9])[RrPp][0-9]{1,2}([^A-Za-z0-9]|$)')
if [ -z "$ids" ]; then ok "no spec ids in added source lines"; else no "no spec ids in added source lines"; echo "$ids" | head -5; fi
# app.css changes with the paint, per phase (AGENTS.md § Conventions): the paint files are the GL
# renderer, the overlay, and the label placer
if [ "$n" -ge 2 ] && this_phase okt-client/src/graph/gl.rs okt-client/src/graph/overlay.rs okt-client/src/graph/labels.rs; then
  want "app.css changed with the paint in this phase" "this_phase okt-client/static/app.css"
fi

# ---- P1: the layout ---------------------------------------------------------------------------
if runs 1; then
  sp=okt-client/src/graph/space.rs
  want "space.rs is staged and pure" "pure $sp"
  want "layout() signature is pinned by a test" "has $sp 'const _: fn\\(&\\[LNode\\]\\) -> Space = layout;'"
  want "model Node carries ft" "has okt-client/src/model.rs 'pub ft: Option<f64>'"
  want "the server is untouched" "! changed okt-server"
fi

# ---- P2: renderer and camera ------------------------------------------------------------------
if runs 2; then
  want "cam.rs is staged and pure" "pure okt-client/src/graph/cam.rs"
  want "gl.rs and overlay.rs are staged" "staged okt-client/src/graph/gl.rs && staged okt-client/src/graph/overlay.rs"
  want "web-sys has the WebGL2 features" "has okt-client/Cargo.toml '\"WebGl2RenderingContext\"' && has okt-client/Cargo.toml '\"WebGlVertexArrayObject\"'"
  want "the radial layout is gone, not renamed" "lacks okt-client/src/graph.rs 'fn layout\\(idx: &Idx\\)|R_TOP|fn polar|dr_dir|label_geom' && lacks okt-client/src/graph.rs CanvasRenderingContext2d"
  want "every camera move goes through cam_to" "[ \$(git show :okt-client/src/graph.rs | grep -Ev '(struct|impl|enum) CamAnim' | grep -c 'CamAnim *[{]') -le 1 ]"
  want "README names the orbit keys" "has README.md PageUp && has README.md PageDown"
  want "? list names the orbit keys" "has okt-client/src/chrome.rs PageUp"
  want "AGENTS.md coupled core names graph/" "has AGENTS.md 'okt-client/src/graph/'"
  want "the http server is untouched" "! changed okt-server/src/http.rs"
  want "profile.mjs parses --graph and reads OKT" "has tests/perf/profile.mjs \"['\\\"]--graph['\\\"]\" && has tests/perf/profile.mjs 'process\\.env\\.OKT'"
  want "zoom-to-cursor seed is filed" "issue graph-zoom-to-cursor"
fi

# ---- P3: scales and show all ------------------------------------------------------------------
if runs 3; then
  want "vis.rs is staged and pure" "pure okt-client/src/graph/vis.rs"
  want "settings carry show_all as showAll" "has okt-client/src/state.rs 'pub show_all: bool' && has okt-client/src/state.rs '\"showAll\"'"
  want "the labels setting is gone" "lacks okt-client/src/state.rs 'pub labels: bool'"
  want "the toolbar has show all, not all labels" "has okt-client/static/index.html 'data-z=\"all\"' && lacks okt-client/static/index.html 'data-z=\"labels\"'"
  want "settings say SHOW ALL" "has okt-client/src/chrome.rs 'SHOW ALL' && lacks okt-client/src/chrome.rs 'ALL LABELS'"
fi

# ---- P4: focus and tiers ----------------------------------------------------------------------
if runs 4; then
  want "app.css carries the tier and halo tokens" "has okt-client/static/app.css '--graph-halo' && has okt-client/static/app.css '--graph-t2-alpha'"
  want "the spotlight layers are retired" "lacks okt-client/static/app.css spotlit"
fi

# ---- P5: labels -------------------------------------------------------------------------------
if runs 5; then
  want "labels.rs is staged and pure" "pure okt-client/src/graph/labels.rs"
  want "the cap of 24 is one constant" "has okt-client/src/graph/labels.rs 'const MAX_CARD_LABELS: usize = 24;'"
fi

# ---- P6: z and the walk -----------------------------------------------------------------------
if runs 6; then
  want "README drops the inspector order" "lacks README.md 'in the order the inspector lists them'"
  want "README says clockwise" "has README.md clockwise"
  want "? list says clockwise" "has okt-client/src/chrome.rs clockwise"
  # the inspector marks a walk target past the chip cap (review cycle 1), so its seed is retired
  want "chip-cap seed is retired" "! staged issues/walk-mark-chip-cap.md && lacks ROADMAP.md 'walk-mark-chip-cap'"
fi

# ---- P7: warp, gate, ledger -------------------------------------------------------------------
if runs 7; then
  want "README says Enter or space flies there" "has README.md 'fl(y|ies) there'"
  want "profile-points seed is filed" "issue perf-profile-points"
  if sh "$root/spec/graph-galaxy/verify/ledger.sh"; then ok "deferral ledger"; else no "deferral ledger"; fi
fi

[ "$fails" -eq 0 ] && echo "static $phase: all passed" || echo "static $phase: $fails failed"
[ "$fails" -eq 0 ]
