//! The arrangement drawn: its split tree as nested flex boxes in `#panes`, each pane a title bar of
//! tabs over a body holding its view roots, each split's boundary an overlay sash to drag. The view
//! roots are moved, never rebuilt (their listeners, the canvas and the observers are bound to
//! them); a view no pane holds waits in `#parking`.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use web_sys::{Element, PointerEvent};

use crate::arrange::{self, Node, Orient, PaneId, View};
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
    let current = arrange::which(&root, arrange::classic_share(cfg().insp));
    for b in dom::qa("#presets [data-preset]") {
        let on = current.is_some_and(|p| dom::data(&b, "preset").as_deref() == Some(p.name()));
        dom::toggle(&b, "on", on);
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

/// Draw the arrangement, focus its default pane, and wire the pointer: a press in a pane focuses
/// it, a press on a sash drags it (the pointer captured, the floors applied, saved on release).
/// Each pane body is watched for crossing the narrow floor.
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
}
