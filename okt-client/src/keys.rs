//! Global events (`wireGlobal()`): click delegation, the keyboard, timers.

use std::cell::Cell;

use wasm_bindgen::JsCast;
use web_sys::{Event, KeyboardEvent, MouseEvent};

use crate::actions;
use crate::api::spawn;
use crate::arrange::{self, Preset};
use crate::chrome::{self, toast};
use crate::dom;
use crate::graph;
use crate::insp;
use crate::js::{self, num};
use crate::nav::{self, SelOpts};
use crate::panes;
use crate::state::{self, ListData, ListItem, Live, boot, s, s_mut};
use crate::views;

thread_local! {
    /// when the pane leader `w` was pressed (`dom::perf_now`), or below zero when none is pending
    static LEADER: Cell<f64> = const { Cell::new(-1.0) };
}

/// How long the leader waits for its second key, in ms.
const LEADER_MS: f64 = 2000.0;

fn leader_pending() -> bool {
    let t = LEADER.get();
    t >= 0.0 && dom::perf_now() - t <= LEADER_MS
}

/// The second key of the pane leader `w`, acting on the focused pane. False for a key that is
/// not one, which is then handled as usual.
fn pane_key(k: &str) -> bool {
    use arrange::Dir::{Next, Prev};
    match k {
        "w" => nav::operate(|n, id| arrange::focus_step(n, id, 1)),
        "W" => nav::operate(|n, id| arrange::focus_step(n, id, -1)),
        "d" => nav::operate(arrange::collapse_toggle),
        "x" => nav::operate(|n, id| arrange::swap(n, id, Next)),
        "r" => nav::operate(arrange::flip),
        "q" => nav::operate(|n, id| arrange::close(n, id, Next)),
        "v" => nav::operate(|n, id| arrange::move_view(n, id, Next)),
        "V" => nav::operate(|n, id| arrange::move_view(n, id, Prev)),
        _ => return false,
    }
    true
}

fn parse_ln(v: Option<String>) -> Option<i64> {
    let v = v?;
    let x = js::parse_float(&v);
    if v.trim().is_empty() {
        return Some(0);
    }
    if x.is_nan() { None } else { Some(x as i64) }
}

fn on_click(e: MouseEvent) {
    let t = e.target();
    let Some(idx) = state::idx() else { return };
    if let Some(go) = dom::closest(&t, "[data-go]") {
        e.prevent_default();
        let id = dom::data(&go, "go").unwrap_or_default();
        nav::select(Some(&id), SelOpts::default());
        return;
    }
    if let Some(kl) = dom::closest(&t, "a[data-kb]") {
        e.prevent_default();
        let href = kl.get_attribute("href").unwrap_or_default();
        // href.includes('#', 2): a fragment after "#/"
        let frag = {
            let u: Vec<u16> = js::u16s(&href);
            let pos = u
                .iter()
                .enumerate()
                .skip(2)
                .find(|(_, c)| **c == b'#' as u16)
                .map(|(i, _)| i);
            match pos {
                Some(p) => js::decode_uri_component(&String::from_utf16_lossy(&u[p + 1..])),
                None => Some(String::new()),
            }
        };
        let Some(frag) = frag else { return };
        let tab = s().tab.clone();
        let target = dom::data(&kl, "kb").unwrap_or_default();
        nav::select(
            Some(&target),
            SelOpts {
                tab: if tab == "content" {
                    Some("content")
                } else {
                    None
                },
                ..Default::default()
            },
        );
        if !frag.is_empty() {
            dom::timeout(250, move || {
                if let Ok(Some(el)) =
                    dom::id("ibody").query_selector(&format!("#{}", js::css_escape(&frag)))
                {
                    dom::scroll_into_view_block(&el, web_sys::ScrollLogicalPosition::Start, false);
                }
            });
        }
        return;
    }
    if let Some(anch) = dom::closest(&t, "a.anchor") {
        e.prevent_default();
        let h = dom::data(&anch, "href").unwrap_or_default();
        let Some(id) = js::decode_uri_component(&js::slice16(&h, 1, None)) else {
            return;
        };
        if let Ok(Some(el)) = dom::id("ibody").query_selector(&format!("#{}", js::css_escape(&id)))
        {
            dom::scroll_into_view_block(&el, web_sys::ScrollLogicalPosition::Start, true);
        }
        return;
    }
    if dom::closest(&t, "a.broken, a.outside").is_some() {
        e.prevent_default();
        return;
    }
    // a pane's tab, or a link to a view: the Inspector goes to its own placement
    if let Some(vw) = dom::closest(&t, "[data-view]") {
        match dom::data(&vw, "view").unwrap_or_default().as_str() {
            "insp" => nav::show_insp(),
            v => nav::set_view(v, false),
        }
        return;
    }
    if let Some(at) = dom::closest(&t, "[data-at]") {
        e.stop_propagation();
        if let Some(p) = dom::data(&at, "atpath").filter(|p| !p.is_empty()) {
            if idx.has(&p) && state::sel().as_deref() != Some(&p) {
                s_mut().sel = Some(p);
                graph::update();
                nav::render_crumbs();
            }
        }
        s_mut().commit = None;
        actions::travel(dom::data(&at, "at").as_deref());
        return;
    }
    if let Some(bl) = dom::closest(&t, ".src .bl") {
        if let Some(c) = dom::data(&bl, "commit").filter(|c| !c.is_empty()) {
            insp::open_commit(&c);
        }
        return;
    }
    if let Some(cm) = dom::closest(&t, "[data-commit]") {
        if let Some(c) = dom::data(&cm, "commit").filter(|c| !c.is_empty()) {
            insp::open_commit(&c);
        }
        return;
    }
    if let Some(op) = dom::closest(&t, "[data-open]") {
        let path = dom::data(&op, "path").or_else(state::sel);
        actions::open_with(&dom::data(&op, "open").unwrap_or_default(), path.as_deref());
        return;
    }
    if dom::closest(&t, "[data-obsidian]").is_some() {
        let sel = state::sel().unwrap_or_default();
        let p = if sel.to_ascii_lowercase().ends_with(".md") {
            sel[..sel.len() - 3].to_string()
        } else {
            sel.clone()
        };
        let url = format!(
            "obsidian://open?vault={}&file={}",
            js::encode_uri_component(&idx.kb.name),
            js::encode_uri_component(&p)
        );
        let _ = dom::win().location().set_href(&url);
        return;
    }
    if dom::closest(&t, "[data-copy]").is_some() {
        let sel = state::sel().unwrap_or_else(|| ".".into());
        let p = if sel == "." {
            idx.kb.root.clone()
        } else {
            format!("{}/{}", idx.kb.root, sel)
        };
        actions::copy_text(&p);
        return;
    }
    if let Some(cs) = dom::closest(&t, "[data-copysha]") {
        actions::copy_text(&dom::data(&cs, "copysha").unwrap_or_default());
        return;
    }
    if let Some(ls) = dom::closest(&t, "[data-list]") {
        views::list::list_of(&dom::data(&ls, "list").unwrap_or_default(), None);
        return;
    }
    if let Some(ty) = dom::closest(&t, "[data-type]") {
        let tv = dom::data(&ty, "type").unwrap_or_default();
        views::list::list_of("TYPE", Some(&tv));
        return;
    }
    if let Some(tb) = dom::closest(&t, "[data-table]") {
        let f = dom::data(&tb, "table").unwrap_or_default();
        spawn(async move { views::table::open(&f, None).await });
        return;
    }
    if let Some(co) = dom::closest(&t, "[data-collapse]") {
        graph::toggle_collapse(&dom::data(&co, "collapse").unwrap_or_default(), None);
        insp::render();
        return;
    }
    if let Some(md) = dom::closest(&t, "[data-mode]") {
        s_mut().mode = dom::data(&md, "mode").unwrap_or_default();
        insp::render();
        return;
    }
    if let Some(dk) = dom::closest(&t, ".dtable th[data-dk]") {
        let k: usize = dom::data(&dk, "dk")
            .and_then(|x| x.parse().ok())
            .unwrap_or(0);
        {
            let mut st = s_mut();
            st.data_sort = match st.data_sort {
                Some((sk, d)) if sk == k => {
                    if d > 0 {
                        Some((k, -1))
                    } else {
                        None
                    }
                }
                _ => Some((k, 1)),
            };
        }
        insp::render();
        return;
    }
    if dom::closest(&t, "[data-blame]").is_some() {
        actions::toggle_blame();
        return;
    }
    if dom::closest(&t, "[data-live]").is_some() {
        actions::travel(None);
        return;
    }
    if dom::closest(&t, "[data-back]").is_some() {
        let h = dom::win().history().unwrap();
        if h.length().unwrap_or(0) > 1 {
            let _ = h.back();
        } else {
            s_mut().commit = None;
            insp::render();
        }
        return;
    }
    if let Some(tg) = dom::closest(&t, ".cfh[data-toggle]") {
        if dom::closest(&t, "button").is_none() {
            if let Some(d) = tg.next_element_sibling() {
                let h = dom::hidden(&d);
                dom::set_hidden(&d, !h);
            }
            return;
        }
    }
    if let Some(ol) = dom::closest(&t, ".outline button") {
        s_mut().jump = parse_ln(dom::data(&ol, "ln"));
        nav::set_tab("content");
        return;
    }
    if let Some(more) = dom::closest(&t, "[data-more]") {
        let id = dom::data(&more, "more").unwrap_or_default();
        let mut items: Vec<ListItem> = idx
            .out_of(&id)
            .iter()
            .map(|&i| &idx.edges[i])
            .map(|e| ListItem {
                id: e.t.clone(),
                ln: None,
                label: e.t.clone(),
                meta: format!("out · {}", e.k),
            })
            .collect();
        items.extend(
            idx.in_of(&id)
                .iter()
                .map(|&i| &idx.edges[i])
                .map(|e| ListItem {
                    id: e.s.clone(),
                    ln: None,
                    label: e.s.clone(),
                    meta: format!("in · {}", e.k),
                }),
        );
        items.retain(|x| idx.has(&x.id));
        let meta = String::new();
        views::list::show(ListData::Paths {
            title: format!("CONNECTIONS {}", idx.name_of(&id)),
            meta,
            items,
        });
        return;
    }
    if let Some(row) = dom::closest(&t, ".lsimple[data-id]") {
        if dom::closest(&t, "#ibody").is_some() {
            let ln = dom::data(&row, "ln");
            s_mut().jump = parse_ln(ln.clone()).filter(|x| *x != 0);
            let id = dom::data(&row, "id").unwrap_or_default();
            let has = ln.as_deref().map(|x| !x.is_empty()).unwrap_or(false);
            nav::select(
                Some(&id),
                SelOpts {
                    tab: if has { Some("content") } else { None },
                    ..Default::default()
                },
            );
            return;
        }
    }
    if dom::closest(&t, "#helpseg").is_some() || dom::closest(&t, "#helpbtn").is_some() {
        chrome::show_help();
    }
}

/// A key while a tab's view is being dragged, as the pane leader takes one: a modifier alone
/// leaves the drag going, Escape cancels it and does nothing else, and any other key cancels it and
/// is then handled as usual. Keys while a tab is only pressed are handled as usual.
fn on_drag_key(e: KeyboardEvent) {
    if !panes::dragging() {
        return;
    }
    let k = e.key();
    if matches!(k.as_str(), "Shift" | "Control" | "Alt" | "Meta" | "CapsLock") {
        return;
    }
    panes::cancel_drag();
    if k == "Escape" {
        e.prevent_default();
        e.stop_immediate_propagation();
    }
}

fn on_key(e: KeyboardEvent) {
    if e.meta_key() || e.ctrl_key() || e.alt_key() {
        return;
    }
    if let Some(t) = dom::target_el(&e) {
        let tag = t.tag_name().to_lowercase();
        let editable = t
            .dyn_ref::<web_sys::HtmlElement>()
            .map(|h| h.is_content_editable())
            .unwrap_or(false);
        if tag == "input" || tag == "textarea" || tag == "select" || editable {
            if e.key() == "Escape" {
                dom::blur(&t);
            }
            return;
        }
    }
    let help = dom::id("help");
    if !dom::hidden(&help) {
        let k = e.key();
        if k == "Escape" || k == "?" {
            dom::set_hidden(&help, true);
            e.prevent_default();
        }
        return;
    }
    let k = e.key();
    // the pane leader: a modifier alone leaves it waiting (so w W and w V reach it), Escape
    // cancels it, and any other key cancels it and is then handled as usual
    if leader_pending() {
        if matches!(k.as_str(), "Shift" | "Control" | "Alt" | "Meta" | "CapsLock") {
            return;
        }
        LEADER.set(-1.0);
        if k == "Escape" || pane_key(&k) {
            e.prevent_default();
            return;
        }
    }
    if k == "/" || k == ":" {
        e.prevent_default();
        dom::focus(&dom::id("go"));
        return;
    }
    // the view keys act on the focused pane's view, or through the Inspector's to the view before
    let view = panes::key_view();
    if view == "files" && views::tree::key(&e) {
        e.prevent_default();
        return;
    }
    if view == "log" && views::log::key(&e) {
        e.prevent_default();
        return;
    }
    let Some(idx) = state::idx() else { return };
    let (rev, travel_nav, sel) = {
        let st = s();
        (st.rev.clone(), st.travel_nav.clone(), st.sel.clone())
    };
    if (k == "," || k == ".") && rev.is_some() && travel_nav.is_some() {
        e.prevent_default();
        let (older, newer) = travel_nav.unwrap();
        let h = if k == "," { older } else { newer };
        match h {
            Some(h) => actions::travel(Some(&h)),
            None if k == "." => actions::travel(None),
            None => {}
        }
        return;
    }
    if k == "," && rev.is_none() {
        if let Some(id) = sel.clone() {
            let tracked = idx
                .node(&id)
                .map(|n| n.t == "file" && n.c.map(|c| c != 0.0).unwrap_or(false))
                .unwrap_or(false);
            if tracked {
                // from the working tree, step to the last committed version that differs from it
                e.prevent_default();
                spawn(async move {
                    let Ok(h) = insp::path_history(&id).await else {
                        return;
                    };
                    if state::sel().as_deref() != Some(&id) {
                        return;
                    }
                    let dirty = state::idx()
                        .map(|i| i.dirty().get(&id).map(|d| !d.is_empty()).unwrap_or(false))
                        .unwrap_or(false);
                    let c = if dirty { h.first() } else { h.get(1) };
                    match c {
                        Some(c) => actions::travel(Some(&c.h)),
                        None => {
                            toast("HISTORY", "No earlier version", true);
                        }
                    }
                });
                return;
            }
        }
    }
    if k == "Escape" {
        if view == "graph" && graph::walk_stop() {
            return;
        }
        let settings = dom::id("settings");
        if !dom::hidden(&settings) {
            dom::set_hidden(&settings, true);
            return;
        }
        if s().commit.is_some() {
            s_mut().commit = None;
            insp::render();
            nav::nav(true);
            return;
        }
        if rev.is_some() {
            actions::travel(None);
            return;
        }
        if sel.is_some() {
            nav::select(None, SelOpts::default());
        }
        return;
    }
    // Walk the graph from the keyboard (README § Keys). j and k step through
    // the selection's connections; Enter or space goes to the one marked, or else opens or
    // closes the selected folder (space, Enter) or shows a file's content (Enter).
    if view == "graph" {
        let walked = match k.as_str() {
            "j" | "ArrowDown" => {
                graph::walk(1);
                true
            }
            "k" | "ArrowUp" => {
                graph::walk(-1);
                true
            }
            "Enter" | " " => {
                if let Some(t) = graph::walk_target() {
                    nav::select(Some(&t), SelOpts::default());
                } else if let Some(id) = sel.as_deref() {
                    if idx.is_dir(id) {
                        let r = graph::rep_of(id);
                        let collapse = !(idx.is_dir(&r) && state::is_collapsed(&idx, &r));
                        actions::expand_cmd("", collapse);
                    } else if k == "Enter" {
                        nav::set_tab("content");
                    }
                }
                true
            }
            _ => false,
        };
        if walked {
            e.prevent_default();
            return;
        }
    }
    let handled = match k.as_str() {
        "g" => {
            nav::set_view("graph", false);
            true
        }
        "f" => {
            nav::set_view("files", false);
            true
        }
        "l" => {
            nav::set_view("log", false);
            true
        }
        "t" => {
            let f = state::current_folder();
            spawn(async move { views::table::open(&f, None).await });
            true
        }
        "o" => {
            nav::set_tab("overview");
            true
        }
        "c" => {
            nav::set_tab("content");
            true
        }
        "h" => {
            nav::set_tab("history");
            true
        }
        "b" => {
            actions::toggle_blame();
            true
        }
        "e" => {
            let r = sel.as_deref().map(graph::rep_of);
            let collapse = !(r
                .as_deref()
                .map(|r| idx.is_dir(r) && state::is_collapsed(&idx, r))
                .unwrap_or(false));
            actions::expand_cmd("", collapse);
            true
        }
        "u" => {
            if let Some(p) = sel
                .as_deref()
                .and_then(|s| idx.node(s))
                .and_then(|n| n.parent.clone())
            {
                nav::select(Some(&p), SelOpts::default());
            }
            true
        }
        "?" => {
            chrome::show_help();
            true
        }
        // the arrangement: a preset each, the cycle, and the pane leader
        ";" | "'" | "\\" | "`" => {
            if let Some(p) = Preset::from_key(&k) {
                nav::preset(p);
            }
            true
        }
        "p" => {
            nav::cycle_preset();
            true
        }
        "w" => {
            LEADER.set(dom::perf_now());
            true
        }
        "R" => {
            actions::rebuild();
            true
        }
        "[" => {
            let _ = dom::win().history().unwrap().back();
            true
        }
        "]" => {
            let _ = dom::win().history().unwrap().forward();
            true
        }
        "0" => {
            graph::fit();
            true
        }
        // reading, finding, pins, hand-offs (README § Keys)
        "J" | "K" => {
            let b = dom::id("ibody");
            let o = web_sys::ScrollToOptions::new();
            let half = b.client_height() as f64 / 2.0;
            o.set_top(if k == "J" { half } else { -half });
            o.set_behavior(web_sys::ScrollBehavior::Smooth);
            b.scroll_by_with_scroll_to_options(&o);
            true
        }
        "z" => {
            if view != "graph" {
                nav::set_view("graph", false);
            }
            graph::frame_selection();
            true
        }
        "s" => {
            let on_content = s().tab == "content";
            {
                let mut st = s_mut();
                st.mode = if on_content && st.mode == "source" {
                    "rendered".into()
                } else {
                    "source".into()
                };
                st.tab = "content".into();
                st.commit = None;
            }
            insp::render();
            nav::nav(false);
            true
        }
        "y" => {
            press("#ihead [data-copy], #ihead [data-copysha]", "COPY");
            true
        }
        "n" => {
            views::list::find_step(1);
            true
        }
        "N" => {
            views::list::find_step(-1);
            true
        }
        "m" => {
            actions::pin_toggle();
            true
        }
        d if d.len() == 1 && matches!(d.as_bytes()[0], b'1'..=b'9') => {
            actions::pin_jump((d.as_bytes()[0] - b'0') as usize);
            true
        }
        "E" => {
            press("#ihead [data-open=editor]", "EDIT");
            true
        }
        "O" => {
            press("#ihead [data-open=app]", "OPEN");
            true
        }
        "F" => {
            press("#ihead [data-open=reveal]", "REVEAL");
            true
        }
        "+" | "=" => {
            graph::zoom_by(1.35, None);
            true
        }
        "-" => {
            graph::zoom_by(1.0 / 1.35, None);
            true
        }
        _ => false,
    };
    if handled {
        e.prevent_default();
    }
}

/// Press one of the inspector header's buttons, so a key does exactly what the button does, and
/// only where the button is offered.
fn press(sel: &str, what: &str) {
    match dom::q(sel).and_then(|b| b.dyn_into::<web_sys::HtmlElement>().ok()) {
        Some(b) => b.click(),
        None => {
            toast(what, "Not offered for this selection", true);
        }
    }
}

/// `wireGlobal()`
pub fn wire() {
    dom::on(dom::doc().as_ref(), "click", on_click);
    for b in dom::qa("#itabs button") {
        let t = dom::data(&b, "tab").unwrap_or_default();
        dom::on(b.as_ref(), "click", move |_: MouseEvent| nav::set_tab(&t));
    }
    dom::on_id("help", "click", |_: MouseEvent| {
        dom::set_hidden(&dom::id("help"), true)
    });
    dom::on_id("refresh", "click", |_: MouseEvent| actions::rebuild());
    dom::on_id("live", "click", |_: MouseEvent| {
        let live = s().live;
        let b = boot();
        let title = match live {
            Live::Ok => "LIVE".to_string(),
            Live::Err => "ERR".into(),
            Live::Off => "OFF".into(),
            Live::Busy => "BUSY".into(),
        };
        let body = if b.watch {
            format!(
                "Watching every {}s for saved files and new commits",
                num(b.interval)
            )
        } else {
            "Watching is off (--no-watch); use REFRESH".to_string()
        };
        toast(&title, &body, false);
    });
    dom::on_id("headchip", "click", |_: MouseEvent| {
        if let Some(h) = state::idx()
            .and_then(|i| i.kb.head.clone())
            .filter(|h| !h.is_empty())
        {
            insp::open_commit(&h);
        }
    });
    dom::on(dom::win().as_ref(), "popstate", |_: Event| {
        spawn(async {
            nav::apply_hash().await;
        })
    });
    // ahead of every other key handler (`#go`'s included): a key during a tab drag cancels it
    let drag_key = wasm_bindgen::closure::Closure::<dyn FnMut(KeyboardEvent)>::new(on_drag_key);
    let _ = dom::win().add_event_listener_with_callback_and_bool(
        "keydown",
        drag_key.as_ref().unchecked_ref(),
        true,
    );
    drag_key.forget();
    dom::on(dom::doc().as_ref(), "keydown", on_key);
    dom::on(dom::win().as_ref(), "blur", |_: Event| {
        LEADER.set(-1.0);
        panes::cancel_drag();
    });
    // a reload inside the settings' save debounce keeps the change
    dom::on(dom::win().as_ref(), "pagehide", |_: Event| state::flush_settings());
    static RESIZE: dom::Debounce = dom::Debounce::new(150);
    // the views follow their panes (their own observers); the window resize keeps the ticker
    dom::on(dom::win().as_ref(), "resize", |_: Event| {
        RESIZE.call(chrome::render_ticker)
    });
    dom::interval(1000, chrome::tick_clock);
    dom::interval(60000, || {
        if panes::visible("files") {
            views::tree::render();
        }
        chrome::render_ticker();
    });
}
