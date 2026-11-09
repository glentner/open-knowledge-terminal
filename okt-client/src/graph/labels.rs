//! Where the flat labels go, decided in screen space with no camera, canvas or DOM, so the native
//! tests can prove the two promises the overlay makes: no more than [`MAX_CARD_LABELS`] labels
//! name cards at once, and no two labels overlap.
//!
//! A label is an axis-aligned rect beside its point (right, left, above or below it). Placement
//! is greedy, in one fixed order:
//!
//! 1. the walk target's label, so the card the keyboard walk is on is never crowded out;
//! 2. the region names, in the order given (they are not counted). A name with no free side
//!    moves further out along its lean, sliding sideways, and is joined to its glyph by a leader
//!    line ([`leader_spots`]), so a crowded rim still names every cluster; only a name with no
//!    room anywhere along that line is dropped for the frame;
//! 3. every other label, by tier, the walk target first within its tier, larger importance first,
//!    then by path, until the cap is reached.
//!
//! Every kind but a region name counts against the cap: cards, folders, the focus and the portal
//! labels that name a card in another region. A rect is accepted only inside the pane and clear,
//! by [`PAD`] px, of the obstacles (the pane's chrome, the compass, the focus's disc) and of every
//! rect placed before it. A label tries the side it was on last frame first, so a moving camera
//! does not make it jump between sides; a region name tries the side it leans toward first.

/// The most labels naming cards that the graph shows at once. Region names are not counted.
pub const MAX_CARD_LABELS: usize = 24;

/// The least room between two rects, and between a rect and an obstacle, in CSS px.
pub const PAD: f64 = 2.0;

/// Room between a point's edge and its label, in CSS px.
const GAP: f64 = 3.0;

/// How far out along its lean a region name may move, in steps of about its own size, and how
/// many steps sideways it may slide at each distance (either way).
const LEADER_STEPS: usize = 14;
const LEADER_SLIDE: usize = 3;

/// What a label names. Every kind but `Region` counts against the cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Region,
    /// the selection itself
    Focus,
    Card,
    Folder,
    /// a link in another region, at its card or at its region's glyph
    Portal,
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
        }
    }

    /// Whether the label counts against [`MAX_CARD_LABELS`].
    pub fn counted(self) -> bool {
        self != Kind::Region
    }
}

/// The side of its point a label sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Right,
    Left,
    Above,
    Below,
}

/// The sides in the order a label with no preference tries them.
const SIDES: [Anchor; 4] = [Anchor::Right, Anchor::Left, Anchor::Above, Anchor::Below];

impl Anchor {
    /// The side's direction from the point, in screen axes (y down).
    fn dir(self) -> [f64; 2] {
        match self {
            Anchor::Right => [1.0, 0.0],
            Anchor::Left => [-1.0, 0.0],
            Anchor::Above => [0.0, -1.0],
            Anchor::Below => [0.0, 1.0],
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
    /// the keyboard walk's target: placed before anything else
    pub walk: bool,
    pub importance: f64,
    pub path: &'a str,
    pub text: &'a str,
    /// the side the label leans toward (zero for none): a name on the rim leans inward
    pub lean: [f64; 2],
}

/// A label as placed: its rect, what it names and the side it took.
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
    /// the point a region name names, when it sits away from its sides: the overlay draws a
    /// leader line from there to the label
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

/// The rect a label takes on side `a` of its point.
fn rect_at(c: &Cand, a: Anchor) -> Rect {
    let gap = c.r.max(0.0) + GAP;
    let (x, y) = match a {
        Anchor::Right => (c.x + gap, c.y - c.h / 2.0),
        Anchor::Left => (c.x - gap - c.w, c.y - c.h / 2.0),
        Anchor::Above => (c.x - c.w / 2.0, c.y - gap - c.h),
        Anchor::Below => (c.x - c.w / 2.0, c.y + gap),
    };
    Rect {
        x,
        y,
        w: c.w,
        h: c.h,
    }
}

/// The spots a region name tries once no side of its glyph is free, nearest first: rects further
/// out along its lean (toward the pane centre when it has none), at growing distances, each
/// distance trying the rect on the line and then slid sideways by about its own size.
pub fn leader_spots(c: &Cand, pane: (f64, f64)) -> Vec<Rect> {
    let unit = |v: [f64; 2]| {
        let l = v[0].hypot(v[1]);
        (l > 1e-9).then(|| [v[0] / l, v[1] / l])
    };
    let u = unit(c.lean)
        .or_else(|| unit([pane.0 / 2.0 - c.x, pane.1 / 2.0 - c.y]))
        .unwrap_or([0.0, -1.0]);
    let v = [-u[1], u[0]];
    // the rect's half-extent along a direction, and a step of about its size along it
    let half = |d: [f64; 2]| d[0].abs() * c.w / 2.0 + d[1].abs() * c.h / 2.0;
    let (out, side) = (2.0 * half(u) + PAD, 2.0 * half(v) + PAD);
    let mut spots = Vec::with_capacity((LEADER_STEPS + 1) * (2 * LEADER_SLIDE + 1));
    for k in 0..=LEADER_STEPS {
        let d = c.r.max(0.0) + GAP + half(u) + k as f64 * out;
        for j in 0..=2 * LEADER_SLIDE {
            // 0, +1, −1, +2, −2, …
            let t = if j % 2 == 1 { (j / 2 + 1) as f64 } else { -((j / 2) as f64) };
            let (cx, cy) = (c.x + u[0] * d + v[0] * t * side, c.y + u[1] * d + v[1] * t * side);
            spots.push(Rect {
                x: cx - c.w / 2.0,
                y: cy - c.h / 2.0,
                w: c.w,
                h: c.h,
            });
        }
    }
    spots
}

/// Place the labels (see the module comment for the order and the rules) in a pane of
/// `pane` = (width, height) CSS px. `last_anchor` names the side a key's label sat on last
/// frame, tried first. Returns the labels placed, in the order they were placed.
pub fn place(
    cands: &[Cand],
    obstacles: &[Rect],
    pane: (f64, f64),
    last_anchor: &dyn Fn(usize) -> Option<Anchor>,
) -> Vec<Placed> {
    let mut placed: Vec<Placed> = Vec::new();
    let mut counted = 0;
    let put = |c: &Cand, placed: &mut Vec<Placed>| -> bool {
        let finite = [c.x, c.y, c.r, c.w, c.h].iter().all(|v| v.is_finite());
        if !finite || c.w <= 0.0 || c.h <= 0.0 {
            return false;
        }
        let mut sides = SIDES;
        if c.lean != [0.0, 0.0] {
            let along = |a: Anchor| a.dir()[0] * c.lean[0] + a.dir()[1] * c.lean[1];
            sides.sort_by(|a, b| along(*b).total_cmp(&along(*a)));
        }
        if c.kind.counted()
            && let Some(last) = last_anchor(c.key)
            && let Some(k) = sides.iter().position(|s| *s == last)
        {
            sides[..=k].rotate_right(1);
        }
        let free = |r: &Rect, placed: &Vec<Placed>| {
            r.inside(pane.0, pane.1)
                && obstacles.iter().all(|o| r.clear_of(o, PAD))
                && placed.iter().all(|p| r.clear_of(&p.rect(), PAD))
        };
        let spot = sides
            .iter()
            .map(|&a| (rect_at(c, a), a, None))
            .find(|(r, _, _)| free(r, placed))
            .or_else(|| {
                if c.kind != Kind::Region {
                    return None;
                }
                leader_spots(c, pane)
                    .into_iter()
                    .find(|r| free(r, placed))
                    .map(|r| (r, sides[0], Some([c.x, c.y])))
            });
        let Some((r, anchor, leader)) = spot else {
            return false;
        };
        placed.push(Placed {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
            kind: c.kind,
            key: c.key,
            anchor,
            text: c.text.to_string(),
            leader,
        });
        true
    };
    let walk = cands.iter().position(|c| c.walk && c.kind.counted());
    if let Some(k) = walk
        && put(&cands[k], &mut placed)
    {
        counted += 1;
    }
    for c in cands.iter().filter(|c| c.kind == Kind::Region) {
        put(c, &mut placed);
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
        if put(c, &mut placed) {
            counted += 1;
        }
    }
    placed
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
