//! Small DOM helpers over web-sys: lookups, HTML and text, classes, listeners and timers.

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, Event, EventTarget, HtmlElement, HtmlInputElement, Window};

pub fn win() -> Window {
    web_sys::window().expect("window")
}

pub fn doc() -> Document {
    win().document().expect("document")
}

/// `document.getElementById(id)`, which must exist (the page template has it).
pub fn id(id: &str) -> Element {
    doc()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("#{id} missing"))
}

pub fn try_id(id: &str) -> Option<Element> {
    doc().get_element_by_id(id)
}

pub fn input(id_: &str) -> HtmlInputElement {
    id(id_).dyn_into().expect("input")
}

/// `$(sel)`.
pub fn q(sel: &str) -> Option<Element> {
    doc().query_selector(sel).ok().flatten()
}

/// `$(sel, el)`.
pub fn q_in(el: &Element, sel: &str) -> Option<Element> {
    el.query_selector(sel).ok().flatten()
}

/// `$$(sel)`.
pub fn qa(sel: &str) -> Vec<Element> {
    list(doc().query_selector_all(sel).ok())
}

/// `$$(sel, el)`.
pub fn qa_in(el: &Element, sel: &str) -> Vec<Element> {
    list(el.query_selector_all(sel).ok())
}

fn list(nl: Option<web_sys::NodeList>) -> Vec<Element> {
    let Some(nl) = nl else { return vec![] };
    (0..nl.length())
        .filter_map(|i| nl.item(i))
        .filter_map(|n| n.dyn_into::<Element>().ok())
        .collect()
}

pub fn set_html(el: &Element, html: &str) {
    el.set_inner_html(html);
}

pub fn set_text(el: &Element, t: &str) {
    el.set_text_content(Some(t));
}

pub fn text(el: &Element) -> String {
    el.text_content().unwrap_or_default()
}

pub fn toggle(el: &Element, class: &str, on: bool) {
    let _ = el.class_list().toggle_with_force(class, on);
}

pub fn has_class(el: &Element, class: &str) -> bool {
    el.class_list().contains(class)
}

pub fn add_class(el: &Element, class: &str) {
    let _ = el.class_list().add_1(class);
}

pub fn remove_class(el: &Element, class: &str) {
    let _ = el.class_list().remove_1(class);
}

pub fn hidden(el: &Element) -> bool {
    el.dyn_ref::<HtmlElement>()
        .map(|h| h.hidden())
        .unwrap_or_else(|| el.has_attribute("hidden"))
}

pub fn set_hidden(el: &Element, v: bool) {
    if let Some(h) = el.dyn_ref::<HtmlElement>() {
        h.set_hidden(v);
    } else if v {
        let _ = el.set_attribute("hidden", "");
    } else {
        let _ = el.remove_attribute("hidden");
    }
}

pub fn data(el: &Element, key: &str) -> Option<String> {
    el.get_attribute(&format!("data-{key}"))
}

pub fn set_style(el: &Element, prop: &str, val: &str) {
    if let Some(h) = el.dyn_ref::<HtmlElement>() {
        let _ = h.style().set_property(prop, val);
    } else if let Some(s) = el.dyn_ref::<web_sys::SvgElement>() {
        let _ = s.style().set_property(prop, val);
    }
}

pub fn focus(el: &Element) {
    if let Some(h) = el.dyn_ref::<HtmlElement>() {
        let _ = h.focus();
    }
}

pub fn blur(el: &Element) {
    if let Some(h) = el.dyn_ref::<HtmlElement>() {
        let _ = h.blur();
    }
}

pub fn active_element() -> Option<Element> {
    doc().active_element()
}

/// `target.closest(sel)` for an event target.
pub fn closest(target: &Option<EventTarget>, sel: &str) -> Option<Element> {
    let t = target.as_ref()?;
    let el: &Element = t.dyn_ref::<Element>()?;
    el.closest(sel).ok().flatten()
}

pub fn target_el(e: &Event) -> Option<Element> {
    e.target().and_then(|t| t.dyn_into::<Element>().ok())
}

pub fn contains(outer: &Element, inner: &Option<EventTarget>) -> bool {
    inner
        .as_ref()
        .and_then(|t| t.dyn_ref::<web_sys::Node>())
        .map(|n| outer.contains(Some(n)))
        .unwrap_or(false)
}

pub fn scroll_into_view_nearest(el: &Element) {
    let o = web_sys::ScrollIntoViewOptions::new();
    o.set_block(web_sys::ScrollLogicalPosition::Nearest);
    el.scroll_into_view_with_scroll_into_view_options(&o);
}

pub fn scroll_into_view_block(el: &Element, block: web_sys::ScrollLogicalPosition, smooth: bool) {
    let o = web_sys::ScrollIntoViewOptions::new();
    o.set_block(block);
    if smooth {
        o.set_behavior(web_sys::ScrollBehavior::Smooth);
    }
    el.scroll_into_view_with_scroll_into_view_options(&o);
}

// ---- listeners -----------------------------------------------------------------------------

/// `target.addEventListener(ty, f)`, kept for the life of the page.
pub fn on<E: JsCast + wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &EventTarget,
    ty: &str,
    f: impl FnMut(E) + 'static,
) {
    let cb = Closure::<dyn FnMut(E)>::new(f);
    let _ = target.add_event_listener_with_callback(ty, cb.as_ref().unchecked_ref());
    cb.forget();
}

/// Like [`on`], with `{ passive: false }` (so `preventDefault` works on wheel).
pub fn on_active<E: JsCast + wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &EventTarget,
    ty: &str,
    f: impl FnMut(E) + 'static,
) {
    let cb = Closure::<dyn FnMut(E)>::new(f);
    let o = web_sys::AddEventListenerOptions::new();
    o.set_passive(false);
    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(
        ty,
        cb.as_ref().unchecked_ref(),
        &o,
    );
    cb.forget();
}

pub fn on_id<E: JsCast + wasm_bindgen::convert::FromWasmAbi + 'static>(
    id_: &str,
    ty: &str,
    f: impl FnMut(E) + 'static,
) {
    on(id(id_).as_ref(), ty, f);
}

// ---- timers --------------------------------------------------------------------------------

/// `setTimeout(f, ms)`; returns the handle.
pub fn timeout(ms: i32, f: impl FnOnce() + 'static) -> i32 {
    let cb = Closure::once_into_js(f);
    win()
        .set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), ms)
        .unwrap_or(0)
}

pub fn clear_timeout(h: i32) {
    win().clear_timeout_with_handle(h);
}

pub fn interval(ms: i32, f: impl FnMut() + 'static) -> i32 {
    let cb = Closure::<dyn FnMut()>::new(f);
    let h = win()
        .set_interval_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), ms)
        .unwrap_or(0);
    cb.forget();
    h
}

/// `requestAnimationFrame(f)`.
pub fn raf(f: impl FnOnce(f64) + 'static) -> i32 {
    let cb = Closure::once_into_js(f);
    win()
        .request_animation_frame(cb.unchecked_ref())
        .unwrap_or(0)
}

pub fn perf_now() -> f64 {
    win()
        .performance()
        .map(|p| p.now())
        .unwrap_or_else(js_sys::Date::now)
}

/// A trailing-edge debounce, like the reference's `debounce(fn, ms)`.
pub struct Debounce {
    handle: std::sync::atomic::AtomicI32,
    ms: i32,
}

impl Debounce {
    pub const fn new(ms: i32) -> Self {
        Debounce {
            handle: std::sync::atomic::AtomicI32::new(0),
            ms,
        }
    }

    pub fn call(&'static self, f: impl FnOnce() + 'static) {
        use std::sync::atomic::Ordering::Relaxed;
        clear_timeout(self.handle.load(Relaxed));
        self.handle.store(timeout(self.ms, f), Relaxed);
    }
}

/// Report a failure the reference would have thrown (unhandled rejection) to the console.
pub fn log_err(ctx: &str, msg: &str) {
    web_sys::console::error_1(&JsValue::from_str(&format!("{ctx}: {msg}")));
}

pub fn location_hash() -> String {
    win().location().hash().unwrap_or_default()
}
