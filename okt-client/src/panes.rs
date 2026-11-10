//! The arrangement drawn: its split tree as nested flex boxes in `#panes`, each pane a title bar of
//! tabs over a body holding its view roots, each split's boundary an overlay sash to drag. The view
//! roots are moved, never rebuilt (their listeners, the canvas and the observers are bound to
//! them); a view no pane holds waits in `#parking`.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use web_sys::{Element, MouseEvent, PointerEvent};

use crate::arrange::{self, Node, Orient, PaneId, View, Zone};
use crate::state::{cfg, cfg_mut, s, s_mut, save_settings};
use crate::{dom, graph, nav};

#[wasm_bindgen]
extern "C" {
    /// An element, for `moveBefore`, which web-sys doesn't bind.
    #[wasm_bindgen(extends = Element)]
    type Host;
    /// `Element.moveBefore`: moves a node keeping its scroll and focus, firing no events. It
    /// throws where the browser lacks it.
    #[wasm_bindgen(method, catch, js_name = moveBefore)]
    fn move_before(this: &Host, node: &Element, child: Option<&web_sys::Node>)
    -> Result<(), JsValue>;
}

/// A split's sash, held by the pointer.
struct Drag {
    path: Vec<usize>,
    /// the split's leading edge and extent along its direction, in client px
    start: f64,
    extent: f64,
    /// where on the sash the pointer took it, from the boundary
    grab: f64,
    row: bool,
    moved: bool,
    /// where it was pressed, in client px
    at: (f64, f64),
    sash: Element,
}

thread_local! {
    /// the shape last drawn (`arrange::shape`)
    static SHAPE: RefCell<String> = const { RefCell::new(String::new()) };
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
    /// the views on screen when last looked at, so a pane growing back over the narrow floor
    /// renders the view it shows
    static ON: RefCell<Vec<View>> = const { RefCell::new(Vec::new()) };
    /// watches every pane body, for a pane crossing the narrow floor without an arrangement change
    static BODIES: RefCell<Option<web_sys::ResizeObserver>> = const { RefCell::new(None) };
    /// the view the keys went to off screen when last drawn, so it is drawn once as it gets there
    /// and not again on every redraw (a sash drag redraws on each move)
    static KEY_OFF: Cell<Option<View>> = const { Cell::new(None) };
    /// a tab held by the pointer, to drag its view onto a pane
    static TAB: RefCell<Tab> = const { RefCell::new(Tab::Idle) };
    /// the one-shot listener that swallows the click ending a drag, while it is armed
    static SWALLOW: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };    /// counts drags begun, so a frame watching an earlier drag's capture stops
    static DRAGS: Cell<u32> = const { Cell::new(0) };
}

/// How far a pressed tab moves, in px, before its view is dragged; under it the press stays a click.
const THRESHOLD: f64 = 4.0;

/// A pressed tab, from the press to the release. Every handler takes it out of `TAB`, acts, and
/// stores the next one, so nothing holds it across a call that can re-enter.
enum Tab {
    Idle,
    /// pressed, not yet moved past the threshold
    Pending {
        pid: i32,
        view: View,
        at: (f64, f64),
        tab: Element,
    },
    /// dragging the view: `tree` is the arrangement as the drag began, which every move aims at
    Dragging {
        pid: i32,
        view: View,
        tab: Element,
        tree: Node,
    },
    /// cancelled with the button still down: the rest of the press does nothing
    Cancelled,
}

fn root_id(v: View) -> &'static str {
    match v {
        View::Graph => "v-graph",
        View::Files => "v-files",
        View::Log => "v-log",
        View::Table => "v-table",
        View::List => "v-list",
        View::Insp => "insp",
    }
}

/// A tab's key hint and label.
fn label(v: View) -> (&'static str, &'static str) {
    match v {
        View::Graph => ("G", "GRAPH"),
        View::Files => ("F", "FILES"),
        View::Log => ("L", "LOG"),
        View::Table => ("T", "TABLE"),
        View::List => ("/", "RESULTS"),
        View::Insp => ("O", "INSPECTOR"),
    }
}

/// Draw the arrangement in the settings. A change of shape builds the new skeleton beside the old
/// one, moves the view roots into it and drops the old; anything else (shares, tabs, focus) is set
/// in place. Nothing is borrowed while roots move: the fallback `appendChild` fires `blur`
/// synchronously. The Graph is refitted last, since a pane can move without resizing.
pub fn apply() {
    let root = cfg().arr.clone();
    let focus = s().focus;
    let shape = arrange::shape(&root);
    let host = dom::id("panes");
    let parking = dom::id("parking");
    if SHAPE.with(|x| *x.borrow() != shape) {
        let old = skeleton(&host);
        let el = build(&root, &mut Vec::new());
        let _ = host.insert_before(&el, Some(&parking));
        for p in arrange::panes(&root) {
            let Some(body) = pane_el(&el, p.id).and_then(|e| dom::q_in(&e, ":scope > .pbody"))
            else {
                continue;
            };
            for v in &p.views {
                move_into(&body, &dom::id(root_id(*v)));
            }
        }
        for v in View::ALL {
            if arrange::holder(&root, v).is_none() {
                let r = dom::id(root_id(v));
                dom::set_hidden(&r, true);
                move_into(&parking, &r);
            }
        }
        if let Some(o) = old {
            o.remove();
        }
        SHAPE.with(|x| *x.borrow_mut() = shape);
        BODIES.with(|b| {
            if let Some(ro) = b.borrow().as_ref() {
                ro.disconnect();
                for body in dom::qa_in(&el, ".pbody") {
                    ro.observe(&body);
                }
            }
        });
    }
    if let Some(el) = skeleton(&host) {
        update(&el, &root, focus);
    }
    graph::fit_pane();
    let now = on_screen();
    ON.with(|o| *o.borrow_mut() = now);
    draw_key_view();
}

/// The view the keys go to is always drawn. Every change of focus or arrangement draws through
/// `apply`, whatever its path: a view placed by its key, its tab or the hash (back and forward, the
/// boot), a pane focused by a click, a pane operation or a preset. So it is decided here: when the
/// key view is off screen (its pane collapsed, its slot showing the Inspector, or under the narrow
/// floor), it is drawn as it gets there, once, and its keys step through what it drew as they do on
/// screen. A window shrinking its pane under the floor afterwards leaves what it drew in place, and
/// a commit arriving redraws the Log wherever it is (`live`).
fn draw_key_view() {
    let off = View::parse(key_view()).filter(|v| !visible(v.name()));
    let last = KEY_OFF.with(|k| k.replace(off));
    if let Some(v) = off.filter(|v| last != Some(*v)) {
        nav::show(v);
    }
}

fn on_screen() -> Vec<View> {
    View::ALL
        .into_iter()
        .filter(|v| visible(v.name()))
        .collect()
}

/// A pane body changed size: render each view that has come on screen since last looked at (a
/// window grown back over a pane's narrow floor). An arrangement change renders its own.
fn bodies_resized() {
    let now = on_screen();
    let came: Vec<View> = ON.with(|o| {
        let before = o.replace(now.clone());
        now.into_iter().filter(|v| !before.contains(v)).collect()
    });
    for v in came {
        nav::show(v);
    }
}

/// The drawn tree: `#panes`'s element other than `#parking`.
fn skeleton(host: &Element) -> Option<Element> {
    let kids = host.children();
    (0..kids.length())
        .filter_map(|i| kids.item(i))
        .find(|e| e.id() != "parking")
}

fn pane_el(scope: &Element, id: PaneId) -> Option<Element> {
    let sel = format!(".pane[data-pane=\"{id}\"]");
    if scope.matches(&sel).unwrap_or(false) {
        return Some(scope.clone());
    }
    dom::q_in(scope, &sel)
}

fn make(tag: &str, class: &str) -> Element {
    let e = dom::doc().create_element(tag).unwrap();
    e.set_class_name(class);
    e
}

fn build(n: &Node, path: &mut Vec<usize>) -> Element {
    match n {
        Node::Pane(p) => {
            let sec = make("section", "pane");
            let _ = sec.set_attribute("data-pane", &p.id.to_string());
            let title = make("header", "ptitle");
            for v in &p.views {
                let (k, name) = label(*v);
                let b = make("button", "ptab");
                let _ = b.set_attribute("data-view", v.name());
                let _ = b.set_attribute("title", name);
                dom::set_html(&b, &format!("<kbd>{k}</kbd><span>{name}</span>"));
                let _ = title.append_child(&b);
            }
            let _ = sec.append_child(&title);
            let _ = sec.append_child(&make("div", "pbody"));
            sec
        }
        Node::Split { kids, .. } => {
            // the first child, the sash, the second: the sash on its boundary comes before
            // anything inside the second child, so an outer split's sash is met first
            let el = make("div", "psplit");
            let _ = el.set_attribute(
                "data-path",
                &path.iter().map(|i| i.to_string()).collect::<String>(),
            );
            let sash = make("div", "sash");
            let _ = sash.set_attribute("title", "Drag to resize");
            for (i, k) in kids.iter().enumerate() {
                if i == 1 {
                    let _ = el.append_child(&sash);
                }
                path.push(i);
                let _ = el.append_child(&build(k, path));
                path.pop();
            }
            el
        }
    }
}

/// Move a view root into `parent`, keeping its scroll.
fn move_into(parent: &Element, el: &Element) {
    if el.parent_element().as_ref() == Some(parent) {
        return;
    }
    if parent.unchecked_ref::<Host>().move_before(el, None).is_ok() {
        return;
    }
    let tops: Vec<(Element, i32)> = std::iter::once(el.clone())
        .chain(dom::qa_in(el, "*"))
        .filter_map(|e| {
            let t = e.scroll_top();
            (t > 0).then_some((e, t))
        })
        .collect();
    let _ = parent.append_child(el);
    for (e, t) in tops {
        e.set_scroll_top(t);
    }
}

fn is_collapsed_leaf(n: &Node) -> bool {
    matches!(n, Node::Pane(p) if p.collapsed)
}

/// Set the drawn tree's attributes from the arrangement (the shapes match).
fn update(el: &Element, n: &Node, focus: PaneId) {
    match n {
        Node::Pane(p) => {
            let names: Vec<&str> = p.views.iter().map(|v| v.name()).collect();
            let _ = el.set_attribute("data-views", &names.join(" "));
            let _ = el.set_attribute("data-shown", p.shown.name());
            let _ = el.set_attribute("data-collapsed", if p.collapsed { "1" } else { "0" });
            let _ = el.set_attribute("data-focused", if p.id == focus { "1" } else { "0" });
            dom::toggle(el, "collapsed", p.collapsed);
            for t in dom::qa_in(el, ":scope > .ptitle > .ptab") {
                let on = dom::data(&t, "view").as_deref() == Some(p.shown.name());
                dom::toggle(&t, "on", on);
            }
            for v in &p.views {
                dom::set_hidden(&dom::id(root_id(*v)), *v != p.shown);
            }
        }
        Node::Split { orient, f, kids } => {
            let _ = el.set_attribute(
                "data-orient",
                if *orient == Orient::Row { "row" } else { "col" },
            );
            dom::set_style(el, "--f", &format!("{f}"));
            let ch = el.children();
            let (Some(a), Some(sash), Some(b)) = (ch.item(0), ch.item(1), ch.item(2)) else {
                return;
            };
            // each child's own flex, from this split's share: no inherited variable, so a nested
            // split never reads its parent's
            let (fa, fb) = if is_collapsed_leaf(&kids[0]) {
                ("0 0 24px".to_string(), "1 1 0px".to_string())
            } else if is_collapsed_leaf(&kids[1]) {
                ("1 1 0px".to_string(), "0 0 24px".to_string())
            } else {
                (
                    format!("{} 1 0px", f * 1000.0),
                    format!("{} 1 0px", (1.0 - f) * 1000.0),
                )
            };
            dom::set_style(&a, "flex", &fa);
            dom::set_style(&b, "flex", &fb);
            dom::set_hidden(&sash, kids.iter().any(is_collapsed_leaf));
            update(&a, &kids[0], focus);
            update(&b, &kids[1], focus);
        }
    }
}

/// The view is on screen: held, its pane's shown tab, and the pane expanded and at least the
/// narrow floor wide (under it, app.css hides the pane's body behind its title bar).
pub fn visible(v: &str) -> bool {
    let Some(r) = View::parse(v).and_then(|v| dom::try_id(root_id(v))) else {
        return false;
    };
    if dom::hidden(&r) {
        return false;
    }
    let Some(body) = r.parent_element().filter(|b| dom::has_class(b, "pbody")) else {
        return false;
    };
    body.parent_element()
        .map(|p| {
            dom::data(&p, "collapsed").as_deref() != Some("1")
                && p.get_bounding_client_rect().width() >= arrange::MIN_W
        })
        .unwrap_or(false)
}

/// The view the view keys act on: the focused pane's, or, while that is the Inspector (which has no
/// view keys of its own), the view that last held key focus, wherever it now lives. That is
/// `S.view`, recorded by view, not by pane: every change of focus or arrangement sets it to the
/// focused pane's view and leaves it while the Inspector is shown there (`nav::sync_view`). So a
/// click in the Inspector, and a swap, move or close of its pane after it, leave the keys where
/// they were. The borrow ends here, before the keys are dispatched.
pub fn key_view() -> &'static str {
    let v = focused_view();
    if v != View::Insp.name() {
        return v;
    }
    s().view
}

/// The view the focused pane shows.
pub fn focused_view() -> &'static str {
    dom::q("#panes .pane[data-focused=\"1\"]")
        .and_then(|p| dom::data(&p, "shown"))
        .and_then(|x| View::parse(&x))
        .map(View::name)
        .unwrap_or("")
}

/// Focus a pane: its title bar is marked, and the hash's view follows it.
pub fn focus(id: PaneId) {
    let cur = s().focus;
    if cur == id {
        return;
    }
    s_mut().focus = id;
    nav::sync_view();
    apply();
    nav::nav(false);
}

/// The pointer's client position, unrounded (web-sys reads `clientX` as an integer).
fn client(e: &PointerEvent) -> (f64, f64) {
    let get = |k: &str| {
        js_sys::Reflect::get(e, &JsValue::from_str(k))
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
    };
    (get("clientX"), get("clientY"))
}

fn on_down(e: PointerEvent) {
    let primary = e.button() == 0 && e.is_primary();
    if primary {
        // a gesture whose release never arrived (the window lost it) ends here, so this press is
        // never swallowed
        TAB.with(|t| t.replace(Tab::Idle));
        disarm_swallow(None);
    }
    let t = e.target();
    if let Some(sash) = dom::closest(&t, ".sash") {
        start_drag(&e, sash);
        return;
    }
    if let Some(id) = dom::closest(&t, ".pane")
        .and_then(|p| dom::data(&p, "pane"))
        .and_then(|x| x.parse::<PaneId>().ok())
    {
        focus(id);
    }
    // a press on a tab may become a drag of its view (the press has focused its pane first)
    let tab = dom::closest(&t, ".ptab[data-view]");
    let view = tab
        .as_ref()
        .and_then(|b| dom::data(b, "view"))
        .and_then(|v| View::parse(&v));
    if let (true, Some(tab), Some(view)) = (primary, tab, view) {
        let next = Tab::Pending {
            pid: e.pointer_id(),
            view,
            at: client(&e),
            tab,
        };
        TAB.with(|t| *t.borrow_mut() = next);
    }
}

fn start_drag(e: &PointerEvent, sash: Element) {
    e.prevent_default();
    let Some(split) = sash.parent_element() else {
        return;
    };
    let path: Vec<usize> = dom::data(&split, "path")
        .unwrap_or_default()
        .chars()
        .filter_map(|c| c.to_digit(10).map(|d| d as usize))
        .collect();
    let Some(f) = arrange::share(&cfg().arr, &path) else {
        return;
    };
    let row = dom::data(&split, "orient").as_deref() != Some("col");
    let r = split.get_bounding_client_rect();
    let (start, extent) = if row {
        (r.left(), r.width())
    } else {
        (r.top(), r.height())
    };
    let (x, y) = client(e);
    let at = if row { x } else { y };
    let _ = sash.set_pointer_capture(e.pointer_id());
    dom::add_class(&sash, "drag");
    let d = Drag {
        path,
        start,
        extent,
        grab: at - (start + f * extent),
        row,
        moved: false,
        at: (x, y),
        sash,
    };
    DRAG.with(|x| *x.borrow_mut() = Some(d));
}

fn on_move(e: PointerEvent) {
    let Some((path, want, extent)) = DRAG.with(|x| {
        x.borrow().as_ref().map(|d| {
            let (x, y) = client(&e);
            let at = if d.row { x } else { y };
            (d.path.clone(), (at - d.grab - d.start) / d.extent, d.extent)
        })
    }) else {
        return;
    };
    let took = {
        let mut c = cfg_mut();
        arrange::drag(&mut c.arr, &path, want, extent)
    };
    if took.is_none() {
        return;
    }
    DRAG.with(|x| {
        if let Some(d) = x.borrow_mut().as_mut() {
            d.moved = true;
        }
    });
    apply();
}

fn on_up(_: PointerEvent) {
    let Some(d) = DRAG.with(|x| x.borrow_mut().take()) else {
        return;
    };
    dom::remove_class(&d.sash, "drag");
    if d.moved {
        cfg_mut().arr_saved = true;
        save_settings();
    } else if let Some(id) = pane_at(d.at) {
        // a click on the sash is a click in the pane beneath it (the sash overlays both edges)
        focus(id);
    }
}

/// The pane under a point, beneath any sash.
fn pane_at((x, y): (f64, f64)) -> Option<PaneId> {
    dom::doc()
        .elements_from_point(x as f32, y as f32)
        .iter()
        .filter_map(|v| v.dyn_into::<Element>().ok())
        .find_map(|e| e.closest(".pane").ok().flatten())
        .and_then(|p| dom::data(&p, "pane"))
        .and_then(|x| x.parse().ok())
}

// ---- dragging a tab's view onto a pane -------------------------------------------------------

/// `#panes`'s rect in client px: its origin, width and height.
fn panes_rect() -> (f64, f64, f64, f64) {
    let r = dom::id("panes").get_bounding_client_rect();
    (r.left(), r.top(), r.width(), r.height())
}

/// The valid zone under a client point for a drag of `view` over `tree`, and the rect in client
/// px the view would take there.
fn aim_at(tree: &Node, view: View, x: f64, y: f64) -> Option<((PaneId, Zone), arrange::Rect)> {
    let (left, top, w, h) = panes_rect();
    let (target, zone) = arrange::aim(tree, w, h, x - left, y - top)?;
    let r = arrange::preview(tree, view, target, zone, w, h)?;
    Some((
        (target, zone),
        arrange::Rect {
            x: left + r.x,
            y: top + r.y,
            ..r
        },
    ))
}

/// Show the outline over `r`, or hide it.
fn outline(r: Option<arrange::Rect>) {
    let Some(el) = dom::try_id("dock") else {
        return;
    };
    match r {
        Some(r) => {
            dom::set_style(&el, "left", &format!("{}px", r.x));
            dom::set_style(&el, "top", &format!("{}px", r.y));
            dom::set_style(&el, "width", &format!("{}px", r.w));
            dom::set_style(&el, "height", &format!("{}px", r.h));
            dom::set_hidden(&el, false);
        }
        None => dom::set_hidden(&el, true),
    }
}

/// Swallow the click that ends a drag: the first click with a pointer behind it (`detail > 0`; a
/// key's click passes), in the capture phase, before anything sees it. It is gone by the next task,
/// since the click comes in the same task as the release.
fn arm_swallow() {
    disarm_swallow(None);
    let cb = Closure::<dyn FnMut(MouseEvent)>::new(|e: MouseEvent| {
        let detail = js_sys::Reflect::get(&e, &JsValue::from_str("detail"))
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        if detail > 0.0 {
            e.stop_immediate_propagation();
            e.prevent_default();
            disarm_swallow(None);
        }
    });
    let f: js_sys::Function = cb.into_js_value().unchecked_into();
    let _ = dom::win().add_event_listener_with_callback_and_bool("click", &f, true);
    SWALLOW.with(|s| *s.borrow_mut() = Some(f.clone()));
    dom::timeout(0, move || disarm_swallow(Some(f)));
}

/// Remove the click swallower: any, or only `this` one if it is still the one armed.
fn disarm_swallow(this: Option<js_sys::Function>) {
    let Some(f) = SWALLOW.with(|s| {
        let mut s = s.borrow_mut();
        let ours = match (&*s, &this) {
            (Some(cur), Some(f)) => JsValue::from(cur.clone()) == JsValue::from(f.clone()),
            (Some(_), None) => true,
            _ => false,
        };
        if ours { s.take() } else { None }
    }) else {
        return;
    };
    let _ = dom::win().remove_event_listener_with_callback_and_bool("click", &f, true);
}

/// A drag is under way.
pub fn dragging() -> bool {
    TAB.with(|t| matches!(*t.borrow(), Tab::Dragging { .. }))
}

/// Cancel a drag under way: the outline goes, the pointer is let go, and the rest of the press
/// does nothing. True when there was one.
pub fn cancel_drag() -> bool {
    let st = TAB.with(|t| t.replace(Tab::Idle));
    let Tab::Dragging { pid, tab, .. } = st else {
        TAB.with(|t| *t.borrow_mut() = st);
        return false;
    };
    outline(None);
    if tab.has_pointer_capture(pid) {
        let _ = tab.release_pointer_capture(pid);
    }
    TAB.with(|t| *t.borrow_mut() = Tab::Cancelled);
    true
}

/// Watch the dragged tab's pointer capture each frame, and cancel the drag when it is gone. Chromium
/// tells of a capture released or a tab removed (`lostpointercapture`) only with the next pointer
/// event, but `hasPointerCapture` reads false at once; so the drag ends with no move after it.
fn watch_capture(drag: u32) {
    dom::raf(move |_| {
        if DRAGS.get() != drag {
            return;
        }
        let lost = TAB.with(|t| match &*t.borrow() {
            Tab::Dragging { tab, pid, .. } => Some(!tab.has_pointer_capture(*pid)),
            _ => None,
        });
        match lost {
            Some(true) => {
                cancel_drag();
            }
            Some(false) => watch_capture(drag),
            None => {}
        }
    });
}

fn tab_move(e: PointerEvent) {
    let st = TAB.with(|t| t.replace(Tab::Idle));
    let next = match st {
        // the release happened where it couldn't be seen
        Tab::Pending { .. } | Tab::Cancelled if e.buttons() == 0 => Tab::Idle,
        Tab::Pending { pid, view, at, tab } if e.pointer_id() == pid => {
            let (x, y) = client(&e);
            if (x - at.0).hypot(y - at.1) < THRESHOLD {
                Tab::Pending { pid, view, at, tab }
            } else {
                let _ = tab.set_pointer_capture(pid);
                let drag = DRAGS.get().wrapping_add(1);
                DRAGS.set(drag);
                watch_capture(drag);
                let tree = cfg().arr.clone();
                outline(aim_at(&tree, view, x, y).map(|h| h.1));
                Tab::Dragging {
                    pid,
                    view,
                    tab,
                    tree,
                }
            }
        }
        Tab::Dragging {
            pid,
            view,
            tab,
            tree,
        } if e.pointer_id() == pid => {
            let (x, y) = client(&e);
            outline(aim_at(&tree, view, x, y).map(|h| h.1));
            Tab::Dragging {
                pid,
                view,
                tab,
                tree,
            }
        }
        other => other,
    };
    TAB.with(|t| *t.borrow_mut() = next);
}

fn tab_up(e: PointerEvent) {
    let st = TAB.with(|t| t.replace(Tab::Idle));
    match st {
        // under the threshold: the click that follows does what it always did
        Tab::Pending { pid, .. } if e.pointer_id() == pid => {}
        Tab::Dragging {
            pid, view, tree, ..
        } if e.pointer_id() == pid => {
            // the release point is aimed as a move is, so it lands where the outline last showed
            let (x, y) = client(&e);
            let hit = aim_at(&tree, view, x, y).map(|h| h.0);
            outline(None);
            arm_swallow();
            let (_, _, w, h) = panes_rect();
            // the arrangement can't have changed under the drag (every key cancels it); if it
            // has, the drop is dropped
            let same = cfg().arr == tree;
            if let (Some((target, zone)), true) = (hit, same) {
                nav::operate(move |n, _| arrange::dock(n, view, target, zone, w, h));
            }
        }
        Tab::Cancelled => arm_swallow(),
        other => TAB.with(|t| *t.borrow_mut() = other),
    }
}

/// The pointer is gone (`pointercancel`): cancel, and no click follows.
fn tab_cancel(e: PointerEvent) {
    let ours = TAB.with(|t| match &*t.borrow() {
        Tab::Pending { pid, .. } | Tab::Dragging { pid, .. } => *pid == e.pointer_id(),
        Tab::Cancelled => true,
        Tab::Idle => false,
    });
    if ours {
        cancel_drag();
        outline(None);
        TAB.with(|t| t.replace(Tab::Idle));
    }
}

/// The dragged tab lost the pointer: cancel (if the frame watch hasn't yet). A capture another
/// element lost is not ours; one lost to a removal arrives at the document.
fn tab_lost(e: PointerEvent) {
    let other = TAB.with(|t| match &*t.borrow() {
        Tab::Dragging { tab, pid, .. } => {
            *pid != e.pointer_id()
                || dom::target_el(&e).is_some_and(|el| el != *tab && el.is_connected())
        }
        _ => true,
    });
    if !other {
        cancel_drag();
    }
}

/// Draw the arrangement, focus its default pane, and wire the pointer: a press in a pane focuses
/// it, a press on a sash drags it (the pointer captured, the floors applied, saved on release), and
/// a tab dragged past the threshold carries its view to the pane and zone it is released over, an
/// outline showing where it will land. Each pane body is watched for crossing the narrow floor.
pub fn wire() {
    let f = arrange::default_focus(&cfg().arr);
    s_mut().focus = f;
    let cb = Closure::<dyn FnMut(js_sys::Array)>::new(move |_: js_sys::Array| bodies_resized());
    if let Ok(ro) = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref()) {
        BODIES.with(|b| *b.borrow_mut() = Some(ro));
    }
    cb.forget();
    apply();
    let host = dom::id("panes");
    dom::on(host.as_ref(), "pointerdown", on_down);
    dom::on(host.as_ref(), "pointermove", on_move);
    dom::on(host.as_ref(), "pointerup", on_up);
    dom::on(host.as_ref(), "lostpointercapture", on_up);
    // a captured tab's moves and release bubble to the document, and so does a capture lost to
    // the tab's removal
    let doc = dom::doc();
    dom::on(doc.as_ref(), "pointermove", tab_move);
    dom::on(doc.as_ref(), "pointerup", tab_up);
    dom::on(doc.as_ref(), "pointercancel", tab_cancel);
    dom::on(doc.as_ref(), "lostpointercapture", tab_lost);
}
