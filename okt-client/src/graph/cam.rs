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

/// A flight to a walk's target lasts this long.
pub const WARP_MS: f64 = 600.0;
/// The van Wijk–Nuij path's trade between zooming out and panning (√2, as d3's `interpolateZoom`).
const RHO: f64 = std::f64::consts::SQRT_2;
/// A flight's highest point is aimed this far above its floor, so a frame drawn near the top
/// still clears it.
const RISE_MARGIN: f64 = 1.1;

/// `ln cosh x`, without overflow.
fn ln_cosh(x: f64) -> f64 {
    let a = x.abs();
    a + (-2.0 * a).exp().ln_1p() - std::f64::consts::LN_2
}

/// A flight from one pose to another: the camera rises and descends on the van Wijk–Nuij path,
/// the optimal pan-and-zoom of a window whose width is the visible height `2·d·tan(FOV/2)`, so a
/// long pan goes up high enough to see both ends. Its highest point is at least a floor (lifted
/// there when the path alone stays lower), and no frame goes past a cap (the galaxy fit) unless
/// an end is already beyond it. With the targets at one place it is a log-lerp of the distance
/// with the same floor. Azimuth and elevation turn over the same eased parameter, so the turn
/// happens high up.
#[derive(Clone, Copy, Debug)]
pub struct Warp {
    from: Pose,
    to: Pose,
    /// the pan distance; 0 on the log-lerp
    dist: f64,
    /// the path parameter `r` at the start, ln(√(b₀²+1) − b₀)
    r_start: f64,
    /// the path's length in its own parameter
    len: f64,
    /// where along the flight (0–1) the lift is anchored: the path's own top, kept off the ends
    pub apex: f64,
    /// the lift added to `ln d` at `apex`, falling to 0 at both ends
    lift: f64,
    /// `ln` of the highest distance a frame may reach
    ln_cap: f64,
}

/// A flight from `from` to `to` that rises to at least `floor` (when the cap allows) and never
/// past `cap`.
pub fn warp(from: &Pose, to: &Pose, floor: f64, cap: f64) -> Warp {
    let ty = (FOV_Y / 2.0).tan();
    let (w0, w1) = (2.0 * from.d() * ty, 2.0 * to.d() * ty);
    let q = sub(to.target, from.target);
    let dist = dot(q, q).sqrt();
    let ln_cap = cap.max(1e-9).ln().max(from.ln_d).max(to.ln_d);
    let mut w = Warp {
        from: *from,
        to: *to,
        dist: 0.0,
        r_start: 0.0,
        len: 0.0,
        apex: 0.5,
        lift: 0.0,
        ln_cap,
    };
    if dist > 1e-9 * (w0 + w1) {
        let (rho2, rho4) = (RHO * RHO, RHO.powi(4));
        let b0 = (w1 * w1 - w0 * w0 + rho4 * dist * dist) / (2.0 * w0 * rho2 * dist);
        let b1 = (w1 * w1 - w0 * w0 - rho4 * dist * dist) / (2.0 * w1 * rho2 * dist);
        // ln(√(b²+1) − b), stably
        let (r_start, r_end) = (-b0.asinh(), -b1.asinh());
        let len = (r_end - r_start) / RHO;
        if len.is_finite() && len > 0.0 {
            w.dist = dist;
            w.r_start = r_start;
            w.len = len;
        }
    }
    // the path's own top: where cosh(ρs + r_start) is least, or the higher end of a log-lerp
    let top = if w.dist > 0.0 {
        -w.r_start / RHO / w.len
    } else if to.ln_d > from.ln_d {
        1.0
    } else {
        0.0
    };
    w.apex = top.clamp(0.25, 0.75);
    let aim = (floor.max(1e-9) * RISE_MARGIN).ln().min(ln_cap);
    w.lift = (aim - w.base_ln_d(w.apex)).max(0.0);
    w
}

impl Warp {
    /// `ln d` on the path alone, `e` of the way along.
    fn base_ln_d(&self, e: f64) -> f64 {
        if self.dist > 0.0 {
            let s = e * self.len;
            self.from.ln_d + ln_cosh(self.r_start) - ln_cosh(RHO * s + self.r_start)
        } else {
            self.from.ln_d + (self.to.ln_d - self.from.ln_d) * e
        }
    }
    /// How far the target has gone, 0–1, `e` of the way along.
    fn pan(&self, e: f64) -> f64 {
        if self.dist > 0.0 {
            let ty = (FOV_Y / 2.0).tan();
            let w0 = 2.0 * self.from.d() * ty;
            let s = e * self.len;
            w0 / (RHO * RHO * self.dist)
                * (self.r_start.cosh() * (RHO * s + self.r_start).tanh() - self.r_start.sinh())
        } else {
            e
        }
    }
    /// The lift's share at `e`: 0 at both ends, 1 at `apex`.
    fn bump(&self, e: f64) -> f64 {
        let h = std::f64::consts::FRAC_PI_2;
        if e <= self.apex {
            (h * e / self.apex).sin()
        } else {
            (h * (1.0 - e) / (1.0 - self.apex)).sin()
        }
    }
    /// The pose `e` (the eased parameter) of the way along; the ends are exact.
    pub fn at(&self, e: f64) -> Pose {
        if e <= 0.0 {
            return self.from;
        }
        if e >= 1.0 {
            return Pose {
                theta: wrap_pi(self.to.theta),
                ..self.to
            };
        }
        let (a, b) = (&self.from, &self.to);
        let k = self.pan(e);
        // x + (y − x)·t keeps a target that does not move exactly where it is
        let lerp = |x: f64, y: f64, t: f64| x + (y - x) * t;
        Pose {
            target: [0, 1, 2].map(|i| lerp(a.target[i], b.target[i], k)),
            theta: wrap_pi(a.theta + wrap_pi(b.theta - a.theta) * e),
            psi: lerp(a.psi, b.psi, e),
            ln_d: (self.base_ln_d(e) + self.lift * self.bump(e)).min(self.ln_cap),
        }
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

/// The direction (from the target toward the eye) to look at a set of points along: its exact
/// smallest principal axis, so the plane it spreads in most faces the screen, signed toward
/// `view`, the current view direction. The covariance is uniform over the points.
///
/// When that axis is not defined, the direction is the one closest to `view` among those that
/// are equally good:
/// - no point, one point, or every point at one place: `view` itself;
/// - two points, or all on one line: perpendicular to the line;
/// - the next eigenvalue within 10% of the smallest (`λ₁/λ₀ < 1.1`, a ratio, never a fraction of
///   the trace, which would call a well-defined axis degenerate): in the plane of the two
///   smallest axes, or anywhere when the largest is within 10% of the middle one as well.
pub fn solve_view(pts: &[V3], view: V3) -> V3 {
    let v = unit(view).unwrap_or([0.0, 0.0, 1.0]);
    let n = pts.len();
    if n < 2 {
        return v;
    }
    let mut m = [0.0; 3];
    for p in pts {
        for k in 0..3 {
            m[k] += p[k] / n as f64;
        }
    }
    let mut c = [[0.0; 3]; 3];
    for p in pts {
        let q = sub(*p, m);
        for a in 0..3 {
            for b in 0..3 {
                c[a][b] += q[a] * q[b] / n as f64;
            }
        }
    }
    let tr = c[0][0] + c[1][1] + c[2][2];
    if tr <= 1e-24 * (1.0 + dot(m, m)) {
        return v;
    }
    let (l, e) = eig3(c);
    let toward = |d: V3| {
        if dot(d, v) < 0.0 {
            [-d[0], -d[1], -d[2]]
        } else {
            d
        }
    };
    // the view, with its part along `away` removed (the closest direction perpendicular to it)
    let across = |away: V3, or: V3| {
        let k = dot(v, away);
        unit([v[0] - k * away[0], v[1] - k * away[1], v[2] - k * away[2]]).unwrap_or(or)
    };
    if n == 2 || l[1] < 1e-9 * tr {
        return toward(across(e[2], e[0]));
    }
    let near = |a: f64, b: f64| b < 1.1 * a.max(0.0);
    if !near(l[0], l[1]) {
        return toward(e[0]);
    }
    if near(l[1], l[2]) {
        return v;
    }
    toward(across(e[2], e[0]))
}

/// The azimuth and elevation that look along `dir` (from the target toward the eye), the
/// elevation clamped short of the poles. Straight up or down keeps `theta0`.
pub fn angles(dir: V3, theta0: f64) -> (f64, f64) {
    let Some(d) = unit(dir) else {
        return (wrap_pi(theta0), 0.0);
    };
    let psi = d[1].clamp(-1.0, 1.0).asin().clamp(-PSI_MAX, PSI_MAX);
    let theta = if d[0].hypot(d[2]) < 1e-12 {
        theta0
    } else {
        d[0].atan2(d[2])
    };
    (wrap_pi(theta), psi)
}

fn unit(a: V3) -> Option<V3> {
    let l = dot(a, a).sqrt();
    (l > 1e-12 && l.is_finite()).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

// ---- scales --------------------------------------------------------------------------------

/// The three scales, innermost first: a card and its links, one region, the whole graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scale {
    Focus,
    System,
    Galaxy,
}

impl Scale {
    pub fn name(self) -> &'static str {
        match self {
            Scale::Focus => "focus",
            Scale::System => "system",
            Scale::Galaxy => "galaxy",
        }
    }
}

/// Hysteresis around a band boundary: a zoom crosses outward past `B·HYST` and inward below
/// `B/HYST`, so a distance sitting near a boundary never flip-flops.
pub const HYST: f64 = 1.15;

/// The camera distances where the scales meet: `inner` between focus and system, `outer` between
/// system and galaxy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bands {
    pub inner: f64,
    pub outer: f64,
}

/// The boundaries between the scales' own distances (a card's approach, a region's fit, the
/// galaxy's fit), each the geometric mean of its neighbours. The three are first spread at
/// least 1.5× apart, so a tiny region or galaxy never inverts the bands.
pub fn bands(d_focus: f64, d_system: f64, d_galaxy: f64) -> Bands {
    let f = d_focus.max(1e-6);
    let s = d_system.max(1.5 * f);
    let g = d_galaxy.max(1.5 * s);
    Bands {
        inner: (f * s).sqrt(),
        outer: (s * g).sqrt(),
    }
}

/// The scale at camera distance `d`, coming from `prev`, never inside `innermost` (a card
/// selected in the active region allows focus; otherwise system is the innermost a zoom
/// reaches). Consulted only on user zoom input; a solved move sets its own scale.
pub fn classify(d: f64, b: &Bands, prev: Scale, innermost: Scale) -> Scale {
    let mut s = prev;
    loop {
        s = match s {
            Scale::Focus if d > b.inner * HYST => Scale::System,
            Scale::System if d > b.outer * HYST => Scale::Galaxy,
            _ => break,
        };
    }
    loop {
        s = match s {
            Scale::Galaxy if d < b.outer / HYST => Scale::System,
            Scale::System if d < b.inner / HYST => Scale::Focus,
            _ => break,
        };
    }
    s.max(innermost)
}

/// A region a zoom-in at galaxy scale may enter: its core, its card count and its name.
#[derive(Clone, Copy, Debug)]
pub struct Candidate<'a> {
    pub core: V3,
    pub cards: usize,
    pub name: &'a str,
}

/// The candidate nearest the pane centre: among cores in front of the camera, the one projecting
/// closest to the centre in pane pixels; only when none is in front, the one nearest the view
/// axis by angle. Ties go to the larger card count, then the name.
pub fn nearest_to_centre(cands: &[Candidate], pose: &Pose, pane: Pane) -> Option<usize> {
    let b = basis(pose.theta, pose.psi);
    let d = pose.d();
    let eye = [
        pose.target[0] + d * b.dir[0],
        pose.target[1] + d * b.dir[1],
        pose.target[2] + d * b.dir[2],
    ];
    // (behind, distance from the centre) per candidate: pixels in front, 1 − cos behind
    let key = |c: &Candidate| match project(c.core, pose, pane) {
        Some(p) => (false, (p[0] - pane.w / 2.0).hypot(p[1] - pane.h / 2.0)),
        None => {
            let v = sub(c.core, eye);
            let l = dot(v, v).sqrt().max(1e-12);
            (true, 1.0 + dot(v, b.dir) / l)
        }
    };
    let keys: Vec<(bool, f64)> = cands.iter().map(key).collect();
    let mut best: Option<usize> = None;
    for i in 0..cands.len() {
        let Some(j) = best else {
            best = Some(i);
            continue;
        };
        let (ki, kj) = (keys[i], keys[j]);
        let tie = (ki.1 - kj.1).abs() <= 1e-9 * (1.0 + kj.1.abs());
        let better = if ki.0 != kj.0 {
            !ki.0
        } else if !tie {
            ki.1 < kj.1
        } else if cands[i].cards != cands[j].cards {
            cands[i].cards > cands[j].cards
        } else {
            cands[i].name < cands[j].name
        };
        if better {
            best = Some(i);
        }
    }
    best
}

/// How far inside each edge of the pane the rim runs, in CSS px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Insets {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// Where a region's rim glyph sits: on the rectangle `inset` inside the pane's edges, where the
/// ray from the pane centre at the on-screen angle of the region's true direction from the target
/// (`p`, in world units) meets it. The angle comes from view coordinates, so a region behind the
/// camera still points its way.
pub fn rim_point(p: V3, pose: &Pose, pane: Pane, inset: Insets) -> [f64; 2] {
    let b = basis(pose.theta, pose.psi);
    let q = sub(p, pose.target);
    let (x, y) = (dot(q, b.r), dot(q, b.u));
    let a = if x == 0.0 && y == 0.0 {
        0.0
    } else {
        y.atan2(x)
    };
    let (dx, dy) = (a.cos(), -a.sin());
    let (cx, cy) = (pane.w / 2.0, pane.h / 2.0);
    // the rectangle, kept around the centre however large the insets
    let x0 = inset.left.min(cx - 1.0);
    let x1 = (pane.w - inset.right).max(cx + 1.0);
    let y0 = inset.top.min(cy - 1.0);
    let y1 = (pane.h - inset.bottom).max(cy + 1.0);
    let reach = |d: f64, lo: f64, hi: f64, c: f64| {
        if d > 1e-12 {
            (hi - c) / d
        } else if d < -1e-12 {
            (lo - c) / d
        } else {
            f64::INFINITY
        }
    };
    let t = reach(dx, x0, x1, cx).min(reach(dy, y0, y1, cy)).max(0.0);
    [cx + t * dx, cy + t * dy]
}

// ---- eigen ---------------------------------------------------------------------------------

/// Eigenvalues (ascending, ties by index) and unit eigenvectors of a symmetric 3×3 matrix, by
/// cyclic Jacobi rotations: orthonormal by construction, robust when eigenvalues repeat, and
/// bitwise deterministic. The view solve takes the smallest eigenvector of a set's covariance.
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
        let start = ease_pose(&a, &b, 0.0);
        let end = ease_pose(&a, &b, 1.0);
        assert_eq!(start, a);
        assert!((end.theta - b.theta).abs() < 1e-15 && end.psi == b.psi && end.ln_d == b.ln_d);
        assert_eq!(end.target, b.target);
        // ln d moves linearly in the eased parameter: an even zoom
        for i in 0..=10 {
            let e = i as f64 / 10.0;
            let p = ease_pose(&a, &b, e);
            assert!((p.ln_d - (a.ln_d + (b.ln_d - a.ln_d) * e)).abs() < 1e-12);
        }
    }

    /// A flight's poses at many points along it, its apex included.
    fn flight(w: &Warp) -> Vec<Pose> {
        let mut es: Vec<f64> = (0..=2000).map(|i| i as f64 / 2000.0).collect();
        es.push(w.apex);
        es.iter().map(|&e| w.at(e)).collect()
    }

    #[test]
    fn warp_endpoints_and_rise() {
        let mut rng = Rng(0x5eed_0f_f11e5);
        let d_g = 2000.0;
        for case in 0..400 {
            let pose = |rng: &mut Rng, d: f64| Pose {
                target: [
                    rng.range(-600.0, 600.0),
                    rng.range(-600.0, 600.0),
                    rng.range(-600.0, 600.0),
                ],
                theta: rng.range(-PI, PI),
                psi: rng.range(-1.2, 1.2),
                ln_d: d.ln(),
            };
            let (d0, d1) = (rng.range(20.0, 400.0), rng.range(20.0, 400.0));
            let a = pose(&mut rng, d0);
            let mut b = pose(&mut rng, d1);
            // every fourth case: one place, a different distance and angles (D = 0)
            let same = case % 4 == 0;
            if same {
                b.target = a.target;
            }
            let d_sys = rng.range(100.0, 900.0);
            let floor = (2.0 * d0.max(d1)).max(d_sys);
            let w = warp(&a, &b, floor, d_g);
            // the ends are exact
            assert_eq!(w.at(0.0), a, "case {case}: start");
            let end = w.at(1.0);
            assert_eq!(end.target, b.target, "case {case}: end target");
            assert!(end.psi == b.psi && end.ln_d == b.ln_d, "case {case}: end");
            assert_eq!(end.theta, wrap_pi(b.theta), "case {case}: end azimuth");
            // the rise: at least the floor (as far as the galaxy fit allows), never past it
            let ps = flight(&w);
            assert!(
                ps.iter()
                    .all(|p| p.ln_d.is_finite() && p.target.iter().all(|c| c.is_finite())),
                "case {case}: finite"
            );
            let peak = ps.iter().map(|p| p.d()).fold(0.0, f64::max);
            assert!(
                peak >= floor.min(d_g) * (1.0 - 1e-9),
                "case {case}: peak {peak} below floor {floor}"
            );
            assert!(
                peak <= d_g * (1.0 + 1e-9),
                "case {case}: peak {peak} past {d_g}"
            );
            if same {
                assert!(
                    ps.iter().all(|p| p.target == a.target),
                    "case {case}: D = 0 pans"
                );
            }
        }
        // D = 0 with the floor below both ends: exactly the log-lerp
        let a = Pose {
            target: [3.0, -4.0, 5.0],
            theta: 0.2,
            psi: 0.1,
            ln_d: 60f64.ln(),
        };
        let b = Pose {
            theta: 1.0,
            ln_d: 90f64.ln(),
            ..a
        };
        let w = warp(&a, &b, 10.0, d_g);
        for i in 0..=100 {
            let e = i as f64 / 100.0;
            let want = a.ln_d + (b.ln_d - a.ln_d) * e;
            assert!((w.at(e).ln_d - want).abs() < 1e-12, "log-lerp at {e}");
        }
        // with a floor it rises to it in the middle, from both ends
        let w = warp(&a, &b, 500.0, d_g);
        let peak = flight(&w).iter().map(|p| p.d()).fold(0.0, f64::max);
        assert!((500.0..=d_g).contains(&peak), "log-lerp floor: {peak}");
        // the path alone: equal widths over a long pan rise to about the pan distance (ρ = √2)
        let ty = (FOV_Y / 2.0).tan();
        let far = Pose {
            target: [a.target[0] + 5000.0, a.target[1], a.target[2]],
            ..a
        };
        let w = warp(&a, &far, 1.0, 1e9);
        let top = flight(&w)
            .iter()
            .map(|p| 2.0 * p.d() * ty)
            .fold(0.0, f64::max);
        assert!(
            (top / 5000.0 - 1.0).abs() < 0.02,
            "van Wijk–Nuij top width {top}"
        );
        // and the cap holds it at the galaxy fit
        let w = warp(&a, &far, 1.0, 1000.0);
        let peak = flight(&w).iter().map(|p| p.d()).fold(0.0, f64::max);
        assert!(peak <= 1000.0 * (1.0 + 1e-12), "capped peak {peak}");
        // the target pans monotonically from end to end
        let ks: Vec<f64> = flight(&warp(&a, &far, 1.0, 1e9))[..=2000]
            .iter()
            .map(|p| p.target[0])
            .collect();
        assert!(
            ks.windows(2).all(|x| x[1] >= x[0] - 1e-9),
            "pan is monotone"
        );
    }

    #[test]
    fn classify_hysteresis() {
        let b = bands(100.0, 400.0, 1600.0);
        assert!((b.inner - 200.0).abs() < 1e-9 && (b.outer - 800.0).abs() < 1e-9);
        let (f, s, g) = (Scale::Focus, Scale::System, Scale::Galaxy);
        // within ±10% of a boundary, either scale it separates stays as it was
        for (boundary, sides) in [(b.inner, [f, s]), (b.outer, [s, g])] {
            for k in -10..=10 {
                let d = boundary * (1.0 + k as f64 / 100.0);
                for prev in sides {
                    assert_eq!(classify(d, &b, prev, f), prev, "flip at {d} from {prev:?}");
                }
            }
        }
        // sweeping out and back in crosses each boundary once per direction, past the margins
        let mut cur = f;
        let mut seen = vec![cur];
        let steps: Vec<f64> = (0..=60)
            .map(|i| 50.0 * (1.06f64).powi(i))
            .chain((0..=60).rev().map(|i| 50.0 * (1.06f64).powi(i)))
            .collect();
        for d in steps {
            let next = classify(d, &b, cur, f);
            if next != cur {
                // outward crossings happen past B·HYST, inward ones below B/HYST
                let edge = if next > cur {
                    if next == s { b.inner } else { b.outer }
                } else if next == f {
                    b.inner
                } else {
                    b.outer
                };
                if next > cur {
                    assert!(d > edge * HYST, "out at {d}");
                } else {
                    assert!(d < edge / HYST, "in at {d}");
                }
                seen.push(next);
            }
            cur = next;
        }
        assert_eq!(seen, vec![f, s, g, s, f]);
        // a distance past both boundaries jumps straight there
        assert_eq!(classify(5000.0, &b, f, f), g);
        assert_eq!(classify(10.0, &b, g, f), f);
        // the innermost cap: with no card in the active region, zooming in stops at system
        assert_eq!(classify(10.0, &b, g, s), s);
        assert_eq!(classify(10.0, &b, s, s), s);
        assert_eq!(classify(10.0, &b, f, s), s);
        assert_eq!(classify(5000.0, &b, s, s), g);
        // bands stay ordered however close the scales' own distances are
        let t = bands(300.0, 200.0, 250.0);
        assert!(t.inner < t.outer && t.inner > 300.0);
    }

    #[test]
    fn nearest_region_front_first() {
        let pane = Pane {
            w: 1200.0,
            h: 800.0,
        };
        // looking at the origin down -z from z = 1000 (theta 0, psi 0)
        let pose = Pose {
            target: [0.0; 3],
            theta: 0.0,
            psi: 0.0,
            ln_d: 1000f64.ln(),
        };
        let c = |core: V3, cards: usize, name: &'static str| Candidate { core, cards, name };
        // in front but far off-centre (even off the pane) beats one just behind the eye
        let far_front = c([3000.0, 0.0, 0.0], 1, "far");
        let just_behind = c([0.0, 0.0, 1100.0], 999, "behind");
        assert_eq!(
            nearest_to_centre(&[just_behind, far_front], &pose, pane),
            Some(1)
        );
        // among those in front, the nearest the centre in pixels wins
        let near = c([50.0, 20.0, 0.0], 1, "near");
        assert_eq!(
            nearest_to_centre(&[far_front, near, just_behind], &pose, pane),
            Some(1)
        );
        // nearer the eye, an equal world offset projects farther from the centre
        let deep = c([60.0, 0.0, -400.0], 1, "deep");
        let shallow = c([60.0, 0.0, 400.0], 1, "shallow");
        assert_eq!(nearest_to_centre(&[shallow, deep], &pose, pane), Some(1));
        // a tie in distance goes to the card count, then the name
        let left = c([-100.0, 0.0, 0.0], 5, "b-left");
        let right = c([100.0, 0.0, 0.0], 9, "c-right");
        assert_eq!(nearest_to_centre(&[left, right], &pose, pane), Some(1));
        let right5 = c([100.0, 0.0, 0.0], 5, "a-right");
        assert_eq!(nearest_to_centre(&[left, right5], &pose, pane), Some(1));
        assert_eq!(nearest_to_centre(&[right5, left], &pose, pane), Some(0));
        // with every core behind, the one nearest the view axis by angle wins
        let b1 = c([0.0, 300.0, 1500.0], 1, "b1");
        let b2 = c([0.0, 30.0, 1500.0], 1, "b2");
        assert_eq!(nearest_to_centre(&[b1, b2], &pose, pane), Some(0));
        assert_eq!(nearest_to_centre(&[], &pose, pane), None);
    }

    #[test]
    fn rim_points_hug_the_edge() {
        let pane = Pane {
            w: 1000.0,
            h: 600.0,
        };
        let pose = Pose {
            target: [0.0; 3],
            theta: 0.0,
            psi: 0.0,
            ln_d: 500f64.ln(),
        };
        let even = Insets {
            left: 30.0,
            top: 30.0,
            right: 30.0,
            bottom: 30.0,
        };
        let right = rim_point([10.0, 0.0, 0.0], &pose, pane, even);
        assert!((right[0] - 970.0).abs() < 1e-9 && (right[1] - 300.0).abs() < 1e-9);
        let up = rim_point([0.0, 10.0, 0.0], &pose, pane, even);
        assert!((up[0] - 500.0).abs() < 1e-9 && (up[1] - 30.0).abs() < 1e-9);
        // behind the camera, still on the side it lies toward
        let behind_left = rim_point([-5.0, 0.0, 900.0], &pose, pane, even);
        assert!((behind_left[0] - 30.0).abs() < 1e-9);
        // uneven insets (a toolbar on the left, a legend at the bottom) move the rim off them
        let chrome = Insets {
            left: 60.0,
            top: 30.0,
            right: 30.0,
            bottom: 80.0,
        };
        for inset in [even, chrome] {
            for k in 0..36 {
                let a = k as f64 * 10.0f64.to_radians();
                let p = rim_point([a.cos(), a.sin(), 0.0], &pose, pane, inset);
                let (x0, x1) = (inset.left, 1000.0 - inset.right);
                let (y0, y1) = (inset.top, 600.0 - inset.bottom);
                let on_x = (p[0] - x0).abs() < 1e-6 || (p[0] - x1).abs() < 1e-6;
                let on_y = (p[1] - y0).abs() < 1e-6 || (p[1] - y1).abs() < 1e-6;
                assert!(on_x || on_y, "{p:?}");
                assert!(p[0] >= x0 - 1e-6 && p[0] <= x1 + 1e-6);
                assert!(p[1] >= y0 - 1e-6 && p[1] <= y1 + 1e-6);
                // on the ray from the centre at the region's angle
                let (vx, vy) = (p[0] - 500.0, 300.0 - p[1]);
                assert!((vy.atan2(vx) - a.sin().atan2(a.cos())).abs() < 1e-9, "{k}");
            }
        }
        let down = rim_point([0.0, -10.0, 0.0], &pose, pane, chrome);
        assert!((down[1] - 520.0).abs() < 1e-9);
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

    fn add(a: V3, b: V3, k: f64) -> V3 {
        [a[0] + k * b[0], a[1] + k * b[1], a[2] + k * b[2]]
    }

    fn near_v(a: V3, b: V3, tol: f64) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() <= tol)
    }

    /// Six points at ±a·u0, ±b·u1, ±c·u2 about `centre`: a covariance whose eigenvectors are
    /// exactly the given axes, with eigenvalues a²/3, b²/3, c²/3.
    fn star(centre: V3, axes: [V3; 3], ext: [f64; 3]) -> Vec<V3> {
        let mut out = Vec::new();
        for k in 0..3 {
            out.push(add(centre, axes[k], ext[k]));
            out.push(add(centre, axes[k], -ext[k]));
        }
        out
    }

    #[test]
    fn solve_view_exact_axis() {
        let mut rng = Rng(7);
        for _ in 0..200 {
            let b = basis(rng.range(-PI, PI), rng.range(-1.4, 1.4));
            let axes = [b.dir, b.r, b.u];
            let centre = [
                rng.range(-900.0, 900.0),
                rng.range(-900.0, 900.0),
                rng.range(-900.0, 900.0),
            ];
            let s0 = rng.range(5.0, 50.0);
            let ext = [s0, s0 * rng.range(1.2, 3.0), s0 * rng.range(3.5, 9.0)];
            let pts = star(centre, axes, ext);
            for sign in [1.0, -1.0] {
                let view = add([0.1, 0.2, -0.3], b.dir, sign * 2.0);
                let d = solve_view(&pts, view);
                let want = if sign > 0.0 {
                    b.dir
                } else {
                    [-b.dir[0], -b.dir[1], -b.dir[2]]
                };
                assert!(near_v(d, want, 1e-9), "{d:?} {want:?}");
            }
        }
        // a ratio test, not a fraction of the trace: eigenvalues (1, 1.5, 7.5) are well defined
        let b = basis(0.7, 0.3);
        let pts = star(
            [10.0, 0.0, -5.0],
            [b.dir, b.r, b.u],
            [1.0, 1.5f64.sqrt(), 7.5f64.sqrt()],
        );
        assert!(near_v(solve_view(&pts, b.dir), b.dir, 1e-9));
        // points in a plane: its normal, whichever side the view is on
        let flat: Vec<V3> = (0..12)
            .map(|k| {
                let a = k as f64 * 0.7;
                add(
                    add([3.0, 4.0, 5.0], b.r, 40.0 * a.cos()),
                    b.u,
                    15.0 * a.sin(),
                )
            })
            .collect();
        assert!(near_v(solve_view(&flat, add(b.dir, b.r, 3.0)), b.dir, 1e-9));
        let back = [-b.dir[0], -b.dir[1], -b.dir[2]];
        assert!(near_v(solve_view(&flat, add(back, b.u, 3.0)), back, 1e-9));
    }

    #[test]
    fn solve_view_degenerate() {
        let view = unit([0.3, 0.5, 0.8]).unwrap();
        let b = basis(-1.1, 0.4);
        let axes = [b.dir, b.r, b.u];
        // isotropic: every direction is as good, so the view is kept
        let iso = star([5.0, 6.0, 7.0], axes, [20.0, 20.0, 20.0]);
        assert!(near_v(solve_view(&iso, view), view, 1e-9));
        // nothing, one point, or every point at one place: the view is kept
        assert!(near_v(solve_view(&[], view), view, 1e-12));
        assert!(near_v(solve_view(&[[1.0, 2.0, 3.0]], view), view, 1e-12));
        assert!(near_v(solve_view(&[[1.0, 2.0, 3.0]; 4], view), view, 1e-12));
        // two points, or a line: perpendicular to it, as close to the view as that allows
        let line = b.r;
        let k = dot(view, line);
        let want = unit(add(view, line, -k)).unwrap();
        let two = [[0.0, 0.0, 0.0], add([0.0; 3], line, 30.0)];
        let d2 = solve_view(&two, view);
        assert!(
            dot(d2, line).abs() < 1e-9 && near_v(d2, want, 1e-9),
            "{d2:?}"
        );
        let col: Vec<V3> = (0..6)
            .map(|i| add([1.0, 1.0, 1.0], line, i as f64 * 7.0))
            .collect();
        let dc = solve_view(&col, view);
        assert!(
            dot(dc, line).abs() < 1e-9 && near_v(dc, want, 1e-9),
            "{dc:?}"
        );
        // the two smallest within 10%: in their plane, as close to the view as that allows
        let pair = star([0.0; 3], axes, [10.0, 10.0 * 1.04f64.sqrt(), 40.0]);
        let k = dot(view, b.u);
        let want = unit(add(view, b.u, -k)).unwrap();
        let dp = solve_view(&pair, view);
        assert!(
            dot(dp, b.u).abs() < 1e-9 && near_v(dp, want, 1e-9),
            "{dp:?}"
        );
        // but with the larger pair within 10% instead, the smallest axis is still exact
        let disc = star([0.0; 3], axes, [10.0, 40.0, 41.0]);
        assert!(near_v(solve_view(&disc, b.dir), b.dir, 1e-9));
    }

    #[test]
    fn angles_round_trip() {
        for ti in -11..=12 {
            for pi in -8..=8 {
                let (theta, psi) = (deg(ti as f64 * 15.0), deg(pi as f64 * 10.0));
                let (t, p) = angles(basis(theta, psi).dir, 0.0);
                assert!(
                    (wrap_pi(t - theta)).abs() < 1e-12 && (p - psi).abs() < 1e-12,
                    "{ti} {pi}"
                );
            }
        }
        // past the clamp: the elevation stops at the limit and the azimuth is kept
        let (t, p) = angles(basis(0.8, deg(89.0)).dir, 0.0);
        assert!((t - 0.8).abs() < 1e-9 && p == PSI_MAX);
        let (t, p) = angles(basis(-2.0, deg(-88.0)).dir, 0.0);
        assert!((t + 2.0).abs() < 1e-9 && p == -PSI_MAX);
        // straight up has no azimuth of its own: the given one is kept
        let (t, p) = angles([0.0, 1.0, 0.0], 1.25);
        assert!((t - 1.25).abs() < 1e-12 && p == PSI_MAX);
        // and the clamped direction stays within 5 degrees of the asked one
        let up = basis(0.3, deg(89.9)).dir;
        let (t, p) = angles(up, 0.0);
        assert!(dot(basis(t, p).dir, up) >= deg(5.0).cos() - 1e-12);
    }

    #[test]
    fn fit_fills_seventy_percent() {
        let mut rng = Rng(11);
        for aspect in [0.5, 1.0, 2.5] {
            let pane = Pane {
                w: 800.0 * aspect,
                h: 800.0,
            };
            for _ in 0..50 {
                let pts: Vec<V3> = (0..rng.range(2.0, 40.0) as usize)
                    .map(|_| {
                        [
                            rng.range(-200.0, 200.0),
                            rng.range(-200.0, 200.0),
                            rng.range(-200.0, 200.0),
                        ]
                    })
                    .collect();
                // looking at their centroid, so some point lies toward the eye and sets the distance
                let mut target = [0.0; 3];
                for p in &pts {
                    target = add(target, *p, 1.0 / pts.len() as f64);
                }
                let (theta, psi) = (rng.range(-PI, PI), rng.range(-1.3, 1.3));
                let d = fit_distance(&pts, target, theta, psi, pane, 0.0, 1.0);
                let pose = Pose {
                    target,
                    theta,
                    psi,
                    ln_d: d.ln(),
                };
                let worst = pts
                    .iter()
                    .map(|p| {
                        let s = project(*p, &pose, pane).unwrap();
                        ((s[0] / pane.w * 2.0 - 1.0).abs()).max((1.0 - s[1] / pane.h * 2.0).abs())
                    })
                    .fold(0.0, f64::max);
                assert!((worst - FILL).abs() < 1e-9, "aspect {aspect}: {worst}");
                // a point nearer the eye than the one that set the distance raises it
                let b = basis(theta, psi);
                let (k, _) = pts
                    .iter()
                    .enumerate()
                    .map(|(k, p)| {
                        let s = project(*p, &pose, pane).unwrap();
                        (
                            k,
                            ((s[0] / pane.w * 2.0 - 1.0).abs())
                                .max((1.0 - s[1] / pane.h * 2.0).abs()),
                        )
                    })
                    .fold((0, 0.0), |a, x| if x.1 > a.1 { x } else { a });
                let mut more = pts.clone();
                more.push(add(pts[k], b.dir, 25.0));
                let d2 = fit_distance(&more, target, theta, psi, pane, 0.0, 1.0);
                assert!(d2 > d && (d2 - d - 25.0).abs() < 1e-6, "{d} {d2}");
            }
            // a single point at the target fits at the closest distance allowed
            let t = [4.0, 5.0, 6.0];
            assert_eq!(fit_distance(&[t], t, 0.3, 0.2, pane, 0.0, 42.0), 42.0);
        }
    }
}
