//! The pane arrangement: a binary tree of splits, side by side or stacked, whose leaves are panes
//! holding views as tabs. Pure (std and serde_json only), so it is tested natively; `panes` draws
//! it, `state` keeps it in the settings, and `nav` places views in it.

use serde_json::{Value, json};

/// A collapsed pane's extent: its title bar, or its strip.
pub const TITLE: f64 = 24.0;
/// The narrowest an expanded pane is dragged, side by side.
pub const MIN_W: f64 = 200.0;
/// The shortest an expanded pane is dragged, stacked.
pub const MIN_H: f64 = 120.0;

/// The views a pane can hold.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum View {
    Graph,
    Files,
    Log,
    Table,
    List,
    Insp,
}

impl View {
    pub const ALL: [View; 6] = [
        View::Graph,
        View::Files,
        View::Log,
        View::Table,
        View::List,
        View::Insp,
    ];
    /// The five main views: every view but the Inspector.
    pub const MAIN: [View; 5] = [View::Graph, View::Files, View::Log, View::Table, View::List];

    pub fn name(self) -> &'static str {
        match self {
            View::Graph => "graph",
            View::Files => "files",
            View::Log => "log",
            View::Table => "table",
            View::List => "list",
            View::Insp => "insp",
        }
    }

    pub fn parse(s: &str) -> Option<View> {
        View::ALL.into_iter().find(|v| v.name() == s)
    }

    pub fn is_main(self) -> bool {
        self != View::Insp
    }
}

/// A split's direction: `Row` lays its two children side by side, `Col` stacks them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Orient {
    Row,
    Col,
}

/// A pane's id: assigned on load and when a tree is built, never saved.
pub type PaneId = u32;

#[derive(Clone, Debug, PartialEq)]
pub struct Pane {
    pub id: PaneId,
    pub views: Vec<View>,
    pub shown: View,
    pub collapsed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// `f` is the first child's share of the split's extent.
    Split {
        orient: Orient,
        f: f64,
        kids: Box<[Node; 2]>,
    },
    Pane(Pane),
}

/// What an operation did, for the caller to act on once its settings borrow is dropped.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// the tree changed, so it is saved
    pub changed: bool,
    /// the pane to focus
    pub focus: Option<PaneId>,
    /// a refusal to show
    pub toast: Option<&'static str>,
}

fn leaf(views: &[View], shown: View) -> Node {
    Node::Pane(Pane {
        id: 0,
        views: views.to_vec(),
        shown,
        collapsed: false,
    })
}

fn split(orient: Orient, f: f64, a: Node, b: Node) -> Node {
    Node::Split {
        orient,
        f,
        kids: Box::new([a, b]),
    }
}

/// The five main views on the left, the Inspector on the right; `f` is the left pane's share.
pub fn classic(f: f64) -> Node {
    let mut n = split(
        Orient::Row,
        f,
        leaf(&View::MAIN, View::Graph),
        leaf(&[View::Insp], View::Insp),
    );
    number(&mut n);
    n
}

/// The five main views on top, the Inspector below, in two equal halves.
pub fn stacked() -> Node {
    let mut n = split(
        Orient::Col,
        0.5,
        leaf(&View::MAIN, View::Graph),
        leaf(&[View::Insp], View::Insp),
    );
    number(&mut n);
    n
}

/// The Inspector on the left at 46% of the width, the five main views on the right.
pub fn swapped() -> Node {
    let mut n = split(
        Orient::Row,
        0.46,
        leaf(&[View::Insp], View::Insp),
        leaf(&View::MAIN, View::Graph),
    );
    number(&mut n);
    n
}

/// The Graph collapsed to its title bar across the top; below it the Inspector on the left and,
/// on the right, Files, the Log, the Table and Results with Files shown. Expanded, the Graph takes
/// 40% of the height.
pub fn three() -> Node {
    let mut n = split(
        Orient::Col,
        0.4,
        Node::Pane(Pane {
            id: 0,
            views: vec![View::Graph],
            shown: View::Graph,
            collapsed: true,
        }),
        split(
            Orient::Row,
            0.5,
            leaf(&[View::Insp], View::Insp),
            leaf(
                &[View::Files, View::Log, View::Table, View::List],
                View::Files,
            ),
        ),
    );
    number(&mut n);
    n
}

/// Classic's share for the Inspector at `insp` percent of the width, clamped as the old splitter
/// clamped it.
pub fn classic_share(insp: f64) -> f64 {
    if insp.is_finite() {
        1.0 - insp.clamp(22.0, 75.0) / 100.0
    } else {
        0.54
    }
}

/// The first-load arrangement: stacked in a window narrower than 900 px, classic otherwise, with
/// the Inspector at a splitter width saved before arrangements existed (`legacy`, a percentage)
/// when classic is the one that starts.
pub fn default_for(width: f64, legacy: Option<f64>) -> Node {
    if width < 900.0 {
        return stacked();
    }
    let f = match legacy.filter(|x| x.is_finite()) {
        Some(insp) => classic_share(insp),
        None => 0.54,
    };
    classic(f)
}

/// The arrangements okt ships, in the order the cycle key steps through them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    Classic,
    Swapped,
    Stacked,
    Three,
}

impl Preset {
    pub const ALL: [Preset; 4] = [
        Preset::Classic,
        Preset::Swapped,
        Preset::Stacked,
        Preset::Three,
    ];

    /// The key that applies it.
    pub fn key(self) -> &'static str {
        match self {
            Preset::Classic => ";",
            Preset::Swapped => "'",
            Preset::Stacked => "\\",
            Preset::Three => "`",
        }
    }

    pub fn from_key(k: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.key() == k)
    }

    /// The tree, with classic's share `c`.
    pub fn tree(self, c: f64) -> Node {
        match self {
            Preset::Classic => classic(c),
            Preset::Swapped => swapped(),
            Preset::Stacked => stacked(),
            Preset::Three => three(),
        }
    }

    /// The next in the cycle, after three classic again.
    pub fn next(self) -> Preset {
        let i = Preset::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Preset::ALL[(i + 1) % Preset::ALL.len()]
    }
}

/// Two trees alike: the same splits, directions and shares, and the same panes holding the same
/// views, collapsed alike. Shown tabs and pane ids aside.
fn alike(a: &Node, b: &Node) -> bool {
    match (a, b) {
        (Node::Pane(x), Node::Pane(y)) => {
            x.collapsed == y.collapsed
                && x.views.len() == y.views.len()
                && x.views.iter().all(|v| y.views.contains(v))
        }
        (
            Node::Split {
                orient: o,
                f,
                kids: k,
            },
            Node::Split {
                orient: o2,
                f: f2,
                kids: k2,
            },
        ) => o == o2 && (f - f2).abs() < 1e-6 && alike(&k[0], &k2[0]) && alike(&k[1], &k2[1]),
        _ => false,
    }
}

/// The preset the tree equals, with classic's share `c`.
pub fn which(n: &Node, c: f64) -> Option<Preset> {
    Preset::ALL.into_iter().find(|p| alike(n, &p.tree(c)))
}

/// What the cycle key applies: the preset after the one the tree equals, or classic when it
/// equals none.
pub fn cycle(n: &Node, c: f64) -> Preset {
    which(n, c).map(Preset::next).unwrap_or(Preset::Classic)
}

/// Give every pane an id, in reading order from 1.
pub fn number(n: &mut Node) {
    fn walk(n: &mut Node, next: &mut PaneId) {
        match n {
            Node::Pane(p) => {
                p.id = *next;
                *next += 1;
            }
            Node::Split { kids, .. } => {
                for k in kids.iter_mut() {
                    walk(k, next);
                }
            }
        }
    }
    let mut next = 1;
    walk(n, &mut next);
}

/// Every pane, in reading order.
pub fn panes(n: &Node) -> Vec<&Pane> {
    fn walk<'a>(n: &'a Node, out: &mut Vec<&'a Pane>) {
        match n {
            Node::Pane(p) => out.push(p),
            Node::Split { kids, .. } => {
                for k in kids.iter() {
                    walk(k, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(n, &mut out);
    out
}

pub fn find(n: &Node, id: PaneId) -> Option<&Pane> {
    panes(n).into_iter().find(|p| p.id == id)
}

fn find_mut(n: &mut Node, id: PaneId) -> Option<&mut Pane> {
    match n {
        Node::Pane(p) => (p.id == id).then_some(p),
        Node::Split { kids, .. } => {
            let [a, b] = &mut **kids;
            find_mut(a, id).or_else(|| find_mut(b, id))
        }
    }
}

/// The pane holding `v`.
pub fn holder(n: &Node, v: View) -> Option<PaneId> {
    panes(n)
        .into_iter()
        .find(|p| p.views.contains(&v))
        .map(|p| p.id)
}

/// The expanded pane showing `v`.
pub fn showing(n: &Node, v: View) -> Option<PaneId> {
    panes(n)
        .into_iter()
        .find(|p| p.shown == v && !p.collapsed && p.views.contains(&v))
        .map(|p| p.id)
}

/// The view pane `id` shows.
pub fn shown_in(n: &Node, id: PaneId) -> Option<View> {
    find(n, id).map(|p| p.shown)
}

/// The pane focused when nothing names one: the first expanded pane holding one of the five main
/// views, else the first expanded pane, else the first.
pub fn default_focus(n: &Node) -> PaneId {
    let ps = panes(n);
    ps.iter()
        .find(|p| !p.collapsed && p.views.iter().any(|v| v.is_main()))
        .or_else(|| ps.iter().find(|p| !p.collapsed))
        .or(ps.first())
        .map(|p| p.id)
        .unwrap_or(0)
}

/// The tree's shape: its splits, pane ids and the views each pane holds (not orientation, shares,
/// tabs or collapse). A change of shape re-parents view roots; anything else is drawn in place.
pub fn shape(n: &Node) -> String {
    match n {
        Node::Pane(p) => format!(
            "({}:{})",
            p.id,
            p.views.iter().map(|v| v.name()).collect::<Vec<_>>().join(",")
        ),
        Node::Split { kids, .. } => format!("[{}|{}]", shape(&kids[0]), shape(&kids[1])),
    }
}

/// The node at `path`, a list of child indexes from the root.
pub fn at<'a>(n: &'a Node, path: &[usize]) -> Option<&'a Node> {
    match (path.split_first(), n) {
        (None, _) => Some(n),
        (Some((&i, rest)), Node::Split { kids, .. }) if i < 2 => at(&kids[i], rest),
        _ => None,
    }
}

fn at_mut<'a>(n: &'a mut Node, path: &[usize]) -> Option<&'a mut Node> {
    match (path.split_first(), n) {
        (None, n) => Some(n),
        (Some((&i, rest)), Node::Split { kids, .. }) if i < 2 => at_mut(&mut kids[i], rest),
        _ => None,
    }
}

// ---- JSON ------------------------------------------------------------------------------------

/// `{"v": 1, "root": NODE}`: a split is `{"split": "row"|"col", "f": …, "kids": [NODE, NODE]}`, a
/// pane `{"views": [...], "shown": …, "collapsed": …}`.
pub fn to_json(root: &Node) -> Value {
    fn node(n: &Node) -> Value {
        match n {
            Node::Pane(p) => json!({
                "views": p.views.iter().map(|v| v.name()).collect::<Vec<_>>(),
                "shown": p.shown.name(),
                "collapsed": p.collapsed,
            }),
            Node::Split { orient, f, kids } => json!({
                "split": if *orient == Orient::Row { "row" } else { "col" },
                "f": f,
                "kids": [node(&kids[0]), node(&kids[1])],
            }),
        }
    }
    json!({"v": 1, "root": node(root)})
}

/// The tree as saved, unrepaired (run [`sanitise`] on it). Unknown views are dropped one by one,
/// so a later okt's views never fail the parse; a document of another version is not read.
pub fn from_json(v: &Value) -> Option<Node> {
    if v.get("v")?.as_u64()? != 1 {
        return None;
    }
    parse_node(v.get("root")?)
}

fn parse_node(v: &Value) -> Option<Node> {
    let o = v.as_object()?;
    if let Some(kids) = o.get("kids").and_then(Value::as_array) {
        let mut ks: Vec<Node> = kids.iter().filter_map(parse_node).take(2).collect();
        let orient = if o.get("split").and_then(Value::as_str) == Some("col") {
            Orient::Col
        } else {
            Orient::Row
        };
        let f = o.get("f").and_then(Value::as_f64).unwrap_or(0.5);
        let b = ks.pop()?;
        return Some(match ks.pop() {
            Some(a) => split(orient, f, a, b),
            None => b,
        });
    }
    let views: Vec<View> = o
        .get("views")?
        .as_array()?
        .iter()
        .filter_map(|x| x.as_str().and_then(View::parse))
        .collect();
    let shown = o
        .get("shown")
        .and_then(Value::as_str)
        .and_then(View::parse)
        .or(views.first().copied())
        .unwrap_or(View::Graph);
    Some(Node::Pane(Pane {
        id: 0,
        views,
        shown,
        collapsed: o.get("collapsed").and_then(Value::as_bool).unwrap_or(false),
    }))
}

/// Repair a loaded tree: known views only, each held once (the first holder in reading order
/// keeps it); no empty panes, and a split left with one child replaced by it; a share outside
/// (0, 1) reset to half; a shown tab the pane doesn't hold replaced by its first; never two
/// collapsed panes in one split, nor a collapsed root. Panes are numbered. `None` when nothing is
/// left. A view no pane holds is legal: placing it adds it.
pub fn sanitise(n: Node) -> Option<Node> {
    let mut seen = Vec::new();
    let mut n = clean(n, &mut seen)?;
    accordion(&mut n);
    if let Node::Pane(p) = &mut n {
        p.collapsed = false;
    }
    number(&mut n);
    Some(n)
}

fn clean(n: Node, seen: &mut Vec<View>) -> Option<Node> {
    match n {
        Node::Pane(mut p) => {
            p.views.retain(|v| {
                let fresh = !seen.contains(v);
                if fresh {
                    seen.push(*v);
                }
                fresh
            });
            let first = *p.views.first()?;
            if !p.views.contains(&p.shown) {
                p.shown = first;
            }
            Some(Node::Pane(p))
        }
        Node::Split { orient, f, kids } => {
            let [a, b] = *kids;
            let (a, b) = (clean(a, seen), clean(b, seen));
            match (a, b) {
                (Some(a), Some(b)) => {
                    let f = if f.is_finite() && f > 0.0 && f < 1.0 {
                        f
                    } else {
                        0.5
                    };
                    Some(split(orient, f, a, b))
                }
                (Some(x), None) | (None, Some(x)) => Some(x),
                (None, None) => None,
            }
        }
    }
}

/// A split never keeps two collapsed panes: the second is expanded.
fn accordion(n: &mut Node) {
    if let Node::Split { kids, .. } = n {
        if let [Node::Pane(a), Node::Pane(b)] = &mut **kids {
            if a.collapsed && b.collapsed {
                b.collapsed = false;
            }
        }
        for k in kids.iter_mut() {
            accordion(k);
        }
    }
}

// ---- operations ------------------------------------------------------------------------------

/// Show `v` where it is held, as that pane's shown tab, expanding the pane; a view no pane holds
/// joins the focused pane. `take_focus` moves focus to that pane (the view keys); the Inspector
/// keys leave focus where it is.
pub fn place(root: &mut Node, v: View, focused: PaneId, take_focus: bool) -> Outcome {
    let (target, added) = match holder(root, v) {
        Some(id) => (id, false),
        None => {
            let id = if find(root, focused).is_some() {
                focused
            } else {
                default_focus(root)
            };
            (id, true)
        }
    };
    let Some(p) = find_mut(root, target) else {
        return Outcome::default();
    };
    if added {
        p.views.push(v);
    }
    let changed = added || p.shown != v || p.collapsed;
    p.shown = v;
    p.collapsed = false;
    Outcome {
        changed,
        focus: take_focus.then_some(target),
        toast: None,
    }
}

/// The child indexes from the root to pane `id`.
fn path_of(n: &Node, id: PaneId) -> Option<Vec<usize>> {
    match n {
        Node::Pane(p) => (p.id == id).then(Vec::new),
        Node::Split { kids, .. } => kids.iter().enumerate().find_map(|(i, k)| {
            path_of(k, id).map(|mut p| {
                p.insert(0, i);
                p
            })
        }),
    }
}

/// Replace the tree with preset `p` (classic at share `c`), saved as a choice. Focus goes to the
/// pane holding `view` when it is expanded, else to the first expanded pane holding one of the
/// five main views.
pub fn apply_preset(root: &mut Node, p: Preset, c: f64, view: Option<View>) -> Outcome {
    *root = p.tree(c);
    let focus = view
        .and_then(|v| holder(root, v))
        .filter(|id| find(root, *id).is_some_and(|q| !q.collapsed))
        .unwrap_or_else(|| default_focus(root));
    Outcome {
        changed: true,
        focus: Some(focus),
        toast: None,
    }
}

/// Collapse pane `id` to its title bar (stacked) or strip (side by side), or expand it. Its share
/// is kept, so expanding restores its size; collapsing beside a collapsed pane expands that pane,
/// so the space always has a taker. The only pane is refused. Focus stays on the pane.
pub fn collapse_toggle(root: &mut Node, id: PaneId) -> Outcome {
    let Some(path) = path_of(root, id) else {
        return Outcome::default();
    };
    let Some((&i, up)) = path.split_last() else {
        return Outcome {
            toast: Some("The only pane doesn't collapse"),
            ..Default::default()
        };
    };
    let Some(Node::Split { kids, .. }) = at_mut(root, up) else {
        return Outcome::default();
    };
    let [a, b] = &mut **kids;
    let (me, other) = if i == 0 { (a, b) } else { (b, a) };
    let Node::Pane(me) = me else {
        return Outcome::default();
    };
    me.collapsed = !me.collapsed;
    if me.collapsed
        && let Node::Pane(o) = other
    {
        o.collapsed = false;
    }
    Outcome {
        changed: true,
        focus: Some(id),
        toast: None,
    }
}

/// Which way an operation reaches for a neighbour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    Next,
    Prev,
}

/// Pane `id`'s neighbour: the sibling subtree after it (before it, for `Prev`) when there is one,
/// else the one on the other side; then down that subtree to a pane, in a split running the
/// parent's way the child nearest `id`, in one running across it the first child. Since `id`
/// spans its parent's whole cross axis, the pane reached shares an edge with it on screen. A
/// split has two children, so the sibling is the same subtree either way. The root pane has none.
pub fn neighbour(root: &Node, id: PaneId, dir: Dir) -> Option<PaneId> {
    let path = path_of(root, id)?;
    let (&i, up) = path.split_last()?;
    let Node::Split { orient, kids, .. } = at(root, up)? else {
        return None;
    };
    let after = match dir {
        Dir::Next if i == 0 => 1,
        Dir::Prev if i == 1 => 0,
        _ => 1 - i,
    };
    // the side of the sibling that faces `id`
    let near = if after > i { 0 } else { 1 };
    let mut n = &kids[after];
    loop {
        match n {
            Node::Pane(p) => return Some(p.id),
            Node::Split { orient: o, kids: k, .. } => {
                n = &k[if o == orient { near } else { 0 }];
            }
        }
    }
}

/// Focus the pane `step` places on from `id` in reading order, round the ends.
pub fn focus_step(root: &Node, id: PaneId, step: isize) -> Outcome {
    let ps = panes(root);
    let n = ps.len() as isize;
    let Some(i) = ps.iter().position(|p| p.id == id) else {
        return Outcome {
            focus: ps.first().map(|p| p.id),
            ..Default::default()
        };
    };
    Outcome {
        focus: Some(ps[(i as isize + step).rem_euclid(n) as usize].id),
        ..Default::default()
    }
}

/// Swap pane `id`'s contents (views, shown tab, collapsed) with its neighbour's, each slot keeping
/// its size; focus follows `id`'s contents. The root pane is refused.
pub fn swap(root: &mut Node, id: PaneId, dir: Dir) -> Outcome {
    if find(root, id).is_none() {
        return Outcome::default();
    }
    let Some(nb) = neighbour(root, id, dir) else {
        return refused("No other pane to swap with");
    };
    let (Some(a), Some(b)) = (find(root, id).cloned(), find(root, nb).cloned()) else {
        return Outcome::default();
    };
    for (slot, from) in [(id, &b), (nb, &a)] {
        if let Some(p) = find_mut(root, slot) {
            p.views = from.views.clone();
            p.shown = from.shown;
            p.collapsed = from.collapsed;
        }
    }
    accordion(root);
    Outcome {
        changed: true,
        focus: Some(nb),
        toast: None,
    }
}

/// Turn the split holding pane `id` between side by side and stacked, keeping its children's
/// order and its share. The root pane is refused.
pub fn flip(root: &mut Node, id: PaneId) -> Outcome {
    let Some(path) = path_of(root, id) else {
        return Outcome::default();
    };
    let Some((_, up)) = path.split_last() else {
        return refused("The only pane has no split to turn");
    };
    let Some(Node::Split { orient, .. }) = at_mut(root, up) else {
        return Outcome::default();
    };
    *orient = match orient {
        Orient::Row => Orient::Col,
        Orient::Col => Orient::Row,
    };
    Outcome {
        changed: true,
        focus: Some(id),
        toast: None,
    }
}

/// Close pane `id`: its views join its neighbour, which shows the view `id` showed, is expanded
/// and takes focus; the split that held `id` gives way to its other child, which takes the space.
/// The only pane is refused.
pub fn close(root: &mut Node, id: PaneId, dir: Dir) -> Outcome {
    let Some(path) = path_of(root, id) else {
        return Outcome::default();
    };
    if path.is_empty() {
        return refused("The only pane doesn't close");
    }
    let (Some(nb), Some(gone)) = (neighbour(root, id, dir), find(root, id).cloned()) else {
        return Outcome::default();
    };
    if let Some(q) = find_mut(root, nb) {
        for v in gone.views {
            if !q.views.contains(&v) {
                q.views.push(v);
            }
        }
        q.shown = gone.shown;
        q.collapsed = false;
    }
    lift(root, &path);
    accordion(root);
    Outcome {
        changed: true,
        focus: Some(nb),
        toast: None,
    }
}

/// Move pane `id`'s shown view to its neighbour, as the neighbour's shown tab, expanded and
/// focused; `id` shows the view after it (the first, after the last), or, left empty, closes as [`close`] closes it. The only
/// pane is refused.
pub fn move_view(root: &mut Node, id: PaneId, dir: Dir) -> Outcome {
    let Some(path) = path_of(root, id) else {
        return Outcome::default();
    };
    if path.is_empty() {
        return refused("No other pane to move it to");
    }
    let Some(nb) = neighbour(root, id, dir) else {
        return Outcome::default();
    };
    let Some(p) = find_mut(root, id) else {
        return Outcome::default();
    };
    let v = p.shown;
    let at = p.views.iter().position(|x| *x == v).unwrap_or(0);
    p.views.retain(|x| *x != v);
    let emptied = p.views.is_empty();
    if !emptied {
        // the view after it, or the first when it was the last
        p.shown = p.views[if at < p.views.len() { at } else { 0 }];
    }
    if let Some(q) = find_mut(root, nb) {
        q.views.push(v);
        q.shown = v;
        q.collapsed = false;
    }
    if emptied {
        lift(root, &path);
    }
    accordion(root);
    Outcome {
        changed: true,
        focus: Some(nb),
        toast: None,
    }
}

fn refused(why: &'static str) -> Outcome {
    Outcome {
        toast: Some(why),
        ..Default::default()
    }
}

/// Replace the split holding the node at `path` by its other child.
fn lift(root: &mut Node, path: &[usize]) {
    let Some((&i, up)) = path.split_last() else {
        return;
    };
    let Some(n) = at_mut(root, up).filter(|n| matches!(n, Node::Split { .. })) else {
        return;
    };
    let hole = Node::Pane(Pane {
        id: 0,
        views: Vec::new(),
        shown: View::Graph,
        collapsed: false,
    });
    if let Node::Split { kids, .. } = std::mem::replace(n, hole) {
        let [a, b] = *kids;
        *n = if i == 0 { b } else { a };
    }
}

/// The least extent `n` can be dragged to along `axis`: 200 px across or 120 px down for an
/// expanded pane, its title bar for a collapsed one; a split adds its children's along its own
/// direction and takes the larger across it, so a floor counts every pane inside.
pub fn min_extent(n: &Node, axis: Orient) -> f64 {
    match n {
        Node::Pane(p) if p.collapsed => TITLE,
        Node::Pane(_) => match axis {
            Orient::Row => MIN_W,
            Orient::Col => MIN_H,
        },
        Node::Split { orient, kids, .. } => {
            let (a, b) = (min_extent(&kids[0], axis), min_extent(&kids[1], axis));
            if *orient == axis { a + b } else { a.max(b) }
        }
    }
}

/// The share of the split at `path`.
pub fn share(root: &Node, path: &[usize]) -> Option<f64> {
    match at(root, path)? {
        Node::Split { f, .. } => Some(*f),
        Node::Pane(_) => None,
    }
}

/// Set the split at `path`, `extent` px along its direction, as near the share `want` as the
/// floors allow: each side keeps its [`min_extent`], and a split too small for both is shared in
/// their ratio. Returns the share taken, or `None` when there is no such split or a side is a
/// collapsed pane (its sash is inert).
pub fn drag(root: &mut Node, path: &[usize], want: f64, extent: f64) -> Option<f64> {
    if !want.is_finite() || !(extent > 0.0) {
        return None;
    }
    let Node::Split { orient, f, kids } = at_mut(root, path)? else {
        return None;
    };
    if kids
        .iter()
        .any(|k| matches!(k, Node::Pane(p) if p.collapsed))
    {
        return None;
    }
    let (lo, hi) = (min_extent(&kids[0], *orient), min_extent(&kids[1], *orient));
    let g = if lo + hi > extent {
        lo / (lo + hi)
    } else {
        want.clamp(lo / extent, 1.0 - hi / extent)
    };
    *f = g;
    Some(g)
}

// ---- docking a dragged view --------------------------------------------------------------------

/// A pane's rect in px, from the root's top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    /// Holds the point, its left and top edges included and its right and bottom ones not.
    fn holds(self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Each pane's rect in a `w` × `h` root, in reading order, as the drawing tiles it: a split's
/// first child takes `f` of its extent and the second the rest, or a collapsed child takes the
/// title bar's extent and its sibling the rest.
pub fn layout(root: &Node, w: f64, h: f64) -> Vec<(PaneId, Rect)> {
    fn tile(n: &Node, x: f64, y: f64, w: f64, h: f64, out: &mut Vec<(PaneId, Rect)>) {
        let folded = |k: &Node| matches!(k, Node::Pane(p) if p.collapsed);
        match n {
            Node::Pane(p) => out.push((p.id, Rect { x, y, w, h })),
            Node::Split { orient, f, kids } => {
                let e = if *orient == Orient::Row { w } else { h };
                let lead = if folded(&kids[0]) {
                    TITLE
                } else if folded(&kids[1]) {
                    e - TITLE
                } else {
                    f * e
                };
                if *orient == Orient::Row {
                    tile(&kids[0], x, y, lead, h, out);
                    tile(&kids[1], x + lead, y, w - lead, h, out);
                } else {
                    tile(&kids[0], x, y, w, lead, out);
                    tile(&kids[1], x, y + lead, w, h - lead, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    tile(root, 0.0, 0.0, w, h, &mut out);
    out
}

/// Where in a pane a dragged view lands: joining it as a tab, or splitting it on a side.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zone {
    Centre,
    Left,
    Right,
    Top,
    Bottom,
}

impl Zone {
    #[cfg(test)]
    pub const ALL: [Zone; 5] = [Zone::Centre, Zone::Left, Zone::Right, Zone::Top, Zone::Bottom];
}

/// An edge zone's depth, as a share of the pane body's width or height.
const BAND: f64 = 0.25;

/// The zone of the pane at `r` under the point. A collapsed pane, and any pane's title bar, is all
/// centre. In the body below the title bar, a point within a quarter of the body's width of its
/// left or right side, or a quarter of its height of its top or bottom, is that edge's zone (the
/// nearest by share where two bands meet, ties going left, right, top, bottom); the rest is the
/// centre.
pub fn zone_at(r: Rect, collapsed: bool, x: f64, y: f64) -> Zone {
    if collapsed || y < r.y + TITLE || r.h - TITLE <= 0.0 || r.w <= 0.0 {
        return Zone::Centre;
    }
    let (by, bh) = (r.y + TITLE, r.h - TITLE);
    let shares = [
        (Zone::Left, (x - r.x) / r.w),
        (Zone::Right, (r.x + r.w - x) / r.w),
        (Zone::Top, (y - by) / bh),
        (Zone::Bottom, (by + bh - y) / bh),
    ];
    let mut best = (Zone::Centre, BAND);
    for (z, s) in shares {
        if s < best.1 {
            best = (z, s);
        }
    }
    best.0
}

/// The pane under a point given relative to the root's top left, and its zone; `None` outside
/// the root.
pub fn aim(root: &Node, w: f64, h: f64, x: f64, y: f64) -> Option<(PaneId, Zone)> {
    let mut r = Rect { x: 0.0, y: 0.0, w, h };
    if !r.holds(x, y) {
        return None;
    }
    // down the splits as `layout` tiles them, into the child whose side of the cut holds the point
    let folded = |k: &Node| matches!(k, Node::Pane(p) if p.collapsed);
    let mut n = root;
    loop {
        match n {
            Node::Pane(p) => return Some((p.id, zone_at(r, p.collapsed, x, y))),
            Node::Split { orient, f, kids } => {
                let row = *orient == Orient::Row;
                let e = if row { r.w } else { r.h };
                let lead = if folded(&kids[0]) {
                    TITLE
                } else if folded(&kids[1]) {
                    e - TITLE
                } else {
                    f * e
                };
                let first = if row { x < r.x + lead } else { y < r.y + lead };
                r = match (row, first) {
                    (true, true) => Rect { w: lead, ..r },
                    (true, false) => Rect { x: r.x + lead, w: r.w - lead, ..r },
                    (false, true) => Rect { h: lead, ..r },
                    (false, false) => Rect { y: r.y + lead, h: r.h - lead, ..r },
                };
                n = &kids[if first { 0 } else { 1 }];
            }
        }
    }
}

/// The tree after view `v` is dropped on `zone` of pane `target` in a `w` × `h` root, and the id
/// of the pane holding `v` there; `None` when the zone is invalid, so a release there changes
/// nothing. The view leaves its pane, which shows the view after it (the first, after the last)
/// if it showed `v`. On the centre it joins `target` as its last tab, shown, the pane expanded; on
/// an edge `target` splits half and half on that side, a new pane holding only `v` taking the
/// side. A pane left empty closes, its sibling taking its space. Invalid: its own pane's centre, a
/// collapsed pane's edges, any zone of its own pane when that holds only `v`, and any drop whose
/// result breaks the arrangement's rules (an empty pane, a view held twice or lost, two collapsed
/// panes in one split, a collapsed root) or, on an edge, leaves the new pane or `target` under the
/// floor across (left, right) or down (top, bottom) in the result's layout.
pub fn docked(
    root: &Node,
    v: View,
    target: PaneId,
    zone: Zone,
    w: f64,
    h: f64,
) -> Option<(Node, PaneId)> {
    let src = holder(root, v)?;
    let onto = find(root, target)?;
    if zone == Zone::Centre && src == target {
        return None;
    }
    if zone != Zone::Centre && onto.collapsed {
        return None;
    }
    if src == target && onto.views.len() == 1 {
        return None;
    }
    let mut n = root.clone();
    // the view leaves its pane
    let p = find_mut(&mut n, src)?;
    let at = p.views.iter().position(|x| *x == v)?;
    p.views.remove(at);
    let emptied = p.views.is_empty();
    if !emptied && p.shown == v {
        // the view after it, or the first when it was the last
        p.shown = p.views[if at < p.views.len() { at } else { 0 }];
    }
    let lands = if zone == Zone::Centre {
        let q = find_mut(&mut n, target)?;
        q.views.push(v);
        q.shown = v;
        q.collapsed = false;
        target
    } else {
        // ids are never renumbered, so the count could name a pane that is still there
        let id = panes(&n).iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let path = path_of(&n, target)?;
        let slot = at_mut(&mut n, &path)?;
        let made = Node::Pane(Pane {
            id,
            views: vec![v],
            shown: v,
            collapsed: false,
        });
        let old = slot.clone();
        let orient = if matches!(zone, Zone::Left | Zone::Right) {
            Orient::Row
        } else {
            Orient::Col
        };
        *slot = if matches!(zone, Zone::Left | Zone::Top) {
            split(orient, 0.5, made, old)
        } else {
            split(orient, 0.5, old, made)
        };
        id
    };
    if emptied {
        let path = path_of(&n, src)?;
        if path.is_empty() {
            return None;
        }
        lift(&mut n, &path);
    }
    if !keeps_the_rules(&n, root) {
        return None;
    }
    if zone != Zone::Centre {
        let rs = layout(&n, w, h);
        let across = matches!(zone, Zone::Left | Zone::Right);
        for id in [lands, target] {
            let (_, r) = rs.iter().find(|(p, _)| *p == id)?;
            let fits = if across { r.w >= MIN_W } else { r.h >= MIN_H };
            if !fits {
                return None;
            }
        }
    }
    Some((n, lands))
}

/// `n` holds exactly the views `before` held, each once, in panes that are none of them empty and
/// each show a view they hold; no split holds two collapsed panes, and the root isn't collapsed.
fn keeps_the_rules(n: &Node, before: &Node) -> bool {
    fn folds(n: &Node) -> bool {
        match n {
            Node::Pane(_) => false,
            Node::Split { kids, .. } => {
                matches!(&**kids, [Node::Pane(a), Node::Pane(b)] if a.collapsed && b.collapsed)
                    || kids.iter().any(folds)
            }
        }
    }
    let held = |n: &Node| {
        let mut vs: Vec<usize> = panes(n)
            .iter()
            .flat_map(|p| p.views.iter().map(|v| *v as usize))
            .collect();
        vs.sort_unstable();
        vs
    };
    let after = held(n);
    !matches!(n, Node::Pane(p) if p.collapsed)
        && !folds(n)
        && panes(n)
            .iter()
            .all(|p| !p.views.is_empty() && p.views.contains(&p.shown))
        && after.windows(2).all(|x| x[0] != x[1])
        && after == held(before)
}

/// Drop view `v` on `zone` of pane `target`, as [`docked`] says: the tree is replaced whole, and
/// focus goes to the pane holding `v`. An invalid zone changes nothing and says nothing: no
/// outline showed there.
pub fn dock(root: &mut Node, v: View, target: PaneId, zone: Zone, w: f64, h: f64) -> Outcome {
    match docked(root, v, target, zone, w, h) {
        Some((n, id)) => {
            *root = n;
            Outcome {
                changed: true,
                focus: Some(id),
                toast: None,
            }
        }
        None => Outcome::default(),
    }
}

/// The rect the pane holding `v` takes after the drop [`docked`] makes, in the same root: what the
/// outline shows while the drag is over that zone. `None` where the zone is invalid.
pub fn preview(root: &Node, v: View, target: PaneId, zone: Zone, w: f64, h: f64) -> Option<Rect> {
    let (n, id) = docked(root, v, target, zone, w, h)?;
    layout(&n, w, h)
        .into_iter()
        .find(|(p, _)| *p == id)
        .map(|(_, r)| r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane_views(n: &Node) -> Vec<Vec<&'static str>> {
        panes(n)
            .iter()
            .map(|p| p.views.iter().map(|v| v.name()).collect())
            .collect()
    }

    #[test]
    fn json_round_trip_keeps_the_tree() {
        let mut n = split(
            Orient::Col,
            0.4,
            Node::Pane(Pane {
                id: 0,
                views: vec![View::Graph],
                shown: View::Graph,
                collapsed: true,
            }),
            split(
                Orient::Row,
                0.5,
                leaf(&[View::Insp], View::Insp),
                leaf(&[View::Files, View::Log, View::Table, View::List], View::Files),
            ),
        );
        number(&mut n);
        let text = to_json(&n).to_string();
        let back = sanitise(from_json(&serde_json::from_str(&text).unwrap()).unwrap()).unwrap();
        assert_eq!(back, n);
        assert_eq!(to_json(&back).to_string(), text);
        let c = classic(0.7);
        assert_eq!(sanitise(from_json(&to_json(&c)).unwrap()).unwrap(), c);
    }

    #[test]
    fn sanitise_drops_unknown_and_duplicate_views() {
        let v: Value = serde_json::from_str(
            r#"{"v": 1, "root": {"split": "row", "f": 0.6, "kids": [
                {"views": ["graph", "chat", "files", "graph"], "shown": "chat", "collapsed": false},
                {"views": ["files", "insp", "dream"], "shown": "files", "collapsed": false}]}}"#,
        )
        .unwrap();
        let n = sanitise(from_json(&v).unwrap()).unwrap();
        assert_eq!(pane_views(&n), vec![vec!["graph", "files"], vec!["insp"]]);
        let ps = panes(&n);
        assert_eq!(ps[0].shown, View::Graph, "an unknown shown tab becomes the first view");
        assert_eq!(ps[1].shown, View::Insp, "a tab no longer held becomes the first view");
        assert_eq!(holder(&n, View::Log), None, "a view no pane holds is legal");
        assert_eq!(share(&n, &[]), Some(0.6));
    }

    #[test]
    fn sanitise_rebuilds_a_broken_tree() {
        // an empty pane and a one-child split fold away; a bad share resets; two collapsed panes
        // in one split keep one expanded; a collapsed root expands
        let v: Value = serde_json::from_str(
            r#"{"v": 1, "root": {"split": "col", "f": 7, "kids": [
                {"split": "row", "f": 0.3, "kids": [{"views": [], "shown": "graph"}, {"views": ["log"], "shown": "log"}]},
                {"split": "row", "f": -1, "kids": [
                    {"views": ["graph"], "shown": "graph", "collapsed": true},
                    {"views": ["insp"], "shown": "insp", "collapsed": true}]}]}}"#,
        )
        .unwrap();
        let n = sanitise(from_json(&v).unwrap()).unwrap();
        assert_eq!(shape(&n), "[(1:log)|[(2:graph)|(3:insp)]]");
        assert_eq!(share(&n, &[]), Some(0.5));
        assert_eq!(share(&n, &[1]), Some(0.5));
        let ps = panes(&n);
        assert!(ps[1].collapsed && !ps[2].collapsed);
        let lone = serde_json::json!({"v": 1, "root": {"views": ["files"], "shown": "files", "collapsed": true}});
        let n = sanitise(from_json(&lone).unwrap()).unwrap();
        assert!(!panes(&n)[0].collapsed);
        let empty = serde_json::json!({"v": 1, "root": {"split": "row", "kids": [{"views": ["nope"]}]}});
        assert!(from_json(&empty).and_then(sanitise).is_none());
        assert!(from_json(&serde_json::json!({"v": 2, "root": {"views": ["graph"]}})).is_none());
        assert!(from_json(&serde_json::json!("classic")).is_none());
    }

    #[test]
    fn default_is_stacked_below_900_and_classic_from_900() {
        let s = default_for(899.0, None);
        assert_eq!(s, stacked());
        assert!(matches!(&s, Node::Split { orient: Orient::Col, f, .. } if *f == 0.5));
        let c = default_for(900.0, None);
        assert_eq!(c, classic(0.54));
        assert_eq!(pane_views(&c), vec![vec!["graph", "files", "log", "table", "list"], vec!["insp"]]);
        assert_eq!(default_for(1600.0, None), classic(0.54));
    }

    #[test]
    fn legacy_insp_sizes_classic_only() {
        assert!((share(&default_for(1600.0, Some(30.0)), &[]).unwrap() - 0.7).abs() < 1e-12);
        assert!((share(&default_for(900.0, Some(30.0)), &[]).unwrap() - 0.7).abs() < 1e-12);
        assert_eq!(default_for(899.0, Some(30.0)), stacked());
        // a saved 46 sizes classic at 46; out of range clamps as the old splitter did
        assert!((share(&default_for(1600.0, Some(46.0)), &[]).unwrap() - 0.54).abs() < 1e-12);
        assert!((share(&default_for(1600.0, Some(90.0)), &[]).unwrap() - 0.25).abs() < 1e-12);
        assert!((share(&default_for(1600.0, Some(5.0)), &[]).unwrap() - 0.78).abs() < 1e-12);
    }

    #[test]
    fn drag_stops_at_the_floor() {
        let mut n = classic(0.54);
        assert_eq!(drag(&mut n, &[], 1000.0 / 1600.0, 1600.0), Some(0.625));
        assert_eq!(drag(&mut n, &[], 0.01, 1600.0), Some(200.0 / 1600.0));
        assert_eq!(drag(&mut n, &[], 0.99, 1600.0), Some(1.0 - 200.0 / 1600.0));
        // a split narrower than both floors is shared half and half
        assert_eq!(drag(&mut n, &[], 0.9, 350.0), Some(0.5));
        let mut s = stacked();
        assert_eq!(drag(&mut s, &[], 0.0, 890.0), Some(120.0 / 890.0));
        assert_eq!(drag(&mut s, &[], 1.0, 890.0), Some(1.0 - 120.0 / 890.0));
        assert_eq!(drag(&mut s, &[0], 0.5, 890.0), None, "a pane is not a split");
        assert_eq!(drag(&mut s, &[], f64::NAN, 890.0), None);
    }

    #[test]
    fn drag_floor_counts_nested_panes() {
        // the Graph beside the Inspector and Files side by side: the inner panes keep 200 px each
        let mut n = split(
            Orient::Row,
            0.5,
            leaf(&[View::Graph], View::Graph),
            split(
                Orient::Row,
                0.5,
                leaf(&[View::Insp], View::Insp),
                leaf(&[View::Files], View::Files),
            ),
        );
        number(&mut n);
        assert_eq!(min_extent(at(&n, &[1]).unwrap(), Orient::Row), 400.0);
        assert_eq!(drag(&mut n, &[], 0.95, 1600.0), Some(0.75));
        // across a stacked side the floor is its widest pane's
        let mut m = split(
            Orient::Row,
            0.5,
            leaf(&[View::Graph], View::Graph),
            split(
                Orient::Col,
                0.5,
                leaf(&[View::Insp], View::Insp),
                leaf(&[View::Files], View::Files),
            ),
        );
        number(&mut m);
        assert_eq!(drag(&mut m, &[], 0.95, 1600.0), Some(1.0 - 200.0 / 1600.0));
        assert_eq!(min_extent(&m, Orient::Col), 240.0);
    }

    #[test]
    fn place_finds_the_holder_or_adds_to_the_focused_pane() {
        let mut n = classic(0.54);
        let o = place(&mut n, View::Files, 2, true);
        assert_eq!(o, Outcome { changed: true, focus: Some(1), toast: None });
        assert_eq!(panes(&n)[0].shown, View::Files);
        // placing what is already shown changes nothing but still focuses
        assert_eq!(place(&mut n, View::Files, 2, true), Outcome { changed: false, focus: Some(1), toast: None });
        // a view no pane holds joins the focused pane and shows there
        let mut m = split(
            Orient::Row,
            0.54,
            leaf(&[View::Graph, View::Files], View::Graph),
            leaf(&[View::Insp], View::Insp),
        );
        number(&mut m);
        let o = place(&mut m, View::Log, 2, true);
        assert_eq!(o.focus, Some(2));
        assert!(o.changed);
        assert_eq!(pane_views(&m), vec![vec!["graph", "files"], vec!["insp", "log"]]);
        assert_eq!(panes(&m)[1].shown, View::Log);
        // a collapsed holder expands
        if let Node::Split { kids, .. } = &mut m {
            if let Node::Pane(p) = &mut kids[0] {
                p.collapsed = true;
            }
        }
        assert!(place(&mut m, View::Graph, 2, true).changed);
        assert!(!panes(&m)[0].collapsed);
    }

    /// Each pane's rect as `[x, y, w, h]`, in reading order ([`layout`]).
    fn boxes(n: &Node, w: f64, h: f64) -> Vec<[f64; 4]> {
        layout(n, w, h)
            .into_iter()
            .map(|(_, r)| [r.x, r.y, r.w, r.h])
            .collect()
    }

    fn collapsed(n: &Node) -> Vec<bool> {
        panes(n).iter().map(|p| p.collapsed).collect()
    }

    #[test]
    fn presets_hold_every_view_once() {
        for p in Preset::ALL {
            let n = p.tree(0.54);
            let held: Vec<View> = panes(&n).iter().flat_map(|q| q.views.clone()).collect();
            assert_eq!(held.len(), View::ALL.len(), "{p:?} holds each view once");
            for v in View::ALL {
                assert!(held.contains(&v), "{p:?} holds {v:?}");
            }
            assert_eq!(sanitise(n.clone()), Some(n.clone()), "{p:?} is already sane");
            assert_eq!(which(&n, 0.54), Some(p));
            assert_eq!(Preset::from_key(p.key()), Some(p));
        }
        assert_eq!(pane_views(&three()), vec![
            vec!["graph"],
            vec!["insp"],
            vec!["files", "log", "table", "list"]
        ]);
        let t = three();
        assert_eq!(panes(&t)[2].shown, View::Files);
        assert_eq!(collapsed(&t), vec![true, false, false]);
        // the cycle steps after the preset the tree equals, from classic when it equals none
        assert_eq!(cycle(&classic(0.54), 0.54), Preset::Swapped);
        assert_eq!(cycle(&swapped(), 0.54), Preset::Stacked);
        assert_eq!(cycle(&stacked(), 0.54), Preset::Three);
        assert_eq!(cycle(&three(), 0.54), Preset::Classic);
        let mut dragged = classic(0.54);
        drag(&mut dragged, &[], 0.6, 1600.0);
        assert_eq!(which(&dragged, 0.54), None);
        assert_eq!(cycle(&dragged, 0.54), Preset::Classic);
        // a shown tab doesn't make another arrangement; classic follows the legacy share
        let mut c = classic(0.7);
        place(&mut c, View::Log, 1, true);
        assert_eq!(which(&c, 0.7), Some(Preset::Classic));
        assert_eq!(which(&c, 0.54), None);
    }

    /// The same rects, to a millionth of a px.
    fn near(a: &[[f64; 4]], b: &[[f64; 4]]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(x, y)| x.iter().zip(y).all(|(u, v)| (u - v).abs() < 1e-6))
    }

    /// The presets' rects at both widths in an 890 px root, as the pane arrangements plan sized them.
    fn preset_rects() -> [(Preset, f64, Vec<[f64; 4]>); 8] {
        let h = 890.0;
        [
            (Preset::Classic, 1600.0, vec![[0.0, 0.0, 864.0, h], [864.0, 0.0, 736.0, h]]),
            (Preset::Swapped, 1600.0, vec![[0.0, 0.0, 736.0, h], [736.0, 0.0, 864.0, h]]),
            (Preset::Stacked, 1600.0, vec![[0.0, 0.0, 1600.0, 445.0], [0.0, 445.0, 1600.0, 445.0]]),
            (
                Preset::Three,
                1600.0,
                vec![[0.0, 0.0, 1600.0, 24.0], [0.0, 24.0, 800.0, 866.0], [800.0, 24.0, 800.0, 866.0]],
            ),
            (Preset::Classic, 900.0, vec![[0.0, 0.0, 486.0, h], [486.0, 0.0, 414.0, h]]),
            (Preset::Swapped, 900.0, vec![[0.0, 0.0, 414.0, h], [414.0, 0.0, 486.0, h]]),
            (Preset::Stacked, 900.0, vec![[0.0, 0.0, 900.0, 445.0], [0.0, 445.0, 900.0, 445.0]]),
            (
                Preset::Three,
                900.0,
                vec![[0.0, 0.0, 900.0, 24.0], [0.0, 24.0, 450.0, 866.0], [450.0, 24.0, 450.0, 866.0]],
            ),
        ]
    }

    #[test]
    fn preset_sizes_match_the_plan() {
        let h = 890.0;
        for (p, w, want) in preset_rects() {
            let got = boxes(&p.tree(0.54), w, h);
            assert!(near(&got, &want), "{p:?} at {w}: {got:?}");
        }
        // classic by key takes a legacy share; three's Graph expanded takes 40% of the height
        assert!(near(&boxes(&Preset::Classic.tree(classic_share(30.0)), 1600.0, h)[..1], &[[0.0, 0.0, 1120.0, h]]));
        let mut t = three();
        place(&mut t, View::Graph, 3, true);
        assert!(near(&boxes(&t, 1600.0, 1000.0)[..1], &[[0.0, 0.0, 1600.0, 400.0]]));
    }

    #[test]
    fn collapse_keeps_the_share_and_expand_restores_it() {
        let mut n = classic(0.54);
        let o = collapse_toggle(&mut n, 1);
        assert!(o.changed);
        assert_eq!(collapsed(&n), vec![true, false]);
        assert_eq!(share(&n, &[]), Some(0.54), "the share is kept");
        assert!((boxes(&n, 1600.0, 890.0)[1][2] - (1600.0 - TITLE)).abs() < 1e-9);
        collapse_toggle(&mut n, 1);
        assert_eq!(n, classic(0.54), "expand gives back what collapse changed");
        // in a stack: the title bar, then back
        let mut s = stacked();
        collapse_toggle(&mut s, 2);
        assert_eq!(boxes(&s, 900.0, 890.0)[1], [0.0, 890.0 - TITLE, 900.0, TITLE]);
        collapse_toggle(&mut s, 2);
        assert_eq!(s, stacked());
        // a collapsed side's sash is inert
        collapse_toggle(&mut s, 1);
        assert_eq!(drag(&mut s, &[], 0.3, 890.0), None);
    }

    #[test]
    fn collapsing_beside_a_collapsed_leaf_expands_it() {
        let mut s = stacked();
        collapse_toggle(&mut s, 1);
        collapse_toggle(&mut s, 2);
        assert_eq!(collapsed(&s), vec![false, true]);
        // three: the Inspector collapses beside Files; then Files, and the Inspector expands
        let mut t = three();
        collapse_toggle(&mut t, 2);
        assert_eq!(collapsed(&t), vec![true, true, false]);
        collapse_toggle(&mut t, 3);
        assert_eq!(collapsed(&t), vec![true, false, true]);
        // beside a split, collapsing leaves the split as it is
        let mut t = three();
        collapse_toggle(&mut t, 1);
        collapse_toggle(&mut t, 1);
        assert_eq!(collapsed(&t), vec![true, false, false]);
    }

    #[test]
    fn the_only_pane_does_not_collapse() {
        let mut n = leaf(&View::ALL, View::Graph);
        number(&mut n);
        let before = n.clone();
        let o = collapse_toggle(&mut n, 1);
        assert!(!o.changed && o.focus.is_none() && o.toast.is_some());
        assert_eq!(n, before);
        // a pane that isn't there changes nothing
        let mut c = classic(0.54);
        assert_eq!(collapse_toggle(&mut c, 9), Outcome::default());
        assert_eq!(c, classic(0.54));
    }

    #[test]
    fn collapse_keeps_focus_on_the_pane() {
        let mut n = classic(0.54);
        assert_eq!(collapse_toggle(&mut n, 2).focus, Some(2));
        assert_eq!(collapse_toggle(&mut n, 2).focus, Some(2));
        // a preset focuses the pane holding the current view when it is expanded, else the first
        // expanded pane holding a main view
        let mut m = classic(0.54);
        assert_eq!(apply_preset(&mut m, Preset::Swapped, 0.54, Some(View::Graph)).focus, Some(2));
        let o = apply_preset(&mut m, Preset::Three, 0.54, Some(View::Graph));
        assert_eq!((o.changed, o.focus), (true, Some(3)));
        assert_eq!(m, three());
        let o = apply_preset(&mut m, Preset::Three, 0.54, Some(View::Insp));
        assert_eq!(o.focus, Some(2));
    }

    fn pane_of(n: &Node, v: View) -> &Pane {
        panes(n).into_iter().find(|p| p.views.contains(&v)).unwrap()
    }

    #[test]
    fn neighbour_shares_an_edge() {
        // two rects share an edge when they touch along one side and overlap along it
        let touch = |a: [f64; 4], b: [f64; 4]| {
            let overlap = |lo: f64, hi: f64, lo2: f64, hi2: f64| lo.max(lo2) < hi.min(hi2);
            let side = ((a[0] + a[2] - b[0]).abs() < 1e-9 || (b[0] + b[2] - a[0]).abs() < 1e-9)
                && overlap(a[1], a[1] + a[3], b[1], b[1] + b[3]);
            let stack = ((a[1] + a[3] - b[1]).abs() < 1e-9 || (b[1] + b[3] - a[1]).abs() < 1e-9)
                && overlap(a[0], a[0] + a[2], b[0], b[0] + b[2]);
            side || stack
        };
        let mut nested = split(
            Orient::Col,
            0.5,
            split(
                Orient::Row,
                0.5,
                leaf(&[View::Graph], View::Graph),
                split(
                    Orient::Row,
                    0.5,
                    leaf(&[View::Files], View::Files),
                    leaf(&[View::Log], View::Log),
                ),
            ),
            split(
                Orient::Col,
                0.5,
                leaf(&[View::Insp], View::Insp),
                leaf(&[View::Table], View::Table),
            ),
        );
        number(&mut nested);
        let mut t = three();
        place(&mut t, View::Graph, 3, true);
        let trees = [classic(0.54), stacked(), t.clone(), nested.clone()];
        for n in &trees {
            let rs = boxes(n, 1600.0, 890.0);
            let ids: Vec<PaneId> = panes(n).iter().map(|p| p.id).collect();
            for (k, &id) in ids.iter().enumerate() {
                for dir in [Dir::Next, Dir::Prev] {
                    let nb = neighbour(n, id, dir).unwrap();
                    let j = ids.iter().position(|x| *x == nb).unwrap();
                    assert_ne!(nb, id);
                    assert!(touch(rs[k], rs[j]), "{} -> {} in {}", id, nb, shape(n));
                }
            }
        }
        // the nearest pane across a split running the same way; the first across one running
        // the other way
        assert_eq!(neighbour(&nested, 1, Dir::Next), Some(2), "the Graph reaches Files");
        assert_eq!(neighbour(&nested, 3, Dir::Prev), Some(2));
        assert_eq!(neighbour(&nested, 2, Dir::Next), Some(3));
        assert_eq!(neighbour(&nested, 4, Dir::Prev), Some(5), "the Inspector reaches the Table below");
        assert_eq!(neighbour(&nested, 5, Dir::Next), Some(4));
        assert_eq!(neighbour(&t, 1, Dir::Next), Some(2), "three's Graph reaches the Inspector");
        // the root pane has none
        let mut one = leaf(&View::ALL, View::Graph);
        number(&mut one);
        assert_eq!(neighbour(&one, 1, Dir::Next), None);
        // focus steps in reading order, round the ends
        assert_eq!(focus_step(&t, 3, 1).focus, Some(1));
        assert_eq!(focus_step(&t, 1, -1).focus, Some(3));
        assert!(!focus_step(&t, 1, 1).changed);
    }

    #[test]
    fn swap_exchanges_contents_in_place() {
        let mut n = classic(0.54);
        place(&mut n, View::Log, 1, true);
        let o = swap(&mut n, 1, Dir::Next);
        assert_eq!(o, Outcome { changed: true, focus: Some(2), toast: None });
        let ps = panes(&n);
        assert_eq!((ps[0].views.clone(), ps[0].shown), (vec![View::Insp], View::Insp));
        assert_eq!((ps[1].views.len(), ps[1].shown), (5, View::Log));
        assert_eq!(share(&n, &[]), Some(0.54), "the slots keep their sizes");
        swap(&mut n, 2, Dir::Prev);
        let mut back = classic(0.54);
        place(&mut back, View::Log, 1, true);
        assert_eq!(n, back);
        // the collapsed state travels with the contents
        let mut t = three();
        let o = swap(&mut t, 1, Dir::Next);
        assert_eq!(o.focus, Some(2));
        assert_eq!(pane_views(&t), vec![
            vec!["insp"],
            vec!["graph"],
            vec!["files", "log", "table", "list"]
        ]);
        assert_eq!(collapsed(&t), vec![false, true, false]);
        let mut one = leaf(&View::ALL, View::Graph);
        number(&mut one);
        let o = swap(&mut one, 1, Dir::Next);
        assert!(!o.changed && o.toast.is_some());
    }

    #[test]
    fn flip_toggles_the_parent_split() {
        let mut n = classic(0.54);
        assert_eq!(flip(&mut n, 1), Outcome { changed: true, focus: Some(1), toast: None });
        assert!(matches!(&n, Node::Split { orient: Orient::Col, f, .. } if *f == 0.54));
        assert_eq!(pane_views(&n), pane_views(&classic(0.54)), "the order is kept");
        flip(&mut n, 2);
        assert_eq!(n, classic(0.54));
        // three: the Graph's split turns side by side, the inner split is untouched
        let mut t = three();
        flip(&mut t, 1);
        assert!(matches!(&t, Node::Split { orient: Orient::Row, .. }));
        assert!(matches!(at(&t, &[1]), Some(Node::Split { orient: Orient::Row, .. })));
        flip(&mut t, 3);
        assert!(matches!(at(&t, &[1]), Some(Node::Split { orient: Orient::Col, .. })));
        let mut one = leaf(&View::ALL, View::Graph);
        number(&mut one);
        let before = one.clone();
        assert!(flip(&mut one, 1).toast.is_some());
        assert_eq!(one, before);
    }

    #[test]
    fn close_gives_views_to_the_neighbour() {
        let mut n = classic(0.54);
        place(&mut n, View::Table, 1, true);
        let o = close(&mut n, 1, Dir::Next);
        assert_eq!(o, Outcome { changed: true, focus: Some(2), toast: None });
        let Node::Pane(p) = &n else { panic!("one pane left: {}", shape(&n)) };
        assert_eq!(p.views.len(), 6);
        assert_eq!(p.shown, View::Table, "the neighbour shows the closed pane's view");
        assert!(!p.collapsed);
        // three: closing the Inspector gives it to Files' pane, which takes the width
        let mut t = three();
        assert_eq!(close(&mut t, 2, Dir::Next).focus, Some(3));
        assert_eq!(shape(&t), "[(1:graph)|(3:files,log,table,list,insp)]");
        assert_eq!(pane_of(&t, View::Insp).shown, View::Insp);
        assert_eq!(boxes(&t, 1600.0, 890.0)[1], [0.0, TITLE, 1600.0, 890.0 - TITLE]);
        // closing the collapsed Graph: the Inspector takes it, expanded, and the row takes the height
        let mut t = three();
        assert_eq!(close(&mut t, 1, Dir::Next).focus, Some(2));
        assert_eq!(shape(&t), "[(2:insp,graph)|(3:files,log,table,list)]");
        assert_eq!(pane_of(&t, View::Graph).shown, View::Graph);
        assert_eq!(collapsed(&t), vec![false, false]);
    }

    #[test]
    fn close_refuses_the_only_pane() {
        let mut one = leaf(&View::ALL, View::Graph);
        number(&mut one);
        let before = one.clone();
        let o = close(&mut one, 1, Dir::Next);
        assert!(!o.changed && o.focus.is_none() && o.toast.is_some());
        assert_eq!(one, before);
    }

    #[test]
    fn moving_the_last_view_closes_the_source() {
        // a view that isn't the last: the neighbour shows it, the source shows its next view
        let mut n = classic(0.54);
        let o = move_view(&mut n, 1, Dir::Next);
        assert_eq!(o, Outcome { changed: true, focus: Some(2), toast: None });
        assert_eq!(pane_views(&n), vec![vec!["files", "log", "table", "list"], vec!["insp", "graph"]]);
        assert_eq!((panes(&n)[0].shown, panes(&n)[1].shown), (View::Files, View::Graph));
        // the last view goes, and the source closes as close would
        let mut t = three();
        assert_eq!(move_view(&mut t, 2, Dir::Next).focus, Some(3));
        let mut c = three();
        close(&mut c, 2, Dir::Next);
        assert_eq!(t, c);
        // a collapsed neighbour expands to show it
        let mut t = three();
        move_view(&mut t, 2, Dir::Prev);
        move_view(&mut t, 3, Dir::Prev);
        assert_eq!(pane_of(&t, View::Files).shown, View::Files);
        assert!(panes(&t).iter().all(|p| !p.collapsed) || panes(&t).len() == 1);
    }

    #[test]
    fn moving_the_only_view_is_refused() {
        let mut one = leaf(&[View::Graph], View::Graph);
        number(&mut one);
        let before = one.clone();
        let o = move_view(&mut one, 1, Dir::Next);
        assert!(!o.changed && o.focus.is_none() && o.toast.is_some());
        assert_eq!(one, before);
        // the only pane holding many views refuses too
        let mut all = leaf(&View::ALL, View::Graph);
        number(&mut all);
        assert!(move_view(&mut all, 1, Dir::Prev).toast.is_some());
        assert_eq!(panes(&all)[0].views.len(), 6);
    }

    #[test]
    fn showing_the_inspector_keeps_focus() {
        let mut n = classic(0.54);
        let o = place(&mut n, View::Insp, 1, false);
        assert_eq!(o.focus, None, "the Inspector keys never move focus");
        // with no pane holding it, it joins the focused pane, which keeps focus
        let mut m = split(
            Orient::Row,
            0.5,
            leaf(&[View::Graph], View::Graph),
            leaf(&[View::Files], View::Files),
        );
        number(&mut m);
        let o = place(&mut m, View::Insp, 2, false);
        assert!(o.changed && o.focus.is_none());
        assert_eq!(pane_views(&m), vec![vec!["graph"], vec!["files", "insp"]]);
        assert_eq!(panes(&m)[1].shown, View::Insp);
    }

    // ---- docking a dragged view ----------------------------------------------------------------

    /// The roots the drop sweeps run in: the page's panes at both preset widths, and two small
    /// windows where the floors refuse most edges.
    const SIZES: [(f64, f64); 4] = [(1600.0, 918.0), (900.0, 918.0), (640.0, 300.0), (430.0, 250.0)];

    fn shut(views: &[View], shown: View) -> Node {
        Node::Pane(Pane {
            id: 0,
            views: views.to_vec(),
            shown,
            collapsed: true,
        })
    }

    /// Every split tree of `n` empty panes: each shape, each split side by side or stacked, at
    /// share 0.5 or 0.3.
    fn frames(n: usize) -> Vec<Node> {
        if n == 1 {
            return vec![leaf(&[], View::Graph)];
        }
        let mut out = Vec::new();
        for k in 1..n {
            for a in frames(k) {
                for b in frames(n - k) {
                    for orient in [Orient::Row, Orient::Col] {
                        for f in [0.5, 0.3] {
                            out.push(split(orient, f, a.clone(), b.clone()));
                        }
                    }
                }
            }
        }
        out
    }

    fn fill(n: &mut Node, f: &mut impl FnMut(&mut Pane)) {
        match n {
            Node::Pane(p) => f(p),
            Node::Split { kids, .. } => {
                for k in kids.iter_mut() {
                    fill(k, f);
                }
            }
        }
    }

    /// The accordion's rules: no split holds two collapsed panes, and the root isn't collapsed.
    fn folded_well(n: &Node) -> bool {
        fn inner(n: &Node) -> bool {
            match n {
                Node::Pane(_) => true,
                Node::Split { kids, .. } => {
                    let [a, b] = &**kids;
                    let shut = |k: &Node| matches!(k, Node::Pane(p) if p.collapsed);
                    !(shut(a) && shut(b)) && inner(a) && inner(b)
                }
            }
        }
        !matches!(n, Node::Pane(p) if p.collapsed) && inner(n)
    }

    /// The ways to deal the six views in order to `n` panes, one or two to a pane, each pane
    /// showing either of its views: bit `i` of the first number gives pane `i` a second view, of
    /// the second number shows it.
    fn deals(n: usize) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        for two in 0..1u32 << n {
            if n + two.count_ones() as usize > View::ALL.len() {
                continue;
            }
            for second in 0..1u32 << n {
                if second & !two == 0 {
                    out.push((two, second));
                }
            }
        }
        out
    }

    /// `frame` dealt `(two, second)`, with the panes in bit set `folded` collapsed. Ids go up in
    /// reading order with gaps, as they stand after a pane closes, so a new pane's id is held to
    /// the largest, not the count.
    fn dealt(frame: &Node, (two, second): (u32, u32), folded: u32) -> Node {
        let mut t = frame.clone();
        let (mut i, mut next) = (0u32, 0usize);
        fill(&mut t, &mut |p| {
            let k = 1 + (two >> i & 1) as usize;
            p.views = View::ALL[next..next + k].to_vec();
            p.shown = p.views[(second >> i & 1) as usize];
            p.collapsed = folded >> i & 1 == 1;
            p.id = 2 * i + 1;
            next += k;
            i += 1;
        });
        t
    }

    /// Each frame of one to four panes, collapsed every way that keeps the accordion's rules,
    /// with `pick` choosing its dealings from [`deals`] (given the count of geometries so far).
    fn arrangements(pick: impl Fn(usize, &[(u32, u32)], usize) -> Vec<(u32, u32)>) -> Vec<Node> {
        let mut out = Vec::new();
        let mut k = 0;
        for n in 1..=4usize {
            let ds = deals(n);
            for frame in frames(n) {
                for folded in 0..1u32 << n {
                    if !folded_well(&dealt(&frame, (0, 0), folded)) {
                        continue;
                    }
                    for d in pick(n, &ds, k) {
                        out.push(dealt(&frame, d, folded));
                    }
                    k += 1;
                }
            }
        }
        out
    }

    /// Every geometry: each frame of one to four panes, collapsed every way the rules allow, each
    /// pane holding one view.
    fn geometries() -> &'static [Node] {
        static ALL: std::sync::OnceLock<Vec<Node>> = std::sync::OnceLock::new();
        ALL.get_or_init(|| arrangements(|_, _, _| vec![(0, 0)]))
    }

    /// Every arrangement the sweeps start from: each geometry of up to three panes with every
    /// dealing; each geometry of four panes with one dealing, taken in turn, so each dealing meets
    /// many four-pane geometries.
    fn domain() -> &'static [Node] {
        static ALL: std::sync::OnceLock<Vec<Node>> = std::sync::OnceLock::new();
        ALL.get_or_init(|| {
            arrangements(|n, ds, k| if n < 4 { ds.to_vec() } else { vec![ds[k % ds.len()]] })
        })
    }

    /// Every drop the sweeps grade: each arrangement in [`domain`] × each view × each pane × each
    /// zone × each size. Returns the count.
    fn sweep(mut f: impl FnMut(&Node, View, PaneId, Zone, f64, f64)) -> usize {
        let mut n = 0;
        for t in domain() {
            let ids: Vec<PaneId> = panes(t).iter().map(|p| p.id).collect();
            for v in View::ALL {
                for &id in &ids {
                    for z in Zone::ALL {
                        for (w, h) in SIZES {
                            f(t, v, id, z, w, h);
                            n += 1;
                        }
                    }
                }
            }
        }
        n
    }

    /// Pane `id`'s rect `[x, y, w, h]` in a `w` × `h` root, found by walking down to it.
    fn rect_of(n: &Node, id: PaneId, x: f64, y: f64, w: f64, h: f64) -> Option<[f64; 4]> {
        match n {
            Node::Pane(p) => (p.id == id).then_some([x, y, w, h]),
            Node::Split { orient, f, kids } => {
                let shut = |k: &Node| matches!(k, Node::Pane(p) if p.collapsed);
                let row = *orient == Orient::Row;
                let e = if row { w } else { h };
                let a = if shut(&kids[0]) {
                    TITLE
                } else if shut(&kids[1]) {
                    e - TITLE
                } else {
                    f * e
                };
                if row {
                    rect_of(&kids[0], id, x, y, a, h).or_else(|| rect_of(&kids[1], id, x + a, y, w - a, h))
                } else {
                    rect_of(&kids[0], id, x, y, w, a).or_else(|| rect_of(&kids[1], id, x, y + a, w, h - a))
                }
            }
        }
    }

    /// The arrangement's rules after a drop: each pane holds a view and shows one it holds, every
    /// view held before is held once and no other is, and the accordion's rules hold.
    fn lawful(n: &Node, before: &Node) -> bool {
        let names = |n: &Node| {
            let mut vs: Vec<&str> = panes(n)
                .iter()
                .flat_map(|p| p.views.iter().map(|v| v.name()))
                .collect();
            vs.sort_unstable();
            vs
        };
        let vs = names(n);
        let mut once = vs.clone();
        once.dedup();
        folded_well(n)
            && once.len() == vs.len()
            && vs == names(before)
            && panes(n)
                .iter()
                .all(|p| !p.views.is_empty() && p.views.contains(&p.shown))
    }

    /// The drop as the goal words it, read apart from `docked`: the tree is rebuilt pane by pane,
    /// the source losing the view and the target joining it or splitting round it, and a pane left
    /// empty folds into its parent; then the rules and the floors are read on the result.
    fn written(root: &Node, v: View, target: PaneId, zone: Zone, w: f64, h: f64) -> Option<(Node, PaneId)> {
        fn redo(n: &Node, v: View, source: PaneId, target: PaneId, zone: Zone, made: &Node) -> Option<Node> {
            match n {
                Node::Pane(p) => {
                    let mut q = p.clone();
                    if q.id == source {
                        let i = q.views.iter().position(|x| *x == v)?;
                        q.views.remove(i);
                        if q.shown == v && !q.views.is_empty() {
                            q.shown = q.views[i % q.views.len()];
                        }
                    }
                    if q.id == target {
                        let (first, second) = (made.clone(), Node::Pane(q.clone()));
                        match zone {
                            Zone::Centre => {
                                q.views.push(v);
                                q.shown = v;
                                q.collapsed = false;
                            }
                            Zone::Left => return Some(split(Orient::Row, 0.5, first, second)),
                            Zone::Right => return Some(split(Orient::Row, 0.5, second, first)),
                            Zone::Top => return Some(split(Orient::Col, 0.5, first, second)),
                            Zone::Bottom => return Some(split(Orient::Col, 0.5, second, first)),
                        }
                    }
                    (!q.views.is_empty()).then_some(Node::Pane(q))
                }
                Node::Split { orient, f, kids } => {
                    let a = redo(&kids[0], v, source, target, zone, made);
                    let b = redo(&kids[1], v, source, target, zone, made);
                    match (a, b) {
                        (Some(a), Some(b)) => Some(split(*orient, *f, a, b)),
                        (a, b) => a.or(b),
                    }
                }
            }
        }
        let all = panes(root);
        let from = all.iter().find(|p| p.views.contains(&v))?;
        let onto = all.iter().find(|p| p.id == target)?;
        let centre = zone == Zone::Centre;
        // no move onto its own pane's centre; a collapsed pane offers only its centre; a pane
        // holding only the dragged view has nothing to split from
        if from.id == onto.id && (centre || from.views.len() == 1) || !centre && onto.collapsed {
            return None;
        }
        let fresh = all.iter().map(|p| p.id).max()? + 1;
        let made = Node::Pane(Pane {
            id: fresh,
            views: vec![v],
            shown: v,
            collapsed: false,
        });
        let out = redo(root, v, from.id, target, zone, &made)?;
        if !lawful(&out, root) {
            return None;
        }
        let lands = if centre { target } else { fresh };
        if !centre {
            let across = matches!(zone, Zone::Left | Zone::Right);
            for id in [lands, target] {
                let r = rect_of(&out, id, 0.0, 0.0, w, h)?;
                if across && r[2] < MIN_W || !across && r[3] < MIN_H {
                    return None;
                }
            }
        }
        Some((out, lands))
    }

    /// The zone the words give a point: a collapsed pane and a title bar are the centre; else the
    /// edge the point is nearest by its share of the body, when that is under a quarter (ties to
    /// the edge named first of left, right, top, bottom), else the centre.
    fn zone_by_words(r: [f64; 4], collapsed: bool, x: f64, y: f64) -> Zone {
        let [px, py, pw, ph] = r;
        if collapsed || y < py + TITLE {
            return Zone::Centre;
        }
        let body = ph - TITLE;
        let shares = [
            (x - px) / pw,
            (px + pw - x) / pw,
            (y - py - TITLE) / body,
            (py + ph - y) / body,
        ];
        let least = shares.iter().copied().fold(f64::INFINITY, f64::min);
        if least >= 0.25 {
            return Zone::Centre;
        }
        let k = shares.iter().position(|s| *s == least).unwrap();
        [Zone::Left, Zone::Right, Zone::Top, Zone::Bottom][k]
    }

    /// Where the browser suite points in a pane: the body's centre, a tenth of the body in from
    /// each side on its middle lines, and 8 px from the title bar's right end; in a collapsed pane
    /// the same tenths of its bar and its middle. With the zone each point is in.
    fn aim_points(r: Rect, collapsed: bool) -> [(f64, f64, Zone); 6] {
        let (bx, by, bw, bh) = if collapsed {
            (r.x, r.y, r.w, r.h)
        } else {
            (r.x, r.y + TITLE, r.w, r.h - TITLE)
        };
        let (mx, my) = (bx + bw / 2.0, by + bh / 2.0);
        let z = |z: Zone| if collapsed { Zone::Centre } else { z };
        let title = if collapsed {
            (mx, my)
        } else {
            (r.x + r.w - 8.0, r.y + TITLE / 2.0)
        };
        [
            (mx, my, Zone::Centre),
            (bx + bw / 10.0, my, z(Zone::Left)),
            (bx + bw * 9.0 / 10.0, my, z(Zone::Right)),
            (mx, by + bh / 10.0, z(Zone::Top)),
            (mx, by + bh * 9.0 / 10.0, z(Zone::Bottom)),
            (title.0, title.1, Zone::Centre),
        ]
    }

    /// A collapsed pane takes the title bar's extent along its split.
    fn strips_are_title_bars(n: &Node, rs: &[(PaneId, Rect)]) {
        if let Node::Split { orient, kids, .. } = n {
            for k in kids.iter() {
                if let Node::Pane(p) = k
                    && p.collapsed
                {
                    let r = rs.iter().find(|(id, _)| *id == p.id).unwrap().1;
                    assert_eq!(if *orient == Orient::Row { r.w } else { r.h }, TITLE, "{}", shape(n));
                }
                strips_are_title_bars(k, rs);
            }
        }
    }

    #[test]
    fn layout_tiles_the_root() {
        let (mut n, mut sound) = (0, 0);
        for t in geometries() {
            for (w, h) in SIZES {
                let rs = layout(t, w, h);
                let ids: Vec<PaneId> = panes(t).iter().map(|p| p.id).collect();
                assert_eq!(rs.iter().map(|(id, _)| *id).collect::<Vec<_>>(), ids, "reading order");
                let area: f64 = rs.iter().map(|(_, r)| r.w * r.h).sum();
                assert!((area - w * h).abs() < 1e-6 * w * h, "{} at {w}x{h}: {area}", shape(t));
                strips_are_title_bars(t, &rs);
                n += 1;
                // a pane beside a collapsed one in a split under the title bar's extent has none
                // left; elsewhere the rects lie inside the root and don't overlap
                if rs.iter().any(|(_, r)| r.w <= 0.0 || r.h <= 0.0) {
                    continue;
                }
                sound += 1;
                for (i, (_, a)) in rs.iter().enumerate() {
                    let inside = a.x >= 0.0 && a.y >= 0.0 && a.x + a.w <= w + 1e-9 && a.y + a.h <= h + 1e-9;
                    assert!(inside, "{} at {w}x{h}: {a:?}", shape(t));
                    for (_, b) in &rs[i + 1..] {
                        let dx = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
                        let dy = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
                        assert!(dx <= 1e-9 || dy <= 1e-9, "{} at {w}x{h}: {a:?} and {b:?}", shape(t));
                    }
                }
            }
        }
        assert!(n >= 10_000 && sound * 10 >= n * 9, "{n} layouts, {sound} with every pane drawn");
        for (p, w, want) in preset_rects() {
            assert!(near(&boxes(&p.tree(0.54), w, 890.0), &want), "{p:?} at {w}");
        }
    }

    #[test]
    fn aim_finds_the_pane_and_zone_under_a_point() {
        // every geometry of up to three panes at each size, and of four in the smallest root
        let shapes = geometries();
        let mut n = 0usize;
        for t in shapes {
            let sizes = if panes(t).len() < 4 { &SIZES[..] } else { &SIZES[3..] };
            for &(w, h) in sizes {
                let rs = layout(t, w, h);
                // a pane squeezed to nothing beside a collapsed one tiles nothing
                if rs.iter().any(|(_, r)| r.w <= 0.0 || r.h <= 0.0) {
                    continue;
                }
                let mut y = 0.0;
                while y < h {
                    let mut x = 0.0;
                    while x < w {
                        let (id, z) = aim(t, w, h, x, y).unwrap_or_else(|| panic!("({x}, {y}) in {}", shape(t)));
                        let r = rs.iter().find(|(p, _)| *p == id).unwrap().1;
                        assert!(r.holds(x, y), "({x}, {y}) in {} found {id} at {r:?}", shape(t));
                        let collapsed = find(t, id).unwrap().collapsed;
                        assert_eq!(z, zone_by_words([r.x, r.y, r.w, r.h], collapsed, x, y), "({x}, {y}) in {r:?}");
                        n += 1;
                        x += 7.0;
                    }
                    y += 7.0;
                }
                for (x, y) in [(-1.0, 1.0), (w, 1.0), (1.0, -1.0), (1.0, h), (w + 3.0, h + 3.0)] {
                    assert_eq!(aim(t, w, h, x, y), None, "({x}, {y}) is outside");
                }
            }
        }
        assert!(shapes.len() > 3000 && n >= 100_000, "{} geometries, {n} points", shapes.len());
    }

    #[test]
    fn dock_matches_the_hand_worked_cases() {
        use View::{Files, Graph, Insp, List, Log, Table};
        use Zone::{Bottom, Centre, Left, Right, Top};
        const W: f64 = 1600.0;
        const H: f64 = 890.0;
        let five = [Graph, Files, Log, Table, List];
        let same = |a: &Node, b: &Node| to_json(a) == to_json(b);
        let id_at = |t: &Node, i: usize| panes(t)[i].id;
        let index = |t: &Node, id: PaneId| panes(t).iter().position(|p| p.id == id).unwrap();
        // to a millionth of a px
        let rect = |r: Option<Rect>| r.map(|r| [r.x, r.y, r.w, r.h].map(|u| (u * 1e6).round() / 1e6));
        let cl = classic(0.54);

        // the last view onto a sibling's edge: its pane closes, and the grown target splits in half
        let (t, id) = docked(&cl, Insp, 1, Left, W, H).unwrap();
        assert!(same(&t, &split(Orient::Row, 0.5, leaf(&[Insp], Insp), leaf(&five, Graph))), "{}", shape(&t));
        assert_eq!(index(&t, id), 0);
        assert_eq!(rect(preview(&cl, Insp, 1, Left, W, H)), Some([0.0, 0.0, 800.0, H]));
        // the last view onto a sibling's centre joins it, and the target takes the root
        let (t, id) = docked(&cl, Insp, 1, Centre, W, H).unwrap();
        assert!(same(&t, &leaf(&[Graph, Files, Log, Table, List, Insp], Insp)));
        assert_eq!(id, 1);
        assert_eq!(rect(preview(&cl, Insp, 1, Centre, W, H)), Some([0.0, 0.0, W, H]));
        // a tab that isn't shown leaves its pane showing what it showed
        let (t, id) = docked(&cl, Log, 2, Centre, W, H).unwrap();
        let want = split(Orient::Row, 0.54, leaf(&[Graph, Files, Table, List], Graph), leaf(&[Insp, Log], Log));
        assert!(same(&t, &want), "{}", shape(&t));
        assert_eq!(index(&t, id), 1);
        assert_eq!(rect(preview(&cl, Log, 2, Centre, W, H)), Some([864.0, 0.0, 736.0, H]));
        // its own pane's edge splits it when it holds other views; the pane shows the next view
        let (t, id) = docked(&cl, Graph, 1, Right, W, H).unwrap();
        let want = split(
            Orient::Row,
            0.54,
            split(Orient::Row, 0.5, leaf(&[Files, Log, Table, List], Files), leaf(&[Graph], Graph)),
            leaf(&[Insp], Insp),
        );
        assert!(same(&t, &want), "{}", shape(&t));
        assert_eq!(index(&t, id), 1);
        assert_eq!(rect(preview(&cl, Graph, 1, Right, W, H)), Some([432.0, 0.0, 432.0, H]));
        // no zone of its own pane when that holds only the dragged view; never its own centre
        assert!(Zone::ALL.iter().all(|z| docked(&cl, Insp, 2, *z, W, H).is_none()));
        assert!(docked(&cl, Graph, 1, Centre, W, H).is_none());
        // the last tab, shown, leaves: the first shows
        let mut t = split(Orient::Row, 0.5, leaf(&[Graph, Files, List], List), leaf(&[Insp], Insp));
        number(&mut t);
        let (r, _) = docked(&t, List, 2, Centre, W, H).unwrap();
        assert_eq!(panes(&r)[0].shown, Graph);

        // three's Graph bar: no edges, and a join expands it at its 40% share
        let th = three();
        assert!([Left, Right, Top, Bottom].iter().all(|z| docked(&th, Files, 1, *z, W, H).is_none()));
        let (r, _) = docked(&th, Files, 1, Centre, W, H).unwrap();
        assert!(same(at(&r, &[0]).unwrap(), &leaf(&[Graph, Files], Files)), "{}", shape(&r));
        assert!(same(at(&r, &[1, 1]).unwrap(), &leaf(&[Log, Table, List], Log)), "{}", shape(&r));
        assert_eq!(rect(preview(&th, Files, 1, Centre, W, H)), Some([0.0, 0.0, W, 356.0]));
        // the collapsed bar's only view onto Files' top: the bar closes, the new pane is expanded
        let (r, _) = docked(&th, Graph, 3, Top, W, H).unwrap();
        let want = split(
            Orient::Row,
            0.5,
            leaf(&[Insp], Insp),
            split(Orient::Col, 0.5, leaf(&[Graph], Graph), leaf(&[Files, Log, Table, List], Files)),
        );
        assert!(same(&r, &want), "{}", shape(&r));
        assert_eq!(rect(preview(&th, Graph, 3, Top, W, H)), Some([800.0, 0.0, 800.0, 445.0]));

        // the floors at 900 wide: classic's main pane (486) splits into 243 and 243; a 243 px pane
        // is too narrow to split side by side and tall enough to stack
        assert_eq!(rect(preview(&cl, List, 1, Right, 900.0, H)), Some([243.0, 0.0, 243.0, H]));
        let (a, _) = docked(&cl, List, 1, Right, 900.0, H).unwrap();
        let mid = id_at(&a, 1);
        assert!([Left, Right].iter().all(|z| docked(&a, Log, mid, *z, 900.0, H).is_none()));
        assert!([Top, Bottom].iter().all(|z| docked(&a, Log, mid, *z, 900.0, H).is_some()));
        // stacked at 890 high: 445, then 222.5, then 111.25, under 120
        let st = stacked();
        assert_eq!(rect(preview(&st, List, 1, Bottom, W, H)), Some([0.0, 222.5, W, 222.5]));
        let (b, _) = docked(&st, List, 1, Bottom, W, H).unwrap();
        assert!(docked(&b, Log, id_at(&b, 1), Top, W, H).is_none());
        // the boundary: a pane exactly 400 wide splits into two of 200
        let mut t = split(Orient::Row, 0.25, leaf(&[Graph, Files], Graph), leaf(&[Insp], Insp));
        number(&mut t);
        assert!(docked(&t, Insp, 1, Right, W, H).is_some());
        assert!(docked(&t, Files, 1, Right, 1599.0, H).is_none());

        // a source closing so that two collapsed panes share a split is refused
        let mut t = split(
            Orient::Row,
            0.5,
            split(
                Orient::Col,
                0.5,
                shut(&[Graph], Graph),
                split(Orient::Row, 0.5, leaf(&[Files], Files), shut(&[Log], Log)),
            ),
            leaf(&[Table, List, Insp], Table),
        );
        number(&mut t);
        assert!(folded_well(&t), "the fixture keeps the rules");
        assert!(Zone::ALL.iter().all(|z| docked(&t, Files, id_at(&t, 3), *z, W, H).is_none()));
        assert!(docked(&t, Table, id_at(&t, 1), Centre, W, H).is_some());
        // stacked: the Inspector on top of the main pane takes the top half of the root
        assert_eq!(rect(preview(&st, Insp, 1, Top, W, H)), Some([0.0, 0.0, W, 445.0]));
        // a centre drop on the neighbour is what the move key does
        let (t, _) = docked(&cl, Graph, 2, Centre, W, H).unwrap();
        let want = split(Orient::Row, 0.54, leaf(&[Files, Log, Table, List], Files), leaf(&[Insp, Graph], Graph));
        assert!(same(&t, &want), "{}", shape(&t));

        // every valid drop from a preset keeps the rules and holds the six views once
        let mut n = 0;
        for p in Preset::ALL {
            let t = p.tree(0.54);
            for (w, h) in [(1600.0, 890.0), (900.0, 890.0), (1600.0, 918.0), (900.0, 918.0)] {
                for v in View::ALL {
                    for q in panes(&t) {
                        for z in Zone::ALL {
                            let Some((r, id)) = docked(&t, v, q.id, z, w, h) else { continue };
                            n += 1;
                            assert!(lawful(&r, &t) && panes(&r).iter().map(|p| p.views.len()).sum::<usize>() == 6);
                            let at = layout(&r, w, h).into_iter().find(|(p, _)| *p == id).map(|(_, r)| r);
                            assert_eq!(preview(&t, v, q.id, z, w, h), at);
                        }
                    }
                }
            }
        }
        assert!(n > 100, "{n} drops");
        // each point the suite aims at is in its pane and zone, the title bar's in the centre
        for p in Preset::ALL {
            let t = p.tree(0.54);
            for w in [1600.0, 900.0] {
                for (id, r) in layout(&t, w, H) {
                    let collapsed = find(&t, id).unwrap().collapsed;
                    for (x, y, z) in aim_points(r, collapsed) {
                        assert_eq!(aim(&t, w, H, x, y), Some((id, z)), "{p:?} at {w}, pane {id}, ({x}, {y})");
                    }
                }
            }
        }
        assert_eq!(zone_at(Rect { x: 0.0, y: 0.0, w: 864.0, h: H }, false, 5.0, 10.0), Centre);
        assert_eq!(zone_at(Rect { x: 864.0, y: 0.0, w: 736.0, h: H }, false, 1000.0, 860.0), Bottom);
        assert_eq!(zone_at(Rect { x: 0.0, y: 0.0, w: 864.0, h: H }, false, 50.0, 400.0), Left);
        assert_eq!(aim(&cl, W, H, 1600.0, 10.0), None);
        assert_eq!(aim(&cl, W, H, 10.0, -1.0), None);
    }

    #[test]
    fn dock_sweep_matches_the_written_rules() {
        // valid and refused, at the centre and on an edge
        let mut seen = [[0usize; 2]; 2];
        let n = sweep(|t, v, id, z, w, h| {
            let got = docked(t, v, id, z, w, h);
            assert_eq!(got, written(t, v, id, z, w, h), "{v:?} onto {id} {z:?} at {w}x{h} in {}", shape(t));
            seen[(z == Zone::Centre) as usize][got.is_some() as usize] += 1;
        });
        assert!(n >= 100_000, "{n} drops");
        assert!(seen.iter().flatten().all(|k| *k > 1000), "{seen:?}");
    }

    #[test]
    fn refused_dock_changes_nothing() {
        let mut refused = 0;
        let n = sweep(|t, v, id, z, w, h| {
            let mut after = t.clone();
            let o = dock(&mut after, v, id, z, w, h);
            match docked(t, v, id, z, w, h) {
                None => {
                    assert_eq!(o, Outcome::default(), "{v:?} onto {id} {z:?} in {}", shape(t));
                    assert_eq!(&after, t);
                    refused += 1;
                }
                Some((r, lands)) => {
                    assert_eq!(o, Outcome { changed: true, focus: Some(lands), toast: None });
                    assert_eq!(after, r);
                }
            }
        });
        assert!(n >= 100_000 && refused * 10 > n, "{n} drops, {refused} refused");
    }

    #[test]
    fn dock_preview_is_the_rect_the_view_lands_in() {
        let mut valid = 0;
        let n = sweep(|t, v, id, z, w, h| {
            let p = preview(t, v, id, z, w, h);
            let Some((r, lands)) = docked(t, v, id, z, w, h) else {
                assert_eq!(p, None, "{v:?} onto {id} {z:?} in {}", shape(t));
                return;
            };
            valid += 1;
            assert_eq!(holder(&r, v), Some(lands));
            let rs = layout(&r, w, h);
            let at = |id: PaneId| rs.iter().find(|(p, _)| *p == id).map(|(_, r)| *r);
            assert_eq!(p, at(lands), "{v:?} onto {id} {z:?} at {w}x{h} in {}", shape(t));
            // an edge halves the target's extent in the result between it and the new pane
            if z != Zone::Centre {
                let (a, b) = (at(lands).unwrap(), at(id).unwrap());
                let across = matches!(z, Zone::Left | Zone::Right);
                let (x, y) = if across { (a.w, b.w) } else { (a.h, b.h) };
                assert!((x - y).abs() < 1e-9, "{v:?} onto {id} {z:?} at {w}x{h} in {}", shape(t));
            }
        });
        assert!(n >= 100_000 && valid * 10 > n, "{n} drops, {valid} valid");
    }

    #[test]
    fn centre_dock_on_the_neighbour_equals_the_move_keys() {
        let mut n = 0;
        for t in domain() {
            for p in panes(t) {
                for dir in [Dir::Next, Dir::Prev] {
                    let Some(nb) = neighbour(t, p.id, dir) else { continue };
                    for (w, h) in SIZES {
                        let (mut a, mut b) = (t.clone(), t.clone());
                        let got = dock(&mut a, p.shown, nb, Zone::Centre, w, h);
                        let want = move_view(&mut b, p.id, dir);
                        assert_eq!(got, want, "pane {} {dir:?} in {}", p.id, shape(t));
                        assert_eq!(a, b, "pane {} {dir:?} in {}", p.id, shape(t));
                        n += 1;
                    }
                }
            }
        }
        assert!(n >= 100_000, "{n} drops");
    }

    #[test]
    fn docked_trees_survive_save_and_load() {
        let mut valid = 0;
        let n = sweep(|t, v, id, z, w, h| {
            let Some((r, _)) = docked(t, v, id, z, w, h) else { return };
            valid += 1;
            let back = from_json(&to_json(&r)).and_then(sanitise);
            let mut want = r.clone();
            number(&mut want);
            assert_eq!(back, Some(want), "{v:?} onto {id} {z:?} in {}", shape(t));
        });
        assert!(n >= 100_000 && valid * 10 > n, "{n} drops, {valid} valid");
    }
}
