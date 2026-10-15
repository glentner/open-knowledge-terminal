//! Selection and navigation: `select()`, views and tabs, the URL hash (back/forward), crumbs.

use crate::api::spawn;
use crate::dom;
use crate::js::esc;
use crate::state::{self, boot, s, s_mut};
use crate::{chrome, graph, insp, views};

#[derive(Default, Clone)]
pub struct SelOpts {
    pub from: &'static str,
    pub tab: Option<&'static str>,
    pub mode: Option<&'static str>,
    pub keep_mode: bool,
}

/// `select(id, opts)`
pub fn select(id: Option<&str>, opts: SelOpts) {
    let Some(idx) = state::idx() else { return };
    if let Some(i) = id {
        if !idx.has(i) {
            chrome::toast("Not found", i, true);
            return;
        }
    }
    let moved = id.map(String::from) != state::sel();
    if moved {
        graph::walk_reset(); // the walk belongs to the old selection
    }
    {
        let mut st = s_mut();
        let changed = id.map(String::from) != st.sel;
        st.sel = id.map(String::from);
        st.commit = None;
        if changed {
            st.rev = None;
            st.blame = false;
            st.data_sort = None;
            if !opts.keep_mode {
                st.mode = "rendered".into();
            }
            if opts.from != "list" {
                st.mark = None;
            }
        }
        if let Some(m) = opts.mode {
            st.mode = m.into();
        }
        if let Some(t) = opts.tab {
            st.tab = t.into();
        }
    }
    graph::update();
    let view = s().view;
    if let Some(i) = id {
        if opts.from != "graph" && view == "graph" {
            graph::focus_node(i, None);
        }
    }
    if view == "files" && opts.from != "tree" {
        views::tree::render();
    }
    if view == "table" {
        for tr in dom::qa("#tgrid tr[data-id]") {
            let on = dom::data(&tr, "id").as_deref() == id;
            dom::toggle(&tr, "on", on);
        }
    }
    render_crumbs();
    insp::render();
    if opts.from != "pop" {
        nav(true);
    }
}

/// `setTab(t)`
pub fn set_tab(t: &str) {
    {
        let mut st = s_mut();
        st.tab = t.into();
        st.commit = None;
    }
    insp::render();
    nav(false);
}

/// `setView(v, opts)`
pub fn set_view(v: &str, quiet: bool) {
    let v: &'static str = match v {
        "files" => "files",
        "log" => "log",
        "table" => "table",
        "list" => "list",
        _ => "graph",
    };
    s_mut().view = v;
    for id in ["graph", "files", "log", "table", "list"] {
        dom::set_hidden(&dom::id(&format!("v-{id}")), id != v);
    }
    for b in dom::qa(".fk") {
        let on = dom::data(&b, "view").as_deref() == Some(v);
        dom::toggle(&b, "on", on);
    }
    match v {
        "files" => views::tree::render(),
        "log" => spawn(async {
            if views::log::ensure(false).await.is_ok() {
                views::log::render();
            }
        }),
        "table" => {
            if s().table_folder.is_some() {
                views::table::render();
            } else {
                let f = state::current_folder();
                spawn(async move { views::table::open(&f, None).await });
            }
        }
        "list" => views::list::render(),
        _ => graph::schedule(),
    }
    if !quiet {
        nav(true);
    }
}

/// `stateHash()`
pub fn state_hash() -> String {
    let p = web_sys::UrlSearchParams::new().unwrap();
    let st = s();
    p.set("v", st.view);
    if let Some(sel) = &st.sel {
        p.set("s", sel);
    }
    if st.tab != "overview" {
        p.set("tab", &st.tab);
    }
    if let Some(c) = &st.commit {
        p.set("c", c);
    }
    if let Some(r) = &st.rev {
        p.set("rev", r);
    }
    if st.view == "table" {
        if let Some(t) = &st.table_folder {
            p.set("t", t);
        }
    }
    if st.view == "list" {
        if let Some(q) = &st.list_query {
            p.set("q", q);
        }
    }
    format!("#{}", String::from(p.to_string()))
}

/// `nav(push)`
pub fn nav(push: bool) {
    if s().restoring {
        return;
    }
    let h = state_hash();
    if h == dom::location_hash() {
        return;
    }
    let hist = dom::win().history().unwrap();
    if push {
        let _ = hist.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&h));
    } else {
        let _ = hist.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&h));
    }
}

/// `applyHash()`
pub async fn apply_hash() {
    s_mut().restoring = true;
    apply_hash_inner().await;
    s_mut().restoring = false;
}

async fn apply_hash_inner() {
    let Some(idx) = state::idx() else { return };
    let hash = dom::location_hash();
    let p = web_sys::UrlSearchParams::new_with_str(hash.get(1..).unwrap_or("")).unwrap();
    let v = p
        .get("v")
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| "graph".into());
    let sel = p.get("s");
    {
        let mut st = s_mut();
        st.tab = p
            .get("tab")
            .filter(|x| !x.is_empty())
            .unwrap_or_else(|| "overview".into());
        st.rev = p.get("rev");
    }
    let c = p.get("c");
    if v == "table" {
        if let Some(t) = p.get("t") {
            if Some(&t) != s().table_folder.as_ref() {
                views::table::open(&t, None).await;
            }
        }
    }
    if v == "list" {
        if let Some(q) = p.get("q") {
            if Some(&q) != s().list_query.as_ref() {
                views::list::find(&q).await;
            }
        }
    }
    if s().view != v {
        set_view(&v, true);
    }
    let prev = state::sel();
    {
        let mut st = s_mut();
        st.sel = sel.filter(|x| idx.has(x));
        if st.sel != prev {
            st.blame = false;
            st.mode = "rendered".into();
            st.mark = None;
        }
        st.commit = c.filter(|x| !x.is_empty());
    }
    graph::update();
    let (now, view) = (state::sel(), s().view);
    if let Some(n) = &now {
        if now != prev && view == "graph" {
            graph::focus_node(n, None);
        }
    }
    if view == "files" {
        views::tree::render();
    }
    render_crumbs();
    insp::render();
}

/// `renderCrumbs()`
pub fn render_crumbs() {
    let el = dom::id("crumbs");
    let app = boot().app_name().to_string();
    let Some(idx) = state::idx() else {
        dom::doc().set_title(&app);
        return;
    };
    let sel = state::sel();
    match &sel {
        Some(x) if x != "." => dom::doc().set_title(&format!("{} — {}", idx.name_of(x), app)),
        _ => dom::doc().set_title(&app),
    }
    let Some(sel) = sel else {
        dom::set_html(&el, &format!("<span>{}</span>", esc(&idx.kb.display)));
        return;
    };
    let n = idx.node(&sel).unwrap();
    if n.t == "surface" {
        dom::set_html(
            &el,
            &format!(
                "<span>external</span><span class=\"sep\">›</span><b>{}</b>",
                esc(&n.name)
            ),
        );
        return;
    }
    // (one trailing slash is stripped: `replace(/\/$/, '')`)
    let parts: Vec<String> = if sel == "." {
        vec![]
    } else {
        sel.strip_suffix('/')
            .unwrap_or(&sel)
            .split('/')
            .map(String::from)
            .collect()
    };
    let mut acc = String::new();
    let mut html = format!("<button data-go=\".\">~/{}</button>", esc(&idx.kb.name));
    let last = parts.len().saturating_sub(1);
    for (i, part) in parts.iter().enumerate() {
        acc.push_str(part);
        if i < last || n.t == "dir" {
            acc.push('/');
        }
        html.push_str("<span class=\"sep\">/</span>");
        if i < last {
            html.push_str(&format!(
                "<button data-go=\"{}\">{}</button>",
                esc(&acc),
                esc(part)
            ));
        } else {
            html.push_str(&format!("<b style=\"color:var(--ink)\">{}</b>", esc(part)));
        }
    }
    dom::set_html(&el, &html);
}
