//! RESULTS: FIND hits and the path lists (BROKEN, DIRTY, RECENT, ORPHANS, HUBS, TYPE).

use std::rc::Rc;

use web_sys::MouseEvent;

use crate::api;
use crate::chrome::toast;
use crate::dom;
use crate::fmt;
use crate::js::{self, esc, fmt_n, num};
use crate::model::SearchResult;
use crate::nav::{self, SelOpts};
use crate::state::{self, ListData, ListItem, s, s_mut};

/// `showList(list)`
pub fn show(list: ListData) {
    {
        let mut st = s_mut();
        st.list_query = match &list {
            ListData::Find { query, .. } if !query.is_empty() => Some(query.clone()),
            _ => None,
        };
        st.list = Some(Rc::new(list));
    }
    nav::set_view("list", false);
}

/// `renderList()`
pub fn render() {
    let el = dom::id("list");
    let Some(l) = s().list.clone() else {
        dom::set_html(
            &el,
            "<div class=\"empty\">Run FIND, BROKEN, DIRTY, RECENT, ORPHANS, HUBS, or TYPE from the command line.</div>",
        );
        dom::set_text(&dom::id("listtitle"), "RESULTS");
        dom::set_text(&dom::id("listmeta"), "");
        return;
    };
    let idx = state::idx();
    match &*l {
        ListData::Find {
            title,
            meta,
            files,
            names,
            ..
        } => {
            dom::set_text(&dom::id("listtitle"), title);
            dom::set_text(&dom::id("listmeta"), meta);
            let mut html = String::new();
            if !names.is_empty() {
                html.push_str(&format!(
                    "<div class=\"lgroup\"><div class=\"lgh\"><span class=\"p\" style=\"color:var(--accent-text)\">NAME MATCHES</span><span class=\"c\">{}</span></div>",
                    names.len()
                ));
                for p in names {
                    let t = idx
                        .as_ref()
                        .and_then(|i| i.node(p))
                        .map(|n| n.title_s().to_string())
                        .unwrap_or_default();
                    html.push_str(&format!("<div class=\"lsimple\" data-id=\"{}\"><span>{}</span><span class=\"m\">{}</span></div>", esc(p), esc(p), esc(&t)));
                }
                html.push_str("</div>");
            }
            for f in files {
                html.push_str(&format!(
                    "<div class=\"lgroup\"><div class=\"lgh\" data-id=\"{}\"><span class=\"p\">{}</span><span class=\"c\">{} line{}</span></div>",
                    esc(&f.path),
                    esc(&f.path),
                    num(f.n),
                    if f.n == 1.0 { "" } else { "s" }
                ));
                for (ln, snip) in &f.matches {
                    html.push_str(&format!(
                        "<div class=\"lrow\" data-id=\"{}\" data-ln=\"{}\"><span class=\"ln\">{}</span><span class=\"tx\">{}</span></div>",
                        esc(&f.path),
                        num(*ln),
                        num(*ln),
                        snip
                    ));
                }
                if f.n > f.matches.len() as f64 {
                    html.push_str(&format!(
                        "<div class=\"lrow\" data-id=\"{}\"><span class=\"ln\"></span><span class=\"tx\" style=\"color:var(--dim)\">… {} more</span></div>",
                        esc(&f.path),
                        num(f.n - f.matches.len() as f64)
                    ));
                }
                html.push_str("</div>");
            }
            dom::set_html(
                &el,
                if html.is_empty() {
                    "<div class=\"empty\">No matches.</div>"
                } else {
                    &html
                },
            );
        }
        ListData::Paths { title, meta, items } => {
            dom::set_text(&dom::id("listtitle"), title);
            dom::set_text(&dom::id("listmeta"), meta);
            if items.is_empty() {
                dom::set_html(&el, "<div class=\"empty\">Nothing here.</div>");
            } else {
                let html: String = items
                    .iter()
                    .map(|it| {
                        format!(
                            "<div class=\"lsimple\" data-id=\"{}\"{}><span>{}</span><span class=\"m\">{}</span></div>",
                            esc(&it.id),
                            match it.ln {
                                Some(ln) if ln != 0 => format!(" data-ln=\"{ln}\""),
                                _ => String::new(),
                            },
                            esc(&it.label),
                            esc(&it.meta)
                        )
                    })
                    .collect();
                dom::set_html(&el, &html);
            }
        }
    }
}

/// `wireList()`
pub fn wire() {
    dom::on_id("list", "click", |e: MouseEvent| {
        let Some(row) = dom::closest(&e.target(), "[data-id]") else {
            return;
        };
        let id = dom::data(&row, "id").unwrap_or_default();
        let Some(idx) = state::idx() else { return };
        if !idx.has(&id) {
            toast("Not in the index", &id, true);
            return;
        }
        let ln = dom::data(&row, "ln");
        {
            let mut st = s_mut();
            st.jump = ln.as_deref().map(|x| js::parse_float(x) as i64);
            st.mark = match st.list.as_deref() {
                Some(ListData::Find { query, .. }) => Some(query.clone()),
                _ => None,
            };
        }
        if state::sel().as_deref() == Some(&id) && ln.is_none() {
            return;
        }
        let has = ln.is_some();
        nav::select(
            Some(&id),
            SelOpts {
                from: "list",
                tab: if has { Some("content") } else { None },
                mode: if has { Some("source") } else { None },
                ..Default::default()
            },
        );
    });
}

/// `find(q)`
pub async fn find(q: &str) {
    let q = js::trim(q).to_string();
    if q.is_empty() {
        toast("FIND", "Give a search term, e.g. FIND docket", true);
        return;
    }
    let (re, query) = match regex_query(&q) {
        Some(inner) => (1, inner),
        None => (0, q.clone()),
    };
    let r: SearchResult = match api::get(&format!(
        "/api/search?q={}&re={}",
        js::encode_uri_component(&query),
        re
    ))
    .await
    {
        Ok(r) => r,
        Err(e) => {
            dom::log_err("FIND", &e.message());
            return;
        }
    };
    if let Some(e) = r.error.as_ref().filter(|e| !e.is_empty()) {
        toast("FIND", e, true);
        return;
    }
    let meta = format!(
        "{} lines in {} files{}",
        fmt_n(r.total),
        fmt_n(r.files.len() as f64),
        if r.truncated { " (truncated)" } else { "" }
    );
    show(ListData::Find {
        title: format!("FIND {q}"),
        meta,
        files: r.files,
        names: r.names,
        query: q,
    });
}

/// `/^\/(.+)\/([i]?)$/` -> the inner pattern.
pub fn regex_query(q: &str) -> Option<String> {
    let rest = q.strip_prefix('/')?;
    let body = rest.strip_suffix("/i").or_else(|| rest.strip_suffix('/'))?;
    if body.is_empty() || body.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    Some(body.to_string())
}

/// `listOf(kind, arg)`
pub fn list_of(kind: &str, arg: Option<&str>) {
    let Some(idx) = state::idx() else { return };
    let mut title = kind.to_string();
    let items: Vec<ListItem> = match kind {
        "BROKEN" => idx
            .broken
            .iter()
            .map(|b| ListItem {
                id: b.0.clone(),
                ln: b.2,
                label: format!(
                    "{}:{}  →  {}",
                    b.0,
                    b.2.filter(|x| *x != 0)
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "?".into()),
                    b.1
                ),
                meta: b.3.clone(),
            })
            .collect(),
        "DIRTY" => js::order_keys(idx.dirty())
            .into_iter()
            .map(|(p, c)| {
                let meta = match c.as_str() {
                    "M" => "modified",
                    "A" => "added",
                    "D" => "deleted",
                    "R" => "renamed",
                    "?" => "untracked",
                    "U" => "conflict",
                    _ => c.as_str(),
                }
                .to_string();
                ListItem {
                    id: p.clone(),
                    ln: None,
                    label: p,
                    meta,
                }
            })
            .collect(),
        "RECENT" => {
            let mut f: Vec<&crate::model::Node> = idx.files().filter(|n| n.lt0() != 0.0).collect();
            f.sort_by(|a, b| {
                b.lt0()
                    .partial_cmp(&a.lt0())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            f.into_iter()
                .take(80)
                .map(|n| ListItem {
                    id: n.id.clone(),
                    ln: None,
                    label: n.id.clone(),
                    meta: format!("{} · {}", fmt::dt(n.lt0()), fmt::ago_text(n.lt0())),
                })
                .collect()
        }
        "ORPHANS" => idx
            .files()
            .filter(|n| {
                !idx.in_of(&n.id)
                    .iter()
                    .any(|&e| idx.edges[e].k != "surface")
                    && !orphan_skip(&n.id)
            })
            .map(|n| ListItem {
                id: n.id.clone(),
                ln: None,
                label: n.id.clone(),
                meta: if !n.type_s().is_empty() {
                    n.type_s().to_string()
                } else {
                    n.kind().to_string()
                },
            })
            .collect(),
        "HUBS" => {
            let mut v: Vec<(&crate::model::Node, usize)> = idx
                .files()
                .map(|n| {
                    (
                        n,
                        idx.in_of(&n.id)
                            .iter()
                            .filter(|&&e| idx.edges[e].k != "surface")
                            .count(),
                    )
                })
                .filter(|x| x.1 > 0)
                .collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            v.into_iter()
                .take(80)
                .map(|(n, c)| ListItem {
                    id: n.id.clone(),
                    ln: None,
                    label: n.id.clone(),
                    meta: format!("{c} in · {} out", idx.out_of(&n.id).len()),
                })
                .collect()
        }
        "TYPE" => {
            let t = js::lower(arg.unwrap_or(""));
            title = format!("TYPE {}", arg.unwrap_or("undefined"));
            idx.files()
                .filter(|n| js::lower(n.type_s()) == t)
                .map(|n| ListItem {
                    id: n.id.clone(),
                    ln: None,
                    label: n.id.clone(),
                    meta: n.title_s().to_string(),
                })
                .collect()
        }
        _ => vec![],
    };
    let meta = format!(
        "{} item{}",
        items.len(),
        if items.len() == 1 { "" } else { "s" }
    );
    show(ListData::Paths { title, meta, items });
}

fn orphan_skip(id: &str) -> bool {
    // /(^|\/)(index|log|readme|_template)\.md$/i
    let base = id.rsplit('/').next().unwrap_or(id);
    let l = base.to_ascii_lowercase();
    // the regex needs the name to start at a '/' or the beginning: `base` is exactly that
    matches!(
        l.as_str(),
        "index.md" | "log.md" | "readme.md" | "_template.md"
    )
}
