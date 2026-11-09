//! Where the flat overlay's text goes, decided in screen space with no camera, canvas or DOM, so
//! the native tests can prove the two promises the overlay makes: no more than
//! [`MAX_CARD_LABELS`] labels name cards at once, and nothing it draws overlaps anything else it
//! draws: no two rects (labels, region names, a collapsed folder's count) overlap, and no leader
//! line crosses a rect or another leader.
//!
//! A label is an axis-aligned rect beside its point (right, left, above or below it; a count
//! tries the four corners of its ring first). Placement is greedy, in one fixed order:
//!
//! 1. the walk target's label, so the card the keyboard walk is on is never crowded out;
//! 2. the region names, in the order given (they are not counted); at rest, when a name is left
//!    with no room, the names are placed again with the ones left out first, and the order that
//!    names the most is kept;
//! 3. the collapsed folders' counts, in the order given (not counted either);
//! 4. every other label, by tier, the walk target first within its tier, larger importance first,
//!    then by path, until the cap is reached.
//!
//! The walk target's label and a region name never give up while there is room on the pane. One
//! with no free side moves out from its point, first along its lean, sliding sideways, then in a
//! fan of other directions, and last anywhere on the pane, nearest first, and is joined to its
//! point by a leader line ([`leader_spots`], [`fan_spots`], [`grid_spots`]); a spot counts only
//! when its leader crosses nothing either. Where no spot is free, its smaller or shorter forms
//! ([`Alt`]: smaller type, then a middle ellipsis) try the same spots. Only then is it dropped
//! for the frame. A count is dropped when no corner or side
//! of its ring is free: never stacked on another.
//!
//! Every kind but a region name and a count counts against the cap: cards, folders, the focus
//! and the portal labels that name a card in another region. A rect is accepted only inside the
//! pane, clear by [`PAD`] px of the obstacles (the pane's chrome, the compass, the focus's disc)
//! and of every rect placed before it, clear by [`LEADER_PAD`] px of every leader placed before
//! it, and off every point a leader may start from (a region's glyph, the walk target's point)
//! but its own. A label tries the side it was on last frame first, so a moving camera does not make
//! it jump between sides; a region name tries the side it leans toward first.

/// The most labels naming cards that the graph shows at once. Region names are not counted.
pub const MAX_CARD_LABELS: usize = 24;

/// The least room between two rects, and between a rect and an obstacle, in CSS px.
pub const PAD: f64 = 2.0;

/// The least room between a leader line and a rect it does not lead to, in CSS px.
pub const LEADER_PAD: f64 = 1.0;

/// Room between a point's edge and its label, in CSS px.
const GAP: f64 = 3.0;

/// How far out along its lean a label may move, in steps of about its own size, and how many
/// steps sideways it may slide at each distance (either way).
const LEADER_STEPS: usize = 14;
const LEADER_SLIDE: usize = 3;

/// The fan: how many directions either side of the lean (each `FAN_DEG` apart, up to straight
/// back), and how far out along each, in steps of about the label's size.
const FAN_DIRS: usize = 6;
const FAN_DEG: f64 = 30.0;
const FAN_STEPS: usize = 18;

/// How many orders the region names are tried in, at rest, while one is left unnamed.
const NAME_ROUNDS: usize = 4;

/// What a label names. Region names and counts do not count against the cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Region,
    /// the selection itself
    Focus,
    Card,
    Folder,
    /// a link in another region, at its card or at its region's glyph
    Portal,
    /// a collapsed folder's file count, beside its ring
    Count,
}

impl Kind {
    /// The kind as the `data-labels` hook writes it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Region => "region",
            Kind::Focus => "focus",
            Kind::Card => "card",
            Kind::Folder => "folder",
            Kind::Portal => "portal",
            Kind::Count => "count",
        }
    }

    /// Whether the label counts against [`MAX_CARD_LABELS`].
    pub fn counted(self) -> bool {
        !matches!(self, Kind::Region | Kind::Count)
    }
}

/// The side of its point a label sits on: one of four sides, or (a count) a corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Right,
    Left,
    Above,
    Below,
    UpRight,
    UpLeft,
    DownRight,
    DownLeft,
}

/// The sides in the order a label with no preference tries them.
const SIDES: [Anchor; 4] = [Anchor::Right, Anchor::Left, Anchor::Above, Anchor::Below];

/// The spots a count tries: up and to the right of its ring first, as it was always drawn, then
/// the other corners, then the sides.
const CORNERS: [Anchor; 8] = [
    Anchor::UpRight,
    Anchor::UpLeft,
    Anchor::DownRight,
    Anchor::DownLeft,
    Anchor::Right,
    Anchor::Left,
    Anchor::Above,
    Anchor::Below,
];

impl Anchor {
    /// The side's direction from the point, in screen axes (y down).
    fn dir(self) -> [f64; 2] {
        let d = std::f64::consts::FRAC_1_SQRT_2;
        match self {
            Anchor::Right => [1.0, 0.0],
            Anchor::Left => [-1.0, 0.0],
            Anchor::Above => [0.0, -1.0],
            Anchor::Below => [0.0, 1.0],
            Anchor::UpRight => [d, -d],
            Anchor::UpLeft => [-d, -d],
            Anchor::DownRight => [d, d],
            Anchor::DownLeft => [-d, d],
        }
    }
}

/// An axis-aligned rect in pane CSS px, `(x, y)` its top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    /// Whether the two rects are at least `pad` apart on some axis.
    pub fn clear_of(&self, o: &Rect, pad: f64) -> bool {
        self.x + self.w + pad <= o.x
            || o.x + o.w + pad <= self.x
            || self.y + self.h + pad <= o.y
            || o.y + o.h + pad <= self.y
    }

    fn inside(&self, w: f64, h: f64) -> bool {
        self.x >= 0.0 && self.y >= 0.0 && self.x + self.w <= w && self.y + self.h <= h
    }

    fn contains(&self, p: [f64; 2]) -> bool {
        p[0] >= self.x && p[0] <= self.x + self.w && p[1] >= self.y && p[1] <= self.y + self.h
    }

    /// The point of the rect nearest `p`: where a leader from `p` ends.
    pub fn nearest(&self, p: [f64; 2]) -> [f64; 2] {
        [
            p[0].clamp(self.x, self.x + self.w),
            p[1].clamp(self.y, self.y + self.h),
        ]
    }
}

/// Whether the segment `a`–`b` passes within `pad` of the rect `r` (clipped against the rect
/// grown by `pad`, Liang–Barsky).
pub fn seg_hits(a: [f64; 2], b: [f64; 2], r: &Rect, pad: f64) -> bool {
    let (x0, y0, x1, y1) = (r.x - pad, r.y - pad, r.x + r.w + pad, r.y + r.h + pad);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [
        (-dx, a[0] - x0),
        (dx, x1 - a[0]),
        (-dy, a[1] - y0),
        (dy, y1 - a[1]),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// Whether two segments cross. Two leaders from one point (a portal named at its region's
/// glyph, beside the region's own name) meet there and do not count as crossing.
pub fn segs_cross(a: ([f64; 2], [f64; 2]), b: ([f64; 2], [f64; 2])) -> bool {
    if (a.0[0] - b.0[0]).hypot(a.0[1] - b.0[1]) < 0.5 {
        return false;
    }
    let orient = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    let (o1, o2) = (orient(a.0, a.1, b.0), orient(a.0, a.1, b.1));
    let (o3, o4) = (orient(b.0, b.1, a.0), orient(b.0, b.1, a.1));
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

/// A smaller or shorter form of a label, tried once its own text fits nowhere: the text, its
/// rect's size, and the scale of its font (1 for the label's own size).
#[derive(Clone, Debug, PartialEq)]
pub struct Alt {
    pub text: String,
    pub w: f64,
    pub h: f64,
    pub scale: f64,
}

/// A label that may be placed: the point it names at (`x`, `y`) with radius `r`, the size of its
/// text, and what orders it.
#[derive(Clone, Debug)]
pub struct Cand<'a> {
    /// who the label is for (a node, for the last side it sat on); unused for region names
    pub key: usize,
    pub kind: Kind,
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub w: f64,
    pub h: f64,
    pub tier: u8,
    /// the keyboard walk's target: placed before anything else, by a leader when it must be
    pub walk: bool,
    pub importance: f64,
    pub path: &'a str,
    pub text: &'a str,
    /// the side the label leans toward (zero for none): a name on the rim leans inward
    pub lean: [f64; 2],
    /// its smaller or shorter forms, in the order they are tried (a region name's and the walk
    /// target's; others have none)
    pub alts: Vec<Alt>,
}

/// A label as placed: its rect, what it names, the side it took, and the text and font scale it
/// is drawn in.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub kind: Kind,
    pub key: usize,
    pub anchor: Anchor,
    pub text: String,
    /// the font's scale (1 for the label's own size)
    pub scale: f64,
    /// the walk target's label
    pub walk: bool,
    /// the point a label names, when it sits away from its sides: the overlay draws a leader
    /// line from there to the label
    pub leader: Option<[f64; 2]>,
}

impl Placed {
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: self.w,
            h: self.h,
        }
    }

    /// The leader line as drawn: from the point named to the nearest point of the rect.
    pub fn leader_seg(&self) -> Option<([f64; 2], [f64; 2])> {
        let p = self.leader?;
        Some((p, self.rect().nearest(p)))
    }
}

/// The order labels fill the cap in: tier, the walk target first, larger importance first, then
/// path (and the key, so equal labels still order the same way every time).
pub fn order(a: &Cand, b: &Cand) -> std::cmp::Ordering {
    a.tier
        .cmp(&b.tier)
        .then(b.walk.cmp(&a.walk))
        .then(b.importance.total_cmp(&a.importance))
        .then(a.path.cmp(b.path))
        .then(a.key.cmp(&b.key))
}

/// The rect a label takes on side `a` of its point. A corner sits off the ring's diagonal.
fn rect_at(c: &Cand, a: Anchor) -> Rect {
    let gap = c.r.max(0.0) + GAP;
    let k = c.r.max(0.0) * 0.7;
    let (x, y) = match a {
        Anchor::Right => (c.x + gap, c.y - c.h / 2.0),
        Anchor::Left => (c.x - gap - c.w, c.y - c.h / 2.0),
        Anchor::Above => (c.x - c.w / 2.0, c.y - gap - c.h),
        Anchor::Below => (c.x - c.w / 2.0, c.y + gap),
        Anchor::UpRight => (c.x + k, c.y - k - c.h),
        Anchor::UpLeft => (c.x - k - c.w, c.y - k - c.h),
        Anchor::DownRight => (c.x + k, c.y + k),
        Anchor::DownLeft => (c.x - k - c.w, c.y + k),
    };
    Rect {
        x,
        y,
        w: c.w,
        h: c.h,
    }
}

/// The unit direction a label moves out along: its lean, else toward the pane centre, else up.
fn outward(c: &Cand, pane: (f64, f64)) -> [f64; 2] {
    let unit = |v: [f64; 2]| {
        let l = v[0].hypot(v[1]);
        (l > 1e-9).then(|| [v[0] / l, v[1] / l])
    };
    unit(c.lean)
        .or_else(|| unit([pane.0 / 2.0 - c.x, pane.1 / 2.0 - c.y]))
        .unwrap_or([0.0, -1.0])
}

/// Rects centred `d` px out from the point along `u`, slid sideways by `slides` steps of about
/// their own size either way (0, +1, −1, +2, −2, …), for `steps + 1` distances.
fn spots_along(c: &Cand, u: [f64; 2], steps: usize, slides: usize, out: &mut Vec<Rect>) {
    let v = [-u[1], u[0]];
    // the rect's half-extent along a direction, and a step of about its size along it
    let half = |d: [f64; 2]| d[0].abs() * c.w / 2.0 + d[1].abs() * c.h / 2.0;
    let (step, side) = (2.0 * half(u) + PAD, 2.0 * half(v) + PAD);
    for k in 0..=steps {
        let d = c.r.max(0.0) + GAP + half(u) + k as f64 * step;
        for j in 0..=2 * slides {
            let t = if j % 2 == 1 {
                (j / 2 + 1) as f64
            } else {
                -((j / 2) as f64)
            };
            let (cx, cy) = (c.x + u[0] * d + v[0] * t * side, c.y + u[1] * d + v[1] * t * side);
            out.push(Rect {
                x: cx - c.w / 2.0,
                y: cy - c.h / 2.0,
                w: c.w,
                h: c.h,
            });
        }
    }
}

/// The spots a label tries once no side of its point is free, nearest first: rects further out
/// along its lean (toward the pane centre when it has none), at growing distances, each distance
/// trying the rect on the line and then slid sideways by about its own size.
pub fn leader_spots(c: &Cand, pane: (f64, f64)) -> Vec<Rect> {
    let mut spots = Vec::with_capacity((LEADER_STEPS + 1) * (2 * LEADER_SLIDE + 1));
    spots_along(c, outward(c, pane), LEADER_STEPS, LEADER_SLIDE, &mut spots);
    spots
}

/// The wider search, once no spot along the lean is free: the same walk out in a fan of other
/// directions, `FAN_DEG` apart either side of the lean, nearer turns first, out to straight back.
pub fn fan_spots(c: &Cand, pane: (f64, f64)) -> Vec<Rect> {
    let u = outward(c, pane);
    let mut spots = Vec::with_capacity(2 * FAN_DIRS * (FAN_STEPS + 1) * 3);
    for k in 1..=FAN_DIRS {
        for s in [1.0, -1.0] {
            if k == FAN_DIRS && s < 0.0 {
                break; // straight back, once
            }
            let a = s * (k as f64 * FAN_DEG).to_radians();
            let (sa, ca) = a.sin_cos();
            let d = [u[0] * ca - u[1] * sa, u[0] * sa + u[1] * ca];
            spots_along(c, d, FAN_STEPS, 1, &mut spots);
        }
    }
    spots
}

/// The last resort, once the fan has no free spot either: every spot on a grid over the whole
/// pane (half the label's size apart), nearest the point first.
pub fn grid_spots(c: &Cand, pane: (f64, f64)) -> Vec<Rect> {
    let (sx, sy) = ((c.w / 2.0).max(6.0), (c.h / 2.0).max(4.0));
    let (nx, ny) = (((pane.0 - c.w) / sx).floor(), ((pane.1 - c.h) / sy).floor());
    if !(nx >= 0.0 && ny >= 0.0) {
        return Vec::new();
    }
    let mut spots: Vec<(f64, Rect)> = Vec::with_capacity(((nx + 1.0) * (ny + 1.0)) as usize);
    for i in 0..=nx as usize {
        for j in 0..=ny as usize {
            let r = Rect {
                x: i as f64 * sx,
                y: j as f64 * sy,
                w: c.w,
                h: c.h,
            };
            let d = (r.x + c.w / 2.0 - c.x).hypot(r.y + c.h / 2.0 - c.y);
            spots.push((d, r));
        }
    }
    spots.sort_by(|a, b| a.0.total_cmp(&b.0));
    spots.into_iter().map(|(_, r)| r).collect()
}

/// What is on the pane so far: the obstacles, the labels placed, and the points a leader may
/// have to start from (the region glyphs and the walk target's point).
struct Board<'a> {
    pane: (f64, f64),
    obstacles: &'a [Rect],
    placed: Vec<Placed>,
    points: Vec<[f64; 2]>,
    /// whether a label with no spot along its lean searches the fan and the grid too
    thorough: bool,
}

impl Board<'_> {
    /// A rect inside the pane, clear of the obstacles, of every rect placed, of every leader, and
    /// of every point a leader may start from but `own` (so no leader starts under a label).
    fn rect_free(&self, r: &Rect, own: [f64; 2]) -> bool {
        r.inside(self.pane.0, self.pane.1)
            && self.obstacles.iter().all(|o| r.clear_of(o, PAD))
            && self.points.iter().all(|p| {
                (p[0] - own[0]).hypot(p[1] - own[1]) < 0.5
                    || !Rect {
                        x: r.x - PAD,
                        y: r.y - PAD,
                        w: r.w + 2.0 * PAD,
                        h: r.h + 2.0 * PAD,
                    }
                    .contains(*p)
            })
            && self.placed.iter().all(|p| {
                r.clear_of(&p.rect(), PAD)
                    && p.leader_seg()
                        .is_none_or(|(a, b)| !seg_hits(a, b, r, LEADER_PAD))
            })
    }

    /// A leader from `a` to `b` crossing no rect placed, no other leader, and no obstacle (but
    /// one its own point lies under: a card under the legend still gets its leader out).
    fn leader_free(&self, a: [f64; 2], b: [f64; 2]) -> bool {
        self.obstacles
            .iter()
            .all(|o| o.contains(a) || !seg_hits(a, b, o, 0.0))
            && self.placed.iter().all(|p| {
                !seg_hits(a, b, &p.rect(), LEADER_PAD)
                    && p.leader_seg().is_none_or(|l| !segs_cross((a, b), l))
            })
    }

    /// Place `c` (see the module comment), trying the side `last` first. True when placed.
    fn put(&mut self, c: &Cand, last: Option<Anchor>) -> bool {
        let finite = [c.x, c.y, c.r, c.w, c.h].iter().all(|v| v.is_finite());
        if !finite || c.w <= 0.0 || c.h <= 0.0 {
            return false;
        }
        let mut sides: Vec<Anchor> = if c.kind == Kind::Count {
            CORNERS.to_vec()
        } else {
            SIDES.to_vec()
        };
        if c.lean != [0.0, 0.0] {
            let along = |a: Anchor| a.dir()[0] * c.lean[0] + a.dir()[1] * c.lean[1];
            sides.sort_by(|a, b| along(*b).total_cmp(&along(*a)));
        }
        if c.kind.counted()
            && let Some(last) = last
            && let Some(k) = sides.iter().position(|s| *s == last)
        {
            sides[..=k].rotate_right(1);
        }
        let leads = c.kind == Kind::Region || (c.walk && c.kind.counted());
        let forms = std::iter::once((c.text, c.w, c.h, 1.0))
            .chain(c.alts.iter().map(|a| (a.text.as_str(), a.w, a.h, a.scale)));
        for (text, w, h, scale) in forms {
            if !(w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite()) {
                continue;
            }
            let form = Cand {
                w,
                h,
                text,
                alts: Vec::new(),
                ..c.clone()
            };
            let at = [c.x, c.y];
            let spot = sides
                .iter()
                .map(|&a| (rect_at(&form, a), a, None))
                .find(|(r, _, _)| self.rect_free(r, at))
                .or_else(|| {
                    if !leads {
                        return None;
                    }
                    let free =
                        |r: &Rect| self.rect_free(r, at) && self.leader_free(at, r.nearest(at));
                    leader_spots(&form, self.pane)
                        .into_iter()
                        .find(free)
                        .or_else(|| {
                            let wide = self.thorough.then(|| fan_spots(&form, self.pane));
                            wide.into_iter().flatten().find(free)
                        })
                        .or_else(|| {
                            let all = self.thorough.then(|| grid_spots(&form, self.pane));
                            all.into_iter().flatten().find(free)
                        })
                        .map(|r| (r, sides[0], Some(at)))
                });
            if let Some((r, anchor, leader)) = spot {
                self.placed.push(Placed {
                    x: r.x,
                    y: r.y,
                    w: r.w,
                    h: r.h,
                    kind: c.kind,
                    key: c.key,
                    anchor,
                    text: text.to_string(),
                    scale,
                    walk: c.walk && c.kind.counted(),
                    leader,
                });
                return true;
            }
        }
        false
    }
}

/// Place the labels (see the module comment for the order and the rules) in a pane of
/// `pane` = (width, height) CSS px, with the camera at rest. `last_anchor` names the side a key's
/// label sat on last frame, tried first. Returns the labels placed, in the order they were
/// placed.
pub fn place(
    cands: &[Cand],
    obstacles: &[Rect],
    pane: (f64, f64),
    last_anchor: &dyn Fn(usize) -> Option<Anchor>,
) -> Vec<Placed> {
    place_with(cands, obstacles, pane, last_anchor, true)
}

/// `place` for a frame in motion: the fan and the grid are left out (a label that needs them is
/// hidden for that frame), so the frame stays cheap.
pub fn place_moving(
    cands: &[Cand],
    obstacles: &[Rect],
    pane: (f64, f64),
    last_anchor: &dyn Fn(usize) -> Option<Anchor>,
) -> Vec<Placed> {
    place_with(cands, obstacles, pane, last_anchor, false)
}

fn place_with(
    cands: &[Cand],
    obstacles: &[Rect],
    pane: (f64, f64),
    last_anchor: &dyn Fn(usize) -> Option<Anchor>,
    thorough: bool,
) -> Vec<Placed> {
    let mut board = Board {
        thorough,
        pane,
        obstacles,
        placed: Vec::new(),
        points: cands
            .iter()
            .filter(|c| c.kind == Kind::Region || (c.walk && c.kind.counted()))
            .filter(|c| c.x.is_finite() && c.y.is_finite())
            .map(|c| [c.x, c.y])
            .collect(),
    };
    let mut counted = 0;
    let walk = cands.iter().position(|c| c.walk && c.kind.counted());
    if let Some(k) = walk
        && board.put(&cands[k], last_anchor(cands[k].key))
    {
        counted += 1;
    }
    // the region names: a name boxed in by the names placed before it goes first in the next
    // round, and the round that names the most is kept (the first, when all fit)
    let mut names: Vec<usize> = (0..cands.len())
        .filter(|&k| cands[k].kind == Kind::Region)
        .collect();
    let before = board.placed.clone();
    let mut best: Option<(usize, Vec<Placed>)> = None;
    for _ in 0..NAME_ROUNDS {
        board.placed = before.clone();
        let missed: Vec<usize> = names
            .iter()
            .copied()
            .filter(|&k| !board.put(&cands[k], None))
            .collect();
        if best.as_ref().is_none_or(|b| missed.len() < b.0) {
            best = Some((missed.len(), board.placed.clone()));
        }
        if missed.is_empty() || !board.thorough {
            break;
        }
        names = missed
            .iter()
            .chain(names.iter().filter(|k| !missed.contains(k)))
            .copied()
            .collect();
    }
    if let Some((_, placed)) = best {
        board.placed = placed;
    }
    for c in cands.iter().filter(|c| c.kind == Kind::Count) {
        board.put(c, None);
    }
    let mut rest: Vec<&Cand> = cands
        .iter()
        .enumerate()
        .filter(|(k, c)| c.kind.counted() && Some(*k) != walk)
        .map(|(_, c)| c)
        .collect();
    rest.sort_by(|a, b| order(a, b));
    for c in rest {
        if counted >= MAX_CARD_LABELS {
            break;
        }
        if board.put(c, last_anchor(c.key)) {
            counted += 1;
        }
    }
    board.placed
}

/// `s` in at most `max` characters, the middle given up first: a path keeps its first folder and
/// its file name (`workspace/…/stories.md`) when they fit, otherwise its two ends.
pub fn ellipsize_middle(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max || max < 3 {
        return s.to_string();
    }
    if let (Some(a), Some(b)) = (s.find('/'), s.rfind('/'))
        && a < b
    {
        let (head, tail) = (&s[..=a], &s[b..]);
        if head.chars().count() + 1 + tail.chars().count() <= max {
            return format!("{head}…{tail}");
        }
    }
    let keep = max - 1;
    let front = keep.div_ceil(2);
    let back = keep - front;
    let head: String = s.chars().take(front).collect();
    let tail: String = s.chars().skip(n - back).collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator, so a failing scene can be replayed.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    fn cand(key: usize, kind: Kind, x: f64, y: f64, w: f64) -> Cand<'static> {
        Cand {
            key,
            kind,
            x,
            y,
            r: 3.0,
            w,
            h: 13.0,
            tier: 1,
            walk: false,
            importance: 1.0,
            path: "",
            text: "label",
            lean: [0.0, 0.0],
            alts: Vec::new(),
        }
    }

    /// Every rect inside the pane and clear of the chrome, no two rects overlapping, no leader
    /// within [`LEADER_PAD`] of a rect but its own, no two leaders crossing.
    fn assert_clear(got: &[Placed], obs: &[Rect], pane: (f64, f64)) {
        for (i, a) in got.iter().enumerate() {
            let ra = a.rect();
            assert!(ra.inside(pane.0, pane.1), "{a:?} outside the pane");
            for o in obs {
                assert!(ra.clear_of(o, PAD), "{a:?} on the chrome {o:?}");
            }
            for b in &got[i + 1..] {
                assert!(ra.clear_of(&b.rect(), PAD), "{a:?} overlaps {b:?}");
            }
            if let Some((p, q)) = a.leader_seg() {
                for (j, b) in got.iter().enumerate() {
                    if j == i {
                        continue;
                    }
                    let shrunk = Rect {
                        x: b.x + 0.01,
                        y: b.y + 0.01,
                        w: b.w - 0.02,
                        h: b.h - 0.02,
                    };
                    assert!(!seg_hits(p, q, &shrunk, LEADER_PAD - 0.02), "the leader of {a:?} crosses {b:?}");
                    if j > i
                        && let Some(l) = b.leader_seg()
                    {
                        assert!(!segs_cross((p, q), l), "the leaders of {a:?} and {b:?} cross");
                    }
                }
            }
        }
    }

    const KINDS: [Kind; 5] = [
        Kind::Card,
        Kind::Folder,
        Kind::Portal,
        Kind::Focus,
        Kind::Region,
    ];

    /// A random scene of `n` labels in a 1600 × 1000 pane: about one in ten a region name, the
    /// rest cards of every kind, at random tiers and importance, some beside the same point.
    fn scene(rng: &mut Lcg, n: usize) -> Vec<Cand<'static>> {
        let mut out = Vec::with_capacity(n);
        for k in 0..n {
            let kind = if rng.next() < 0.1 {
                Kind::Region
            } else {
                KINDS[(rng.next() * 4.0) as usize]
            };
            let (x, y) = if k > 0 && rng.next() < 0.1 {
                let o: &Cand = &out[(rng.next() * out.len() as f64) as usize];
                (o.x, o.y)
            } else {
                (rng.next() * 1700.0 - 50.0, rng.next() * 1100.0 - 50.0)
            };
            let mut c = cand(k, kind, x, y, 20.0 + rng.next() * 180.0);
            c.r = rng.next() * 22.0;
            c.h = 12.0 + rng.next() * 4.0;
            c.tier = (rng.next() * 2.0) as u8;
            c.walk = k == 3;
            c.importance = rng.next() * 3.0;
            c.lean = if kind == Kind::Region {
                [rng.next() - 0.5, rng.next() - 0.5]
            } else {
                [0.0, 0.0]
            };
            out.push(c);
        }
        out
    }

    fn obstacles() -> Vec<Rect> {
        vec![
            // the toolbar, the legend, the hint, the compass
            Rect {
                x: 10.0,
                y: 10.0,
                w: 26.0,
                h: 160.0,
            },
            Rect {
                x: 10.0,
                y: 960.0,
                w: 700.0,
                h: 30.0,
            },
            Rect {
                x: 1300.0,
                y: 10.0,
                w: 290.0,
                h: 28.0,
            },
            Rect {
                x: 1534.0,
                y: 934.0,
                w: 56.0,
                h: 56.0,
            },
        ]
    }

    fn counted(p: &[Placed]) -> usize {
        p.iter().filter(|p| p.kind.counted()).count()
    }

    #[test]
    fn cap_24() {
        let mut rng = Lcg(7);
        let none = |_: usize| None;
        for n in [0, 1, 10, 25, 60, 200, 700, 2000] {
            for _ in 0..6 {
                let cands = scene(&mut rng, n);
                let got = place(&cands, &obstacles(), (1600.0, 1000.0), &none);
                assert!(counted(&got) <= MAX_CARD_LABELS, "n={n}: {}", counted(&got));
            }
        }
        // where there is room for all of them, exactly the cap is placed
        let roomy: Vec<Cand> = (0..100)
            .map(|k| {
                cand(
                    k,
                    Kind::Card,
                    60.0 + (k % 10) as f64 * 150.0,
                    60.0 + (k / 10) as f64 * 90.0,
                    60.0,
                )
            })
            .collect();
        let got = place(&roomy, &[], (1600.0, 1000.0), &none);
        assert_eq!(counted(&got), MAX_CARD_LABELS);
        // region names are not counted: 24 cards and every name
        let mut mixed = roomy.clone();
        for c in mixed.iter_mut().skip(50).take(10) {
            c.kind = Kind::Region;
        }
        let got = place(&mixed, &[], (1600.0, 1000.0), &none);
        assert_eq!(counted(&got), MAX_CARD_LABELS);
        assert_eq!(got.iter().filter(|p| p.kind == Kind::Region).count(), 10);
    }

    #[test]
    fn no_overlap() {
        let mut rng = Lcg(42);
        let mut last = std::collections::BTreeMap::new();
        let mut total = 0;
        for n in [5, 30, 120, 400, 2000] {
            for _ in 0..6 {
                let cands = scene(&mut rng, n);
                let obs = obstacles();
                let got = place(&cands, &obs, (1600.0, 1000.0), &|k| last.get(&k).copied());
                total += got.len();
                for (i, a) in got.iter().enumerate() {
                    let ra = a.rect();
                    assert!(ra.inside(1600.0, 1000.0), "{a:?} outside the pane");
                    for o in &obs {
                        assert!(ra.clear_of(o, PAD), "{a:?} on the chrome {o:?}");
                    }
                    for b in &got[i + 1..] {
                        assert!(ra.clear_of(&b.rect(), PAD), "{a:?} overlaps {b:?}");
                    }
                }
                // next scene: the sides just taken are tried first
                last.clear();
                for p in &got {
                    last.insert(p.key, p.anchor);
                }
            }
        }
        assert!(total > 100, "scenes placed almost nothing: {total}");
    }

    #[test]
    fn fill_order() {
        // 30 cards with room for all: the cap takes them by tier, the walk target, importance,
        // then path
        let paths: Vec<String> = (0..30).map(|k| format!("r/{:02}.md", 29 - k)).collect();
        let mut cands: Vec<Cand> = (0..30)
            .map(|k| {
                let mut c = cand(
                    k,
                    Kind::Card,
                    60.0 + (k % 6) as f64 * 250.0,
                    60.0 + (k / 6) as f64 * 180.0,
                    80.0,
                );
                c.tier = if k < 26 { 1 } else { 0 };
                c.importance = if k % 3 == 0 { 2.0 } else { 1.0 };
                c
            })
            .collect();
        for (c, p) in cands.iter_mut().zip(&paths) {
            c.path = Box::leak(p.clone().into_boxed_str());
        }
        cands[17].walk = true;
        let got = place(&cands, &[], (1600.0, 1600.0), &|_| None);
        let keys: Vec<usize> = got.iter().map(|p| p.key).collect();
        let mut want: Vec<&Cand> = cands.iter().collect();
        want.sort_by(|a, b| order(a, b));
        let mut want: Vec<usize> = want.iter().map(|c| c.key).collect();
        // the walk target is placed first of all; then the order
        want.retain(|&k| k != 17);
        want.insert(0, 17);
        want.truncate(MAX_CARD_LABELS);
        assert_eq!(keys, want);
        let t0: Vec<usize> = {
            let mut v: Vec<&Cand> = cands.iter().filter(|c| c.tier == 0).collect();
            v.sort_by(|a, b| order(a, b));
            v.iter().map(|c| c.key).collect()
        };
        // 27 is the most important of tier 0 (27 % 3 == 0); then 29, 28, 26 by path (r/00..r/03)
        assert_eq!(t0, vec![27, 29, 28, 26]);
        let mut sorted: Vec<&Cand> = cands.iter().collect();
        sorted.sort_by(|a, b| order(a, b));
        assert_eq!(sorted[4].key, 17, "the walk target leads its tier");
        // a region name that fits nowhere free is dropped, and nothing overlaps it
        let mut a = cand(0, Kind::Region, 100.0, 15.0, 80.0);
        a.r = 0.0;
        let mut b = cand(1, Kind::Region, 100.0, 15.0, 120.0);
        b.r = 0.0;
        let got = place(&[a, b], &[], (200.0, 30.0), &|_| None);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].key, 0);
    }

    #[test]
    fn walk_target_before_regions() {
        // the walk target and a wider region name beside the same point, in a pane with room for
        // only one of them: the walk target's label survives, the name is dropped
        let mut walk = cand(9, Kind::Portal, 100.0, 15.0, 80.0);
        walk.r = 0.0;
        walk.walk = true;
        walk.tier = 1;
        let mut name = cand(1, Kind::Region, 100.0, 15.0, 120.0);
        name.r = 0.0;
        let got = place(&[name.clone(), walk.clone()], &[], (200.0, 30.0), &|_| None);
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].key, 9);
        assert_eq!(got[0].kind, Kind::Portal);
        // the same card, not the walk target, comes after the name and gives way to it
        walk.walk = false;
        walk.w = 120.0;
        name.w = 80.0;
        let got = place(&[walk, name], &[], (200.0, 30.0), &|_| None);
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].kind, Kind::Region);
    }

    #[test]
    fn rim_names_all_placed() {
        // 21 region names on the rim of a 1600 × 1000 pane, in tight clusters of three (glyphs a
        // few px apart, as regions in nearly one direction land), the chrome in three corners:
        // every name is placed, inside the pane, clear of the chrome and of each other, and the
        // crowded ones reach their spot by a leader
        let names = [
            "activity", "agent", "contact", "decision", "design", "doc", "event", "group",
            "memory", "message", "method", "note", "org", "policy", "project", "resource",
            "skill", "system", "talk", "venue", "workspace",
        ];
        let at = [
            (300.0, 34.0), (700.0, 34.0), (1100.0, 34.0), (1500.0, 40.0),
            (34.0, 400.0), (1566.0, 500.0), (800.0, 966.0),
        ];
        let obs = obstacles();
        let cands: Vec<Cand> = names
            .iter()
            .enumerate()
            .map(|(k, n)| {
                let (x, y) = at[k / 3];
                let (x, y) = (x + (k % 3) as f64 * 4.0, y + (k % 3) as f64 * 3.0);
                let (dx, dy) = (800.0 - x, 500.0 - y);
                let l = dx.hypot(dy);
                let mut c = cand(k, Kind::Region, x, y, 8.0 + 7.0 * n.len() as f64);
                c.r = 5.0;
                c.h = 14.0;
                c.text = n;
                c.lean = [dx / l, dy / l];
                c
            })
            .collect();
        let got = place(&cands, &obs, (1600.0, 1000.0), &|_| None);
        let mut named: Vec<&str> = got.iter().map(|p| p.text.as_str()).collect();
        named.sort();
        assert_eq!(named, names.to_vec(), "every rim region is named");
        assert_clear(&got, &obs, (1600.0, 1000.0));
        for (i, a) in got.iter().enumerate() {
            let ra = a.rect();
            assert!(ra.inside(1600.0, 1000.0), "{a:?} outside the pane");
            for o in &obs {
                assert!(ra.clear_of(o, PAD), "{a:?} on the chrome {o:?}");
            }
            for b in &got[i + 1..] {
                assert!(ra.clear_of(&b.rect(), PAD), "{a:?} overlaps {b:?}");
            }
        }
        assert!(got.iter().any(|p| p.leader.is_some()), "no name needed a leader");
    }

    #[test]
    fn counts_never_stack() {
        // two collapsed sibling folders a few px apart (their counts would print on one spot),
        // then a tight row of them: every count placed is clear of the others and of the labels
        let count = |key: usize, x: f64, y: f64| {
            let mut c = cand(key, Kind::Count, x, y, 15.0);
            c.r = 6.0;
            c.h = 11.0;
            c.text = "44";
            c
        };
        let pair = [count(0, 325.0, 222.0), count(1, 331.0, 226.0)];
        let got = place(&pair, &[], (820.0, 900.0), &|_| None);
        assert_eq!(got.len(), 2, "room for both: {got:?}");
        assert_clear(&got, &[], (820.0, 900.0));
        assert_eq!(got[0].anchor, Anchor::UpRight, "the first keeps its usual corner");
        // forty counts on a 3 px grid, with card labels about them: nothing stacks, and the
        // counts that fit nowhere are dropped
        let mut many: Vec<Cand> = (0..40)
            .map(|k| count(k, 400.0 + (k % 8) as f64 * 3.0, 300.0 + (k / 8) as f64 * 3.0))
            .collect();
        many.extend((40..60).map(|k| cand(k, Kind::Card, 380.0 + (k % 5) as f64 * 9.0, 290.0 + (k / 5) as f64 * 2.0, 50.0)));
        let got = place(&many, &obstacles(), (1600.0, 1000.0), &|_| None);
        assert_clear(&got, &obstacles(), (1600.0, 1000.0));
        let n = got.iter().filter(|p| p.kind == Kind::Count).count();
        assert!((2..40).contains(&n), "some counts placed, the rest dropped: {n}");
        assert!(!Kind::Count.counted());
        // no room at all: dropped, never stacked
        let got = place(&pair, &[], (20.0, 12.0), &|_| None);
        assert!(got.len() <= 1, "{got:?}");
    }

    #[test]
    fn leaders_cross_nothing() {
        // a name whose sides are under the chrome, beside a name already placed across its lean:
        // the nearest spots out along its lean are free, but their leaders would run through the
        // other name; it is placed where its leader crosses nothing
        let mut b = cand(1, Kind::Region, 120.0, 500.0, 80.0);
        b.lean = [1.0, 0.0];
        b.r = 4.0;
        let mut a = cand(0, Kind::Region, 100.0, 500.0, 80.0);
        a.lean = [1.0, 0.0];
        a.r = 4.0;
        let obs = vec![Rect {
            x: 0.0,
            y: 440.0,
            w: 118.0,
            h: 120.0,
        }];
        let got = place(&[b, a], &obs, (1600.0, 1000.0), &|_| None);
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(got[1].leader.is_some(), "{got:?}");
        assert_clear(&got, &obs, (1600.0, 1000.0));
        // random scenes of names on the rim, counts, a walk target with no free side and cards:
        // every rect clear of every other, no leader through a rect, no two leaders crossing
        let mut rng = Lcg(99);
        let mut leaders = 0;
        for n in [10, 40, 120, 400] {
            for _ in 0..8 {
                let mut cands = scene(&mut rng, n);
                for c in cands.iter_mut() {
                    if rng.next() < 0.15 {
                        c.kind = Kind::Count;
                        c.w = 10.0 + rng.next() * 12.0;
                        c.h = 11.0;
                        c.lean = [0.0, 0.0];
                    }
                }
                let got = place(&cands, &obstacles(), (1600.0, 1000.0), &|_| None);
                leaders += got.iter().filter(|p| p.leader.is_some()).count();
                assert_clear(&got, &obstacles(), (1600.0, 1000.0));
            }
        }
        assert!(leaders > 10, "the scenes drew almost no leaders: {leaders}");
    }

    #[test]
    fn walk_label_always_placed() {
        // the walk target's point under the legend, its long path wider than any room beside it:
        // it is still labelled, away from its point by a leader, in a shorter form
        let path = "workspace/code-harness/inventory/a-very-long-folder-name/stories.md";
        let mut w = cand(7, Kind::Portal, 60.0, 975.0, 7.2 * path.len() as f64);
        w.walk = true;
        w.text = path;
        w.alts = [34usize, 22, 14]
            .iter()
            .map(|&m| {
                let t = ellipsize_middle(path, m);
                Alt {
                    w: 7.2 * t.chars().count() as f64,
                    h: 13.0,
                    text: t,
                    scale: 1.0,
                }
            })
            .collect();
        let pane = (420.0, 1000.0);
        let mut cands = vec![w.clone()];
        // a crowd of names and cards about it
        for k in 0..30 {
            let mut c = cand(100 + k, if k % 3 == 0 { Kind::Region } else { Kind::Card }, 30.0 + (k % 6) as f64 * 60.0, 880.0 + (k / 6) as f64 * 14.0, 70.0);
            c.lean = [0.0, -1.0];
            cands.push(c);
        }
        let obs = obstacles();
        let got = place(&cands, &obs, pane, &|_| None);
        let walk: Vec<&Placed> = got.iter().filter(|p| p.walk).collect();
        assert_eq!(walk.len(), 1, "{got:?}");
        assert_eq!(walk[0].key, 7);
        assert!(walk[0].leader.is_some(), "{:?}", walk[0]);
        assert!(walk[0].text.contains('…') && walk[0].text.ends_with("stories.md"), "{:?}", walk[0]);
        assert_clear(&got, &obs, pane);
        // with room, it keeps its whole path beside its point
        w.x = 20.0;
        w.y = 500.0;
        let got = place(&[w], &obs, (1600.0, 1000.0), &|_| None);
        assert_eq!(got[0].text, path);
        assert!(got[0].leader.is_none());
    }

    #[test]
    fn last_side_first() {
        let c = cand(5, Kind::Card, 800.0, 500.0, 60.0);
        let got = place(std::slice::from_ref(&c), &[], (1600.0, 1000.0), &|_| {
            Some(Anchor::Below)
        });
        assert_eq!(got[0].anchor, Anchor::Below);
        let got = place(&[c], &[], (1600.0, 1000.0), &|_| None);
        assert_eq!(got[0].anchor, Anchor::Right);
    }

    #[test]
    fn portal_text_ellipsis() {
        assert_eq!(ellipsize_middle("a/b.md", 34), "a/b.md");
        let p = "workspace/code-harness/inventory/deep/stories.md";
        let e = ellipsize_middle(p, 34);
        assert_eq!(e, "workspace/…/stories.md");
        let long = "averyveryverylongfoldername/averyveryverylongfilename.md";
        let e = ellipsize_middle(long, 34);
        assert_eq!(e.chars().count(), 34);
        assert!(e.starts_with("averyvery") && e.ends_with("name.md") && e.contains('…'));
    }
}
