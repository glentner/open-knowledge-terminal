//! The inspector (right pane): overview, content, history, and commit detail.

use std::cell::Cell;
use std::rc::Rc;

use serde_json::{Map, Value};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, future_to_promise};
use web_sys::Element;

use crate::api::{self, ApiError, spawn};
use crate::chrome::toast;
use crate::dom;
use crate::fmt::{self, short};
use crate::js::{self, esc, fmt_n, num};
use crate::model::{
    Blame, Commit, CommitDetail, DataTable, FilePayload, Idx, Node, ext_of, fam_var, kind_name,
};
use crate::state::{self, FileEntry, HistEntry, boot, cfg, s, s_mut};
use crate::views;
use crate::{graph, nav};

thread_local! {
    static TOKEN: Cell<u64> = const { Cell::new(0) };
    static KEY: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn token() -> u64 {
    TOKEN.with(|t| t.get())
}

fn err_js(e: &ApiError) -> JsValue {
    JsValue::from_str(&match e {
        ApiError::Network(m) => format!("N{m}"),
        ApiError::Http(m) => format!("H{m}"),
    })
}

fn js_err(v: JsValue) -> ApiError {
    let s = v.as_string().unwrap_or_default();
    match s.split_at(s.len().min(1)) {
        ("N", m) => ApiError::Network(m.into()),
        (_, m) => ApiError::Http(m.into()),
    }
}

/// `filePayload(path, rev)`: shared while in flight, cached per index version.
pub async fn file_payload(path: &str, rev: Option<&str>) -> Result<Rc<FilePayload>, ApiError> {
    let version = state::idx().map(|i| i.version).unwrap_or(0);
    let key = format!(
        "{path}@{}",
        rev.filter(|r| !r.is_empty())
            .map(String::from)
            .unwrap_or_else(|| version.to_string())
    );
    let pending = {
        let st = s();
        match st.files.get(&key) {
            Some(FileEntry::Ready(p)) => return Ok(p.clone()),
            Some(FileEntry::Pending(p)) => Some(p.clone()),
            None => None,
        }
    };
    let promise = match pending {
        Some(p) => p,
        None => {
            let url = format!(
                "/api/file?path={}{}",
                js::encode_uri_component(path),
                match rev.filter(|r| !r.is_empty()) {
                    Some(r) => format!("&rev={}", js::encode_uri_component(r)),
                    None => String::new(),
                }
            );
            let p = future_to_promise(async move {
                api::get_text(&url)
                    .await
                    .map(|t| JsValue::from_str(&t))
                    .map_err(|e| err_js(&e))
            });
            let mut st = s_mut();
            st.files.insert(key.clone(), FileEntry::Pending(p.clone()));
            if st.files.len() > 120 {
                st.files.shift_remove_index(0);
            }
            p
        }
    };
    match JsFuture::from(promise).await {
        Ok(text) => {
            let text = text.as_string().unwrap_or_default();
            let payload: FilePayload = serde_json::from_str(&text)
                .map_err(|e| ApiError::Http(format!("bad response: {e}")))?;
            let rc = Rc::new(payload);
            let mut st = s_mut();
            if matches!(st.files.get(&key), Some(FileEntry::Pending(_))) {
                if let Some(e) = st.files.get_mut(&key) {
                    *e = FileEntry::Ready(rc.clone());
                }
            }
            Ok(rc)
        }
        Err(e) => {
            s_mut().files.shift_remove(&key);
            Err(js_err(e))
        }
    }
}

/// `pathHistory(path)`
pub async fn path_history(path: &str) -> Result<Rc<Vec<Commit>>, ApiError> {
    let head = state::idx()
        .and_then(|i| i.kb.head.clone())
        .unwrap_or_else(|| "null".into());
    let key = format!("{path}@{head}");
    let pending = {
        let st = s();
        match st.hist.get(&key) {
            Some(HistEntry::Ready(p)) => return Ok(p.clone()),
            Some(HistEntry::Pending(p)) => Some(p.clone()),
            None => None,
        }
    };
    let promise = match pending {
        Some(p) => p,
        None => {
            let url = format!("/api/log?path={}", js::encode_uri_component(path));
            let p = future_to_promise(async move {
                api::get_text(&url)
                    .await
                    .map(|t| JsValue::from_str(&t))
                    .map_err(|e| err_js(&e))
            });
            s_mut()
                .hist
                .insert(key.clone(), HistEntry::Pending(p.clone()));
            p
        }
    };
    match JsFuture::from(promise).await {
        Ok(text) => {
            let d: crate::model::LogData =
                serde_json::from_str(&text.as_string().unwrap_or_default())
                    .map_err(|e| ApiError::Http(format!("bad response: {e}")))?;
            let rc = Rc::new(d.commits);
            if let Some(e) = s_mut().hist.get_mut(&key) {
                *e = HistEntry::Ready(rc.clone());
            }
            Ok(rc)
        }
        Err(e) => {
            s_mut().hist.remove(&key);
            Err(js_err(e))
        }
    }
}

// ---- reading position ----------------------------------------------------------------------

fn ctool_height(body: &Element) -> f64 {
    dom::q_in(body, ".ctool")
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        .map(|e| e.offset_height() as f64)
        .unwrap_or(0.0)
}

/// `readingLine(body)`
fn reading_line(body: &Element) -> Option<i64> {
    let top = body.get_bounding_client_rect().top() + ctool_height(body) + 2.0;
    for r in dom::qa_in(body, ".src .l, article.md [data-line]") {
        if r.get_bounding_client_rect().bottom() > top {
            let v = dom::data(&r, "n")
                .or_else(|| dom::data(&r, "line"))
                .unwrap_or_default();
            let n = js::parse_float(&v);
            return if n.is_nan() || n == 0.0 {
                None
            } else {
                Some(n as i64)
            };
        }
    }
    None
}

/// `lineEl(body, ln)`
fn line_el(body: &Element, ln: i64) -> Option<Element> {
    if let Some(row) = dom::q_in(body, &format!(".src .l[data-n=\"{ln}\"]")) {
        return Some(row);
    }
    let mut best = None;
    for b in dom::qa_in(body, "article.md [data-line]") {
        let v = js::parse_float(&dom::data(&b, "line").unwrap_or_default());
        if v <= ln as f64 {
            best = Some(b);
        } else {
            break;
        }
    }
    best
}

/// `scrollToEl(body, el)`
fn scroll_to_el(body: &Element, el: &Element) {
    let travel = dom::q_in(body, ".travel")
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        .map(|e| e.offset_height() as f64)
        .unwrap_or(0.0);
    let off = ctool_height(body) + travel;
    let delta =
        el.get_bounding_client_rect().top() - body.get_bounding_client_rect().top() - off - 4.0;
    body.set_scroll_top(body.scroll_top() + delta as i32);
}

// ---- render -----------------------------------------------------------------------------------

/// `renderInspector()`. Once the body is in, the graph walk's target gets its chip marked again
/// (`mark_chip`), so the mark survives a re-render such as a tab switch.
pub fn render() {
    let Some(idx) = state::idx() else { return };
    let tok = TOKEN.with(|t| {
        t.set(t.get() + 1);
        t.get()
    });
    let head = dom::id("ihead");
    let body = dom::id("ibody");
    let (commit, sel, tab, rev, jump) = {
        let st = s();
        (
            st.commit.clone(),
            st.sel.clone(),
            st.tab.clone(),
            st.rev.clone(),
            st.jump,
        )
    };
    let key = format!(
        "{}|{}|{}",
        match &commit {
            Some(c) => format!("c:{c}"),
            None => sel.clone().unwrap_or_else(|| "null".into()),
        },
        tab,
        rev.clone().unwrap_or_default()
    );
    let same = KEY.with(|k| *k.borrow() == key) && jump.is_none();
    let keep_line = if same && tab == "content" && body.scroll_top() > 4 {
        reading_line(&body)
    } else {
        None
    };
    let keep_top = body.scroll_top();
    for b in dom::qa("#itabs button") {
        let on = dom::data(&b, "tab").as_deref() == Some(tab.as_str()) && commit.is_none();
        dom::toggle(&b, "on", on);
    }
    if let Some(c) = commit {
        KEY.with(|k| *k.borrow_mut() = key);
        spawn(async move { render_commit(&c, tok).await });
        return;
    }
    let n = sel.as_deref().and_then(|id| idx.node(id)).cloned();
    dom::set_html(&head, &insp_head(&idx, n.as_ref()));
    let hist_n = n
        .as_ref()
        .filter(|n| n.t == "file")
        .map(|n| n.c.unwrap_or(0.0));
    if let Some(b) = dom::q("#itabs button[data-tab=history]") {
        dom::set_html(
            &b,
            &format!(
                "<kbd>H</kbd>HISTORY{}",
                match hist_n {
                    Some(h) if h != 0.0 => format!(" <span class=\"cnt\">{}</span>", num(h)),
                    _ => String::new(),
                }
            ),
        );
    }
    spawn(async move {
        let res = match tab.as_str() {
            "overview" => match &n {
                Some(n) => overview(&idx, n).await,
                None => Ok(dashboard(&idx)),
            },
            "content" => {
                let target = n.clone().or_else(|| idx.node(".").cloned());
                match target {
                    Some(t) => content(&idx, &t).await,
                    None => Ok(String::new()),
                }
            }
            _ => history_view(&idx, n.as_ref()).await,
        };
        if tok != token() {
            return;
        }
        let body = dom::id("ibody");
        match res {
            Ok(html) => {
                dom::set_html(&body, &html);
                size_md_tables(&body);
                body.set_scroll_top(0);
                KEY.with(|k| *k.borrow_mut() = key);
                if same {
                    if keep_line.is_some() {
                        s_mut().keep = keep_line;
                    } else {
                        body.set_scroll_top(keep_top);
                    }
                }
                after(n.as_ref());
                // the walk target is read (and its borrow dropped) before the mark is applied
                let walked = graph::walk_target();
                if walked.is_some() {
                    mark_chip(walked.as_deref());
                }
            }
            Err(msg) => dom::set_html(&body, &format!("<div class=\"empty\">{}</div>", esc(&msg))),
        }
    });
}

fn fam_badge(idx: &Idx, n: &Node) -> String {
    let fam = n.family.as_deref().unwrap_or("");
    let label = idx
        .family_label(fam)
        .map(String::from)
        .unwrap_or_else(|| n.family.clone().unwrap_or_else(|| "undefined".into()));
    format!(
        "<span class=\"badge fam\"><i style=\"background:{}\"></i>{}</span>",
        fam_var(fam),
        esc(&label)
    )
}

/// `inspHead(n)`
fn insp_head(idx: &Idx, n: Option<&Node>) -> String {
    let Some(n) = n else {
        let kb = &idx.kb;
        return format!(
            "<div class=\"ipath\">{}</div><div class=\"ititle\">{} · overview</div><div class=\"ibadges\"><span class=\"badge acc\">{} @ {}</span>{}{}<div class=\"iacts\"><button class=\"act\" data-open=\"reveal\" data-path=\".\">REVEAL</button></div></div>",
            esc(&kb.display),
            esc(&kb.name),
            esc(kb
                .branch
                .as_deref()
                .filter(|b| !b.is_empty())
                .unwrap_or("detached")),
            short(kb.head.as_deref().unwrap_or("")),
            if kb.ahead != 0 {
                format!("<span class=\"badge warn\">↑{} unpushed</span>", kb.ahead)
            } else {
                String::new()
            },
            if kb.behind != 0 {
                format!("<span class=\"badge warn\">↓{} behind</span>", kb.behind)
            } else {
                String::new()
            },
        );
    };
    let dirty = idx.dirty();
    let is_file = n.t == "file";
    let filer = boot().filer();
    let acts = if n.t == "surface" {
        String::new()
    } else {
        format!(
            "<div class=\"iacts\">{}{}<button class=\"act\" data-open=\"reveal\" title=\"Reveal in {}\">REVEAL</button>{}<button class=\"act\" data-copy=\"1\" title=\"Copy the path\">COPY</button></div>",
            if is_file && ["markdown", "code", "text", "html"].contains(&n.kind()) {
                "<button class=\"act\" data-open=\"editor\" title=\"Open in your editor\">EDIT</button>"
            } else {
                ""
            },
            if is_file {
                "<button class=\"act\" data-open=\"app\" title=\"Open in the default app\">OPEN</button>"
            } else {
                ""
            },
            filer,
            if idx.kb.obsidian && is_file && n.kind() == "markdown" {
                "<button class=\"act\" data-obsidian=\"1\" title=\"Open in Obsidian\">OBSIDIAN</button>"
            } else {
                ""
            },
        )
    };
    let path = if n.t == "surface" {
        "external system".to_string()
    } else if n.id == "." {
        idx.kb.display.clone()
    } else {
        n.id.clone()
    };
    let mut badges = vec![fam_badge(idx, n)];
    if n.t == "dir" {
        badges.push(format!(
            "<span class=\"badge\">folder · {} files</span>",
            fmt_n(idx.dinfo(&n.id).map(|d| d.files).unwrap_or(0.0))
        ));
    }
    if is_file {
        let t = if !n.type_s().is_empty() {
            format!("type: {}", n.type_s())
        } else if n.kind() == "markdown" {
            "markdown".into()
        } else {
            let e = ext_of(&n.name);
            if e.is_empty() {
                n.kind().to_string()
            } else {
                e
            }
        };
        badges.push(format!("<span class=\"badge\">{}</span>", esc(&t)));
        if let Some(d) = dirty.get(&n.id).filter(|d| !d.is_empty()) {
            let w = match d.as_str() {
                "M" => "modified",
                "?" => "untracked",
                "A" => "added",
                "D" => "deleted",
                "R" => "renamed",
                x => x,
            };
            badges.push(format!("<span class=\"badge warn\">{}</span>", esc(w)));
        }
    }
    if let Some(r) = s().rev.clone().filter(|r| !r.is_empty()) {
        badges.push(format!("<span class=\"badge acc\">@ {}</span>", short(&r)));
    }
    let label = idx.label_of(&n.id);
    format!(
        "<div class=\"ipath\">{}</div><div class=\"ititle\" title=\"{}\">{}</div><div class=\"ibadges\">{}{}</div>",
        esc(&path),
        esc(&label),
        esc(&label),
        badges.join(""),
        acts
    )
}

/// `nchip(id, small)`
pub fn nchip(idx: &Idx, id: &str, small: &str) -> String {
    let Some(n) = idx.node(id) else {
        return String::new();
    };
    format!(
        "<button class=\"nchip\" data-go=\"{}\" title=\"{}\"><i style=\"background:{}\"></i><span>{}</span>{}</button>",
        esc(id),
        esc(id),
        fam_var(n.family.as_deref().unwrap_or("")),
        esc(&if n.t == "file" {
            n.name.clone()
        } else {
            idx.name_of(id)
        }),
        if small.is_empty() {
            String::new()
        } else {
            format!("<small>{}</small>", esc(small))
        }
    )
}

const ORDER: [&str; 7] = [
    "link", "wiki", "mention", "field", "embed", "symlink", "surface",
];

fn edge_label(k: &str, out: bool) -> &'static str {
    match (k, out) {
        ("link", true) => "Links to",
        ("wiki", true) => "Wiki links to",
        ("embed", true) => "Embeds",
        ("mention", true) => "Mentions",
        ("field", true) => "Frontmatter points to",
        ("symlink", true) => "Symlink to",
        ("surface", true) => "Reaches out through",
        ("link", false) => "Linked from",
        ("wiki", false) => "Wiki-linked from",
        ("embed", false) => "Embedded in",
        ("mention", false) => "Mentioned in",
        ("field", false) => "Frontmatter of",
        ("symlink", false) => "Symlinked from",
        ("surface", false) => "Wired in from",
        _ => "undefined",
    }
}

/// Who j and k step through in the graph: a folder's subfolders and files, then the
/// Connections groups, each node once. This is the membership only: the graph walks it in
/// on-screen angular order (`graph::walk`), and `mark_chip` keeps this list's row of the walk's
/// target marked, so the two stay linked although their orders differ.
pub fn walk_list(idx: &Idx, id: &str) -> Vec<(String, &'static str)> {
    let mut out: Vec<(String, &'static str)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |x: &str, label: &'static str| {
        if x != id && idx.has(x) && seen.insert(x.to_string()) {
            out.push((x.to_string(), label));
        }
    };
    if idx.is_dir(id) {
        let (dirs, files) = idx.sorted_kids(id, true);
        for d in dirs {
            push(d, "Subfolder");
        }
        for f in files {
            push(f, "In this folder");
        }
    }
    for k in ORDER {
        for &ei in idx.out_of(id) {
            let e = &idx.edges[ei];
            if e.k == k {
                push(&e.t, edge_label(k, true));
            }
        }
    }
    for k in ORDER {
        for &ei in idx.in_of(id) {
            let e = &idx.edges[ei];
            if e.k == k {
                push(&e.s, edge_label(k, false));
            }
        }
    }
    out
}

/// How many chips a Connections group shows before its "+N more" button.
const GROUP_CHIPS: usize = 40;

/// Mark the inspector's row for the node the graph walk is on (none: unmark). Every walk target
/// has one while the selection's overview is shown: a target with no chip in the page gets one,
/// marked (`show_row`). That extra chip carries `data-walked` and goes again at the next mark, so
/// the inspector is back to what it was once the walk moves on or stops.
pub fn mark_chip(id: Option<&str>) {
    for c in dom::qa("#ibody .nchip.kb") {
        dom::remove_class(&c, "kb");
    }
    for c in dom::qa("#ibody [data-walked]") {
        c.remove();
    }
    let Some(id) = id else { return };
    let chip = dom::qa("#ibody .nchip[data-go]")
        .into_iter()
        .find(|c| dom::data(c, "go").as_deref() == Some(id))
        .or_else(|| show_row(id));
    if let Some(c) = chip {
        dom::add_class(&c, "kb");
        dom::scroll_into_view_nearest(&c);
    }
}

/// Give `id`, a walk target of the selection with no chip in the page, a chip of its own, while
/// the selection's overview is shown. A connection past its group's `GROUP_CHIPS` shown chips
/// goes at the end of that group (`show_past_cap`); a target no group lists (a folder's own
/// files, a link on a page with no Connections section) goes in a section of its own, headed as
/// the walk names it (`walk_list`), before the Connections section.
fn show_row(id: &str) -> Option<Element> {
    let sel = state::sel()?;
    let overview = format!("{sel}|overview|");
    if !KEY.with(|k| k.borrow().starts_with(&overview)) {
        return None;
    }
    show_past_cap(id).or_else(|| {
        let idx = state::idx()?;
        let label = walk_list(&idx, &sel).into_iter().find(|(x, _)| x == id)?.1;
        let html = format!(
            "<div class=\"sect\" data-walked><div class=\"sh\">{}</div><div class=\"conn\">{}</div></div>",
            esc(label),
            walked_chip(&idx, id, "")
        );
        match dom::qa("#ibody [data-conns]").into_iter().next() {
            Some(conns) => conns.insert_adjacent_html("beforebegin", &html).ok()?,
            None => dom::id("ibody")
                .insert_adjacent_html("beforeend", &html)
                .ok()?,
        }
        dom::qa("#ibody .sect[data-walked] .nchip[data-go]").into_iter().next()
    })
}

/// A chip shown only while the walk is on it.
fn walked_chip(idx: &Idx, id: &str, small: &str) -> String {
    nchip(idx, id, small).replacen("<button class=\"nchip\"", "<button class=\"nchip\" data-walked", 1)
}

/// Put the chip of `id`, a connection of the selection past its group's shown chips, into that
/// group (the first group holding it, as `walk_list` counts it), before the "+N more" button.
fn show_past_cap(id: &str) -> Option<Element> {
    let idx = state::idx()?;
    let sel = state::sel()?;
    let (gi, small) = conn_groups(&idx, &sel)
        .into_iter()
        .enumerate()
        .find_map(|(gi, (_, items))| {
            items.into_iter().find(|(x, _)| x == id).map(|(_, sm)| (gi, sm))
        })?;
    let sect = dom::qa("#ibody [data-conns]")
        .into_iter()
        .find(|e| dom::data(e, "conns").as_deref() == Some(sel.as_str()))?;
    let conn = dom::qa_in(&sect, ".cgroup .conn").into_iter().nth(gi)?;
    let html = walked_chip(&idx, id, &small);
    match dom::qa_in(&conn, "[data-more]").into_iter().next() {
        Some(more) => {
            more.insert_adjacent_html("beforebegin", &html).ok()?;
            more.previous_element_sibling()
        }
        None => {
            conn.insert_adjacent_html("beforeend", &html).ok()?;
            conn.last_element_child()
        }
    }
}

/// The selection's Connections groups, in the order the inspector shows them: outgoing edges by
/// kind, then incoming, each `(label, [(other end, small text)])`.
fn conn_groups(idx: &Idx, id: &str) -> Vec<(&'static str, Vec<(String, String)>)> {
    let mut groups: Vec<(&'static str, Vec<(String, String)>)> = Vec::new();
    let small = |e: &crate::model::Edge| {
        if e.k == "surface" || e.k == "field" {
            e.label.clone()
        } else if e.n > 1.0 {
            format!("×{}", num(e.n))
        } else {
            String::new()
        }
    };
    for k in ORDER {
        let items: Vec<(String, String)> = idx
            .out_of(id)
            .iter()
            .map(|&i| &idx.edges[i])
            .filter(|e| e.k == k)
            .map(|e| (e.t.clone(), small(e)))
            .collect();
        if !items.is_empty() {
            groups.push((edge_label(k, true), items));
        }
    }
    for k in ORDER {
        let items: Vec<(String, String)> = idx
            .in_of(id)
            .iter()
            .map(|&i| &idx.edges[i])
            .filter(|e| e.k == k)
            .map(|e| (e.s.clone(), small(e)))
            .collect();
        if !items.is_empty() {
            groups.push((edge_label(k, false), items));
        }
    }
    groups
}

/// `connections(id)`
fn connections(idx: &Idx, id: &str) -> String {
    let groups = conn_groups(idx, id);
    if groups.is_empty() {
        return "<div class=\"sect\"><div class=\"sh\">Connections</div><div style=\"color:var(--dim)\">No links in or out.</div></div>".into();
    }
    let total = idx.out_of(id).len() + idx.in_of(id).len();
    let mut html = format!(
        "<div class=\"sect\" data-conns=\"{}\"><div class=\"sh\">Connections <span class=\"c\">{total}</span></div>",
        esc(id)
    );
    for (label, items) in groups {
        let more = items.len() as i64 - GROUP_CHIPS as i64;
        html.push_str(&format!(
            "<div class=\"cgroup\"><div class=\"cgl\">{} · {}</div><div class=\"conn\">",
            esc(label),
            items.len()
        ));
        for (x, sm) in items.iter().take(GROUP_CHIPS) {
            html.push_str(&nchip(idx, x, sm));
        }
        if more > 0 {
            html.push_str(&format!(
                "<button class=\"nchip\" data-more=\"{}\"><small>+{more} more</small></button>",
                esc(id)
            ));
        }
        html.push_str("</div></div>");
    }
    html + "</div>"
}

fn is_obj(v: &Value) -> bool {
    matches!(v, Value::Object(_) | Value::Array(_))
}

/// `fmValue(v)`
fn fm_value(idx: &Idx, v: &Value) -> String {
    match v {
        Value::Null => "<span class=\"todo\">—</span>".into(),
        Value::String(s) if s.is_empty() => "<span class=\"todo\">—</span>".into(),
        Value::Array(a) => {
            if a.is_empty() {
                return "<span class=\"todo\">[]</span>".into();
            }
            a.iter()
                .map(|x| {
                    if is_obj(x) {
                        format!("<span class=\"obj\">{}</span>", fm_obj(idx, x))
                    } else {
                        format!("<span class=\"li\">{}</span>", fm_scalar(idx, x))
                    }
                })
                .collect()
        }
        Value::Object(_) => format!("<span class=\"obj\">{}</span>", fm_obj(idx, v)),
        _ => fm_scalar(idx, v),
    }
}

/// `fmObj(o)` (arrays are objects too)
fn fm_obj(idx: &Idx, o: &Value) -> String {
    let entries: Vec<(String, Value)> = match o {
        Value::Object(m) => js::entries(m)
            .into_iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        Value::Array(a) => a
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v.clone()))
            .collect(),
        _ => vec![],
    };
    entries
        .iter()
        .map(|(k, x)| {
            format!(
                "<b>{}:</b> {}",
                esc(k),
                if is_obj(x) {
                    esc(&js::json_stringify(x))
                } else {
                    fm_scalar(idx, x)
                }
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn todo_prefix(s: &str) -> bool {
    // /^todo\b/i
    let b = s.as_bytes();
    if b.len() < 4 || !s[..4].eq_ignore_ascii_case("todo") {
        return false;
    }
    match b.get(4) {
        None => true,
        Some(c) => !(c.is_ascii_alphanumeric() || *c == b'_'),
    }
}

/// `fmScalar(x)`
fn fm_scalar(idx: &Idx, x: &Value) -> String {
    let s = js::string(x);
    if todo_prefix(&s) {
        return format!("<span class=\"todo\">{}</span>", esc(&s));
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return format!(
            "<a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">{}</a>",
            esc(&s),
            esc(&s)
        );
    }
    if (s.contains('/') || s.contains('.')) && !s.chars().any(js::is_js_space) {
        let t = s.strip_prefix("./").unwrap_or(&s);
        if let Some(n) = idx.node(t) {
            if n.t != "surface" {
                return nchip(idx, t, "");
            }
            if t.ends_with('/') {
                return nchip(idx, t, "");
            }
        }
    }
    esc(&s)
}

/// `fmTable(fm)`
fn fm_table(idx: &Idx, fm: Option<&Map<String, Value>>) -> String {
    let Some(fm) = fm.filter(|m| !m.is_empty()) else {
        return String::new();
    };
    let rows: String = js::entries(fm)
        .into_iter()
        .map(|(k, v)| format!("<tr><td class=\"k\">{}</td><td class=\"v\"><span class=\"fmv\">{}</span></td></tr>", esc(k), fm_value(idx, v)))
        .collect();
    format!("<table class=\"fmtbl\">{rows}</table>")
}

fn stats(cells: &[(String, &str)]) -> String {
    let inner: String = cells
        .iter()
        .map(|(v, l)| format!("<div><b>{v}</b><span>{}</span></div>", esc(l)))
        .collect();
    format!("<div class=\"stats4\">{inner}</div>")
}

/// `overview(n)`
async fn overview(idx: &Idx, n: &Node) -> Result<String, String> {
    if n.t == "surface" {
        return Ok(surface_view(idx, n));
    }
    if n.t == "dir" || n.t == "root" {
        return Ok(folder_view(idx, n));
    }
    let p = file_payload(&n.id, None).await.map_err(|e| e.message())?;
    let git = p.git.clone().unwrap_or_default();
    let dirty = idx
        .dirty()
        .get(&n.id)
        .map(|d| !d.is_empty())
        .unwrap_or(false);
    let mut html = format!(
        "<div class=\"sect\">{}</div>",
        stats(&[
            (
                match n.lines {
                    Some(l) if l != 0.0 => fmt_n(l),
                    _ => "—".into(),
                },
                "lines"
            ),
            (
                match n.words {
                    Some(w) if w != 0.0 => fmt_n(w),
                    _ => "—".into(),
                },
                "words"
            ),
            (fmt::size(n.size), "size"),
            (fmt_n(git.count), "commits"),
            (
                git.first
                    .as_ref()
                    .map(|c| fmt::date(c.t))
                    .unwrap_or_else(|| "—".into()),
                "created"
            ),
            (
                git.last
                    .as_ref()
                    .map(|c| fmt::ago(c.t))
                    .unwrap_or_else(|| if dirty {
                        "uncommitted".into()
                    } else {
                        "—".into()
                    }),
                "last change"
            ),
        ])
    );
    if let Some(last) = &git.last {
        html.push_str(&format!(
            "<div class=\"sect\"><div class=\"sh\">Last commit</div><button class=\"hrow\" data-commit=\"{}\" style=\"padding-left:0\"><span class=\"h\">{}</span><span class=\"dt\">{}</span><span class=\"st\"></span><span class=\"sj\">{}</span><span class=\"rt\"><span class=\"plus\">+{}</span> <span class=\"minus\">−{}</span></span></button></div>",
            last.h,
            short(&last.h),
            fmt::dt(last.t),
            esc(&last.s),
            num(last.a),
            num(last.d)
        ));
    }
    if let Some(err) = p.fm_error.as_ref().filter(|e| !e.is_empty()) {
        html.push_str(&format!("<div class=\"sect\"><div class=\"sh\">Frontmatter</div><div class=\"fmerr\">YAML error: {}</div></div>", esc(err)));
    } else if let Some(fm) = p.fm.as_ref().filter(|m| !m.is_empty()) {
        html.push_str(&format!("<div class=\"sect\"><div class=\"sh\">Frontmatter <span class=\"c\">{} keys</span></div>{}</div>", fm.len(), fm_table(idx, Some(fm))));
    }
    if n.kind() == "symlink" {
        let l = match &p.link {
            Some(l) if !l.is_empty() => nchip(idx, l, ""),
            _ => esc(p.link.as_deref().filter(|l| !l.is_empty()).unwrap_or("?")),
        };
        html.push_str(&format!(
            "<div class=\"sect\"><div class=\"sh\">Symlink</div>→ {l}</div>"
        ));
    }
    html.push_str(&connections(idx, &n.id));
    let broken: Vec<&crate::model::Broken> = idx.broken.iter().filter(|b| b.0 == n.id).collect();
    if !broken.is_empty() {
        html.push_str(&format!("<div class=\"sect\"><div class=\"sh\" style=\"color:var(--down)\">Broken links <span class=\"c\">{}</span></div>", broken.len()));
        for b in broken {
            let ln = b.2.map(|x| x.to_string()).unwrap_or_else(|| "null".into());
            html.push_str(&format!(
                "<div class=\"lsimple\" data-id=\"{}\" data-ln=\"{}\"><span>{}</span><span class=\"m\">line {}</span></div>",
                esc(&n.id),
                ln,
                esc(&b.1),
                b.2.filter(|x| *x != 0).map(|x| x.to_string()).unwrap_or_else(|| "?".into())
            ));
        }
        html.push_str("</div>");
    }
    if p.headings.len() > 1 {
        html.push_str(&format!("<div class=\"sect outline\"><div class=\"sh\">Outline <span class=\"c\">{}</span></div>", p.headings.len()));
        for h in &p.headings {
            let text: String =
                h.1.chars()
                    .filter(|c| !matches!(c, '`' | '*' | '_'))
                    .collect();
            html.push_str(&format!(
                "<button data-ln=\"{}\" data-anchor=\"{}\" style=\"padding-left:{}px\">{}</button>",
                num(h.3),
                esc(&h.2),
                num((h.0 - 1.0) * 12.0),
                esc(&text)
            ));
        }
        html.push_str("</div>");
    }
    Ok(html)
}

fn rel(id: &str, x: &str) -> String {
    if id == "." {
        x.to_string()
    } else {
        js::slice16(x, js::len16(id) as i64, None)
    }
}

/// `folderView(n)`
fn folder_view(idx: &Idx, n: &Node) -> String {
    let id = n.id.as_str();
    let info = idx.dinfo(id).cloned().unwrap_or_default();
    let mut desc: Vec<&Node> = Vec::new();
    fn walk<'a>(idx: &'a Idx, x: &str, desc: &mut Vec<&'a Node>) {
        for k in idx.kids_of(x) {
            let c = idx.node(k).unwrap();
            if c.t == "dir" {
                walk(idx, k, desc);
            } else {
                desc.push(c);
            }
        }
    }
    walk(idx, id, &mut desc);
    let lt = fmt::ago(info.lt);
    let mut html = format!(
        "<div class=\"sect\">{}</div>",
        stats(&[
            (fmt_n(info.files), "files"),
            (fmt_n(info.direct), "direct"),
            (fmt_n(info.dirs), "folders"),
            (fmt_n(info.lines), "lines"),
            (if lt.is_empty() { "—".into() } else { lt }, "last change"),
        ])
    );
    let mut types = js::order_keys(&info.types);
    types.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let chips: String = if types.is_empty() {
        "<span style=\"color:var(--dim)\">No typed cards</span>".into()
    } else {
        types.iter().map(|(t, c)| format!("<button class=\"nchip\" data-type=\"{}\"><span>{}</span><small>{}</small></button>", esc(t), esc(t), num(*c))).collect()
    };
    html.push_str(&format!(
        "<div class=\"sect\"><div class=\"sh\">Cards by type</div><div class=\"conn\">{chips}<button class=\"nchip\" data-table=\"{}\" style=\"border-color:var(--accent)\"><span style=\"color:var(--accent-text)\">TABLE ›</span></button>{}</div></div>",
        esc(id),
        if id != "." {
            format!(
                "<button class=\"nchip\" data-collapse=\"{}\"><span>{}</span></button>",
                esc(id),
                if state::is_collapsed(idx, id) { "EXPAND in graph" } else { "COLLAPSE in graph" }
            )
        } else {
            String::new()
        }
    ));
    let (dirs, _) = idx.sorted_kids(id, false);
    if !dirs.is_empty() {
        let chips: String = dirs
            .iter()
            .map(|d| nchip(idx, d, &fmt_n(idx.dinfo(d).map(|x| x.files).unwrap_or(0.0))))
            .collect();
        html.push_str(&format!("<div class=\"sect\"><div class=\"sh\">Subfolders <span class=\"c\">{}</span></div><div class=\"conn\">{chips}</div></div>", dirs.len()));
    }
    let mut recent: Vec<&&Node> = desc.iter().filter(|x| x.lt0() != 0.0).collect();
    recent.sort_by(|a, b| {
        b.lt0()
            .partial_cmp(&a.lt0())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if !recent.is_empty() {
        html.push_str("<div class=\"sect\"><div class=\"sh\">Recently changed</div>");
        for x in recent.iter().take(10) {
            html.push_str(&format!(
                "<div class=\"lsimple\" data-id=\"{}\"><span>{}</span><span class=\"m\">{}</span></div>",
                esc(&x.id),
                esc(&rel(id, &x.id)),
                fmt::ago_text(x.lt0())
            ));
        }
        html.push_str("</div>");
    }
    let mut hubs: Vec<(&Node, usize)> = desc
        .iter()
        .map(|x| {
            (
                *x,
                idx.in_of(&x.id)
                    .iter()
                    .filter(|&&e| idx.edges[e].k != "surface")
                    .count(),
            )
        })
        .filter(|x| x.1 > 0)
        .collect();
    hubs.sort_by(|a, b| b.1.cmp(&a.1));
    if !hubs.is_empty() {
        html.push_str("<div class=\"sect\"><div class=\"sh\">Most linked</div>");
        for (x, c) in hubs.iter().take(10) {
            html.push_str(&format!("<div class=\"lsimple\" data-id=\"{}\"><span>{}</span><span class=\"m\">{c} in</span></div>", esc(&x.id), esc(&rel(id, &x.id))));
        }
        html.push_str("</div>");
    }
    // edges crossing the folder boundary
    let inside: std::collections::HashSet<&str> = desc.iter().map(|x| x.id.as_str()).collect();
    let mut ext: indexmap::IndexMap<String, usize> = indexmap::IndexMap::new();
    for x in &desc {
        for &ei in idx.out_of(&x.id) {
            let t = &idx.edges[ei].t;
            if !inside.contains(t.as_str()) && !(t.starts_with(id) && id != ".") {
                *ext.entry(t.clone()).or_insert(0) += 1;
            }
        }
    }
    if id != "." && !ext.is_empty() {
        let mut v: Vec<(String, usize)> = ext.into_iter().collect();
        let len = v.len();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let chips: String = v
            .iter()
            .take(40)
            .map(|(x, c)| nchip(idx, x, &format!("×{c}")))
            .collect();
        html.push_str(&format!("<div class=\"sect\"><div class=\"sh\">Points outside <span class=\"c\">{len}</span></div><div class=\"conn\">{chips}</div></div>"));
    }
    if id != "." {
        html.push_str(&connections(idx, id));
    }
    html
}

/// `surfaceView(n)`
fn surface_view(idx: &Idx, n: &Node) -> String {
    let mut groups: indexmap::IndexMap<String, Vec<String>> = indexmap::IndexMap::new();
    for &ei in idx.in_of(&n.id) {
        let e = &idx.edges[ei];
        let k = if e.label.is_empty() {
            "linked".to_string()
        } else {
            e.label.clone()
        };
        groups.entry(k).or_default().push(e.s.clone());
    }
    let mut html = format!(
        "<div class=\"sect\">{}</div><div class=\"sect\"><div class=\"sh\">How the KB reaches {}</div>",
        stats(&[
            (fmt_n(idx.in_of(&n.id).len() as f64), "files reach it"),
            (fmt_n(groups.len() as f64), "ways in")
        ]),
        esc(&n.name)
    );
    let mut gs: Vec<(String, Vec<String>)> = groups.into_iter().collect();
    gs.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for (k, ids) in gs {
        let chips: String = ids.iter().take(60).map(|x| nchip(idx, x, "")).collect();
        html.push_str(&format!(
            "<div class=\"cgroup\"><div class=\"cgl\">{} · {}</div><div class=\"conn\">{chips}{}</div></div>",
            esc(&k),
            ids.len(),
            if ids.len() > 60 { format!("<small style=\"color:var(--dim)\">+{} more</small>", ids.len() - 60) } else { String::new() }
        ));
    }
    html + "</div>"
}

/// `dashboard()`
fn dashboard(idx: &Idx) -> String {
    let kb = &idx.kb;
    let files: Vec<&Node> = idx.files().collect();
    let dirty = js::order_keys(&kb.dirty);
    let mut html = format!(
        "<div class=\"sect\">{}</div>",
        stats(&[
            (fmt_n(files.len() as f64), "files"),
            (
                fmt_n(idx.nodes.iter().filter(|n| n.t == "dir").count() as f64),
                "folders"
            ),
            (fmt_n(idx.edges.len() as f64), "edges"),
            (fmt_n(idx.commits), "commits"),
            (fmt_n(idx.broken.len() as f64), "broken links"),
            (fmt_n(dirty.len() as f64), "uncommitted"),
        ])
    );
    html.push_str("<div class=\"sect\"><div class=\"sh\">Families</div>");
    for f in &idx.families {
        let c = files
            .iter()
            .filter(|n| n.family.as_deref() == Some(&f.id))
            .count();
        if c == 0 {
            continue;
        }
        html.push_str(&format!(
            "<div class=\"lsimple\" style=\"cursor:default\"><span><i style=\"display:inline-block;width:8px;height:8px;background:{};margin-right:7px\"></i>{}</span><span class=\"m\">{}</span></div>",
            fam_var(&f.id),
            esc(&f.label),
            fmt_n(c as f64)
        ));
    }
    html.push_str("</div>");
    if !dirty.is_empty() {
        html.push_str(&format!("<div class=\"sect\"><div class=\"sh\" style=\"color:var(--warn)\">Uncommitted <span class=\"c\">{}</span></div>", dirty.len()));
        for (p, c) in dirty.iter().take(12) {
            html.push_str(&format!("<div class=\"lsimple\" data-id=\"{}\"><span>{}</span><span class=\"m\">{}</span></div>", esc(p), esc(p), esc(c)));
        }
        html.push_str("</div>");
    }
    let mut recent: Vec<&&Node> = files.iter().filter(|n| n.lt0() != 0.0).collect();
    recent.sort_by(|a, b| {
        b.lt0()
            .partial_cmp(&a.lt0())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    html.push_str("<div class=\"sect\"><div class=\"sh\">Recently changed</div>");
    for n in recent.iter().take(10) {
        html.push_str(&format!("<div class=\"lsimple\" data-id=\"{}\"><span>{}</span><span class=\"m\">{}</span></div>", esc(&n.id), esc(&n.id), fmt::ago_text(n.lt0())));
    }
    html.push_str("</div>");
    let mut hubs: Vec<(&Node, usize)> = files
        .iter()
        .map(|n| {
            (
                *n,
                idx.in_of(&n.id)
                    .iter()
                    .filter(|&&e| idx.edges[e].k != "surface")
                    .count(),
            )
        })
        .collect();
    hubs.sort_by(|a, b| b.1.cmp(&a.1));
    html.push_str("<div class=\"sect\"><div class=\"sh\">Most linked</div>");
    for (n, c) in hubs.iter().take(8) {
        html.push_str(&format!("<div class=\"lsimple\" data-id=\"{}\"><span>{}</span><span class=\"m\">{c} in</span></div>", esc(&n.id), esc(&n.id)));
    }
    html.push_str("</div>");
    if !idx.broken.is_empty() {
        html.push_str(&format!("<div class=\"sect\"><div class=\"sh\" style=\"color:var(--down)\">Broken links <span class=\"c\">{}</span></div>", idx.broken.len()));
        for b in idx.broken.iter().take(8) {
            html.push_str(&format!(
                "<div class=\"lsimple\" data-id=\"{}\" data-ln=\"{}\"><span>{}</span><span class=\"m\">→ {}</span></div>",
                esc(&b.0),
                b.2.map(|x| x.to_string()).unwrap_or_else(|| "null".into()),
                esc(&b.0),
                esc(&b.1)
            ));
        }
        html.push_str("</div>");
    }
    html.push_str("<div class=\"sect\" style=\"color:var(--dim)\">Select anything in the graph or the tree, or type in the GO bar. <kbd style=\"color:var(--accent-text)\">?</kbd> lists every command and key.</div>");
    html
}

// ---- content ---------------------------------------------------------------------------------

fn index_file(idx: &Idx, n: &Node) -> Option<String> {
    let (_, files) = idx.sorted_kids(&n.id, false);
    let base = |f: &str| f.rsplit('/').next().unwrap_or(f).to_ascii_lowercase();
    files
        .iter()
        .find(|f| base(f) == "index.md")
        .or_else(|| files.iter().find(|f| base(f) == "readme.md"))
        .or_else(|| {
            if n.id == "." {
                files
                    .iter()
                    .find(|f| f.to_ascii_lowercase().ends_with("agents.md"))
            } else {
                None
            }
        })
        .cloned()
}

/// `content(n)`
async fn content(idx: &Idx, n: &Node) -> Result<String, String> {
    if n.t == "surface" {
        return Ok(surface_view(idx, n));
    }
    if n.t == "dir" || n.t == "root" {
        let Some(f) = index_file(idx, n) else {
            return Ok(folder_view(idx, n));
        };
        let p = file_payload(&f, None).await.map_err(|e| e.message())?;
        return Ok(format!(
            "<div class=\"ctool\"><span class=\"info\">showing {}</span></div>{}",
            nchip(idx, &f, ""),
            md_block(&p, &f)
        ));
    }
    if n.kind() == "symlink" {
        let l = n.link.clone().unwrap_or_default();
        let n2 = idx.node(&l);
        return Ok(format!(
            "<div class=\"card\"><div class=\"big\">Symlink → {}</div><div style=\"color:var(--dim)\">Git tracks this as a link; it has no content of its own.</div><div class=\"acts\">{}</div></div>",
            esc(if l.is_empty() { "?" } else { &l }),
            if n2.is_some() {
                format!(
                    "<button class=\"btn\" data-go=\"{}\">OPEN {}</button>",
                    esc(&l),
                    esc(&l)
                )
            } else {
                String::new()
            }
        ));
    }
    let (rev, mode, blame) = {
        let st = s();
        (
            st.rev.clone().filter(|r| !r.is_empty()),
            st.mode.clone(),
            st.blame,
        )
    };
    let p = match file_payload(&n.id, rev.as_deref()).await {
        Ok(p) => p,
        Err(e) => return Ok(format!("<div class=\"empty\">{}</div>", esc(&e.message()))),
    };
    let tracked = n.c.map(|c| c != 0.0).unwrap_or(false);
    let textual = ["markdown", "code", "text", "html"].contains(&p.kind.as_str());
    let modes = if p.kind == "markdown" || p.kind == "html" || p.table.is_some() {
        format!(
            "<span class=\"seg\"><button data-mode=\"rendered\" class=\"{}\">RENDERED</button><button data-mode=\"source\" class=\"{}\">SOURCE</button></span>",
            if mode == "rendered" { "on" } else { "" },
            if mode == "source" { "on" } else { "" }
        )
    } else {
        String::new()
    };
    let blame_btn = if textual && (tracked || rev.is_some()) {
        format!(
            "<span class=\"seg\"><button data-blame=\"1\" class=\"{}\" title=\"Line ages from git blame (B)\">BLAME</button></span>{}",
            if blame { "on" } else { "" },
            if blame {
                "<span class=\"bleg\" id=\"bleg\"></span>"
            } else {
                ""
            }
        )
    } else {
        String::new()
    };
    let info: Vec<String> = [
        p.lines
            .filter(|l| *l != 0.0)
            .map(|l| format!("{} lines", fmt_n(l)))
            .unwrap_or_default(),
        fmt::size(p.size),
        p.kind.clone(),
        if p.truncated {
            "truncated".into()
        } else {
            String::new()
        },
    ]
    .into_iter()
    .filter(|x| !x.is_empty())
    .collect();
    let mut html = format!(
        "<div class=\"ctool\">{modes}{blame_btn}<span class=\"info\">{}</span></div>",
        esc(&info.join(" · "))
    );
    if let Some(r) = &p.rev {
        let hist = path_history(&n.id)
            .await
            .map(|h| (*h).clone())
            .unwrap_or_default();
        let i = hist.iter().position(|c| c.h == r.h);
        let older = match i {
            Some(i) => hist.get(i + 1),
            None => hist.iter().find(|c| c.t < r.t),
        };
        let newer = match i {
            Some(i) if i > 0 => hist.get(i - 1),
            Some(_) => None,
            None => hist.iter().rev().find(|c| c.t > r.t),
        };
        s_mut().travel_nav = Some((older.map(|c| c.h.clone()), newer.map(|c| c.h.clone())));
        let step = |c: Option<&Commit>, label: &str, key: &str| match c {
            Some(c) => format!(
                "<button data-at=\"{}\" title=\"{}\">{label}</button>",
                c.h,
                esc(&format!(
                    "{key} · {} · {} · {}",
                    short(&c.h),
                    fmt::dt(c.t),
                    c.s
                ))
            ),
            None => format!("<button disabled>{label}</button>"),
        };
        html.push_str(&format!(
            "<div class=\"travel\">{}<span class=\"tt\" title=\"{}\">⏱ <b>{}</b> · {}{} · {}</span>{}<button data-live=\"1\" class=\"lv\" title=\"Back to the working tree (Esc)\">LIVE</button></div>",
            step(older, "◂ OLDER", "Previous change to this file (,)"),
            esc(&r.s),
            short(&r.h),
            esc(&fmt::dt(r.t)),
            match &p.at_path {
                Some(a) if !a.is_empty() => format!(" · <i>was {}</i>", esc(a)),
                _ => String::new(),
            },
            esc(&r.s),
            if newer.is_some() { step(newer, "NEWER ▸", "Next change to this file (.)") } else { "<button data-live=\"1\" title=\"Next: the working tree (.)\">NEWER ▸</button>".into() }
        ));
    } else {
        s_mut().travel_nav = None;
    }
    let rev_q = rev
        .as_ref()
        .map(|r| format!("?rev={}", js::encode_uri_component(r)))
        .unwrap_or_default();
    let body = match p.kind.as_str() {
        "markdown" => {
            if mode == "source" {
                src_block(p.source.as_deref())
            } else {
                md_block(&p, &n.id)
            }
        }
        "html" => {
            if mode == "source" {
                src_block(p.source.as_deref())
            } else {
                format!(
                    "<iframe class=\"frame\" sandbox=\"allow-scripts allow-popups\" src=\"{}{rev_q}\" title=\"{}\"></iframe>",
                    esc(&p.raw),
                    esc(&n.name)
                )
            }
        }
        _ if p.table.is_some() && mode != "source" => {
            data_table(p.table.as_ref().unwrap(), s().data_sort)
        }
        "code" | "text" => src_block(p.source.as_deref()),
        "pdf" => format!(
            "<iframe class=\"frame\" src=\"{}{rev_q}#view=FitH\" title=\"{}\"></iframe>",
            esc(&p.raw),
            esc(&n.name)
        ),
        "image" => format!(
            "<div class=\"media\"><img src=\"{}{rev_q}\" alt=\"{}\"></div>",
            esc(&p.raw),
            esc(&n.name)
        ),
        "audio" => format!(
            "<div class=\"media\"><audio controls src=\"{}\"></audio></div>",
            esc(&p.raw)
        ),
        "video" => format!(
            "<div class=\"media\"><video controls src=\"{}\"></video></div>",
            esc(&p.raw)
        ),
        _ => {
            let filer = boot().filer();
            let filer_up = filer.strip_prefix("the ").unwrap_or(filer).to_uppercase();
            format!(
                "<div class=\"card\"><div class=\"big\">{}</div><div style=\"color:var(--dim)\">{} · {}. The browser can't show this format; open it in its app.</div><div class=\"acts\"><button class=\"btn\" data-open=\"app\">OPEN IN DEFAULT APP</button><button class=\"btn\" data-open=\"reveal\">REVEAL IN {}</button></div></div>",
                esc(&n.name),
                esc(kind_name(&ext_of(&n.name))
                    .map(String::from)
                    .or_else(|| p.mime.clone().filter(|m| !m.is_empty()))
                    .as_deref()
                    .unwrap_or("binary")),
                fmt::size(p.size),
                filer_up
            )
        }
    };
    html.push_str(&body);
    Ok(html)
}

fn parse_num_cell(v: &str) -> Option<f64> {
    let t: String = v
        .chars()
        .filter(|c| !matches!(c, '$' | '€' | '£' | ',' | '%') && !js::is_js_space(*c))
        .collect();
    let x = js::parse_float(&t);
    if x.is_nan() { None } else { Some(x) }
}

/// `dataTable(T, sort)`: CSV/TSV as a sortable grid.
fn data_table(t: &DataTable, sort: Option<(usize, i32)>) -> String {
    let mut rows: Vec<(usize, &Vec<String>)> = t.rows.iter().enumerate().collect();
    let empty = String::new();
    let cell = |r: &Vec<String>, k: usize| -> String { r.get(k).cloned().unwrap_or_default() };
    if let Some((k, dir)) = sort.filter(|s| s.0 < t.header.len()) {
        let is_num = t.numeric.get(k).copied().unwrap_or(false);
        if is_num {
            rows.sort_by(|a, b| {
                let (p, q) = (parse_num_cell(&cell(a.1, k)), parse_num_cell(&cell(b.1, k)));
                let c = match (p, q) {
                    (None, None) => 0.0,
                    (None, _) => 1.0,
                    (_, None) => -1.0,
                    (Some(p), Some(q)) => (p - q) * dir as f64,
                };
                c.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            let texts: Vec<String> = rows.iter().map(|r| cell(r.1, k)).collect();
            let ranks = js::collation_ranks(texts.iter().map(|s| s.as_str()), true);
            rows.sort_by(|a, b| {
                let ra = ranks[&cell(a.1, k)] as i64;
                let rb = ranks[&cell(b.1, k)] as i64;
                let c = (ra - rb).signum() * dir as i64;
                if c != 0 { c.cmp(&0) } else { a.0.cmp(&b.0) }
            });
        }
    }
    let arrow = |j: usize| match sort {
        Some((k, d)) if k == j => {
            if d > 0 {
                " ▲"
            } else {
                " ▼"
            }
        }
        _ => "",
    };
    let mut html =
        String::from("<div class=\"dtable tgrid\"><table><thead><tr><th class=\"num rn\">#</th>");
    for (j, h) in t.header.iter().enumerate() {
        let isn = t.numeric.get(j).copied().unwrap_or(false);
        html.push_str(&format!(
            "<th data-dk=\"{j}\" class=\"{}{}\">{}{}</th>",
            if isn { "num" } else { "" },
            if sort.map(|s| s.0) == Some(j) {
                " sorted"
            } else {
                ""
            },
            esc(if h.is_empty() { "—" } else { h }),
            arrow(j)
        ));
    }
    html.push_str("</tr></thead><tbody>");
    for (i, r) in rows {
        html.push_str(&format!("<tr><td class=\"num rn\">{}</td>", i + 1));
        for j in 0..t.header.len() {
            let v = r.get(j).unwrap_or(&empty);
            let isn = t.numeric.get(j).copied().unwrap_or(false);
            html.push_str(&format!(
                "<td class=\"{}{}\"{}>{}</td>",
                if isn { "num" } else { "" },
                if v.is_empty() { " nil" } else { "" },
                if js::len16(v) > 40 {
                    format!(" title=\"{}\"", esc(v))
                } else {
                    String::new()
                },
                if v.is_empty() {
                    "·".to_string()
                } else {
                    esc(v)
                }
            ));
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table>");
    if t.truncated {
        html.push_str(&format!("<div class=\"empty\">Showing the first {} rows. SOURCE has the rest up to the size limit.</div>", fmt_n(t.rows.len() as f64)));
    }
    html + "</div>"
}

/// Short markdown table cells stay on one line (`nw`): a single word, number or identifier of up
/// to 24 characters, or any text of up to 12. A column of labels or numbers is then as wide as its
/// widest value, and the long-text columns share the rest; a longer phrase may still wrap between
/// words, so a tight table can narrow it instead of scrolling. Runs before any scrolling, since it
/// changes the height of the page.
fn size_md_tables(root: &Element) {
    for cell in dom::qa_in(root, "article.md table th, article.md table td") {
        if short_cell(&dom::text(&cell)) {
            dom::add_class(&cell, "nw");
        }
    }
}

fn short_cell(text: &str) -> bool {
    let t = text.trim();
    let n = t.chars().count();
    n <= 12 || (n <= 24 && !t.contains(char::is_whitespace))
}

/// `mdBlock(p, path)`
fn md_block(p: &FilePayload, path: &str) -> String {
    let reader = cfg().reader_font();
    let props = match p.fm.as_ref().filter(|m| !m.is_empty()) {
        Some(fm) => {
            let idx = state::idx().unwrap();
            format!(
                "<div class=\"props\"><div class=\"sh\">Properties</div>{}</div>",
                fm_table(&idx, Some(fm))
            )
        }
        None => String::new(),
    };
    format!(
        "{props}<article class=\"md {reader}{}\" data-path=\"{}\">{}</article>",
        if s().blame { " blame" } else { "" },
        esc(path),
        p.html.as_deref().unwrap_or("")
    )
}

/// `srcBlock(lines)`
fn src_block(lines: Option<&[String]>) -> String {
    let Some(lines) = lines else {
        return String::new();
    };
    let mut html = String::with_capacity(lines.iter().map(|l| l.len() + 110).sum::<usize>() + 32);
    html.push_str(&format!(
        "<div class=\"src{}\">",
        if s().blame { " blame" } else { "" }
    ));
    for (i, l) in lines.iter().enumerate() {
        let n = i + 1;
        html.push_str(&format!(
            "<div class=\"l\" data-n=\"{n}\"><span class=\"bl\"></span><span class=\"no\">{n}</span><span class=\"tx\">{}</span></div>",
            if l.is_empty() { " " } else { l }
        ));
    }
    html + "</div>"
}

fn is_wt(h: &str) -> bool {
    h.starts_with("0000000")
}

/// `applyBlame(n)`
async fn apply_blame(n: &Node) {
    if !s().blame || n.t != "file" {
        return;
    }
    let tok = token();
    let rev = s().rev.clone().filter(|r| !r.is_empty());
    let url = format!(
        "/api/blame?path={}{}",
        js::encode_uri_component(&n.id),
        rev.map(|r| format!("&rev={}", js::encode_uri_component(&r)))
            .unwrap_or_default()
    );
    let b: Blame = match api::get(&url).await {
        Ok(b) => b,
        Err(e) => {
            toast("BLAME", &e.message(), true);
            return;
        }
    };
    if tok != token() {
        return;
    }
    let times: Vec<f64> = b
        .commits
        .iter()
        .filter(|c| !is_wt(&c.h))
        .map(|c| c.t)
        .filter(|t| *t != 0.0)
        .collect();
    let t0 = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let t1 = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let (t0, t1) = if times.is_empty() {
        (0.0, 0.0)
    } else {
        (t0, t1)
    };
    let age = |t: f64| -> f64 {
        if t != 0.0 {
            (20.0 + 80.0 * if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 } + 0.5).floor()
        } else {
            100.0
        }
    };
    let color = |c: &crate::model::BlameCommit| {
        if is_wt(&c.h) {
            "var(--warn)".to_string()
        } else {
            format!(
                "color-mix(in oklab, var(--accent) {}%, var(--faint))",
                num(age(c.t))
            )
        }
    };
    if let Some(src) = dom::q("#ibody .src") {
        dom::add_class(&src, "blame");
        let rows = src.children();
        let mut prev: i64 = -1;
        let n_rows = rows.length() as usize;
        for i in 0..n_rows.min(b.lines.len()) {
            let li = b.lines[i];
            let Some(c) = b.commits.get(li) else { continue };
            let Some(row) = rows.item(i as u32) else {
                continue;
            };
            let Some(bl) = dom::q_in(&row, ".bl") else {
                continue;
            };
            let summary: String = js::slice16(&c.s, 0, Some(26));
            dom::set_html(
                &bl,
                &format!(
                    "<i style=\"background:{}\"></i><b>{}</b><span>{} {}</span>",
                    color(c),
                    if is_wt(&c.h) {
                        "working".to_string()
                    } else {
                        short(&c.h)
                    },
                    if c.t != 0.0 {
                        fmt::date(c.t)
                    } else {
                        String::new()
                    },
                    esc(&summary)
                ),
            );
            let _ = bl.set_attribute(
                "title",
                &format!("{} · {} · {}", short(&c.h), fmt::dt(c.t), c.s),
            );
            let _ = bl.set_attribute("data-commit", if is_wt(&c.h) { "" } else { &c.h });
            dom::toggle(&bl, "cont", li as i64 == prev);
            prev = li as i64;
        }
    }
    if let Some(leg) = dom::try_id("bleg") {
        dom::set_html(
            &leg,
            &format!(
                "{}<i class=\"ramp\"></i>{}{}",
                esc(&fmt::date(t0)),
                esc(&fmt::date(t1)),
                if b.commits.iter().any(|c| is_wt(&c.h)) {
                    "<i style=\"background:var(--warn)\"></i>unsaved"
                } else {
                    ""
                }
            ),
        );
    }
    if let Some(md) = dom::q("#ibody article.md") {
        dom::add_class(&md, "blame");
        let els = dom::qa_in(&md, "[data-line]");
        let mut marks: Vec<(Element, Option<Element>, crate::model::BlameCommit)> = Vec::new();
        for (i, el) in els.iter().enumerate() {
            let ln = js::parse_float(&dom::data(el, "line").unwrap_or_default()) as i64;
            let nx = els.get(i + 1);
            let end = match nx {
                Some(nx) => {
                    (ln + 1).max(js::parse_float(&dom::data(nx, "line").unwrap_or_default()) as i64)
                }
                None => b.lines.len() as i64 + 1,
            };
            let mut best: Option<&crate::model::BlameCommit> = None;
            let mut j = ln - 1;
            while j < end - 1 && j < b.lines.len() as i64 {
                if j >= 0 {
                    if let Some(c) = b.commits.get(b.lines[j as usize]) {
                        let take = match best {
                            None => true,
                            Some(bb) => is_wt(&c.h) || (!is_wt(&bb.h) && c.t > bb.t),
                        };
                        if take {
                            best = Some(c);
                        }
                    }
                }
                j += 1;
            }
            if let Some(bb) = best {
                let child = nx.filter(|nx| el.contains(Some(nx))).cloned();
                marks.push((el.clone(), child, bb.clone()));
            }
        }
        let colors: Vec<String> = marks.iter().map(|m| color(&m.2)).collect();
        blame_bars(&md, marks, colors);
    }
}

thread_local! {
    static BLAME_RO: std::cell::RefCell<Option<web_sys::ResizeObserver>> = const { std::cell::RefCell::new(None) };
}

/// `blameBars(md, marks, color)`
fn blame_bars(
    md: &Element,
    marks: Vec<(Element, Option<Element>, crate::model::BlameCommit)>,
    colors: Vec<String>,
) {
    let g = match dom::q_in(md, ":scope > .bgut") {
        Some(g) => g,
        None => {
            let g = dom::doc().create_element("div").unwrap();
            g.set_class_name("bgut");
            let _ = md.append_child(&g);
            g
        }
    };
    let md2 = md.clone();
    let marks = Rc::new(marks);
    let colors = Rc::new(colors);
    let draw = Rc::new(move || {
        if !md2.is_connected() {
            BLAME_RO.with(|r| {
                if let Some(ro) = r.borrow().as_ref() {
                    ro.disconnect();
                }
            });
            return;
        }
        let top0 = md2.get_bounding_client_rect().top();
        let mut html = String::new();
        for ((el, child, c), col) in marks.iter().zip(colors.iter()) {
            let r = el.get_bounding_client_rect();
            let h = match child {
                Some(ch) => ch.get_bounding_client_rect().top(),
                None => r.bottom(),
            } - r.top();
            if h < 3.0 {
                continue;
            }
            let wt = is_wt(&c.h);
            let title = if wt {
                "not committed yet".to_string()
            } else {
                format!("{} · {} · {}", short(&c.h), fmt::dt(c.t), c.s)
            };
            html.push_str(&format!(
                "<i data-commit=\"{}\" title=\"{}\" style=\"top:{}px;height:{}px;background:{}\"></i>",
                if wt { "" } else { &c.h },
                esc(&title),
                js::to_fixed(r.top() - top0, 1),
                js::to_fixed(h, 1),
                col
            ));
        }
        dom::set_html(&g, &html);
    });
    draw();
    BLAME_RO.with(|r| {
        if let Some(ro) = r.borrow().as_ref() {
            ro.disconnect();
        }
    });
    static DEB: dom::Debounce = dom::Debounce::new(80);
    let d2 = draw.clone();
    let cb = wasm_bindgen::closure::Closure::<dyn FnMut(js_sys::Array)>::new(
        move |_e: js_sys::Array| {
            let d3 = d2.clone();
            DEB.call(move || d3());
        },
    );
    if let Ok(ro) = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref()) {
        ro.observe(md);
        BLAME_RO.with(|r| *r.borrow_mut() = Some(ro));
    }
    cb.forget();
}

/// `markText(root, query)`: highlight FIND matches in the open file.
fn mark_text(root: &Element, query: &str) -> usize {
    if query.is_empty() {
        return 0;
    }
    let pattern = match views::list::regex_query(query) {
        Some(inner) => inner,
        None => {
            let mut esc_q = String::new();
            for c in query.chars() {
                if ".*+?^${}()|[]\\".contains(c) {
                    esc_q.push('\\');
                }
                esc_q.push(c);
            }
            esc_q
        }
    };
    // `new RegExp(...)` may throw: no marks then
    let re = js::regexp(&pattern, "gi");
    let Some(re) = re else { return 0 };
    let skip = ".bl, .no, .bgut, mark, .props .sh";
    let mut nodes: Vec<web_sys::Node> = Vec::new();
    fn walk(n: &web_sys::Node, skip: &str, out: &mut Vec<web_sys::Node>) {
        let mut c = n.first_child();
        while let Some(ch) = c {
            if ch.node_type() == web_sys::Node::TEXT_NODE {
                let rejected = ch
                    .parent_element()
                    .and_then(|p| p.closest(skip).ok().flatten())
                    .is_some();
                if !rejected {
                    out.push(ch.clone());
                }
            } else if ch.node_type() == web_sys::Node::ELEMENT_NODE {
                walk(&ch, skip, out);
            }
            c = ch.next_sibling();
        }
    }
    walk(root.as_ref(), skip, &mut nodes);
    let doc = dom::doc();
    let mut count = 0usize;
    for tn in nodes {
        let s = js_sys::JsString::from(tn.node_value().unwrap_or_default());
        re.set_last_index(0);
        if !re.test(&String::from(&s)) {
            continue;
        }
        re.set_last_index(0);
        let frag = doc.create_document_fragment();
        let mut last: u32 = 0;
        while count < 3000 {
            let Some(m) = re.exec(&String::from(&s)) else {
                break;
            };
            let m0: String = m.get(0).as_string().unwrap_or_default();
            if m0.is_empty() {
                re.set_last_index(re.last_index() + 1);
                continue;
            }
            let index = js_sys::Reflect::get(&m, &"index".into())
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as u32;
            let _ = frag.append_with_str_1(&String::from(s.slice(last, index)));
            let mk = doc.create_element("mark").unwrap();
            mk.set_class_name("fm");
            mk.set_text_content(Some(&m0));
            let _ = frag.append_with_node_1(&mk);
            last = index + js::len16(&m0) as u32;
            count += 1;
        }
        let _ = frag.append_with_str_1(&String::from(s.slice(last, s.length())));
        if let Some(p) = tn.parent_node() {
            let _ = p.replace_child(&frag, &tn);
        }
    }
    count
}

/// `afterInspector(n)`
fn after(n: Option<&Node>) {
    let body = dom::id("ibody");
    let (tab, mark, jump, keep) = {
        let st = s();
        (st.tab.clone(), st.mark.clone(), st.jump, st.keep)
    };
    if tab == "content" {
        if let (Some(m), Some(n)) = (&mark, n) {
            if n.t == "file" {
                if let Some(root) =
                    dom::q_in(&body, ".src").or_else(|| dom::q_in(&body, "article.md"))
                {
                    mark_text(&root, m);
                }
            }
        }
    }
    if let Some(ln) = jump {
        s_mut().jump = None;
        let find_cur = s_mut().find_cur.take();
        if let Some(el) = line_el(&body, ln) {
            dom::add_class(
                &el,
                if dom::has_class(&el, "l") {
                    "hl"
                } else {
                    "jump"
                },
            );
            dom::scroll_into_view_block(&el, web_sys::ScrollLogicalPosition::Center, false);
            if let Some(d) = find_cur {
                let marks = dom::qa_in(&el, "mark.fm");
                if let Some(m) = if d < 0 { marks.last() } else { marks.first() } {
                    dom::add_class(m, "cur");
                }
            }
        }
    } else if let Some(ln) = keep {
        s_mut().keep = None;
        if let Some(el) = line_el(&body, ln) {
            scroll_to_el(&body, &el);
        }
    }
    if tab == "content" {
        if let Some(n) = n.cloned() {
            spawn(async move { apply_blame(&n).await });
        }
    }
}

// ---- history ---------------------------------------------------------------------------------

/// `historyView(n)`
async fn history_view(idx: &Idx, n: Option<&Node>) -> Result<String, String> {
    let Some(n) = n.filter(|n| n.t != "root") else {
        let data = views::log::ensure(false).await.map_err(|e| e.message())?;
        let mut html = format!(
            "<div class=\"sect\"><div class=\"sh\">Repository history <span class=\"c\">{} commits</span></div></div>",
            fmt_n(data.total)
        );
        for c in data.commits.iter().take(300) {
            html.push_str(&hrow(c, None));
        }
        if data.total > 300.0 {
            html.push_str("<div class=\"empty\"><button class=\"btn\" data-view=\"log\">OPEN THE FULL LOG</button></div>");
        }
        return Ok(html);
    };
    if n.t == "surface" {
        return Ok("<div class=\"empty\">External systems have no git history.</div>".into());
    }
    let commits = path_history(&n.id).await.map_err(|e| e.message())?;
    if commits.is_empty() {
        let untracked = idx.dirty().get(&n.id).map(|d| d == "?").unwrap_or(false);
        return Ok(format!(
            "<div class=\"empty\">{}</div>",
            if untracked {
                "Untracked: this file has never been committed."
            } else {
                "No history."
            }
        ));
    }
    let spark = sparkline(&commits);
    let mut html = format!(
        "<div class=\"sect\"><div class=\"sh\">{} <span class=\"c\">{}</span></div>{spark}</div>",
        if n.t == "dir" {
            "Commits touching this folder"
        } else {
            "Commits on this file"
        },
        commits.len()
    );
    for c in commits.iter() {
        html.push_str(&hrow(c, Some(n)));
    }
    Ok(html)
}

/// `sparkline(commits)`
fn sparkline(commits: &[Commit]) -> String {
    if commits.len() < 2 {
        return String::new();
    }
    let t1 = commits
        .iter()
        .map(|c| c.t)
        .fold(f64::NEG_INFINITY, f64::max);
    let t0 = commits.iter().map(|c| c.t).fold(f64::INFINITY, f64::min);
    let (w, h, bins) = (400.0, 34.0, 60usize);
    let mut counts = vec![0.0f64; bins];
    for c in commits {
        let b = (((c.t - t0) / (t1 - t0).max(1.0)) * bins as f64).floor() as usize;
        counts[b.min(bins - 1)] += 1.0;
    }
    let max = counts.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut rects = String::new();
    for (i, v) in counts.iter().enumerate() {
        if *v == 0.0 {
            continue;
        }
        rects.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
            num(i as f64 * w / bins as f64 + 0.5),
            num(h - h * v / max),
            num(w / bins as f64 - 1.0),
            num(h * v / max)
        ));
    }
    format!(
        "<svg class=\"spark\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"none\">{rects}</svg><div style=\"display:flex;justify-content:space-between;color:var(--dim);font-size:10.5px\"><span>{}</span><span>{}</span></div>",
        num(w),
        num(h),
        fmt::date(t0),
        fmt::date(t1)
    )
}

/// `hrow(c, n)`
fn hrow(c: &Commit, n: Option<&Node>) -> String {
    let is_file = n.map(|n| n.t == "file").unwrap_or(false);
    let st = c.st.clone().unwrap_or_default();
    let at = if is_file && st != "D" {
        format!(
            "<button data-at=\"{}\" data-atpath=\"{}\" title=\"View the file as of this commit\">@</button>",
            c.h,
            esc(c
                .path
                .as_deref()
                .filter(|p| !p.is_empty())
                .unwrap_or(&n.unwrap().id))
        )
    } else {
        String::new()
    };
    let px = match (&c.path, n) {
        (Some(p), Some(n)) if is_file && !p.is_empty() && *p != n.id => {
            format!(" <span class=\"px\">({})</span>", esc(p))
        }
        _ => String::new(),
    };
    format!(
        "<div class=\"hrow\" data-commit=\"{}\"><span class=\"h\">{}</span><span class=\"dt\">{}</span><span class=\"st\">{}</span><span class=\"sj\" title=\"{}\">{}{px}</span><span class=\"rt\"><span class=\"plus\">+{}</span> <span class=\"minus\">−{}</span>{at}</span></div>",
        c.h,
        short(&c.h),
        fmt::dt(c.t),
        esc(&st),
        esc(&c.s),
        esc(&c.s),
        fmt_n(c.a),
        fmt_n(c.d)
    )
}

// ---- commit detail ---------------------------------------------------------------------------

/// `openCommit(h)`
pub fn open_commit(h: &str) {
    s_mut().commit = Some(h.to_string());
    if s().view == "log" {
        for r in dom::qa("#ltable .crow") {
            let on = dom::data(&r, "h").as_deref() == Some(h);
            dom::toggle(&r, "on", on);
        }
    }
    render();
    nav::nav(true);
}

/// `renderCommit(h, tok)`
async fn render_commit(h: &str, tok: u64) {
    let head = dom::id("ihead");
    let body = dom::id("ibody");
    dom::set_html(
        &head,
        &format!(
            "<div class=\"ipath\"><button class=\"back\" data-back=\"1\">‹ back</button> · commit</div><div class=\"ititle\">{}</div>",
            short(h)
        ),
    );
    let c: CommitDetail =
        match api::get(&format!("/api/commit/{}", js::encode_uri_component(h))).await {
            Ok(c) => c,
            Err(e) => {
                dom::set_html(
                    &body,
                    &format!("<div class=\"empty\">{}</div>", esc(&e.message())),
                );
                return;
            }
        };
    if tok != token() {
        return;
    }
    let parents = if c.p.is_empty() {
        "<span class=\"badge\">root commit</span>".to_string()
    } else {
        c.p.iter().map(|p| format!("<button class=\"badge\" data-commit=\"{p}\" title=\"parent\">parent {}</button>", short(p))).collect()
    };
    dom::set_html(
        &head,
        &format!(
            "<div class=\"ipath\"><button class=\"back\" data-back=\"1\">‹ back</button> · {} · {}</div><div class=\"ititle\" title=\"{}\">{} {}</div><div class=\"ibadges\"><span class=\"badge\">{} files</span><span class=\"badge\"><span class=\"plus\">+{}</span> <span class=\"minus\">−{}</span></span>{parents}<div class=\"iacts\"><button class=\"act\" data-copysha=\"{}\">COPY SHA</button></div></div>",
            esc(&fmt::dt(c.t)),
            esc(&c.an),
            esc(&c.s),
            short(&c.h),
            esc(&match &c.x {
                Some(x) if !x.is_empty() => format!("[{x}]"),
                _ => String::new(),
            }),
            c.files.len(),
            fmt_n(c.a),
            fmt_n(c.d),
            c.h
        ),
    );
    let mut html = format!(
        "<div class=\"sect cmeta\"><div class=\"subj\">{}</div>{}</div>",
        esc(&c.s),
        if c.body.is_empty() {
            String::new()
        } else {
            format!("<div class=\"body\">{}</div>", esc(&c.body))
        }
    );
    let many = c.files.len() > 12;
    let sel = state::sel();
    let mut files: Vec<&crate::model::CommitFile> = c.files.iter().collect();
    let hit = |f: &crate::model::CommitFile| {
        sel.as_deref()
            .map(|s| f.path == s || f.old.as_deref() == Some(s))
            .unwrap_or(false)
    };
    files.sort_by(|a, b| (hit(b) as i32).cmp(&(hit(a) as i32)));
    for (i, f) in files.iter().enumerate() {
        let exists = c.exists.get(&f.path).copied().unwrap_or(false);
        let open = !many || i < 4 || sel.as_deref() == Some(f.path.as_str());
        html.push_str(&format!(
            "<div class=\"cfile{}\"><div class=\"cfh\" data-toggle=\"1\"><span class=\"st\">{}</span><span class=\"p\">{}{}</span><span class=\"ad\"><span class=\"plus\">+{}</span> <span class=\"minus\">−{}</span></span><span>{}{}</span></div>",
            if open { " open" } else { "" },
            esc(&f.st),
            esc(&f.path),
            match &f.old {
                Some(o) if !o.is_empty() => format!(" <small>← {}</small>", esc(o)),
                _ => String::new(),
            },
            num(f.a),
            num(f.d),
            if exists { format!("<button class=\"go2\" data-go=\"{}\" title=\"Select\">›</button>", esc(&f.path)) } else { String::new() },
            if f.st != "D" { format!(" <button class=\"go2\" data-at=\"{}\" data-atpath=\"{}\" title=\"View at this commit\">@</button>", c.h, esc(&f.path)) } else { String::new() }
        ));
        html.push_str(&format!(
            "<div class=\"diff\"{}>",
            if open { "" } else { " hidden" }
        ));
        if f.binary {
            html.push_str("<div class=\"more\">Binary file</div>");
        } else {
            for hk in &f.hunks {
                html.push_str(&format!("<div class=\"hh\">{}</div>", esc(&hk.h)));
                for l in &hk.lines {
                    let cls = match l.0.as_str() {
                        "+" => "p",
                        "-" => "m",
                        _ => "",
                    };
                    let numv = |v: &Value| match v {
                        Value::Null => String::new(),
                        v => js::string(v),
                    };
                    let text = if l.0 == " " {
                        format!(" {}", l.3)
                    } else {
                        format!("{}{}", l.0, l.3)
                    };
                    html.push_str(&format!("<div class=\"dl {cls}\"><span>{}</span><span>{}</span><span>{}</span></div>", numv(&l.1), numv(&l.2), esc(&text)));
                }
            }
        }
        if f.truncated {
            html.push_str("<div class=\"more\">… diff truncated</div>");
        }
        html.push_str("</div></div>");
    }
    dom::set_html(&body, &html);
    body.set_scroll_top(0);
}
