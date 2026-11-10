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

    /// Each pane's rect `[x, y, w, h]` in a `w` × `h` root, as the drawing tiles it: a split's
    /// children take `f` and `1 − f` of its extent, or the title bar and the rest beside a
    /// collapsed pane.
    fn rects(n: &Node, x: f64, y: f64, w: f64, h: f64, out: &mut Vec<[f64; 4]>) {
        let folded = |k: &Node| matches!(k, Node::Pane(p) if p.collapsed);
        match n {
            Node::Pane(_) => out.push([x, y, w, h]),
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
                    rects(&kids[0], x, y, lead, h, out);
                    rects(&kids[1], x + lead, y, w - lead, h, out);
                } else {
                    rects(&kids[0], x, y, w, lead, out);
                    rects(&kids[1], x, y + lead, w, h - lead, out);
                }
            }
        }
    }

    fn layout(n: &Node, w: f64, h: f64) -> Vec<[f64; 4]> {
        let mut out = Vec::new();
        rects(n, 0.0, 0.0, w, h, &mut out);
        out
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

    #[test]
    fn preset_sizes_match_the_plan() {
        let close = |a: &[[f64; 4]], b: &[[f64; 4]]| {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(x, y)| x.iter().zip(y).all(|(u, v)| (u - v).abs() < 1e-6))
        };
        let h = 890.0;
        let cases: [(Preset, f64, Vec<[f64; 4]>); 8] = [
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
        ];
        for (p, w, want) in cases {
            let got = layout(&p.tree(0.54), w, h);
            assert!(close(&got, &want), "{p:?} at {w}: {got:?}");
        }
        // classic by key takes a legacy share; three's Graph expanded takes 40% of the height
        assert!(close(&layout(&Preset::Classic.tree(classic_share(30.0)), 1600.0, h)[..1], &[[0.0, 0.0, 1120.0, h]]));
        let mut t = three();
        place(&mut t, View::Graph, 3, true);
        assert!(close(&layout(&t, 1600.0, 1000.0)[..1], &[[0.0, 0.0, 1600.0, 400.0]]));
    }

    #[test]
    fn collapse_keeps_the_share_and_expand_restores_it() {
        let mut n = classic(0.54);
        let o = collapse_toggle(&mut n, 1);
        assert!(o.changed);
        assert_eq!(collapsed(&n), vec![true, false]);
        assert_eq!(share(&n, &[]), Some(0.54), "the share is kept");
        assert!((layout(&n, 1600.0, 890.0)[1][2] - (1600.0 - TITLE)).abs() < 1e-9);
        collapse_toggle(&mut n, 1);
        assert_eq!(n, classic(0.54), "expand gives back what collapse changed");
        // in a stack: the title bar, then back
        let mut s = stacked();
        collapse_toggle(&mut s, 2);
        assert_eq!(layout(&s, 900.0, 890.0)[1], [0.0, 890.0 - TITLE, 900.0, TITLE]);
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
            let rs = layout(n, 1600.0, 890.0);
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
        assert_eq!(layout(&t, 1600.0, 890.0)[1], [0.0, TITLE, 1600.0, 890.0 - TITLE]);
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
}
