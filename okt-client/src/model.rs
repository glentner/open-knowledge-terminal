//! The server's JSON payloads, and the index as the client works with it (`ingest()`: node map,
//! children, edges in and out, per-folder rollups).

use std::collections::HashMap;
use std::rc::Rc;

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::js;

// ---- payloads ------------------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Kb {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub root: String,
    #[serde(default)]
    pub display: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    #[serde(default)]
    pub ahead: i64,
    #[serde(default)]
    pub behind: i64,
    #[serde(default)]
    pub dirty: IndexMap<String, String>,
    #[serde(default)]
    pub obsidian: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Family {
    pub id: String,
    pub label: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Built {
    #[serde(default)]
    pub ms: f64,
    #[serde(default)]
    pub at: f64,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Node {
    pub id: String,
    pub t: String,
    #[serde(default)]
    pub name: String,
    pub parent: Option<String>,
    pub family: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub size: Option<f64>,
    pub lines: Option<f64>,
    pub words: Option<f64>,
    pub aliases: Option<Vec<String>>,
    pub c: Option<f64>,
    /// first-commit time (seconds), following renames; absent while uncommitted
    #[serde(default)]
    pub ft: Option<f64>,
    pub lt: Option<f64>,
    pub link: Option<String>,
}

impl Node {
    pub fn fam(&self) -> &str {
        self.family
            .as_deref()
            .filter(|f| !f.is_empty())
            .unwrap_or("other")
    }
    pub fn kind(&self) -> &str {
        self.kind.as_deref().unwrap_or("")
    }
    /// `n.lt || 0`
    pub fn lt0(&self) -> f64 {
        self.lt.unwrap_or(0.0)
    }
    pub fn title_s(&self) -> &str {
        self.title.as_deref().unwrap_or("")
    }
    pub fn type_s(&self) -> &str {
        self.type_.as_deref().unwrap_or("")
    }
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub s: String,
    pub t: String,
    pub k: String,
    pub n: f64,
    pub label: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Broken(pub String, pub String, pub Option<i64>, pub String);

#[derive(Deserialize)]
struct IndexWire {
    version: u64,
    kb: Kb,
    families: Vec<Family>,
    #[serde(default)]
    collapse: Option<i64>,
    nodes: Vec<Node>,
    edges: Vec<(String, String, String, f64, Value)>,
    #[serde(default)]
    broken: Vec<Broken>,
    #[serde(default)]
    commits: f64,
    #[serde(default)]
    built: Built,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Commit {
    pub h: String,
    #[serde(default)]
    pub t: f64,
    #[serde(default)]
    pub s: String,
    pub x: Option<String>,
    #[serde(default)]
    pub an: String,
    #[serde(default)]
    pub a: f64,
    #[serde(default)]
    pub d: f64,
    #[serde(default)]
    pub n: f64,
    /// [[status, path], ...] (renames carry the old path too)
    #[serde(default)]
    pub f: Vec<Vec<String>>,
    pub st: Option<String>,
    pub path: Option<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct LogData {
    #[serde(default)]
    pub total: f64,
    #[serde(default)]
    pub commits: Vec<Commit>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct GitInfo {
    #[serde(default)]
    pub count: f64,
    pub first: Option<Commit>,
    pub last: Option<Commit>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DataTable {
    #[serde(default)]
    pub header: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
    #[serde(default)]
    pub numeric: Vec<bool>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct RevInfo {
    pub h: String,
    #[serde(default)]
    pub t: f64,
    #[serde(default)]
    pub s: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct FilePayload {
    #[serde(default)]
    pub kind: String,
    pub rev: Option<RevInfo>,
    #[serde(default)]
    pub raw: String,
    pub size: Option<f64>,
    pub lines: Option<f64>,
    pub fm: Option<Map<String, Value>>,
    pub fm_error: Option<String>,
    #[serde(default)]
    pub headings: Vec<(f64, String, String, f64)>,
    pub link: Option<String>,
    pub mime: Option<String>,
    #[serde(default)]
    pub truncated: bool,
    pub html: Option<String>,
    pub source: Option<Vec<String>>,
    pub table: Option<DataTable>,
    pub git: Option<GitInfo>,
    pub at_path: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TableColumn {
    pub key: String,
    #[serde(default)]
    pub count: f64,
    #[serde(default)]
    pub numeric: bool,
    #[serde(default)]
    pub compound: bool,
    #[serde(default)]
    pub avglen: f64,
    #[serde(default)]
    pub order: f64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TableRow {
    pub path: String,
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    #[serde(default)]
    pub fm: Map<String, Value>,
    pub lt: Option<f64>,
    pub lines: Option<f64>,
    pub dirty: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TableData {
    #[serde(default)]
    pub rows: Vec<TableRow>,
    #[serde(default)]
    pub columns: Vec<TableColumn>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SearchFile {
    pub path: String,
    #[serde(default)]
    pub n: f64,
    #[serde(default)]
    pub matches: Vec<(f64, String)>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SearchResult {
    #[serde(default)]
    pub total: f64,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub files: Vec<SearchFile>,
    #[serde(default)]
    pub names: Vec<String>,
    pub error: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BlameCommit {
    pub h: String,
    #[serde(default)]
    pub t: f64,
    #[serde(default)]
    pub s: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Blame {
    #[serde(default)]
    pub lines: Vec<usize>,
    #[serde(default)]
    pub commits: Vec<BlameCommit>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Hunk {
    #[serde(default)]
    pub h: String,
    #[serde(default)]
    pub lines: Vec<(String, Value, Value, String)>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CommitFile {
    pub path: String,
    pub old: Option<String>,
    #[serde(default)]
    pub st: String,
    #[serde(default)]
    pub a: f64,
    #[serde(default)]
    pub d: f64,
    #[serde(default)]
    pub binary: bool,
    #[serde(default)]
    pub hunks: Vec<Hunk>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CommitDetail {
    pub h: String,
    #[serde(default)]
    pub p: Vec<String>,
    #[serde(default)]
    pub t: f64,
    #[serde(default)]
    pub an: String,
    #[serde(default)]
    pub s: String,
    pub x: Option<String>,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub files: Vec<CommitFile>,
    #[serde(default)]
    pub a: f64,
    #[serde(default)]
    pub d: f64,
    #[serde(default)]
    pub exists: HashMap<String, bool>,
}

// ---- the index -----------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct DInfo {
    pub files: f64,
    pub direct: f64,
    pub dirs: f64,
    pub lt: f64,
    pub lines: f64,
    pub types: IndexMap<String, f64>,
}

pub const FAM_ORDER: &[(&str, i32)] = &[
    ("rules", 0),
    ("memory", 1),
    ("cards", 2),
    ("workspace", 3),
    ("records", 4),
    ("core", 5),
    ("other", 6),
    ("world", 7),
];

pub fn fam_order(f: &str) -> i32 {
    FAM_ORDER
        .iter()
        .find(|(k, _)| *k == f)
        .map(|(_, v)| *v)
        .unwrap_or(9)
}

/// `SHAPE[fam]` (`None` for a family with no shape).
pub fn shape_of(f: &str) -> Option<&'static str> {
    Some(match f {
        "core" => "diamond",
        "cards" => "circle",
        "rules" => "square",
        "memory" => "triangle",
        "records" => "hsquare",
        "workspace" => "hex",
        "other" => "dot",
        "world" => "hexo",
        _ => return None,
    })
}

pub fn fam_var(f: &str) -> String {
    format!("var(--f-{})", if f.is_empty() { "other" } else { f })
}

pub fn edge_group(k: &str) -> &'static str {
    match k {
        "surface" => "surface",
        _ => "link",
    }
}

pub struct Idx {
    pub version: u64,
    pub kb: Kb,
    pub families: Vec<Family>,
    pub collapse: Option<i64>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub broken: Vec<Broken>,
    pub commits: f64,
    pub built: Built,
    pub by_id: HashMap<String, usize>,
    pub kids: HashMap<String, Vec<String>>,
    pub out: HashMap<String, Vec<usize>>,
    pub inn: HashMap<String, Vec<usize>>,
    pub dinfo: HashMap<String, DInfo>,
    /// sortedKids(id, 'graph') and sortedKids(id, 'tree'), per folder
    sorted_graph: HashMap<String, (Vec<String>, Vec<String>)>,
    sorted_tree: HashMap<String, (Vec<String>, Vec<String>)>,
}

const EMPTY: &[usize] = &[];

impl Idx {
    pub fn parse(text: &str) -> Result<Rc<Idx>, String> {
        let w: IndexWire = serde_json::from_str(text).map_err(|e| format!("bad index: {e}"))?;
        Ok(Rc::new(Idx::ingest(w)))
    }

    fn ingest(w: IndexWire) -> Idx {
        let mut by_id = HashMap::with_capacity(w.nodes.len());
        let mut kids: HashMap<String, Vec<String>> = HashMap::new();
        for (i, n) in w.nodes.iter().enumerate() {
            by_id.insert(n.id.clone(), i);
            if let Some(p) = &n.parent {
                kids.entry(p.clone()).or_default().push(n.id.clone());
            }
        }
        let edges: Vec<Edge> = w
            .edges
            .into_iter()
            .map(|(s, t, k, n, label)| Edge {
                s,
                t,
                k,
                n,
                label: label
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| js::string(&label)),
            })
            .collect();
        let mut out: HashMap<String, Vec<usize>> = HashMap::new();
        let mut inn: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            if !by_id.contains_key(&e.s) || !by_id.contains_key(&e.t) {
                continue;
            }
            out.entry(e.s.clone()).or_default().push(i);
            inn.entry(e.t.clone()).or_default().push(i);
        }
        let mut idx = Idx {
            version: w.version,
            kb: w.kb,
            families: w.families,
            collapse: w.collapse,
            nodes: w.nodes,
            edges,
            broken: w.broken,
            commits: w.commits,
            built: w.built,
            by_id,
            kids,
            out,
            inn,
            dinfo: HashMap::new(),
            sorted_graph: HashMap::new(),
            sorted_tree: HashMap::new(),
        };
        let mut dinfo = HashMap::new();
        idx.walk_info(".", &mut dinfo);
        idx.dinfo = dinfo;
        idx.sort_all();
        idx
    }

    fn walk_info(&self, id: &str, dinfo: &mut HashMap<String, DInfo>) -> DInfo {
        let mut info = DInfo::default();
        for k in self.kids.get(id).map(|v| v.as_slice()).unwrap_or(&[]) {
            let n = self.node(k).unwrap();
            if n.t == "dir" {
                let c = self.walk_info(k, dinfo);
                info.files += c.files;
                info.dirs += 1.0 + c.dirs;
                info.lt = info.lt.max(c.lt);
                info.lines += c.lines;
                for (t, v) in js::order_keys(&c.types) {
                    *info.types.entry(t).or_insert(0.0) += v;
                }
            } else if n.t == "file" {
                info.files += 1.0;
                info.direct += 1.0;
                info.lt = info.lt.max(n.lt0());
                info.lines += n.lines.unwrap_or(0.0);
                if let Some(t) = n.type_.as_deref().filter(|t| !t.is_empty()) {
                    *info.types.entry(t.to_string()).or_insert(0.0) += 1.0;
                }
            }
        }
        dinfo.insert(id.to_string(), info.clone());
        info
    }

    /// Precompute `sortedKids()` for both modes (the sorts use `localeCompare`, so they go
    /// through the browser's collator once per index).
    fn sort_all(&mut self) {
        let tilde = |s: &str| {
            if let Some(r) = s.strip_prefix('.') {
                format!("~{r}")
            } else {
                s.to_string()
            }
        };
        let mut universe: Vec<String> = self.nodes.iter().map(|n| n.id.clone()).collect();
        for n in &self.nodes {
            if n.t == "dir" {
                universe.push(tilde(&n.id));
            }
        }
        let ranks = js::collation_ranks(universe.iter().map(|s| s.as_str()), false);
        let r = |s: &str| ranks.get(s).copied().unwrap_or(u32::MAX);
        let parents: Vec<String> = self.kids.keys().cloned().collect();
        for id in parents {
            let ks = &self.kids[&id];
            let mut dirs = Vec::new();
            let mut files = Vec::new();
            for k in ks {
                if self.node(k).map(|n| n.t == "dir").unwrap_or(false) {
                    dirs.push(k.clone());
                } else {
                    files.push(k.clone());
                }
            }
            let rank = |k: &str| -> i32 {
                match self.node(k).map(|n| js::lower(&n.name)).as_deref() {
                    Some("index.md") => 0,
                    Some("readme.md") => 1,
                    Some("agents.md") => 2,
                    Some("log.md") => 3,
                    _ => 4,
                }
            };
            files.sort_by(|a, b| rank(a).cmp(&rank(b)).then(r(a).cmp(&r(b))));
            let mut gdirs = dirs.clone();
            if id == "." {
                gdirs.sort_by(|a, b| {
                    let fa = fam_order(self.node(a).map(|n| n.fam()).unwrap_or(""));
                    let fb = fam_order(self.node(b).map(|n| n.fam()).unwrap_or(""));
                    fa.cmp(&fb).then(r(a).cmp(&r(b)))
                });
            } else {
                gdirs.sort_by(|a, b| r(&tilde(a)).cmp(&r(&tilde(b))));
            }
            let mut tdirs = dirs;
            tdirs.sort_by(|a, b| r(&tilde(a)).cmp(&r(&tilde(b))));
            self.sorted_graph.insert(id.clone(), (gdirs, files.clone()));
            self.sorted_tree.insert(id, (tdirs, files));
        }
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.by_id.get(id).map(|&i| &self.nodes[i])
    }

    pub fn has(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    pub fn is_dir(&self, id: &str) -> bool {
        self.node(id)
            .map(|n| n.t == "dir" || n.t == "root")
            .unwrap_or(false)
    }

    pub fn out_of(&self, id: &str) -> &[usize] {
        self.out.get(id).map(|v| v.as_slice()).unwrap_or(EMPTY)
    }

    pub fn in_of(&self, id: &str) -> &[usize] {
        self.inn.get(id).map(|v| v.as_slice()).unwrap_or(EMPTY)
    }

    pub fn kids_of(&self, id: &str) -> &[String] {
        self.kids.get(id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn degree(&self, id: &str) -> f64 {
        (self.out_of(id).len() + self.in_of(id).len()) as f64
    }

    /// `sortedKids(id, mode)` -> (dirs, files)
    pub fn sorted_kids(&self, id: &str, tree: bool) -> (&[String], &[String]) {
        let m = if tree {
            &self.sorted_tree
        } else {
            &self.sorted_graph
        };
        match m.get(id) {
            Some((d, f)) => (d.as_slice(), f.as_slice()),
            None => (&[], &[]),
        }
    }

    pub fn dinfo(&self, id: &str) -> Option<&DInfo> {
        self.dinfo.get(id)
    }

    pub fn name_of(&self, id: &str) -> String {
        let Some(n) = self.node(id) else {
            return id.to_string();
        };
        if n.t == "root" {
            return format!("~/{}", n.name);
        }
        if n.t == "surface" {
            return n.name.clone();
        }
        format!("{}{}", n.name, if n.t == "dir" { "/" } else { "" })
    }

    pub fn label_of(&self, id: &str) -> String {
        let Some(n) = self.node(id) else {
            return id.to_string();
        };
        match &n.title {
            Some(t) if n.t == "file" && !t.is_empty() && *t != n.name => t.clone(),
            _ => self.name_of(id),
        }
    }

    pub fn ancestors(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut p = self.node(id).and_then(|n| n.parent.clone());
        while let Some(x) = p {
            if x == "." {
                break;
            }
            p = self.node(&x).and_then(|n| n.parent.clone());
            out.push(x);
        }
        out
    }

    pub fn files(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(|n| n.t == "file")
    }

    pub fn dirty(&self) -> &IndexMap<String, String> {
        &self.kb.dirty
    }

    pub fn family_label(&self, f: &str) -> Option<&str> {
        self.families
            .iter()
            .find(|x| x.id == f)
            .map(|x| x.label.as_str())
    }
}

/// `extOf(name)`: the lowercased extension (letters and digits only), or ''.
pub fn ext_of(name: &str) -> String {
    let Some(i) = name.rfind('.') else {
        return String::new();
    };
    let e = &name[i + 1..];
    if e.is_empty() || !e.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return String::new();
    }
    e.to_ascii_lowercase()
}

/// `kindLabel(n)`
pub fn kind_label(n: &Node) -> String {
    if n.t != "file" {
        return String::new();
    }
    if n.kind() == "markdown" {
        return n.type_s().to_string();
    }
    if n.kind() == "symlink" {
        return "symlink".into();
    }
    let e = ext_of(&n.name);
    if e.is_empty() {
        n.kind().to_string()
    } else {
        e
    }
}

pub fn kind_name(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "xlsx" | "xls" => "Excel workbook",
        "docx" | "doc" => "Word document",
        "pptx" | "ppt" => "PowerPoint deck",
        "key" => "Keynote deck",
        "numbers" => "Numbers sheet",
        "pages" => "Pages document",
        "zip" => "zip archive",
        "gz" => "gzip archive",
        "tar" => "tar archive",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_sample() {
        let p = std::env::var("OKT_LOG_JSON").unwrap_or_default();
        if p.is_empty() {
            return;
        }
        let text = std::fs::read_to_string(p).unwrap();
        let d: Result<LogData, _> = serde_json::from_str(&text);
        if let Err(e) = &d {
            panic!("{e}");
        }
    }
}
