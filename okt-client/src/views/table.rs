//! TABLE: the frontmatter screener for a folder.

use std::rc::Rc;

use serde_json::Value;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;

use crate::api::{self, spawn};
use crate::chrome::toast;
use crate::dom;
use crate::fmt;
use crate::js::{self, esc, fmt_n, num};
use crate::model::{TableData, TableRow};
use crate::nav::{self, SelOpts};
use crate::state::{self, cfg, cfg_mut, s, s_mut, save_settings};

const PRIORITY_COLS: &[&str] = &[
    "composite",
    "score",
    "rank",
    "rating",
    "priority",
    "status",
    "stage",
    "state",
    "city",
    "role",
    "team",
    "owner",
    "my-role",
    "date",
    "due",
    "updated",
];

/// `openTable(folder, recursive)`
pub async fn open(folder: &str, recursive: Option<bool>) {
    let Some(idx) = state::idx() else { return };
    let mut folder = if folder.is_empty() {
        ".".to_string()
    } else {
        folder.to_string()
    };
    if !idx.is_dir(&folder) && folder != "." {
        let with_slash = if folder.ends_with('/') {
            folder.clone()
        } else {
            format!("{folder}/")
        };
        let m = idx.nodes.iter().find(|n| {
            n.t == "dir"
                && (n.id == folder
                    || n.id == format!("{folder}/")
                    || n.id.ends_with(&format!("/{with_slash}")))
        });
        match m {
            Some(m) => folder = m.id.clone(),
            None => {
                toast("TABLE", &format!("No folder {folder}"), true);
                return;
            }
        }
    }
    {
        let mut st = s_mut();
        st.table_folder = Some(folder.clone());
        if let Some(r) = recursive {
            st.table_recursive = r;
        }
        st.table_sort = None;
        st.table_cols = None;
    }
    let rec = s().table_recursive;
    let q = |r: bool| {
        format!(
            "/api/table?folder={}&recursive={}",
            js::encode_uri_component(&folder),
            if r { 1 } else { 0 }
        )
    };
    let mut t: TableData = match api::get(&q(rec)).await {
        Ok(t) => t,
        Err(e) => return dom::log_err("table", &e.message()),
    };
    if t.rows.is_empty() && !rec && idx.dinfo(&folder).map(|d| d.files).unwrap_or(0.0) > 0.0 {
        s_mut().table_recursive = true;
        t = match api::get(&q(true)).await {
            Ok(t) => t,
            Err(e) => return dom::log_err("table", &e.message()),
        };
    }
    let saved = cfg()
        .table_cols
        .as_ref()
        .and_then(|tc| tc.get(&folder).cloned());
    let cols = saved.unwrap_or_else(|| default_cols(&t));
    {
        let mut st = s_mut();
        st.table = Some(Rc::new(t));
        st.table_cols = Some(cols);
    }
    if s().view != "table" {
        nav::set_view("table", false);
    } else {
        render();
        nav::nav(true);
    }
}

fn default_cols(t: &TableData) -> Vec<String> {
    let n = t.rows.len() as f64;
    let mut cand: Vec<&crate::model::TableColumn> = t
        .columns
        .iter()
        .filter(|c| {
            !["title", "type", "name", "key", "aliases"].contains(&c.key.as_str())
                && c.count >= (n * 0.25).max(1.0)
                && !c.compound
                && (c.numeric || c.avglen <= 34.0)
        })
        .collect();
    let pri = |k: &str| {
        PRIORITY_COLS
            .iter()
            .position(|x| *x == k)
            .map(|i| i as i32)
            .unwrap_or(99)
    };
    cand.sort_by(|a, b| {
        pri(&a.key).cmp(&pri(&b.key)).then(
            a.order
                .partial_cmp(&b.order)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    cand.into_iter().take(9).map(|c| c.key.clone()).collect()
}

/// `numOf(v)`
pub fn num_of(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => num_of_str(s),
        _ => None,
    }
}

fn num_of_str(s: &str) -> Option<f64> {
    let t: String = s
        .chars()
        .filter(|c| !matches!(c, '$' | '€' | '£' | ',') && !js::is_js_space(*c))
        .collect();
    // /^-?\d+(\.\d+)?%?$/
    let b = t.as_bytes();
    let mut i = 0;
    if b.first() == Some(&b'-') {
        i += 1;
    }
    let ds = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == ds {
        return None;
    }
    if i < b.len() && b[i] == b'.' {
        let fs = i + 1;
        let mut k = fs;
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        if k == fs {
            return None;
        }
        i = k;
    }
    if i < b.len() && b[i] == b'%' {
        i += 1;
    }
    if i != b.len() {
        return None;
    }
    Some(js::parse_float(&t))
}

/// `cellText(v)`
pub fn cell_text(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) if s.is_empty() => String::new(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| cell_text(Some(x)))
            .collect::<Vec<_>>()
            .join(", "),
        Some(Value::Object(m)) => js::entries(m)
            .into_iter()
            .map(|(k, x)| format!("{k}: {}", cell_text(Some(x))))
            .collect::<Vec<_>>()
            .join("; "),
        Some(Value::Bool(b)) => (if *b { "✓" } else { "✗" }).into(),
        Some(v) => js::string(v),
    }
}

fn row_get<'a>(r: &'a TableRow, k: &str, tmp: &'a mut Value) -> Option<&'a Value> {
    let s = |x: &Option<String>| {
        x.as_ref()
            .map(|s| Value::String(s.clone()))
            .unwrap_or(Value::Null)
    };
    match k {
        "title" => {
            *tmp = s(&r.title);
            Some(&*tmp)
        }
        "type" => {
            *tmp = s(&r.type_);
            Some(&*tmp)
        }
        "path" => {
            *tmp = Value::String(r.path.clone());
            Some(&*tmp)
        }
        _ => r.fm.get(k),
    }
}

fn split_tokens(f: &str) -> Vec<String> {
    // /"[^"]*"|\S+/g
    let mut out = Vec::new();
    let chars: Vec<char> = f.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"' {
            if let Some(j) = chars[i + 1..].iter().position(|c| *c == '"') {
                out.push(chars[i..i + 2 + j].iter().collect());
                i += 2 + j;
                continue;
            }
        }
        if js::is_js_space(chars[i]) {
            i += 1;
            continue;
        }
        let st = i;
        while i < chars.len() && !js::is_js_space(chars[i]) {
            i += 1;
        }
        out.push(chars[st..i].iter().collect());
    }
    out
}

fn parse_op(tok: &str) -> Option<(String, &'static str, String)> {
    // /^([\w.-]+)(>=|<=|>|<|=|:)(.+)$/
    let k_end = tok
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-'))
        .unwrap_or(tok.len());
    if k_end == 0 {
        return None;
    }
    let rest = &tok[k_end..];
    let op = [">=", "<=", ">", "<", "=", ":"]
        .into_iter()
        .find(|o| rest.starts_with(*o))?;
    let raw = &rest[op.len()..];
    if raw.is_empty() || raw.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    Some((tok[..k_end].to_string(), op, raw.to_string()))
}

/// `tableRows()`
pub fn rows() -> Vec<TableRow> {
    let Some(t) = s().table.clone() else {
        return vec![];
    };
    let mut rows: Vec<&TableRow> = t.rows.iter().collect();
    let filter = js::trim(&s().table_filter).to_string();
    if !filter.is_empty() {
        let toks = split_tokens(&filter);
        rows.retain(|r| {
            toks.iter().all(|tok| {
                if let Some((k, op, raw)) = parse_op(tok) {
                    let mut tmp = Value::Null;
                    let v = row_get(r, &k, &mut tmp);
                    let txt = match v {
                        Some(Value::Bool(b)) => b.to_string(),
                        _ => cell_text(v),
                    };
                    let rawl = js::lower(&raw).replace('"', "");
                    if op == ":" {
                        return js::lower(&txt).contains(&rawl);
                    }
                    if op == "=" {
                        return js::lower(&txt) == rawl;
                    }
                    let (Some(a), Some(b)) = (num_of(v), num_of_str(&raw)) else {
                        return false;
                    };
                    return match op {
                        ">" => a > b,
                        "<" => a < b,
                        ">=" => a >= b,
                        _ => a <= b,
                    };
                }
                let w = js::lower(&tok.replace('"', ""));
                let vals: Vec<String> = js::entries(&r.fm)
                    .into_iter()
                    .map(|(_, v)| cell_text(Some(v)))
                    .collect();
                js::lower(&format!(
                    "{} {} {}",
                    r.path,
                    r.title.as_deref().unwrap_or(""),
                    vals.join(" ")
                ))
                .contains(&w)
            })
        });
    }
    let mut rows: Vec<TableRow> = rows.into_iter().cloned().collect();
    let sort = s().table_sort.clone();
    if let Some((key, dir)) = sort {
        let get = |r: &TableRow| -> Option<Value> {
            match key.as_str() {
                "_name" => Some(Value::String(r.path.clone())),
                "_title" => r.title.clone().map(Value::String).or(Some(Value::Null)),
                "_type" => r.type_.clone().map(Value::String).or(Some(Value::Null)),
                "_lt" => r.lt.map(|x| serde_json::json!(x)).or(Some(Value::Null)),
                "_lines" => r.lines.map(|x| serde_json::json!(x)).or(Some(Value::Null)),
                k => r.fm.get(k).cloned(),
            }
        };
        let vals: Vec<Option<Value>> = rows.iter().map(get).collect();
        let texts: Vec<String> = vals.iter().map(|v| cell_text(v.as_ref())).collect();
        let ranks = js::collation_ranks(texts.iter().map(|s| s.as_str()), true);
        let empty = |v: &Option<Value>| {
            matches!(v, None | Some(Value::Null))
                || matches!(v, Some(Value::String(s)) if s.is_empty())
        };
        let mut order: Vec<usize> = (0..rows.len()).collect();
        order.sort_by(|&a, &b| {
            let (va, vb) = (&vals[a], &vals[b]);
            let (na, nb) = (num_of(va.as_ref()), num_of(vb.as_ref()));
            let c: f64 = if let (Some(x), Some(y)) = (na, nb) {
                x - y
            } else if empty(va) {
                1.0
            } else if empty(vb) {
                -1.0
            } else {
                ranks[&texts[a]] as f64 - ranks[&texts[b]] as f64
            };
            let c = if empty(va) != empty(vb) {
                c
            } else {
                c * dir as f64
            };
            c.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut sorted = Vec::with_capacity(rows.len());
        for i in order {
            sorted.push(rows[i].clone());
        }
        rows = sorted;
    }
    rows
}

/// `renderTable()`
pub fn render() {
    let (t, folder, recursive, sort, filter, cols) = {
        let st = s();
        let Some(t) = st.table.clone() else { return };
        (
            t,
            st.table_folder.clone().unwrap_or_else(|| ".".into()),
            st.table_recursive,
            st.table_sort.clone(),
            st.table_filter.clone(),
            st.table_cols.clone().unwrap_or_default(),
        )
    };
    let Some(idx) = state::idx() else { return };
    let fold = if folder == "." {
        format!("~/{}", idx.kb.name)
    } else {
        folder.clone()
    };
    dom::set_text(
        &dom::id("tfold"),
        &format!(
            "{fold}{}",
            if recursive { "  (with subfolders)" } else { "" }
        ),
    );
    let rows = rows();
    let numeric: std::collections::HashSet<&str> = t
        .columns
        .iter()
        .filter(|c| c.numeric)
        .map(|c| c.key.as_str())
        .collect();
    let sort_cls = |k: &str| {
        if sort.as_ref().map(|s| s.0.as_str()) == Some(k) {
            " sorted"
        } else {
            ""
        }
    };
    let mut types = std::collections::HashSet::new();
    for r in &t.rows {
        types.insert(r.type_.clone());
    }
    let one_type = types.len() <= 1;
    let arrow = |k: &str| match &sort {
        Some((sk, d)) if sk == k => {
            if *d > 0 {
                " ▲"
            } else {
                " ▼"
            }
        }
        _ => "",
    };
    let sel = state::sel();
    let mut html = format!(
        "<table><thead><tr><th data-k=\"_name\" class=\"{}\">name{}</th><th data-k=\"_title\" class=\"{}\">title{}</th>{}",
        sort_cls("_name"),
        arrow("_name"),
        sort_cls("_title"),
        arrow("_title"),
        if one_type {
            String::new()
        } else {
            format!(
                "<th data-k=\"_type\" class=\"{}\">type{}</th>",
                sort_cls("_type"),
                arrow("_type")
            )
        }
    );
    for k in &cols {
        html.push_str(&format!(
            "<th data-k=\"{}\" class=\"{}{}\">{}{}</th>",
            esc(k),
            if numeric.contains(k.as_str()) {
                "num"
            } else {
                ""
            },
            sort_cls(k),
            esc(k),
            arrow(k)
        ));
    }
    html.push_str(&format!(
        "<th data-k=\"_lt\" class=\"num{}\">changed{}</th></tr></thead><tbody>",
        sort_cls("_lt"),
        arrow("_lt")
    ));
    let cut = if folder == "." {
        0
    } else {
        js::len16(&folder) as i64
    };
    for r in rows.iter().take(3000) {
        let nm = js::slice16(&r.path, cut, None);
        html.push_str(&format!(
            "<tr data-id=\"{}\"{}><td class=\"k\">{}{}</td><td title=\"{}\">{}</td>{}",
            esc(&r.path),
            if sel.as_deref() == Some(r.path.as_str()) {
                " class=\"on\""
            } else {
                ""
            },
            esc(&nm),
            match &r.dirty {
                Some(d) if !d.is_empty() =>
                    format!(" <span style=\"color:var(--warn)\">{}</span>", esc(d)),
                _ => String::new(),
            },
            esc(r.title.as_deref().unwrap_or("")),
            esc(r.title.as_deref().unwrap_or("")),
            if one_type {
                String::new()
            } else {
                format!("<td>{}</td>", esc(r.type_.as_deref().unwrap_or("")))
            }
        ));
        for k in &cols {
            let v = r.fm.get(k);
            let tx = cell_text(v);
            let isnum = numeric.contains(k.as_str()) || num_of(v).is_some();
            html.push_str(&format!(
                "<td class=\"{}{}\" title=\"{}\">{}</td>",
                if isnum { "num" } else { "" },
                if tx.is_empty() { " nil" } else { "" },
                esc(if js::len16(&tx) > 40 { &tx } else { "" }),
                if tx.is_empty() {
                    "·".to_string()
                } else {
                    esc(&tx)
                }
            ));
        }
        html.push_str(&format!(
            "<td class=\"num\">{}</td></tr>",
            fmt::ago(r.lt.unwrap_or(0.0))
        ));
    }
    html.push_str("</tbody></table>");
    let grid = dom::id("tgrid");
    if rows.is_empty() {
        dom::set_html(
            &grid,
            &format!(
                "<div class=\"empty\">No markdown cards {}.</div>",
                if filter.is_empty() {
                    "in this folder"
                } else {
                    "match the filter"
                }
            ),
        );
    } else {
        dom::set_html(&grid, &html);
    }
    dom::set_text(
        &dom::id("tmeta"),
        &format!(
            "{} of {} rows",
            fmt_n(rows.len() as f64),
            fmt_n(t.rows.len() as f64)
        ),
    );
    let mut pick = format!(
        "<button class=\"pchip{}\" data-rec=\"1\">INCLUDE SUBFOLDERS</button>",
        if recursive { " on" } else { "" }
    );
    for c in &t.columns {
        pick.push_str(&format!(
            "<button class=\"pchip{}\" data-col=\"{}\">{}<b>{}</b></button>",
            if cols.contains(&c.key) { " on" } else { "" },
            esc(&c.key),
            esc(&c.key),
            num(c.count)
        ));
    }
    dom::set_html(&dom::id("tcolpick"), &pick);
}

static FILTER: dom::Debounce = dom::Debounce::new(100);

/// `wireTable()`
pub fn wire() {
    dom::on_id("tfilter", "input", |_: web_sys::Event| {
        FILTER.call(|| {
            s_mut().table_filter = dom::input("tfilter").value();
            render();
        })
    });
    dom::on_id("tgrid", "click", |e: MouseEvent| {
        if let Some(th) = dom::closest(&e.target(), "th") {
            let k = dom::data(&th, "k").unwrap_or_default();
            let numeric_col = s()
                .table
                .as_ref()
                .map(|t| t.columns.iter().any(|c| c.key == k && c.numeric))
                .unwrap_or(false);
            {
                let mut st = s_mut();
                st.table_sort = match &st.table_sort {
                    Some((sk, d)) if *sk == k => {
                        if *d > 0 {
                            Some((k, -1))
                        } else {
                            None
                        }
                    }
                    _ => {
                        let dir = if k == "_lt" || numeric_col { -1 } else { 1 };
                        Some((k, dir))
                    }
                };
            }
            render();
            return;
        }
        if let Some(tr) = dom::closest(&e.target(), "tr[data-id]") {
            let id = dom::data(&tr, "id").unwrap_or_default();
            nav::select(
                Some(&id),
                SelOpts {
                    from: "table",
                    ..Default::default()
                },
            );
            for x in dom::qa("#tgrid tr.on") {
                dom::remove_class(&x, "on");
            }
            dom::add_class(&tr, "on");
        }
    });
    dom::on_id("tcols", "click", |_: MouseEvent| {
        let p = dom::id("tcolpick");
        let h = !dom::hidden(&p);
        dom::set_hidden(&p, h);
        dom::toggle(&dom::id("tcols"), "on", !h);
    });
    dom::on_id("tcolpick", "click", |e: MouseEvent| {
        let Some(b) = dom::closest(&e.target(), ".pchip") else {
            return;
        };
        if dom::data(&b, "rec").is_some() {
            let (f, r) = {
                let st = s();
                (
                    st.table_folder.clone().unwrap_or_else(|| ".".into()),
                    st.table_recursive,
                )
            };
            spawn(async move { open(&f, Some(!r)).await });
            return;
        }
        let k = dom::data(&b, "col").unwrap_or_default();
        let (folder, cols) = {
            let mut st = s_mut();
            let mut cols = st.table_cols.clone().unwrap_or_default();
            if cols.contains(&k) {
                cols.retain(|x| *x != k);
            } else {
                cols.push(k);
            }
            st.table_cols = Some(cols.clone());
            (st.table_folder.clone().unwrap_or_else(|| ".".into()), cols)
        };
        {
            let mut c = cfg_mut();
            c.table_cols
                .get_or_insert_with(Default::default)
                .insert(folder, cols);
        }
        save_settings();
        render();
    });
    dom::on_id("tcsv", "click", |_: MouseEvent| export_csv());
}

fn export_csv() {
    let rows = rows();
    let (cols, folder) = {
        let st = s();
        (
            st.table_cols.clone().unwrap_or_default(),
            st.table_folder.clone().unwrap_or_else(|| ".".into()),
        )
    };
    let q = |v: &str| format!("\"{}\"", v.replace('"', "\"\""));
    let mut head = vec![q("path"), q("title"), q("type")];
    head.extend(cols.iter().map(|c| q(c)));
    let mut lines = vec![head.join(",")];
    for r in &rows {
        let mut cells = vec![
            q(&r.path),
            q(r.title.as_deref().unwrap_or("")),
            q(r.type_.as_deref().unwrap_or("")),
        ];
        cells.extend(cols.iter().map(|k| q(&cell_text(r.fm.get(k)))));
        lines.push(cells.join(","));
    }
    let parts = js_sys::Array::new();
    parts.push(&wasm_bindgen::JsValue::from_str(&lines.join("\n")));
    let bag = web_sys::BlobPropertyBag::new();
    bag.set_type("text/csv");
    let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &bag) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    let a: web_sys::HtmlAnchorElement = dom::doc().create_element("a").unwrap().unchecked_into();
    a.set_href(&url);
    let name = if folder == "." {
        "kb".to_string()
    } else {
        folder
            .strip_suffix('/')
            .unwrap_or(&folder)
            .replace('/', "-")
    };
    a.set_download(&format!("{name}.csv"));
    let _ = dom::doc().body().unwrap().append_child(&a);
    a.click();
    a.remove();
    dom::timeout(2000, move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
}
