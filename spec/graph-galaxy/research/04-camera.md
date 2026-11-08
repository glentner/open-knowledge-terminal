# Research 04: camera math

Scope: the spherical camera, how the view axis is solved, the 70% fit, easing and the warp, the scale bands, and orbit input. All of it is pure Rust with no new crate (`okt-client/Cargo.toml` already has `MediaQueryList`, `WheelEvent`, `PointerEvent`, `ResizeObserver` and `Performance`). A throwaway Rust lab verified the solver numbers below. The lab was deleted afterwards.

## Where it lives

The plan should create a new `okt-client/src/cam.rs` that uses no `web_sys` or `js_sys`, so `make test` (`cargo test -p okt-client`, native) can link it. It would follow the `#[cfg(test)] mod tests` pattern in `js.rs:588`. `graph.rs` holds a `Pose` and a `CamAnim` in `G` and calls into `cam`. Math runs in `f64`. Values become `f32` only when they are uploaded to the GPU. World units are layout units.

## 1. Camera model

- Recommendation: keep the pose as θ/ψ angles and an up vector rather than switching to a quaternion. `Pose { target: V3, theta, psi, ln_d }`, with world up = +Y.
  - The GOAL and the compass speak in θ/ψ, so angles are the natural state.
  - Easing angles is simple.
  - ψ is clamped to ±85°, which avoids the up-vector flip at the poles. When the solved axis lands near a pole (for example, a set lying flat in XZ), the clamp costs a foreshortening of cos 5° = 0.996, which is negligible. The lab confirmed that the "coplanar XZ" case clamps cleanly to ψ = 85°.
- Basis, in closed form with no cross products:
  - `dir = (cosψ sinθ, sinψ, cosψ cosθ)`: from the target toward the eye.
  - `r = (cosθ, 0, −sinθ)`
  - `u = (−sinψ sinθ, cosψ, −sinψ cosθ)`
  - `eye = target + d·dir`, where `d = exp(ln_d)`.
- Camera coordinates of p, with `q = p − target`: `x = q·r`, `y = q·u`, and depth `s = d − q·dir`. A point is visible only when `s > near`.
- Projection, using a vertical FOV of 40°. That is enough perspective for the depth cue without fish-eye in a wide pane. Let `ty = tan(fov/2)`, `aspect = W/H` from the pane rect (`gs.w`, `gs.h`, never the window), and `tx = ty·aspect`. Then:
  - `ndc = (x/(s·tx), y/(s·ty))`
  - pane CSS px: `px = (ndc.x+1)/2·W`, `py = (1−ndc.y)/2·H`
  - backing px = CSS px × `gs.dpr`. Canvas: `setTransform(dpr,…)`. WebGL: `viewport(W·dpr, H·dpr)`.
  - pointer → pane px: `client − host_rect.{left,top}`, as `user_point` does today.
- Matrices for WebGL, column-major on upload:
  - `V = [r; u; dir]` rows, with translation `−R·eye`.
  - `P = [[1/tx,0,0,0],[0,1/ty,0,0],[0,0,(f+n)/(n−f),2fn/(n−f)],[0,0,−1,0]]` (row-major as written).
  - `n = max(0.02·d, ε)`, `f = d + 2.5·R_galaxy`. The ratio stays near 1e4, which a 24-bit depth buffer handles.
  - The Canvas candidate uses the same `project()` in software, so both renderers share one tested function.

## 2. Solving the view axis

- Use the covariance of the focus set: `C = Σ(p−m)(p−m)ᵀ/n`, with uniform weights. A per-point weight hook is optional; R3 says only "the focus set".
- Recommended solver: Jacobi rotations (cyclic, 3×3). It is robust when eigenvalues repeat, gives orthonormal vectors by construction, is deterministic, and converges in ≤ 6 sweeps.
  - Stop when `off² ≤ (1e-15·Σ|aᵢᵢ|)²`, with a cap of 32 sweeps.
  - Sort ascending, breaking ties by index.
  - The analytic cubic (Smith's trigonometric form) loses eigenvector accuracy near repeated roots, and those are exactly the degenerate cases here. Reject it.
  - Lab results: worst relative residual and orthonormality error 1.2e-15 over 20 000 random anisotropic sets (including slabs 1e5 thin), with bitwise-identical repeats.
- One formula handles every degenerate case: a biased eigenproblem. Take the smallest eigenvector of `C' = C − κ·tr(C)·v_c v_cᵀ`, where `v_c` is the current `dir` and κ = 0.02. This minimises `vᵀCv + κ·tr(C)·(1 − (v·v_c)²)`: lost on-screen spread, plus a small penalty for turning. Then flip v when `v·v_c < 0`, so the camera stays in its hemisphere. There are no special-case branches, and the result is continuous in the input.

| Case | Behaviour (verified) |
|---|---|
| 2 points / collinear | Picks the direction in the plane ⟂ the segment that is nearest `v_c` (lab: `v·v_c` = 0.961 = the projection of `v_c`) |
| Collinear exactly along `v_c` | Any perpendicular is optimal. Jacobi returns one deterministically (`v·v_c` = 0) |
| Coplanar | The plane normal. The bias only resolves the sign |
| Isotropic (cube corners) | Keeps `v_c` exactly: no gratuitous spin |
| Tilted slab | Within 0.8° of the true e₀. The bias is small when the eigen-gap is large |

- Guards:
  - `n == 0`: no-op.
  - `n == 1`, or `tr(C) < 1e-12` (coincident points): keep θ/ψ and use `d = d_min` (step 3).
- Convert to angles: `θ = atan2(v.x, v.z)`, `ψ = asin(v.y)` clamped to ±85°.
- `z` converges rather than being strictly idempotent, because the bias pulls toward `v_c`. A second press moves ≤ 1°. The plan can skip a move when the angle change is < 0.5° and the change in `ln d` is < 0.02.
- Target:
  - R3, and `z` with a selection: the selected card. Keeping the focus at the pane centre is what makes the clock-hand walk order (R5) meaningful.
  - R4 Results (no single focus): the midpoint of the set's view-plane bbox `(x_mid·r + y_mid·u)`, at its mean depth.

## 3. Fit to 70%, in closed form

With fill `F = 0.70` and per-point pad ρ (node radius plus label room, in world units), a point i at view-plane `(xᵢ, yᵢ)` and depth offset `zᵢ = qᵢ·dir` fits when `(|xᵢ|+ρ)/((d−zᵢ)·tx) ≤ F`, and likewise for y. So:

```
d = max( d_min,  max_i zᵢ + near_pad,
         max_i [ zᵢ + (|xᵢ|+ρ)/(F·tx) ],
         max_i [ zᵢ + (|yᵢ|+ρ)/(F·ty) ] )
```

- The formula is exact under perspective, because near points (`zᵢ > 0`) automatically need more distance. It takes both axes into account, along with aspect and FOV, so whichever axis is tighter fills 70%. No bisection is needed. The lab measured the max |ndc| at exactly 0.7000 in every case.
- `d_min` = the distance that fits a sphere of radius `r_min` (for example, 1.5× the layout's file spacing): `r_min/(F·min(tx,ty))`.
- The same function serves `0` (fit galaxy: the region centroids ± their radii) and the region fit for system scale.

## 4. Easing, warp, interruption, reduced motion

- `CamAnim { from: Pose, to: Pose, t0, ms, kind: Ease | Warp }`, kept in `G` in place of today's `Anim` (`graph.rs:958`).
- Ease (R3, `z`, `[`/`]`, keys): 450 ms, with `e = smootherstep(u) = u³(6u²−15u+10)`.
  - target: lerp
  - θ: `θ₀ + wrap_pi(θ₁−θ₀)·e` (shortest arc)
  - ψ: lerp
  - `ln_d`: lerp, which gives a perceptually even zoom.
- Warp (R6): 600 ms, using the van Wijk–Nuij optimal zoom-and-pan path (the one in `d3.interpolateZoom`), with ρ = √2.
  - Visible width: `w = 2·d·ty`. Pan distance: `D = |target₁ − target₀|`.
  - For `D > ε`:
    - `bᵢ = (w₁² − w₀² ± ρ⁴D²)/(2·wᵢ·ρ²·D)` (plus sign for i = 0, minus for i = 1)
    - `rᵢ = ln(√(bᵢ²+1) − bᵢ)`
    - `S = (r₁ − r₀)/ρ`
    - at `s = e(u)·S`: path fraction `k = w₀/(ρ²D)·(cosh r₀·tanh(ρs+r₀) − sinh r₀)`, and `w = w₀·cosh r₀ / cosh(ρs+r₀)`
    - target = lerp(target₀, target₁, k); `d = w/(2ty)`
  - For `D ≤ ε`: `k = u`, `w = w₀·(w₁/w₀)^u`.
  - The camera rises to roughly the width that spans both ends, then descends. Lab: endpoints exact; equal widths 10 → 10 over D = 500 peak at w ≈ 500.
  - Cap the peak at the galaxy fit: `d ← min(d, d_fit_galaxy)` per frame. The cap is rarely hit, because the rise to D is already "toward galaxy".
  - Ease θ/ψ with smootherstep over the same u, so the turn happens while the camera is high.
  - Pass `e(u)` (ease-in-out) into `s` so take-off and landing are soft.
- Interruption: any new move sets `from` to the pose the current frame evaluates (`frame_tick` already writes the pose each frame), so nothing snaps. Drag, wheel and `+`/`−` cancel the animation and act on that pose.
- Borrow rule (AGENTS § 1): `frame_tick` evaluates and stores the pose under one short `g()` borrow, then drops it, as the existing pattern at `graph.rs:1343` does. On-arrival work (scale change, visible-set rebuild, walk mark) runs after the drop. The `dom::raf` closure never captures a `RefMut`.
- Reduced motion: no client code reads it today. The only matches are the CSS at `app.css:513` and the `match_media` pattern in `chrome.rs:62` (`is_standalone`). Add `fn reduced_motion() -> bool` with the same pattern on `(prefers-reduced-motion: reduce)`, read at the start of each move. It costs nothing and follows a live OS toggle. When it is true, set `pose = to` and `schedule()` one frame. Tests: Playwright `page.emulateMedia({ reducedMotion: 'reduce' })`.

## 5. Scale bands, with hysteresis

- Scale is a function of selection (which sets the innermost reachable scale) and distance (which decides within that).

| Selection | Innermost scale |
|---|---|
| card | focus |
| folder | system |
| none | system (zooming in from galaxy; R2) |

- Fit distances:
  - `d_F`: the focus fit from the last solve.
  - `d_S`: the fit of the active region's bounding sphere.
  - `d_G`: the galaxy fit.
- Band boundary between inner and outer: `B = √(d_inner·d_outer)`. Crossing outward requires `d > B·h`; crossing inward requires `d < B/h`, with h = 1.15 (±14% in log distance). Store `prev` and classify with `classify(d, bands, prev, innermost) -> Scale`.
- Crossings never change the selection (R2):
  - Zoom-out, focus → system: only the drawn tiers change.
  - System → galaxy: ease the target (350 ms, cut under reduced motion) to the galaxy origin, so orbiting is about the whole.
  - Zoom-in at galaxy (`d < B_SG/h`): choose the region whose centroid projects (with `s > near`) nearest the pane centre in CSS px. Ties go to the larger card count, then the name. Make it the active region and ease the target to its centroid.
- Drawing (pairs with R8): the scale picks the tier set (galaxy: region glyphs and bundles; system: the active region's tier 3+ points plus glyphs; focus: tiers 0–2). Focus-set cards are drawn at every scale (Clarifications). Cross-fade the alpha of a region's interior points with `smoothstep(ln d; ln(B/h), ln(B·h))` across the band, so nothing pops.

## 6. Orbit input

- Drag ("grab" semantics): `Δθ = −dx/min(W,H)·π` and `Δψ = +dy/min(W,H)·π`, so a drag across the short side turns 180°. Clamp ψ. Keep the existing 4 px dead zone and the `moved` flag (`graph.rs:2207`).
- Wheel:
  - Normalise `deltaMode` to px: line ×16, page ×H.
  - `ln_d += dy·k`, with k = 0.0016 for a wheel, or 0.012 when `ctrlKey` (a macOS trackpad pinch). These are the constants at `graph.rs:2180`.
  - Clamp each event to `|Δln_d| ≤ 0.5`, and clamp d to `[d_min, 1.5·d_G]`.
  - Zoom about the target rather than the cursor, so orbit and zoom share one centre. This is a deliberate change from today's pointer-anchored 2D zoom. The plan can add a cursor anchor later.
- Keyboard: one step per press, eased over 140 ms from the current pose, so auto-repeat chains smoothly through the interruption rule.
  - azimuth: ±15°
  - elevation: ±10°
  - `+`/`=` and `−`: `ln_d ∓ ln 1.35`, today's factor
  - `0`: galaxy fit
  - The plan picks the keys and proves them free (GOAL suggests ArrowLeft/Right and Shift+ArrowUp/Down).
- Resize (the `ResizeObserver` at `graph.rs:2155`): keep the target, θ and ψ. When `autofit` is set (after a select, `z` or `0`; cleared by wheel, `+`/`−` or a manual zoom), re-run the step 3 fit for the new aspect as a cut, with no re-orientation. Otherwise keep d.

## 7. Unit-testable surface (`cam.rs`)

Functions: `basis`, `project`, `view_matrix`, `perspective`, `covariance`, `eig3_jacobi`, `solve_view(pts, v_cur, κ)`, `dir_to_angles`, `fit_distance`, `smootherstep`, `wrap_pi`, `lerp_pose`, `warp_pose(a, b, u)`, `classify`, `nearest_to_centre`, `wheel_delta(dy, mode, ctrl, h)`.

Native `cargo test` cases:

1. Basis is orthonormal and right-handed, and `u.y > 0` over a θ/ψ grid.
2. `project(target)` lands at the pane centre, and `project` matches `P·V` within 1e-9.
3. The Jacobi residual `‖Cv − λv‖ ≤ 1e-12·λmax` and orthonormality hold on random, diagonal, repeated-root and zero matrices, and repeats are bitwise deterministic.
4. `solve_view`:
   - isotropic → `v_c`
   - two points → the projection of `v_c` onto the ⟂ plane
   - coplanar → ± the normal, with `v·v_c ≥ 0`
   - coincident → `v_c`
   - repeated application: angle change < 1°
5. `dir_to_angles(basis(θ,ψ).dir)` round-trips, and a pole input clamps to 85°.
6. `fit_distance`:
   - max |ndc| of the set = 0.70 ± 1e-9 at aspect 0.5, 1 and 2.5
   - a point nearer the camera raises d
   - a single point gives `d_min`
7. `wrap_pi`: 350° → −10°, and θ eases across ±180° by the short arc.
8. `warp_pose`: endpoints exact; mid-flight d ≥ max(d₀, d₁) when D ≫ w; never above `d_G`; D = 0 falls back to log-lerp.
9. `classify`: no flip-flop when d oscillates ±10% around B; it respects the innermost cap.
10. `nearest_to_centre`: skips points behind the camera, and ties are deterministic.
11. `wheel_delta`: the line, page and ctrl factors, and the clamp.
