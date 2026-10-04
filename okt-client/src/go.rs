//! The command line (GO bar): commands, fuzzy matching, suggestions.

use std::cell::RefCell;

use web_sys::{Event, FocusEvent, KeyboardEvent, MouseEvent};

use crate::actions;
use crate::api::spawn;
use crate::chrome;
use crate::dom;
use crate::fmt::short;
use crate::insp;
use crate::js::{self, esc, fmt_n};
use crate::model::{Idx, Node};
use crate::nav::{self, SelOpts};
use crate::state::{self, s, s_mut};
use crate::theme::ACCENTS;
use crate::views;

pub struct Cmd {
    pub k: &'static str,
    pub a: &'static [&'static str],
    pub d: &'static str,
    pub arg: Option<&'static str>,
    pub req: bool,
}

pub const CMDS: &[Cmd] = &[
    Cmd {
        k: "GRAPH",
        a: &["G"],
        d: "Graph view",
        arg: None,
        req: false,
    },
    Cmd {
        k: "FILES",
        a: &["F", "TREE"],
        d: "File tree",
        arg: None,
        req: false,
    },
    Cmd {
        k: "LOG",
        a: &["L", "HIST"],
        d: "Repo history. LOG <text> filters it",
        arg: Some("text"),
        req: false,
    },
    Cmd {
        k: "TABLE",
        a: &["T", "SCREEN"],
        d: "Frontmatter screener for a folder",
        arg: Some("folder"),
        req: false,
    },
    Cmd {
        k: "FIND",
        a: &["/", "GREP", "SEARCH"],
        d: "Full-text search. FIND /regex/ for a regex",
        arg: Some("text"),
        req: true,
    },
    Cmd {
        k: "OPEN",
        a: &[],
        d: "Open in its default app",
        arg: Some("path"),
        req: false,
    },
    Cmd {
        k: "EDIT",
        a: &["E!"],
        d: "Open in your editor",
        arg: Some("path"),
        req: false,
    },
    Cmd {
        k: "REVEAL",
        a: &[],
        d: "Reveal in ",
        arg: Some("path"),
        req: false,
    },
    Cmd {
        k: "BLAME",
        a: &["B"],
        d: "Toggle line ages on the open file",
        arg: None,
        req: false,
    },
    Cmd {
        k: "AT",
        a: &[],
        d: "View the selected file at a commit, e.g. AT HEAD~5",
        arg: Some("rev"),
        req: true,
    },
    Cmd {
        k: "LIVE",
        a: &[],
        d: "Back to the working tree",
        arg: None,
        req: false,
    },
    Cmd {
        k: "SHOW",
        a: &["COMMIT"],
        d: "Show a commit",
        arg: Some("rev"),
        req: false,
    },
    Cmd {
        k: "EXPAND",
        a: &[],
        d: "Expand a folder in the graph (EXPAND ALL)",
        arg: Some("folder"),
        req: false,
    },
    Cmd {
        k: "COLLAPSE",
        a: &[],
        d: "Collapse a folder (COLLAPSE ALL resets)",
        arg: Some("folder"),
        req: false,
    },
    Cmd {
        k: "REFRESH",
        a: &["RR", "REBUILD"],
        d: "Rebuild the index from disk",
        arg: None,
        req: false,
    },
    Cmd {
        k: "THEME",
        a: &[],
        d: "terminal, dark, or light",
        arg: Some("theme"),
        req: true,
    },
    Cmd {
        k: "ACCENT",
        a: &[],
        d: "#hex or amber, green, cyan, blue, magenta, red, white",
        arg: Some("accent"),
        req: true,
    },
    Cmd {
        k: "BROKEN",
        a: &[],
        d: "Broken links",
        arg: None,
        req: false,
    },
    Cmd {
        k: "DIRTY",
        a: &[],
        d: "Uncommitted changes",
        arg: None,
        req: false,
    },
    Cmd {
        k: "RECENT",
        a: &[],
        d: "Recently changed files",
        arg: None,
        req: false,
    },
    Cmd {
        k: "ORPHANS",
        a: &[],
        d: "Files nothing links to",
        arg: None,
        req: false,
    },
    Cmd {
        k: "HUBS",
        a: &[],
        d: "Most linked files",
        arg: None,
        req: false,
    },
    Cmd {
        k: "TYPE",
        a: &[],
        d: "Cards of a frontmatter type, e.g. TYPE person",
        arg: Some("type"),
        req: true,
    },
    Cmd {
        k: "UP",
        a: &["U"],
        d: "Select the parent folder",
        arg: None,
        req: false,
    },
    Cmd {
        k: "HELP",
        a: &["?"],
        d: "Keys and commands",
        arg: None,
        req: false,
    },
];

/// A command's description (REVEAL names the platform's file manager).
pub fn desc(c: &Cmd) -> String {
    if c.k == "REVEAL" {
        format!("Reveal in {}", state::boot().filer())
    } else {
        c.d.to_string()
    }
}

/// `cmdFor(word)`
pub fn cmd_for(word: &str) -> Option<&'static Cmd> {
    let w = word.to_uppercase();
    CMDS.iter().find(|c| c.k == w || c.a.contains(&w.as_str()))
}

fn run_named(c: &Cmd, a: &str) {
    let Some(idx) = state::idx() else { return };
    match c.k {
        "GRAPH" => nav::set_view("graph", false),
        "FILES" => nav::set_view("files", false),
        "LOG" => {
            nav::set_view("log", false);
            s_mut().log_filter = a.to_string();
            dom::input("lfilter").set_value(a);
            spawn(async {
                if views::log::ensure(false).await.is_ok() {
                    views::log::render();
                }
            });
        }
        "TABLE" => {
            let f = if a.is_empty() {
                state::current_folder()
            } else if idx.has(a) {
                a.to_string()
            } else if a.ends_with('/') {
                a.to_string()
            } else {
                format!("{a}/")
            };
            spawn(async move { views::table::open(&f, None).await });
        }
        "FIND" => {
            let q = a.to_string();
            spawn(async move { views::list::find(&q).await });
        }
        "OPEN" => actions::open_with("app", Some(a)),
        "EDIT" => actions::open_with("editor", Some(a)),
        "REVEAL" => actions::open_with("reveal", Some(a)),
        "BLAME" => actions::toggle_blame(),
        "AT" => actions::travel(Some(a)),
        "LIVE" => actions::travel(None),
        "SHOW" => insp::open_commit(if a.is_empty() { "HEAD" } else { a }),
        "EXPAND" => actions::expand_cmd(a, false),
        "COLLAPSE" => actions::expand_cmd(a, true),
        "REFRESH" => actions::rebuild(),
        "THEME" => {
            let t = js::lower(a);
            if ["terminal", "dark", "light"].contains(&t.as_str()) {
                state::cfg_mut().theme = t;
                state::save_settings();
                crate::theme::apply_theme();
                crate::graph::build(&[]);
                insp::render();
            } else {
                chrome::toast("THEME", "terminal, dark, or light", true);
            }
        }
        "ACCENT" => actions::set_accent(a),
        "BROKEN" | "DIRTY" | "RECENT" | "ORPHANS" | "HUBS" => views::list::list_of(c.k, None),
        "TYPE" => views::list::list_of("TYPE", Some(a)),
        "UP" => {
            if let Some(p) = state::sel().and_then(|s| idx.node(&s).and_then(|n| n.parent.clone()))
            {
                nav::select(Some(&p), SelOpts::default());
            }
        }
        "HELP" => chrome::show_help(),
        _ => {}
    }
}

/// `fuzzy(q, s)` on UTF-16 code units, like the reference.
fn fuzzy(q: &[u16], s: Option<&str>) -> f64 {
    let Some(s) = s.filter(|s| !s.is_empty()) else {
        return -1.0;
    };
    let s = js::u16s(&js::lower(s));
    let n = s.len() as f64;
    let i = js::index_of16(&s, q);
    if i == 0 {
        return 100.0 - n * 0.02;
    }
    if i > 0 {
        let prev = s[(i - 1) as usize];
        let sep = [
            b'/' as u16,
            b'_' as u16,
            b'-' as u16,
            b'.' as u16,
            b' ' as u16,
        ]
        .contains(&prev);
        return (if sep { 82.0 } else { 62.0 }) - i as f64 * 0.05 - n * 0.01;
    }
    let (mut j, mut gaps, mut last) = (0usize, 0i64, -1i64);
    let mut k = 0usize;
    while k < s.len() && j < q.len() {
        if s[k] == q[j] {
            if last >= 0 {
                gaps += k as i64 - last - 1;
            }
            last = k as i64;
            j += 1;
        }
        k += 1;
    }
    if j == q.len() && q.len() > 1 {
        (35.0 - gaps as f64 * 0.6).max(1.0)
    } else {
        -1.0
    }
}

/// `matchNodes(q, filter, limit)`
pub fn match_nodes<'a>(
    idx: &'a Idx,
    q: &str,
    filter: &dyn Fn(&Node) -> bool,
    limit: usize,
) -> Vec<&'a Node> {
    let words: Vec<Vec<u16>> = js::words(&js::lower(q))
        .iter()
        .map(|w| js::u16s(w))
        .collect();
    if words.is_empty() {
        return vec![];
    }
    let mut out: Vec<(&Node, f64)> = Vec::new();
    for n in &idx.nodes {
        if !filter(n) {
            continue;
        }
        let mut hay: Vec<(Option<&str>, f64)> = vec![
            (Some(n.name.as_str()), 2.0),
            (n.title.as_deref(), 1.0),
            (Some(n.id.as_str()), 0.0),
        ];
        if let Some(al) = &n.aliases {
            for a in al {
                hay.push((Some(a.as_str()), 0.0));
            }
        }
        let mut total = 0.0;
        let mut ok = true;
        for w in &words {
            let mut best = -1.0f64;
            for (h, bonus) in &hay {
                let f = fuzzy(w, *h);
                if f >= 0.0 {
                    best = best.max(f + bonus);
                }
            }
            if best < 0.0 {
                ok = false;
                break;
            }
            total += best;
        }
        if ok {
            let depth = n.id.split('/').count() as f64;
            out.push((
                n,
                total + if n.t == "dir" { 4.0 } else { 0.0 } - depth * 0.3,
            ));
        }
    }
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out.into_iter().take(limit).map(|x| x.0).collect()
}

/// `hl(text, q)`: mark the longest query word in `text`.
fn hl(text: &str, q: &str) -> String {
    let mut ws = js::words(&js::lower(q));
    ws.sort_by_key(|w| std::cmp::Reverse(js::len16(w)));
    let Some(w) = ws.first() else {
        return esc(text);
    };
    let s16 = js::u16s(text);
    let low = js::u16s(&js::lower(text));
    let w16 = js::u16s(w);
    let i = js::index_of16(&low, &w16);
    if i < 0 {
        return esc(text);
    }
    let i = i as usize;
    let cut =
        |a: usize, b: usize| String::from_utf16_lossy(&s16[a.min(s16.len())..b.min(s16.len())]);
    format!(
        "{}<mark>{}</mark>{}",
        esc(&cut(0, i)),
        esc(&cut(i, i + w16.len())),
        esc(&cut(i + w16.len(), s16.len()))
    )
}

#[derive(Clone)]
enum Run {
    Cmd(String),
    Select(String),
    Commit(String),
    Find(String),
}

#[derive(Clone)]
struct Sug {
    kind: String,
    label: String,
    sub: String,
    fill: Option<String>,
    q: Option<String>,
    run: Run,
}

thread_local! {
    static SUG: RefCell<(Vec<Sug>, usize)> = const { RefCell::new((Vec::new(), 0)) };
}

fn suggestions(raw: &str) -> Vec<Sug> {
    let Some(idx) = state::idx() else {
        return vec![];
    };
    let v = raw.trim_start_matches(js::is_js_space).to_string();
    let mut out = Vec::new();
    if v.is_empty() {
        for c in CMDS.iter().take(14) {
            out.push(Sug {
                kind: "cmd".into(),
                label: c.k.into(),
                sub: desc(c),
                fill: Some(format!("{}{}", c.k, if c.arg.is_some() { " " } else { "" })),
                q: None,
                run: Run::Cmd(c.k.into()),
            });
        }
        return out;
    }
    let sp = v.find(' ');
    let (first, rest) = match sp {
        Some(i) => (v[..i].to_string(), v[i + 1..].to_string()),
        None => (v.clone(), String::new()),
    };
    let cmd = cmd_for(&first);
    if let (Some(cmd), Some(_)) = (cmd, sp) {
        let q = js::trim(&rest).to_string();
        let mut add = |label: String, fill: String, sub: String| {
            out.push(Sug {
                kind: cmd.k.into(),
                label,
                sub,
                fill: Some(format!("{} {fill}", cmd.k)),
                q: None,
                run: Run::Cmd(format!("{} {fill}", cmd.k)),
            });
        };
        // Deviation from the reference: ACCENT, TYPE, SHOW and AT get their suggestions. The
        // reference's if/else chain has a dangling `else` (the branches after "theme" attach to
        // the `if` inside the theme loop), so they only ever list the command itself.
        match cmd.arg {
            Some("folder") => {
                let list: Vec<&Node> = if !q.is_empty() {
                    match_nodes(&idx, &q, &|n| n.t == "dir", 12)
                } else {
                    idx.nodes.iter().filter(|n| n.t == "dir").take(12).collect()
                };
                for n in list {
                    add(
                        n.id.clone(),
                        n.id.clone(),
                        format!(
                            "{} files",
                            fmt_n(idx.dinfo(&n.id).map(|d| d.files).unwrap_or(0.0))
                        ),
                    );
                }
            }
            Some("path") => {
                let qq = if q.is_empty() {
                    state::sel().unwrap_or_default()
                } else {
                    q.clone()
                };
                for n in match_nodes(&idx, &qq, &|n| n.t == "file", 12) {
                    let sub = match &n.title {
                        Some(t) if *t != n.name => t.clone(),
                        _ => String::new(),
                    };
                    add(n.id.clone(), n.id.clone(), sub);
                }
            }
            Some("theme") => {
                for t in ["terminal", "dark", "light"] {
                    if t.starts_with(&js::lower(&q)) {
                        add(t.into(), t.into(), String::new());
                    }
                }
            }
            Some("accent") => {
                for (k, hex) in ACCENTS {
                    if k.starts_with(&js::lower(&q)) {
                        add(k.to_string(), k.to_string(), hex.to_string());
                    }
                }
            }
            Some("type") => {
                let mut types: indexmap::IndexMap<String, usize> = indexmap::IndexMap::new();
                for n in &idx.nodes {
                    if let Some(t) = n.type_.as_ref().filter(|t| !t.is_empty()) {
                        *types.entry(t.clone()).or_insert(0) += 1;
                    }
                }
                // a Map in the reference: insertion order, then a stable sort by count
                let ql = js::lower(&q);
                let mut tv: Vec<(String, usize)> = types
                    .into_iter()
                    .filter(|(t, _)| t.starts_with(&ql))
                    .collect();
                tv.sort_by(|a, b| b.1.cmp(&a.1));
                for (t, c) in tv.into_iter().take(12) {
                    add(t.clone(), t, format!("{c} cards"));
                }
            }
            Some("rev") => {
                // the rev as typed comes first, so Enter runs it (HEAD~5, a branch, a tag); the
                // recent commits whose hash or subject match follow, to pick with the arrows
                if !q.is_empty() {
                    add(format!("{} {q}", cmd.k), q.clone(), desc(cmd));
                }
                if let Some(ld) = s().log_data.clone() {
                    let ql = js::lower(&q);
                    for c in ld
                        .commits
                        .iter()
                        .filter(|c| {
                            q.is_empty() || c.h.starts_with(&ql) || js::lower(&c.s).contains(&ql)
                        })
                        .take(10)
                    {
                        add(short(&c.h), short(&c.h), c.s.clone());
                    }
                }
            }
            _ => {}
        }
        if out.is_empty() {
            out.push(Sug {
                kind: cmd.k.into(),
                label: js::trim(&format!("{} {q}", cmd.k)).to_string(),
                sub: desc(cmd),
                fill: Some(format!("{} {q}", cmd.k)),
                q: None,
                run: Run::Cmd(format!("{} {q}", cmd.k)),
            });
        }
        return out;
    }
    if sp.is_none() {
        let up = first.to_uppercase();
        for c in CMDS
            .iter()
            .filter(|c| c.k.starts_with(&up) || c.a.contains(&up.as_str()))
            .take(5)
        {
            out.push(Sug {
                kind: "cmd".into(),
                label: c.k.into(),
                sub: desc(c),
                fill: Some(format!("{}{}", c.k, if c.arg.is_some() { " " } else { "" })),
                q: None,
                run: Run::Cmd(c.k.into()),
            });
        }
    }
    let hexish = v.len() >= 4 && v.len() <= 40 && v.bytes().all(|b| b.is_ascii_hexdigit());
    if hexish {
        if let Some(ld) = s().log_data.clone() {
            let vl = v.to_ascii_lowercase();
            for c in ld.commits.iter().filter(|c| c.h.starts_with(&vl)).take(4) {
                out.push(Sug {
                    kind: "commit".into(),
                    label: short(&c.h),
                    sub: c.s.clone(),
                    fill: None,
                    q: None,
                    run: Run::Commit(c.h.clone()),
                });
            }
        }
    }
    for n in match_nodes(&idx, &v, &|n| n.t != "root", 12) {
        let kind = if n.t == "dir" {
            "folder".to_string()
        } else if n.t == "surface" {
            "external".to_string()
        } else if !n.type_s().is_empty() {
            n.type_s().to_string()
        } else {
            n.kind().to_string()
        };
        let label = if n.id == "." {
            "~".to_string()
        } else if n.t == "surface" {
            n.name.clone()
        } else {
            n.id.clone()
        };
        let sub = if n.t == "file" {
            match &n.title {
                Some(t) if *t != n.name => t.clone(),
                _ => String::new(),
            }
        } else {
            String::new()
        };
        out.push(Sug {
            kind,
            label,
            sub,
            fill: None,
            q: Some(v.clone()),
            run: Run::Select(n.id.clone()),
        });
    }
    out.push(Sug {
        kind: "find".into(),
        label: format!("FIND {v}"),
        sub: "search every file".into(),
        fill: None,
        q: None,
        run: Run::Find(v.clone()),
    });
    out
}

/// `renderSug()`
pub fn render_sug() {
    let el = dom::id("gosug");
    let go = dom::id("go");
    let v = dom::input("go").value();
    let sug = suggestions(&v);
    let empty = sug.is_empty();
    let focused = dom::active_element().map(|a| a == go).unwrap_or(false);
    let html: String = if !empty && focused {
        let mut h: String = sug
            .iter()
            .enumerate()
            .map(|(i, s)| {
                format!(
                    "<div class=\"gs{}\" data-i=\"{i}\"><span class=\"gk\">{}</span><span class=\"gl\">{}{}</span><span class=\"gm\"></span></div>",
                    if i == 0 { " on" } else { "" },
                    esc(&s.kind),
                    match &s.q {
                        Some(q) if !q.is_empty() => hl(&s.label, q),
                        _ => esc(&s.label),
                    },
                    if s.sub.is_empty() { String::new() } else { format!("<small>{}</small>", esc(&s.sub)) }
                )
            })
            .collect();
        h.push_str("<div class=\"gh\">↑↓ choose · ⏎ go · ⇥ complete · esc close</div>");
        h
    } else {
        String::new()
    };
    SUG.with(|s| *s.borrow_mut() = (sug, 0));
    if empty || !focused {
        dom::set_hidden(&el, true);
        return;
    }
    dom::set_html(&el, &html);
    dom::set_hidden(&el, false);
}

fn run(r: Run) {
    match r {
        Run::Cmd(c) => run_cmd(&c),
        Run::Select(id) => nav::select(Some(&id), SelOpts::default()),
        Run::Commit(h) => insp::open_commit(&h),
        Run::Find(q) => spawn(async move { views::list::find(&q).await }),
    }
}

/// `runCmd(raw)`
pub fn run_cmd(raw: &str) {
    let v = js::trim(raw).to_string();
    dom::input("go").set_value("");
    dom::set_hidden(&dom::id("gosug"), true);
    if v.is_empty() {
        return;
    }
    let (first, rest) = match v.find(' ') {
        Some(i) => (v[..i].to_string(), js::trim(&v[i + 1..]).to_string()),
        None => (v.clone(), String::new()),
    };
    if let Some(cmd) = cmd_for(&first) {
        if cmd.req && rest.is_empty() {
            let k = cmd.k;
            dom::timeout(0, move || {
                dom::input("go").set_value(&format!("{k} "));
                dom::focus(&dom::id("go"));
                render_sug();
            });
            return;
        }
        run_named(cmd, &rest);
        return;
    }
    if let Some(q) = v.strip_prefix('/') {
        let q = q.to_string();
        spawn(async move { views::list::find(&q).await });
        return;
    }
    if v.len() >= 7 && v.len() <= 40 && v.bytes().all(|b| b.is_ascii_hexdigit()) {
        insp::open_commit(&v);
        return;
    }
    let Some(idx) = state::idx() else { return };
    let hit = if idx.has(&v) {
        Some(v.clone())
    } else {
        match_nodes(&idx, &v, &|n| n.t != "root", 1)
            .first()
            .map(|n| n.id.clone())
    };
    match hit {
        Some(h) => nav::select(Some(&h), SelOpts::default()),
        None => spawn(async move { views::list::find(&v).await }),
    }
}

fn pick(s: Option<Sug>) {
    let go = dom::id("go");
    dom::input("go").set_value("");
    dom::set_hidden(&dom::id("gosug"), true);
    dom::blur(&go);
    if let Some(s) = s {
        run(s.run);
    }
}

/// `wireGo()`
pub fn wire() {
    dom::on_id("go", "input", |_: Event| render_sug());
    dom::on_id("go", "focus", |_: FocusEvent| render_sug());
    dom::on_id("go", "blur", |_: FocusEvent| {
        dom::timeout(150, || dom::set_hidden(&dom::id("gosug"), true));
    });
    dom::on_id("go", "keydown", |e: KeyboardEvent| {
        let k = e.key();
        match k.as_str() {
            "ArrowDown" | "ArrowUp" => {
                e.prevent_default();
                let i = SUG.with(|s| {
                    let mut s = s.borrow_mut();
                    let n = s.0.len();
                    if n == 0 {
                        return None;
                    }
                    s.1 = (s.1 + if k == "ArrowDown" { 1 } else { n - 1 }) % n;
                    Some(s.1)
                });
                let Some(i) = i else { return };
                let rows = dom::qa("#gosug .gs");
                for (j, el) in rows.iter().enumerate() {
                    dom::toggle(el, "on", j == i);
                }
                if let Some(el) = rows.get(i) {
                    dom::scroll_into_view_nearest(el);
                }
            }
            "Tab" => {
                e.prevent_default();
                let s = SUG.with(|s| {
                    let s = s.borrow();
                    s.0.get(s.1).cloned()
                });
                if let Some(s) = s {
                    let v = s.fill.clone().unwrap_or_else(|| {
                        if s.kind == "cmd" {
                            format!("{} ", s.label)
                        } else {
                            s.label.clone()
                        }
                    });
                    dom::input("go").set_value(&v);
                    render_sug();
                }
            }
            "Escape" => {
                dom::input("go").set_value("");
                dom::blur(&dom::id("go"));
                dom::set_hidden(&dom::id("gosug"), true);
            }
            _ => {}
        }
    });
    dom::on_id("goform", "submit", |e: Event| {
        e.prevent_default();
        let typed = js::trim(&dom::input("go").value()).to_string();
        if typed.is_empty() {
            dom::blur(&dom::id("go"));
            return;
        }
        let s = SUG.with(|s| {
            let s = s.borrow();
            s.0.get(s.1).cloned()
        });
        match s {
            Some(s) => pick(Some(s)),
            None => {
                dom::input("go").set_value("");
                run_cmd(&typed);
                dom::blur(&dom::id("go"));
            }
        }
    });
    dom::on_id("gosug", "mousedown", |e: MouseEvent| {
        let Some(row) = dom::closest(&e.target(), ".gs") else {
            return;
        };
        e.prevent_default();
        let i: usize = dom::data(&row, "i")
            .and_then(|x| x.parse().ok())
            .unwrap_or(0);
        let s = SUG.with(|s| s.borrow().0.get(i).cloned());
        pick(s);
    });
}
