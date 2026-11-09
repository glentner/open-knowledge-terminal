//! FILES: the tree view, rendered from a row model; when only the selection moved (arrow keys),
//! the rows stay and just the highlight moves.

use std::cell::RefCell;
use std::collections::HashSet;

use web_sys::{KeyboardEvent, MouseEvent};

use crate::dom;
use crate::fmt;
use crate::graph::shape_path;
use crate::js::{self, esc, fmt_n, num};
use crate::model::{Idx, fam_var, kind_label, shape_of};
use crate::nav::{self, SelOpts};
use crate::state::{self, cfg, cfg_mut, s, s_mut, save_settings};

struct Row {
    id: String,
    depth: usize,
    open: bool,
}

thread_local! {
    /// What is on screen: the rows' HTML without the selection, and their ids in order.
    static LAST: RefCell<(String, Vec<String>)> = const { RefCell::new((String::new(), Vec::new())) };
}

fn tree_rows(idx: &Idx) -> (Vec<Row>, Option<usize>) {
    let q = js::lower(js::trim(&dom::input("ffilter").value()));
    let mut rows = Vec::new();
    if !q.is_empty() {
        let words = js::split_ws(&q);
        let hits: Vec<&str> = idx
            .files()
            .filter(|n| {
                let hay = js::lower(&format!(
                    "{} {} {} {}",
                    n.id,
                    n.title_s(),
                    n.aliases.as_ref().map(|a| a.join(" ")).unwrap_or_default(),
                    n.type_s()
                ));
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .map(|n| n.id.as_str())
            .collect();
        let mut need = HashSet::new();
        for h in &hits {
            for a in idx.ancestors(h) {
                need.insert(a);
            }
        }
        let hit_set: HashSet<&str> = hits.iter().copied().collect();
        fn walk(
            idx: &Idx,
            id: &str,
            depth: usize,
            need: &HashSet<String>,
            hits: &HashSet<&str>,
            rows: &mut Vec<Row>,
        ) {
            let (dirs, files) = idx.sorted_kids(id, true);
            if id == "." {
                for f in files {
                    if hits.contains(f.as_str()) {
                        rows.push(Row {
                            id: f.clone(),
                            depth,
                            open: false,
                        });
                    }
                }
            }
            for d in dirs {
                if need.contains(d) {
                    rows.push(Row {
                        id: d.clone(),
                        depth,
                        open: true,
                    });
                    walk(idx, d, depth + 1, need, hits, rows);
                }
            }
            if id != "." {
                for f in files {
                    if hits.contains(f.as_str()) {
                        rows.push(Row {
                            id: f.clone(),
                            depth,
                            open: false,
                        });
                    }
                }
            }
        }
        walk(idx, ".", 0, &need, &hit_set, &mut rows);
        return (rows, Some(hits.len()));
    }
    let sel = state::sel();
    if let Some(sel) = &sel {
        if s().tree_revealed.as_ref() != Some(sel) {
            s_mut().tree_revealed = Some(sel.clone());
            let mut list = idx.ancestors(sel);
            if idx.is_dir(sel) {
                list.push(sel.clone());
            }
            for a in list {
                s_mut().tree_auto.insert(a.clone());
                let mut c = cfg_mut();
                if c.tree_open.get(&a) == Some(&false) {
                    c.tree_open.shift_remove(&a);
                }
            }
        }
    }
    let auto = s().tree_auto.clone();
    let opened = cfg().tree_open.clone();
    let open = |id: &str| {
        opened.get(id) == Some(&true) || (auto.contains(id) && opened.get(id) != Some(&false))
    };
    fn walk(idx: &Idx, id: &str, depth: usize, open: &dyn Fn(&str) -> bool, rows: &mut Vec<Row>) {
        let (dirs, files) = idx.sorted_kids(id, true);
        if id == "." {
            for f in files {
                rows.push(Row {
                    id: f.clone(),
                    depth,
                    open: false,
                });
            }
        }
        for d in dirs {
            let o = open(d);
            rows.push(Row {
                id: d.clone(),
                depth,
                open: o,
            });
            if o {
                walk(idx, d, depth + 1, open, rows);
            }
        }
        if id != "." {
            for f in files {
                rows.push(Row {
                    id: f.clone(),
                    depth,
                    open: false,
                });
            }
        }
    }
    walk(idx, ".", 0, &open, &mut rows);
    (rows, None)
}

fn glyph(n: &crate::model::Node) -> String {
    let fam = n.family.as_deref().unwrap_or("");
    let shape = if n.t == "dir" {
        "circle"
    } else {
        shape_of(fam).unwrap_or("dot")
    };
    let hollow_file = shape_of(fam) == Some("hsquare") && n.t != "dir";
    format!(
        "<svg class=\"gl\" width=\"10\" height=\"10\" viewBox=\"-5 -5 10 10\"><path d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.2\"/></svg>",
        shape_path(shape, 3.2),
        if hollow_file {
            "none".to_string()
        } else {
            fam_var(fam)
        },
        fam_var(fam)
    )
}

/// `renderTree()`
pub fn render() {
    let Some(idx) = state::idx() else { return };
    let (rows, hits) = tree_rows(&idx);
    let dirty = idx.dirty();
    let mut html = String::with_capacity(rows.len() * 600);
    html.push_str("<div class=\"tcols\"><div class=\"trow thead\"><span class=\"nm\">Name</span><span class=\"ti\">Title</span><span class=\"ty\">Type</span><span class=\"num lines\">Lines</span><span class=\"num size\">Size</span><span class=\"age\">Changed</span><span class=\"st\"></span></div>");
    for r in &rows {
        let n = idx.node(&r.id).unwrap();
        let ind = format!(
            "<span style=\"width:{}px;flex:none\"></span>",
            num((r.depth * 14) as f64)
        );
        if n.t == "dir" {
            let info = idx.dinfo(&r.id).cloned().unwrap_or_default();
            let mut types = js::order_keys(&info.types);
            types.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let ty: Vec<String> = types
                .iter()
                .take(2)
                .map(|(t, c)| format!("{t} ×{}", num(*c)))
                .collect();
            html.push_str(&format!(
                "<div class=\"trow dir\" data-id=\"{}\"><span class=\"nm\" title=\"{}\">{ind}<span class=\"tw\">{}</span>{}<span class=\"t\">{}/</span></span><span class=\"ti\">{} files</span><span class=\"ty\">{}</span><span class=\"num lines\">{}</span><span class=\"num size\"></span><span class=\"age\">{}</span><span class=\"st\"></span></div>",
                esc(&r.id),
                esc(&r.id),
                if r.open { "▾" } else { "▸" },
                glyph(n),
                esc(&n.name),
                fmt_n(info.files),
                esc(&ty.join(", ")),
                fmt_n(info.lines),
                fmt::ago(info.lt),
            ));
        } else {
            let ti = match &n.title {
                Some(t) if !t.is_empty() && *t != n.name => t.as_str(),
                _ => "",
            };
            let d = dirty.get(&r.id).map(|s| s.as_str()).unwrap_or("");
            let age = fmt::ago(n.lt0());
            html.push_str(&format!(
                "<div class=\"trow\" data-id=\"{}\"><span class=\"nm{}\" title=\"{}\">{ind}<span class=\"tw\"></span>{}<span class=\"t\">{}</span></span><span class=\"ti{}\">{}</span><span class=\"ty\">{}</span><span class=\"num lines\">{}</span><span class=\"num size\">{}</span><span class=\"age\">{}</span><span class=\"st\" title=\"{}\">{}</span></div>",
                esc(&r.id),
                if ti.is_empty() { " wide" } else { "" },
                esc(&r.id),
                glyph(n),
                esc(&n.name),
                if ti.is_empty() { " none" } else { "" },
                esc(ti),
                esc(&kind_label(n)),
                match n.lines { Some(l) if l != 0.0 => fmt_n(l), _ => String::new() },
                fmt::size(n.size),
                if !age.is_empty() { age } else if !d.is_empty() { "new".into() } else { String::new() },
                if !d.is_empty() { "uncommitted" } else { "" },
                esc(d),
            ));
        }
    }
    html.push_str("</div>");
    let tree = dom::id("tree");
    let sel = state::sel();
    let ids: Vec<String> = rows.into_iter().map(|r| r.id).collect();
    let same = LAST.with(|l| l.borrow().0 == html);
    if same {
        // only the selection moved
        for el in dom::qa_in(&tree, ".trow.on") {
            dom::remove_class(&el, "on");
        }
    } else {
        let st = tree.scroll_top();
        dom::set_html(&tree, &html);
        tree.set_scroll_top(st);
    }
    // the rows sit in the .tcols wrapper, the header first
    let kids = tree.first_element_child().map(|w| w.children());
    if let (Some(sel), Some(kids)) = (&sel, &kids) {
        if let Some(i) = ids.iter().position(|x| x == sel) {
            if let Some(el) = kids.item((i + 1) as u32) {
                dom::add_class(&el, "on");
            }
        }
    }
    LAST.with(|l| *l.borrow_mut() = (html, ids));
    let meta = match hits {
        Some(h) => format!("{h} match{}", if h == 1 { "" } else { "es" }),
        None => format!("{} files", fmt_n(idx.files().count() as f64)),
    };
    dom::set_text(&dom::id("fmeta"), &meta);
    if sel.is_some() && crate::panes::visible("files") {
        if let Some(on) = dom::q_in(&tree, ".trow.on") {
            dom::scroll_into_view_nearest(&on);
        }
    }
}

/// Toggle a folder row open/closed (from its current arrow). Closing folds everything inside.
fn toggle_row(id: &str, row: &web_sys::Element) {
    let o = dom::q_in(row, ".tw")
        .map(|t| dom::text(&t))
        .unwrap_or_default()
        != "▾";
    if o {
        cfg_mut().tree_open.insert(id.to_string(), true);
        save_settings();
    } else {
        fold(id);
    }
}

/// Closing a folder closes every folder under it too, so opening it again shows one level, not the
/// subfolders that were open before.
fn fold(id: &str) {
    fold_below(id);
    cfg_mut().tree_open.insert(id.to_string(), false);
    s_mut().tree_auto.remove(id);
    save_settings();
}

/// Close every folder under `id` (all of them for the root), leaving `id` as it is.
fn fold_below(id: &str) {
    let under = |k: &str| id == "." || (k.starts_with(id) && k != id);
    cfg_mut().tree_open.retain(|k, _| !under(k));
    s_mut().tree_auto.retain(|k| !under(k));
    save_settings();
}

static FILTER: dom::Debounce = dom::Debounce::new(80);

/// `wireTree()`
pub fn wire() {
    dom::on_id("tree", "click", |e: MouseEvent| {
        let Some(row) = dom::closest(&e.target(), ".trow") else {
            return;
        };
        if dom::has_class(&row, "thead") {
            return;
        }
        let id = dom::data(&row, "id").unwrap_or_default();
        let Some(idx) = state::idx() else { return };
        if idx.is_dir(&id)
            && (dom::closest(&e.target(), ".tw").is_some() || state::sel().as_deref() == Some(&id))
        {
            toggle_row(&id, &row);
            s_mut().tree_revealed = Some(id.clone());
        }
        nav::select(
            Some(&id),
            SelOpts {
                from: "tree",
                ..Default::default()
            },
        );
        render();
    });
    dom::on_id("tree", "dblclick", |e: MouseEvent| {
        let Some(row) = dom::closest(&e.target(), ".trow") else {
            return;
        };
        let id = dom::data(&row, "id").unwrap_or_default();
        if state::idx().map(|i| !i.is_dir(&id)).unwrap_or(false) {
            nav::set_tab("content");
        }
    });
    dom::on_id("ffilter", "input", |_: web_sys::Event| FILTER.call(render));
}

/// `treeKey(e)`: arrows, j/k, Enter in the tree. True when handled. The keys step from the marked
/// row, which a render marks from the selection. Off screen (its pane collapsed, or its slot showing
/// the Inspector) the tree is not redrawn when the selection moves elsewhere, so it is drawn here
/// first, and the keys step from the selection as they do on screen.
pub fn key(e: &KeyboardEvent) -> bool {
    if !crate::panes::visible("files") {
        render();
    }
    let rows = dom::qa("#tree .trow:not(.thead)");
    if rows.is_empty() {
        return false;
    }
    let Some(idx) = state::idx() else {
        return false;
    };
    let mut i: i64 = rows
        .iter()
        .position(|r| dom::has_class(r, "on"))
        .map(|x| x as i64)
        .unwrap_or(-1);
    let id = if i >= 0 {
        dom::data(&rows[i as usize], "id")
    } else {
        None
    };
    let k = e.key();
    let n = rows.len() as i64;
    match k.as_str() {
        "ArrowDown" | "j" => i = (i + 1).min(n - 1),
        "ArrowUp" | "k" => i = (i - 1).max(0),
        "ArrowRight" if id.as_ref().map(|x| idx.is_dir(x)).unwrap_or(false) => {
            cfg_mut().tree_open.insert(id.unwrap(), true);
            save_settings();
            render();
            return true;
        }
        "ArrowLeft" if id.is_some() => {
            let id = id.unwrap();
            let row = &rows[i as usize];
            if idx.is_dir(&id)
                && dom::q_in(row, ".tw").map(|t| dom::text(&t)).as_deref() == Some("▾")
            {
                fold(&id);
                render();
                return true;
            }
            if let Some(p) = idx.node(&id).and_then(|n| n.parent.clone()) {
                if p != "." {
                    nav::select(
                        Some(&p),
                        SelOpts {
                            from: "tree",
                            ..Default::default()
                        },
                    );
                    render();
                }
            }
            return true;
        }
        "Enter" if id.is_some() => {
            let id = id.unwrap();
            if idx.is_dir(&id) {
                toggle_row(&id, &rows[i as usize]);
                render();
            } else {
                nav::set_tab("content");
            }
            return true;
        }
        // Home-row keys, as in ncdu: space opens or closes a folder and shows a file in the graph;
        // u goes up to the parent folder and folds everything under it, so walking up tidies the
        // tree behind you (at the top, the whole tree folds).
        " " if id.is_some() => {
            let id = id.unwrap();
            if idx.is_dir(&id) {
                toggle_row(&id, &rows[i as usize]);
                render();
            } else {
                nav::set_view("graph", false);
                crate::graph::focus_node(&id, None);
            }
            return true;
        }
        "u" => {
            let cur = id.or_else(state::sel);
            let Some(parent) = cur
                .as_deref()
                .and_then(|c| idx.node(c))
                .and_then(|n| n.parent.clone())
            else {
                return true;
            };
            fold_below(&parent);
            nav::select(
                Some(&parent),
                SelOpts {
                    from: "tree",
                    ..Default::default()
                },
            );
            render();
            return true;
        }
        _ => return false,
    }
    let i = i.max(0) as usize;
    let target = dom::data(&rows[i], "id").unwrap_or_default();
    s_mut().tree_revealed = Some(target.clone()); // arrowing onto a folder doesn't expand it
    nav::select(
        Some(&target),
        SelOpts {
            from: "tree",
            ..Default::default()
        },
    );
    render();
    true
}
