# graph-galaxy — archived as a failed experiment (2026-11-08)

Geoffrey drove a preview of `6ba9484` (five screenshots, 2026-11-08 21:29–21:31) and stopped the
cycle. The branch is kept, not landed: `archive/graph-galaxy` on origin.

**What he saw.**
- The per-region glyphs (diamond, square, triangle, open square, open circle, …) and the family
  colours from the first graph were gone.
- The view was less legible than the radial graph it replaced, and the proportions were off.
- At focus scale, the links to other regions shot out to the pane edge as spokes.
- The outermost rotation was "not terrible".

**The foreman's reading.**
- The GOAL graded camera mechanics and never made the existing visual vocabulary a requirement,
  so the plan replaced it and two blind reviews passed it.
- A free 3D camera let perspective decide size, overlap and label room. The result was halo
  "bokeh", near-camera discs and fog gradients.
- All-pairs region bundles made a hairball at galaxy scale.
- Portals as lines to the rim dominated every focused frame.

**What a restart should do first.**
- Concept art from the real index: galaxy, system, focus, and a mid-warp frame, drawn in v1's
  glyphs and colours, before any GOAL.
- Then visual R-IDs graded against the approved frames.
- The direction under discussion was a designed 2D layout per scale, with 3D used only in the
  motion between scales.

**Reusable from this branch:**
- the deterministic layout (`graph/space.rs`);
- the camera and easing maths, warp included (`graph/cam.rs`);
- the label placer (`graph/labels.rs`);
- history poses in `history.state`;
- the `keys.mjs` hook harness and `profile.mjs --graph`.

**State at archive.**
- Review cycle 2 was changes-requested (`REVIEW.md`).
- Remediation 2 was halted mid-run. Its unfinished edits are committed here as they stood,
  untested.
- The cycle's metrics record is copied verbatim to `metrics-abandoned.yaml`, because the cycle
  never reached `landed`.
