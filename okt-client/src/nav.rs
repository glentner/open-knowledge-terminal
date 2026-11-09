//! Selection and navigation: `select()`, views and tabs placed in the arrangement, the URL hash
//! (back/forward), crumbs.

use std::cell::Cell;

use crate::api::spawn;
use crate::arrange::{self, View};
use crate::dom;
use crate::js::esc;
use crate::state::{self, boot, cfg, cfg_mut, s, s_mut, save_settings};
use crate::{chrome, graph, insp, panes, views};

thread_local! {
    /// the first `apply_hash`, which places by the boot rule
    static BOOTING: Cell<bool> = const { Cell::new(false) };
}

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
    if let Some(i) = id {
        if opts.from != "graph" && panes::visible("graph") {
            graph::focus_node(i, None);
        }
    }
    if panes::visible("files") && opts.from != "tree" {
        views::tree::render();
    }
    if panes::visible("table") {
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

/// `setTab(t)`: the Inspector's tab, shown by the placement rule without moving focus.
pub fn set_tab(t: &str) {
    {
        let mut st = s_mut();
        st.tab = t.into();
        st.commit = None;
    }
    place(View::Insp, false);
    insp::render();
    nav(false);
}

/// Show the Inspector where it is held, or in the focused pane, leaving focus where it is.
pub fn show_insp() {
    place(View::Insp, false);
    insp::render();
}

/// `setView(v, opts)`: show the view where it is held (or in the focused pane), focus its pane,
/// render it, and push the history entry unless `quiet`.
pub fn set_view(v: &str, quiet: bool) {
    let v = match v {
        "files" => View::Files,
        "log" => View::Log,
        "table" => View::Table,
        "list" => View::List,
        _ => View::Graph,
    };
    place(v, true);
    if panes::visible(v.name()) {
        show(v);
    }
    if !quiet {
        nav(true);
    }
}

/// Place a view by the placement rule, borrowing the settings only for the pure operation, then
/// draw and save. On the first load an arrangement already saved is left as it is (the view only
/// picks the focus), and one not yet saved is placed in without saving it.
fn place(v: View, take_focus: bool) {
    let booting = BOOTING.get();
    let focus = s().focus;
    let out = {
        let mut c = cfg_mut();
        if booting && c.arr_saved {
            let f = arrange::showing(&c.arr, v).unwrap_or_else(|| arrange::default_focus(&c.arr));
            arrange::Outcome {
                focus: take_focus.then_some(f),
                ..Default::default()
            }
        } else {
            let out = arrange::place(&mut c.arr, v, focus, take_focus);
            if !booting {
                c.arr_saved |= out.changed;
            }
            out
        }
    };
    if let Some(f) = out.focus {
        s_mut().focus = f;
    }
    sync_view();
    panes::apply();
    if out.changed && !booting {
        save_settings();
    }
}

/// Run an arrangement operation on the focused pane: the pure operation under one short settings
/// borrow, which is dropped before anything else; then the focus, the drawing, the save, a render
/// of every view that came on screen, and the hash. A refusal toasts and changes nothing.
pub fn operate(op: impl FnOnce(&mut arrange::Node, arrange::PaneId) -> arrange::Outcome) {
    let before: Vec<View> = View::ALL
        .into_iter()
        .filter(|v| panes::visible(v.name()))
        .collect();
    let focus = s().focus;
    let out = {
        let mut c = cfg_mut();
        let out = op(&mut c.arr, focus);
        c.arr_saved |= out.changed;
        out
    };
    if let Some(t) = out.toast {
        chrome::toast("PANES", t, true);
    }
    if !out.changed && out.focus.is_none() {
        return;
    }
    if let Some(f) = out.focus {
        s_mut().focus = f;
    }
    sync_view();
    panes::apply();
    if out.changed {
        save_settings();
    }
    for v in View::ALL {
        if !before.contains(&v) && panes::visible(v.name()) {
            show(v);
        }
    }
    nav(false);
}

/// Classic's share: the Inspector at the splitter width saved before arrangements, else 46%.
pub fn classic_share() -> f64 {
    arrange::classic_share(cfg().insp)
}

/// Apply a preset; focus goes to the pane holding the current view, when it is expanded.
pub fn preset(p: arrange::Preset) {
    let c = classic_share();
    let view = View::parse(s().view);
    operate(|n, _| arrange::apply_preset(n, p, c, view));
}

/// Apply the preset after the one the arrangement equals, or classic.
pub fn cycle_preset() {
    let next = {
        let c = cfg();
        arrange::cycle(&c.arr, arrange::classic_share(c.insp))
    };
    preset(next);
}

/// `S.view` follows the focused pane when it shows one of the five main views. While the Inspector
/// is shown there it keeps the view that last held key focus, by view and not by pane, so the view
/// keys still reach it wherever an operation has moved it (`panes::key_view`).
pub fn sync_view() {
    let shown = {
        let c = cfg();
        let f = s().focus;
        arrange::shown_in(&c.arr, f)
    };
    if let Some(v) = shown.filter(|v| v.is_main()) {
        s_mut().view = v.name();
    }
}

/// Render a view that has just come on screen, or the view the keys go to off screen
/// (`panes::apply`). This only draws: focus, the hash and the history stay as they are, since only
/// the user's action moves them.
pub fn show(v: View) {
    match v {
        View::Files => views::tree::render(),
        View::Log => spawn(async {
            if views::log::ensure(false).await.is_ok() {
                views::log::render();
            }
        }),
        View::Table => {
            if s().table_folder.is_some() {
                views::table::render();
            } else {
                let f = state::current_folder();
                spawn(async move {
                    if views::table::load(&f, None).await && panes::visible("table") {
                        views::table::render();
                    }
                });
            }
        }
        View::List => views::list::render(),
        View::Graph => graph::schedule(),
        View::Insp => insp::render(),
    }
}

/// The first view after the index loads: the hash's (or the Graph), placed by the boot rule; then
/// every other view on screen is drawn, and the hash names what is shown.
pub async fn boot_view() {
    BOOTING.set(true);
    let first = apply_hash().await;
    BOOTING.set(false);
    for v in View::MAIN {
        if Some(v) != first && v != View::Files && panes::visible(v.name()) {
            show(v);
        }
    }
    nav(false);
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

/// `applyHash()`: the hash's view placed and focused, its selection and tab restored. Returns the
/// view it placed.
pub async fn apply_hash() -> Option<View> {
    s_mut().restoring = true;
    let v = apply_hash_inner().await;
    s_mut().restoring = false;
    v
}

async fn apply_hash_inner() -> Option<View> {
    let idx = state::idx()?;
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
                views::table::load(&t, None).await;
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
    set_view(&v, true);
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
    let now = state::sel();
    if let Some(n) = &now {
        if now != prev && panes::visible("graph") {
            graph::focus_node(n, None);
        }
    }
    if panes::visible("files") {
        views::tree::render();
    }
    render_crumbs();
    insp::render();
    View::parse(&v).filter(|v| v.is_main()).or(Some(View::Graph))
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
