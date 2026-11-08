//! The graph's camera, as pure math: a spherical pose around a target, its basis and projection,
//! the view-projection matrix the renderer draws with, the eases between poses, the fit distances,
//! wheel normalisation, and a symmetric 3×3 eigensolver for solving the view axis.
//!
//! It uses `std` only, so `cargo test` runs it natively. Everything is `f64`; values become `f32`
//! only when they are uploaded to the GPU. World units are layout units, with +Y up.

use std::f64::consts::PI;

pub type V3 = [f64; 3];

/// Vertical field of view: enough perspective for depth to read, no fish-eye in a wide pane.
pub const FOV_Y: f64 = 40.0 * PI / 180.0;
/// Elevation stays inside ±85°, short of the poles where the up vector would flip.
pub const PSI_MAX: f64 = 85.0 * PI / 180.0;
/// A fitted set fills this fraction of the pane's tighter half-extent.
pub const FILL: f64 = 0.70;
/// One `+` or `-` press changes the distance by this factor.
pub const ZOOM_STEP: f64 = 1.35;
/// A solved move (a selection, a fit) eases over this long.
pub const EASE_MS: f64 = 450.0;
/// One orbit or zoom key press eases over this long, so auto-repeat chains smoothly.
pub const KEY_MS: f64 = 140.0;
/// Wheel and pinch: `ln d` per pixel of wheel delta (a macOS pinch arrives as a ctrl wheel).
pub const WHEEL_K: f64 = 0.0016;
pub const PINCH_K: f64 = 0.012;
/// No single wheel event moves `ln d` by more than this.
pub const WHEEL_MAX: f64 = 0.5;

/// Where the camera is: looking at `target` from azimuth `theta` and elevation `psi`, at
/// distance `exp(ln_d)`. Distance is kept as a logarithm so eases and zooms are even.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub target: V3,
    pub theta: f64,
    pub psi: f64,
    pub ln_d: f64,
}

impl Pose {
    pub fn d(&self) -> f64 {
        self.ln_d.exp()
    }
}

/// The camera frame: `dir` points from the target toward the eye, `r` is screen right and `u`
/// screen up. Right-handed: `r × u = dir`.
#[derive(Clone, Copy, Debug)]
pub struct Basis {
    pub dir: V3,
    pub r: V3,
    pub u: V3,
}

/// The pane a pose is projected into, in CSS pixels (never the window).
#[derive(Clone, Copy, Debug)]
pub struct Pane {
    pub w: f64,
    pub h: f64,
}

impl Pane {
    /// Half-extents of the view at unit depth: (`tan` of the horizontal and vertical half-FOV).
    fn tans(&self) -> (f64, f64) {
        let ty = (FOV_Y / 2.0).tan();
        let aspect = if self.h > 0.0 { self.w / self.h } else { 1.0 };
        (ty * aspect, ty)
    }
}

pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The camera frame for an azimuth and elevation, in closed form.
pub fn basis(theta: f64, psi: f64) -> Basis {
    let (st, ct) = theta.sin_cos();
    let (sp, cp) = psi.sin_cos();
    Basis {
        dir: [cp * st, sp, cp * ct],
        r: [ct, 0.0, -st],
        u: [-sp * st, cp, -sp * ct],
    }
}

/// The near plane: close enough to keep a fitted card, far enough to keep depth precision.
pub fn near(d: f64) -> f64 {
    (0.02 * d).max(1e-3)
}

/// The far plane: past the whole galaxy from wherever the eye is.
pub fn far(d: f64, r_gal: f64) -> f64 {
    d + 2.5 * r_gal.max(1.0)
}

/// A point in pane CSS pixels, with its view depth `s` (distance in front of the eye along the
/// view axis); `None` when it is not in front of the near plane.
pub fn project(p: V3, pose: &Pose, pane: Pane) -> Option<[f64; 3]> {
    let b = basis(pose.theta, pose.psi);
    let d = pose.d();
    let q = sub(p, pose.target);
    let s = d - dot(q, b.dir);
    if s <= near(d) {
        return None;
    }
    let (tx, ty) = pane.tans();
    let nx = dot(q, b.r) / (s * tx);
    let ny = dot(q, b.u) / (s * ty);
    Some([(nx + 1.0) / 2.0 * pane.w, (1.0 - ny) / 2.0 * pane.h, s])
}

/// The view-projection matrix `P·V`, column-major (as WebGL takes it). Clip `w` equals the view
/// depth `s` that [`project`] reports.
pub fn view_proj(pose: &Pose, pane: Pane, r_gal: f64) -> [f64; 16] {
    let b = basis(pose.theta, pose.psi);
    let d = pose.d();
    let eye = [
        pose.target[0] + d * b.dir[0],
        pose.target[1] + d * b.dir[1],
        pose.target[2] + d * b.dir[2],
    ];
    // V: rows r, u, dir, translated by -R·eye
    let v = [
        [b.r[0], b.r[1], b.r[2], -dot(b.r, eye)],
        [b.u[0], b.u[1], b.u[2], -dot(b.u, eye)],
        [b.dir[0], b.dir[1], b.dir[2], -dot(b.dir, eye)],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let (tx, ty) = pane.tans();
    let (n, f) = (near(d), far(d, r_gal));
    let p = [
        [1.0 / tx, 0.0, 0.0, 0.0],
        [0.0, 1.0 / ty, 0.0, 0.0],
        [0.0, 0.0, (f + n) / (n - f), 2.0 * f * n / (n - f)],
        [0.0, 0.0, -1.0, 0.0],
    ];
    let mut m = [0.0; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut acc = 0.0;
            for k in 0..4 {
                acc += p[row][k] * v[k][col];
            }
            m[col * 4 + row] = acc;
        }
    }
    m
}

/// How many CSS pixels one world unit spans at view depth 1, vertically: a world radius `r` at
/// depth `s` is `r · px_per_unit / s` pixels.
pub fn px_per_unit(pane: Pane) -> f64 {
    let (_, ty) = pane.tans();
    pane.h / 2.0 / ty
}

// ---- moves ---------------------------------------------------------------------------------

/// An angle in (-π, π].
pub fn wrap_pi(a: f64) -> f64 {
    if a > -PI && a <= PI {
        return a;
    }
    let t = (a + PI).rem_euclid(2.0 * PI) - PI;
    if t <= -PI { t + 2.0 * PI } else { t }
}

/// `u³(6u² − 15u + 10)`, with exact endpoints: the ease every camera move follows.
pub fn smootherstep(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    u * u * u * (u * (6.0 * u - 15.0) + 10.0)
}

/// The pose `e` of the way from `a` to `b`: the target and elevation linearly, the azimuth by
/// the short arc, and the distance linearly in `ln d` (an even zoom).
pub fn ease_pose(a: &Pose, b: &Pose, e: f64) -> Pose {
    if e >= 1.0 {
        return Pose {
            theta: wrap_pi(b.theta),
            ..*b
        };
    }
    let lerp = |x: f64, y: f64| x * (1.0 - e) + y * e;
    Pose {
        target: [
            lerp(a.target[0], b.target[0]),
            lerp(a.target[1], b.target[1]),
            lerp(a.target[2], b.target[2]),
        ],
        theta: wrap_pi(a.theta + wrap_pi(b.theta - a.theta) * e),
        psi: lerp(a.psi, b.psi),
        ln_d: lerp(a.ln_d, b.ln_d),
    }
}

/// `pose` turned by `d_theta` in azimuth and `d_psi` in elevation (radians), elevation clamped.
pub fn orbit(pose: &Pose, d_theta: f64, d_psi: f64) -> Pose {
    Pose {
        theta: wrap_pi(pose.theta + d_theta),
        psi: (pose.psi + d_psi).clamp(-PSI_MAX, PSI_MAX),
        ..*pose
    }
}

/// `pose` with its elevation and distance kept in range.
pub fn clamp(pose: &Pose, d_min: f64, d_max: f64) -> Pose {
    let (lo, hi) = (d_min.max(1e-6).ln(), d_max.max(d_min.max(1e-6)).ln());
    Pose {
        theta: wrap_pi(pose.theta),
        psi: pose.psi.clamp(-PSI_MAX, PSI_MAX),
        ln_d: pose.ln_d.clamp(lo, hi),
        ..*pose
    }
}

/// The change in `ln d` one wheel event asks for. `mode` is the event's `deltaMode` (0 pixels,
/// 1 lines of 16 px, 2 pages of `page_h` px); a ctrl wheel is a trackpad pinch, which moves
/// faster. Clamped to ±[`WHEEL_MAX`].
pub fn wheel_delta(dy: f64, mode: u32, ctrl: bool, page_h: f64) -> f64 {
    let px = match mode {
        1 => dy * 16.0,
        2 => dy * page_h,
        _ => dy,
    };
    (px * if ctrl { PINCH_K } else { WHEEL_K }).clamp(-WHEEL_MAX, WHEEL_MAX)
}

// ---- fitting -------------------------------------------------------------------------------

/// The distance at which a sphere of `radius` around the target fills [`FILL`] of the pane's
/// tighter half-extent, whatever the orientation.
pub fn fit_sphere(radius: f64, pane: Pane) -> f64 {
    let (tx, ty) = pane.tans();
    let alpha = (FILL * tx.min(ty)).atan();
    radius.max(1e-6) / alpha.sin()
}

/// The distance at which every point, padded by `pad` world units, fits inside [`FILL`] of the
/// pane under perspective, looking at `target` from `theta`/`psi`; never below `d_min`. Closed
/// form: a point at view-plane offset `x` and depth offset `z` toward the eye fits when
/// `(|x| + pad) / ((d − z)·tx) ≤ FILL`, and likewise for `y`.
pub fn fit_distance(
    pts: &[V3],
    target: V3,
    theta: f64,
    psi: f64,
    pane: Pane,
    pad: f64,
    d_min: f64,
) -> f64 {
    let b = basis(theta, psi);
    let (tx, ty) = pane.tans();
    let mut d = d_min;
    for p in pts {
        let q = sub(*p, target);
        let (x, y, z) = (dot(q, b.r), dot(q, b.u), dot(q, b.dir));
        d = d
            .max(z + pad)
            .max(z + (x.abs() + pad) / (FILL * tx))
            .max(z + (y.abs() + pad) / (FILL * ty));
    }
    d
}

// ---- eigen ---------------------------------------------------------------------------------

/// Eigenvalues (ascending, ties by index) and unit eigenvectors of a symmetric 3×3 matrix, by
/// cyclic Jacobi rotations: orthonormal by construction, robust when eigenvalues repeat, and
/// bitwise deterministic. The view solve takes the smallest eigenvector of a set's covariance.
#[cfg_attr(not(test), allow(dead_code))]
pub fn eig3(m: [[f64; 3]; 3]) -> ([f64; 3], [V3; 3]) {
    let mut a = m;
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..32 {
        let off = a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2];
        let diag = a[0][0].abs() + a[1][1].abs() + a[2][2].abs();
        if off == 0.0 || off <= (1e-15 * diag) * (1e-15 * diag) {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            let apq = a[p][q];
            if apq == 0.0 {
                continue;
            }
            let th = (a[q][q] - a[p][p]) / (2.0 * apq);
            let t = if th.abs() > 1e150 {
                0.5 / th
            } else {
                let sgn = if th < 0.0 { -1.0 } else { 1.0 };
                sgn / (th.abs() + (th * th + 1.0).sqrt())
            };
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            a[p][p] -= t * apq;
            a[q][q] += t * apq;
            a[p][q] = 0.0;
            a[q][p] = 0.0;
            let r = 3 - p - q;
            let (arp, arq) = (a[r][p], a[r][q]);
            a[r][p] = c * arp - s * arq;
            a[p][r] = a[r][p];
            a[r][q] = s * arp + c * arq;
            a[q][r] = a[r][q];
            for row in v.iter_mut() {
                let (vp, vq) = (row[p], row[q]);
                row[p] = c * vp - s * vq;
                row[q] = s * vp + c * vq;
            }
        }
    }
    let mut order = [0usize, 1, 2];
    order.sort_by(|&i, &j| a[i][i].total_cmp(&a[j][j]).then(i.cmp(&j)));
    let vals = order.map(|k| a[k][k]);
    let vecs = order.map(|k| [v[0][k], v[1][k], v[2][k]]);
    (vals, vecs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// splitmix64 as a seeded generator (the crate has no `rand`).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^= z >> 31;
            (z >> 11) as f64 / (1u64 << 53) as f64
        }
        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.next()
        }
    }

    fn cross(a: V3, b: V3) -> V3 {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    fn deg(x: f64) -> f64 {
        x * PI / 180.0
    }

    #[test]
    fn basis_orthonormal() {
        for ti in -12..=12 {
            for pi in -8..=8 {
                let (theta, psi) = (deg(ti as f64 * 15.0), deg(pi as f64 * 10.6));
                let b = basis(theta, psi);
                for v in [b.dir, b.r, b.u] {
                    assert!((dot(v, v) - 1.0).abs() < 1e-12, "unit at {ti},{pi}");
                }
                assert!(dot(b.dir, b.r).abs() < 1e-12);
                assert!(dot(b.dir, b.u).abs() < 1e-12);
                assert!(dot(b.r, b.u).abs() < 1e-12);
                let c = cross(b.r, b.u);
                for k in 0..3 {
                    assert!((c[k] - b.dir[k]).abs() < 1e-12, "right-handed at {ti},{pi}");
                }
                assert!(b.u[1] > 0.0, "up at {ti},{pi}");
            }
        }
    }

    #[test]
    fn project_matches_matrices() {
        let mut rng = Rng(7);
        for _ in 0..200 {
            let pose = Pose {
                target: [
                    rng.range(-500.0, 500.0),
                    rng.range(-500.0, 500.0),
                    rng.range(-500.0, 500.0),
                ],
                theta: rng.range(-PI, PI),
                psi: rng.range(-PSI_MAX, PSI_MAX),
                ln_d: rng.range(3.0, 8.5),
            };
            let pane = Pane {
                w: rng.range(200.0, 2400.0),
                h: rng.range(200.0, 1600.0),
            };
            let m = view_proj(&pose, pane, 1100.0);
            let c = project(pose.target, &pose, pane).expect("the target is in front");
            assert!((c[0] - pane.w / 2.0).abs() < 1e-9 && (c[1] - pane.h / 2.0).abs() < 1e-9);
            for _ in 0..20 {
                let p = [
                    rng.range(-1200.0, 1200.0),
                    rng.range(-1200.0, 1200.0),
                    rng.range(-1200.0, 1200.0),
                ];
                let clip: Vec<f64> = (0..4)
                    .map(|row| m[row] * p[0] + m[4 + row] * p[1] + m[8 + row] * p[2] + m[12 + row])
                    .collect();
                match project(p, &pose, pane) {
                    Some(s) => {
                        let px = (clip[0] / clip[3] + 1.0) / 2.0 * pane.w;
                        let py = (1.0 - clip[1] / clip[3]) / 2.0 * pane.h;
                        let tol = 1e-9 * (1.0 + px.abs().max(py.abs()));
                        assert!(
                            (px - s[0]).abs() < tol && (py - s[1]).abs() < tol,
                            "{px},{py} vs {s:?}"
                        );
                        assert!(
                            (clip[3] - s[2]).abs() < 1e-9 * (1.0 + s[2]),
                            "clip w is the depth"
                        );
                    }
                    None => assert!(clip[3] <= near(pose.d()) + 1e-9),
                }
            }
        }
    }

    fn residual_ok(m: [[f64; 3]; 3]) {
        let (vals, vecs) = eig3(m);
        let scale = vals.iter().fold(0.0f64, |a, v| a.max(v.abs()));
        for k in 0..3 {
            let v = vecs[k];
            let mv = [dot(m[0], v), dot(m[1], v), dot(m[2], v)];
            let res = ((mv[0] - vals[k] * v[0]).powi(2)
                + (mv[1] - vals[k] * v[1]).powi(2)
                + (mv[2] - vals[k] * v[2]).powi(2))
            .sqrt();
            assert!(res <= 1e-12 * scale, "residual {res} vs {scale} for {m:?}");
            for j in 0..3 {
                let want = if j == k { 1.0 } else { 0.0 };
                assert!(
                    (dot(vecs[k], vecs[j]) - want).abs() < 1e-12,
                    "orthonormal for {m:?}"
                );
            }
        }
        assert!(vals[0] <= vals[1] && vals[1] <= vals[2], "ascending");
        let (vals2, vecs2) = eig3(m);
        assert!(
            vals.iter()
                .zip(vals2.iter())
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        for k in 0..3 {
            assert!(
                vecs[k]
                    .iter()
                    .zip(vecs2[k].iter())
                    .all(|(a, b)| a.to_bits() == b.to_bits())
            );
        }
    }

    #[test]
    fn jacobi_residual_and_determinism() {
        let mut rng = Rng(11);
        for _ in 0..2000 {
            // a random covariance-like matrix: B·Bᵀ with anisotropic scales
            let s = [
                rng.range(1e-4, 1.0),
                rng.range(1e-2, 10.0),
                rng.range(1.0, 1e3),
            ];
            let mut b = [[0.0; 3]; 3];
            for row in b.iter_mut() {
                for (k, x) in row.iter_mut().enumerate() {
                    *x = rng.range(-1.0, 1.0) * s[k];
                }
            }
            let mut m = [[0.0; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    m[i][j] = dot(b[i], b[j]);
                }
            }
            residual_ok(m);
            // a general symmetric matrix, negative eigenvalues included
            let (x, y, z) = (
                rng.range(-5.0, 5.0),
                rng.range(-5.0, 5.0),
                rng.range(-5.0, 5.0),
            );
            residual_ok([
                [rng.range(-9.0, 9.0), x, y],
                [x, rng.range(-9.0, 9.0), z],
                [y, z, rng.range(-9.0, 9.0)],
            ]);
        }
        residual_ok([[3.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 2.0]]);
        residual_ok([[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]]);
        residual_ok([[2.0, 1.0, 0.0], [1.0, 2.0, 0.0], [0.0, 0.0, 3.0]]); // eigenvalues 1, 3, 3
        residual_ok([[1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 1.0]]); // 0, 0, 3
        residual_ok([[0.0; 3]; 3]);
        let (vals, vecs) = eig3([[0.0; 3]; 3]);
        assert_eq!(vals, [0.0; 3]);
        assert_eq!(vecs, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    }

    #[test]
    fn wrap_pi_short_arc() {
        assert!((wrap_pi(deg(350.0)) - deg(-10.0)).abs() < 1e-12);
        assert!((wrap_pi(deg(-190.0)) - deg(170.0)).abs() < 1e-12);
        assert!((wrap_pi(deg(180.0)) - PI).abs() < 1e-12);
        assert!((wrap_pi(deg(-180.0)) - PI).abs() < 1e-12);
        assert!((wrap_pi(deg(720.0 + 30.0)) - deg(30.0)).abs() < 1e-12);
        for k in -40..=40 {
            let a = k as f64 * 0.37;
            let w = wrap_pi(a);
            assert!(w > -PI && w <= PI);
            assert!(((a - w) / (2.0 * PI)).round() * 2.0 * PI - (a - w) < 1e-9);
        }
        // easing from 170° to -170° crosses 180°, never 0°
        let a = Pose {
            target: [0.0; 3],
            theta: deg(170.0),
            psi: 0.0,
            ln_d: 5.0,
        };
        let b = Pose {
            theta: deg(-170.0),
            ..a
        };
        for i in 0..=20 {
            let p = ease_pose(&a, &b, i as f64 / 20.0);
            assert!(p.theta.abs() >= deg(170.0) - 1e-9, "step {i}: {}", p.theta);
        }
        let mid = ease_pose(&a, &b, 0.5);
        assert!((mid.theta.abs() - PI).abs() < 1e-9);
    }

    #[test]
    fn wheel_delta_modes() {
        assert!((wheel_delta(100.0, 0, false, 800.0) - 100.0 * WHEEL_K).abs() < 1e-15);
        assert!((wheel_delta(3.0, 1, false, 800.0) - 48.0 * WHEEL_K).abs() < 1e-15);
        assert!((wheel_delta(0.25, 2, false, 800.0) - 200.0 * WHEEL_K).abs() < 1e-15);
        assert!((wheel_delta(-10.0, 0, true, 800.0) + 10.0 * PINCH_K).abs() < 1e-15);
        assert_eq!(wheel_delta(1000.0, 0, false, 800.0), WHEEL_MAX);
        assert_eq!(wheel_delta(-100.0, 0, true, 800.0), -WHEEL_MAX);
        assert_eq!(wheel_delta(1.0, 2, false, 1e6), WHEEL_MAX);
        assert_eq!(wheel_delta(0.0, 1, true, 800.0), 0.0);
        // the wheel moves away for a positive delta
        assert!(wheel_delta(1.0, 0, false, 800.0) > 0.0);
    }

    #[test]
    fn ease_endpoints_and_monotone() {
        assert_eq!(smootherstep(0.0), 0.0);
        assert_eq!(smootherstep(1.0), 1.0);
        assert_eq!(smootherstep(-0.5), 0.0);
        assert_eq!(smootherstep(1.5), 1.0);
        let mut last = 0.0;
        for i in 1..=1000 {
            let e = smootherstep(i as f64 / 1000.0);
            assert!(e >= last, "monotone at {i}");
            last = e;
        }
        assert!((smootherstep(0.5) - 0.5).abs() < 1e-15);
        let a = Pose {
            target: [1.0, 2.0, 3.0],
            theta: 0.3,
            psi: -0.2,
            ln_d: 4.0,
        };
        let b = Pose {
            target: [-5.0, 8.0, 0.5],
            theta: 1.1,
            psi: 0.6,
            ln_d: 7.5,
        };
        let p0 = ease_pose(&a, &b, 0.0);
        let p1 = ease_pose(&a, &b, 1.0);
        assert_eq!(p0, a);
        assert!((p1.theta - b.theta).abs() < 1e-15 && p1.psi == b.psi && p1.ln_d == b.ln_d);
        assert_eq!(p1.target, b.target);
        // ln d moves linearly in the eased parameter: an even zoom
        for i in 0..=10 {
            let e = i as f64 / 10.0;
            let p = ease_pose(&a, &b, e);
            assert!((p.ln_d - (a.ln_d + (b.ln_d - a.ln_d) * e)).abs() < 1e-12);
        }
    }

    #[test]
    fn fits_fill_and_clamp() {
        let pane = Pane {
            w: 1200.0,
            h: 800.0,
        };
        let d = fit_sphere(500.0, pane);
        let (_, ty) = pane.tans();
        assert!((500.0 / (d * d - 250000.0f64).sqrt() - FILL * ty).abs() < 1e-12);
        let pts = [[100.0, 0.0, 0.0], [-40.0, 60.0, 10.0], [0.0, -30.0, -80.0]];
        let d = fit_distance(&pts, [0.0; 3], 0.4, 0.2, pane, 0.0, 1.0);
        let pose = Pose {
            target: [0.0; 3],
            theta: 0.4,
            psi: 0.2,
            ln_d: d.ln(),
        };
        let worst = pts
            .iter()
            .map(|p| {
                let s = project(*p, &pose, pane).unwrap();
                ((s[0] / pane.w * 2.0 - 1.0).abs()).max((1.0 - s[1] / pane.h * 2.0).abs())
            })
            .fold(0.0, f64::max);
        assert!((worst - FILL).abs() < 1e-9, "{worst}");
        let c = clamp(
            &Pose {
                ln_d: 100.0,
                psi: 3.0,
                ..pose
            },
            10.0,
            1000.0,
        );
        assert!((c.d() - 1000.0).abs() < 1e-9 && c.psi == PSI_MAX);
        let o = orbit(&pose, deg(400.0), deg(200.0));
        assert!((o.theta - wrap_pi(0.4 + deg(400.0))).abs() < 1e-12 && o.psi == PSI_MAX);
    }
}
