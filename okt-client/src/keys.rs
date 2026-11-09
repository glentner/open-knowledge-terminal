//! Global events (`wireGlobal()`): click delegation, the keyboard, the splitter, timers.

use wasm_bindgen::JsCast;
use web_sys::{Event, KeyboardEvent, MouseEvent, PointerEvent};

use crate::actions;
use crate::api::spawn;
use crate::chrome::{self, toast};
use crate::dom;
use crate::graph;
use crate::insp;
use crate::js::{self, num};
use crate::nav::{self, SelOpts};
use crate::state::{self, ListData, ListItem, Live, boot, cfg, cfg_mut, s, s_mut, save_settings};
use crate::views;

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
    if let Some(vw) = dom::closest(&t, "[data-view]") {
        if !dom::has_class(&vw, "fk") {
            nav::set_view(&dom::data(&vw, "view").unwrap_or_default(), false);
            return;
        }
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
    if k == "/" || k == ":" {
        e.prevent_default();
        dom::focus(&dom::id("go"));
        return;
    }
    let view = s().view;
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
    // Walk the graph from the keyboard (README § Keys). j and k step through the selection's
    // connections clockwise in screen order, around the selection from 12 o'clock; Enter or space
    // flies to the one marked (the camera rises toward the galaxy and descends on it, and the
    // selection pushes a history entry, so [ comes back), or else opens or closes the selected
    // folder (space, Enter) or shows a file's content (Enter).
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
                    nav::select(
                        Some(&t),
                        SelOpts {
                            warp: true,
                            ..Default::default()
                        },
                    );
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
        // orbit the camera (README § Keys): only while focus is on the page, so a focused
        // inspector or input keeps its own arrow and page keys
        let orbit = match k.as_str() {
            "ArrowLeft" => Some((-graph::ORBIT_DEG, 0.0)),
            "ArrowRight" => Some((graph::ORBIT_DEG, 0.0)),
            "PageUp" => Some((0.0, graph::TILT_DEG)),
            "PageDown" => Some((0.0, -graph::TILT_DEG)),
            _ => None,
        };
        if let Some((d_theta, d_psi)) = orbit {
            let elsewhere = dom::active_element().is_some_and(|a| {
                let tag = a.tag_name().to_lowercase();
                a.closest("#insp").ok().flatten().is_some()
                    || matches!(tag.as_str(), "input" | "textarea" | "select")
            });
            if !elsewhere {
                graph::orbit_by(d_theta, d_psi);
                e.prevent_default();
                return;
            }
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
        // z frames in place, moving only the camera and pushing no history entry: with the
        // Results view active, the list's matches across regions; otherwise the selection and
        // its links (README § Keys)
        "z" => {
            let matches: Option<Vec<String>> = {
                let st = s();
                (st.view == "list")
                    .then(|| st.list.clone())
                    .flatten()
                    .map(|l| match &*l {
                        ListData::Find { files, names, .. } => names
                            .iter()
                            .cloned()
                            .chain(files.iter().map(|f| f.path.clone()))
                            .collect(),
                        ListData::Paths { items, .. } => {
                            items.iter().map(|it| it.id.clone()).collect()
                        }
                    })
            };
            if view != "graph" {
                nav::set_view("graph", false);
            }
            match matches {
                Some(ids) => graph::frame_set(&ids),
                None => graph::frame_selection(),
            }
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
            graph::zoom_by(1.35);
            true
        }
        "-" => {
            graph::zoom_by(1.0 / 1.35);
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
    for b in dom::qa(".fk") {
        let v = dom::data(&b, "view").unwrap_or_default();
        dom::on(b.as_ref(), "click", move |_: MouseEvent| {
            nav::set_view(&v, false)
        });
    }
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
        spawn(async { nav::apply_hash().await })
    });
    dom::on(dom::doc().as_ref(), "keydown", on_key);
    // resizable inspector
    let panes = dom::id("panes");
    dom::set_style(&panes, "--insp-w", &format!("{}%", num(cfg().insp)));
    dom::on_id("split", "pointerdown", |e: PointerEvent| {
        e.prevent_default();
        let split = dom::id("split");
        dom::add_class(&split, "drag");
        let mv =
            wasm_bindgen::closure::Closure::<dyn FnMut(PointerEvent)>::new(|ev: PointerEvent| {
                let panes = dom::id("panes");
                let r = panes.get_bounding_client_rect();
                let pct =
                    (100.0 * (r.right() - ev.client_x() as f64) / r.width()).clamp(22.0, 75.0);
                let v = (pct * 10.0 + 0.5).floor() / 10.0;
                cfg_mut().insp = v;
                dom::set_style(&panes, "--insp-w", &format!("{}%", num(v)));
            });
        let w = dom::win();
        let _ = w.add_event_listener_with_callback("pointermove", mv.as_ref().unchecked_ref());
        let mv_fn: js_sys::Function = mv.as_ref().unchecked_ref::<js_sys::Function>().clone();
        mv.forget();
        let up_slot: std::rc::Rc<std::cell::RefCell<Option<js_sys::Function>>> = Default::default();
        let up_slot2 = up_slot.clone();
        let up = wasm_bindgen::closure::Closure::<dyn FnMut(PointerEvent)>::new(
            move |_: PointerEvent| {
                dom::remove_class(&dom::id("split"), "drag");
                let w = dom::win();
                let _ = w.remove_event_listener_with_callback("pointermove", &mv_fn);
                // taking the handle out breaks the cycle (this closure -> slot -> its own
                // function), so both closures are freed once the drag ends
                if let Some(f) = up_slot2.borrow_mut().take() {
                    let _ = w.remove_event_listener_with_callback("pointerup", &f);
                }
                save_settings();
                if s().view == "log" {
                    views::log::render();
                }
            },
        );
        let up_fn: js_sys::Function = up.as_ref().unchecked_ref::<js_sys::Function>().clone();
        *up_slot.borrow_mut() = Some(up_fn.clone());
        let _ = w.add_event_listener_with_callback("pointerup", &up_fn);
        up.forget();
    });
    static RESIZE: dom::Debounce = dom::Debounce::new(150);
    dom::on(dom::win().as_ref(), "resize", |_: Event| {
        RESIZE.call(|| {
            if s().view == "log" {
                views::log::render();
            }
            chrome::render_ticker();
        })
    });
    dom::interval(1000, chrome::tick_clock);
    dom::interval(60000, || {
        if s().view == "files" {
            views::tree::render();
        }
        chrome::render_ticker();
    });
}
