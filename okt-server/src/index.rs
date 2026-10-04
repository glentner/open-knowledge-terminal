//! The index: every file git would track, parsed into a graph of links, mentions, wiki links,
//! frontmatter references and external systems, plus history read straight from git.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{Map, Value, json};

use crate::config::*;
use crate::git::{Commit, Git, GitError, Status};
use crate::parse::{self, FileEntry, PATHLIKE_RE, SCHEME_RE, Stat};
use crate::util::*;

pub type StatKey = (u64, i128, bool);

/// [source, target, kind, count, label]
#[derive(Clone, Debug)]
pub struct Edge {
    pub s: String,
    pub t: String,
    pub k: &'static str,
    pub n: u64,
    pub label: String,
}

#[derive(Clone, Debug, Default)]
pub struct Change {
    pub reason: String,
    pub changed: Vec<String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

impl Change {
    pub fn to_json(&self) -> Value {
        json!({"reason": self.reason, "changed": self.changed, "added": self.added, "removed": self.removed})
    }
}

pub struct Index {
    pub root: PathBuf,
    pub git: Git,
    pub files: IndexMap<String, Arc<FileEntry>>,
    pub stats: IndexMap<String, StatKey>,
    pub dirs: HashSet<String>,
    pub edges: Vec<Edge>,
    pub broken: Vec<(String, String, usize, &'static str)>,
    pub version: u64,
    pub head: Option<String>,
    pub status: Status,
    pub commits: Vec<Commit>,
    /// path -> [(commit index, path at commit, status)] newest first
    pub hist: HashMap<String, Vec<(usize, String, String)>>,
    pub commit_paths: Vec<Vec<String>>,
    pub ci_of: HashMap<String, usize>,
    pub renamed: HashMap<String, String>,
    pub renamed_dirs: HashMap<String, String>,
    pub built_ms: f64,
    pub built_at: f64,
    pub last_change: Option<Change>,
    pub json_cache: std::sync::Mutex<Option<(u64, Arc<Vec<u8>>)>>,
    /// Bumped whenever the set of paths or the history changes: rendered links depend on those,
    /// never on the content of other files.
    pub link_epoch: u64,
    pub obsidian: bool,
    pub collapse: i64,
    root_real: PathBuf,
}

static BROKEN_SKIP_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[<>{}*$|]").unwrap());
static DRIVE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z]:[\\/]").unwrap());

impl Index {
    pub fn new(root: &Path, git: Git) -> Self {
        let root_real = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        Index {
            root: root.to_path_buf(),
            git,
            files: IndexMap::new(),
            stats: IndexMap::new(),
            dirs: HashSet::new(),
            edges: vec![],
            broken: vec![],
            version: 0,
            head: None,
            status: Status::default(),
            commits: vec![],
            hist: HashMap::new(),
            commit_paths: vec![],
            ci_of: HashMap::new(),
            renamed: HashMap::new(),
            renamed_dirs: HashMap::new(),
            built_ms: 0.0,
            built_at: 0.0,
            last_change: None,
            json_cache: std::sync::Mutex::new(None),
            link_epoch: 0,
            obsidian: root.join(".obsidian").is_dir(),
            collapse: DEFAULT_COLLAPSE,
            root_real,
        }
    }

    pub fn root_name(&self) -> String {
        self.root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    // ---- scanning -------------------------------------------------------------------------
    pub fn scan(&self) -> Result<IndexMap<String, StatKey>, GitError> {
        let mut out = IndexMap::new();
        for p in self.git.ls_files()? {
            let full = self.root.join(&p);
            let Ok(md) = std::fs::symlink_metadata(&full) else {
                continue;
            };
            if md.is_dir() {
                continue; // submodule or nested repo
            }
            let st = match parse::lstat(&full) {
                Ok(s) => s,
                Err(_) => continue,
            };
            out.insert(p, (st.size, st.mtime_ns, st.is_link));
        }
        Ok(out)
    }

    fn parse_one(&self, p: &str, s: &StatKey) -> Option<FileEntry> {
        let st = parse::lstat(&self.root.join(p)).ok()?;
        Some(parse::parse_file(
            &self.root,
            p,
            &Stat {
                size: st.size,
                mtime_ns: st.mtime_ns,
                is_link: s.2,
            },
        ))
    }

    // ---- building -------------------------------------------------------------------------
    pub fn build(&mut self, reason: &str) -> Result<Change, GitError> {
        let t0 = Instant::now();
        let stats = self.scan()?;
        let mut files = IndexMap::new();
        for (p, s) in &stats {
            if reason != "rebuild" {
                if let (Some(old), Some(olds)) = (self.files.get(p), self.stats.get(p)) {
                    if olds == s {
                        files.insert(p.clone(), old.clone());
                        continue;
                    }
                }
            }
            if let Some(e) = self.parse_one(p, s) {
                files.insert(p.clone(), Arc::new(e));
            }
        }
        self.commit_files(files, stats);
        self.history(true);
        self.refresh_status();
        self.finish(t0);
        let change = Change {
            reason: reason.into(),
            ..Default::default()
        };
        self.last_change = Some(change.clone());
        self.link_epoch += 1;
        Ok(change)
    }

    /// Re-scan; re-parse only files whose size or mtime changed. Returns the change, or None.
    pub fn update(&mut self, reason: &str, head_moved: bool) -> Result<Option<Change>, GitError> {
        let t0 = Instant::now();
        let stats = self.scan()?;
        let added: Vec<String> = stats
            .keys()
            .filter(|p| !self.stats.contains_key(*p))
            .cloned()
            .collect();
        let removed: Vec<String> = self
            .stats
            .keys()
            .filter(|p| !stats.contains_key(*p))
            .cloned()
            .collect();
        let changed: Vec<String> = stats
            .iter()
            .filter(|(p, s)| self.stats.get(*p).map(|o| o != *s).unwrap_or(false))
            .map(|(p, _)| p.clone())
            .collect();
        if added.is_empty() && removed.is_empty() && changed.is_empty() && !head_moved {
            return Ok(None);
        }
        let mut files = self.files.clone();
        for p in &removed {
            files.shift_remove(p);
        }
        for p in added.iter().chain(changed.iter()) {
            match self.parse_one(p, &stats[p]) {
                Some(e) => {
                    files.insert(p.clone(), Arc::new(e));
                }
                None => {
                    files.shift_remove(p);
                }
            }
        }
        self.commit_files(files, stats);
        if head_moved {
            self.history(true);
        }
        if !added.is_empty() || !removed.is_empty() || head_moved {
            self.link_epoch += 1;
        }
        self.refresh_status();
        self.finish(t0);
        let mut c = Change {
            reason: reason.into(),
            changed,
            added,
            removed,
        };
        c.changed.sort();
        c.added.sort();
        c.removed.sort();
        self.last_change = Some(c.clone());
        Ok(Some(c))
    }

    fn commit_files(
        &mut self,
        files: IndexMap<String, Arc<FileEntry>>,
        stats: IndexMap<String, StatKey>,
    ) {
        let mut dirs = HashSet::new();
        for p in files.keys() {
            let mut d = parent_dir(p);
            while d != "." {
                dirs.insert(d.clone());
                d = parent_dir(&d);
            }
        }
        self.files = files;
        self.stats = stats;
        self.dirs = dirs;
        self.resolve_all();
    }

    fn finish(&mut self, t0: Instant) {
        self.version += 1;
        self.built_ms = t0.elapsed().as_secs_f64() * 1000.0;
        self.built_at = now_secs();
        *self.json_cache.lock().unwrap() = None;
    }

    pub fn refresh_status(&mut self) {
        self.status = self.git.status();
    }

    // ---- history ----------------------------------------------------------------------------
    pub fn history(&mut self, force: bool) {
        let head = self.git.head();
        if !force && head == self.head {
            return;
        }
        self.head = head;
        let commits = match self.git.log_all() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("git log failed: {e}");
                vec![]
            }
        };
        struct Lineage {
            name: String,
            commits: Vec<(usize, String, String)>,
        }
        let n = commits.len();
        let mut arena: Vec<Lineage> = Vec::new();
        let mut live: IndexMap<String, usize> = IndexMap::new();
        let mut touched: Vec<Vec<usize>> = vec![vec![]; n];
        let new_lineage = |arena: &mut Vec<Lineage>, name: &str| {
            arena.push(Lineage {
                name: name.to_string(),
                commits: vec![],
            });
            arena.len() - 1
        };
        for ci in (0..n).rev() {
            for f in &commits[ci].f {
                let st = f[0].as_str();
                let path = f[1].as_str();
                let l = if (st == "R" || st == "C") && f.len() > 2 {
                    let old = f[2].as_str();
                    let l = if st == "R" {
                        live.shift_remove(old)
                            .unwrap_or_else(|| new_lineage(&mut arena, path))
                    } else {
                        new_lineage(&mut arena, path)
                    };
                    arena[l].name = path.to_string();
                    live.insert(path.to_string(), l);
                    l
                } else if st == "D" {
                    live.shift_remove(path)
                        .unwrap_or_else(|| new_lineage(&mut arena, path))
                } else {
                    match live.get(path) {
                        Some(l) => *l,
                        None => {
                            let l = new_lineage(&mut arena, path);
                            live.insert(path.to_string(), l);
                            l
                        }
                    }
                };
                arena[l]
                    .commits
                    .push((ci, path.to_string(), st.to_string()));
                touched[ci].push(l);
            }
        }
        let mut hist = HashMap::new();
        let mut renamed = HashMap::new();
        let mut dir_votes: IndexMap<String, IndexMap<String, u64>> = IndexMap::new();
        for (name, &l) in &live {
            let mut h = arena[l].commits.clone();
            h.reverse();
            hist.insert(name.clone(), h);
            for (_, old, _) in &arena[l].commits {
                if old != name {
                    renamed.insert(old.clone(), name.clone());
                    // vote for folder renames (Contacts/ -> contact/) from files that kept their basename
                    if basename(old) == basename(name) {
                        let (mut a, mut b) = (
                            posix_dirname(old).to_string(),
                            posix_dirname(name).to_string(),
                        );
                        while !a.is_empty() && !b.is_empty() && a != b {
                            *dir_votes
                                .entry(format!("{a}/"))
                                .or_default()
                                .entry(format!("{b}/"))
                                .or_insert(0) += 1;
                            if basename(&a) != basename(&b) {
                                break;
                            }
                            a = posix_dirname(&a).to_string();
                            b = posix_dirname(&b).to_string();
                        }
                    }
                }
            }
        }
        let renamed_dirs = dir_votes
            .iter()
            .map(|(old, votes)| {
                let mut best: Option<(&String, u64)> = None;
                for (k, v) in votes {
                    if best.map(|(_, bv)| *v > bv).unwrap_or(true) {
                        best = Some((k, *v));
                    }
                }
                (old.clone(), best.unwrap().0.clone())
            })
            .collect();
        self.ci_of = commits
            .iter()
            .enumerate()
            .map(|(i, c)| (c.h.clone(), i))
            .collect();
        self.commit_paths = touched
            .iter()
            .map(|ls| ls.iter().map(|l| arena[*l].name.clone()).collect())
            .collect();
        self.commits = commits;
        self.hist = hist;
        self.renamed = renamed;
        self.renamed_dirs = renamed_dirs;
    }

    /// The name a file had at commit sha (following renames), or None if it did not exist then.
    pub fn path_at(&self, path: &str, sha: &str) -> Option<String> {
        let (Some(ci), Some(entries)) = (self.ci_of.get(sha), self.hist.get(path)) else {
            return Some(path.to_string());
        };
        if entries.is_empty() {
            return Some(path.to_string());
        }
        for (c, at, st) in entries {
            if c >= ci {
                return if st == "D" { None } else { Some(at.clone()) };
            }
        }
        None
    }

    pub fn commit_summary(c: &Commit) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert("h".into(), json!(c.h));
        m.insert("t".into(), json!(c.t));
        m.insert("s".into(), json!(c.s));
        m.insert("x".into(), json!(c.x));
        m.insert("an".into(), json!(c.an));
        m.insert("a".into(), json!(c.a));
        m.insert("d".into(), json!(c.d));
        m.insert("n".into(), json!(c.f.len()));
        m
    }

    pub fn file_history(&self, path: &str) -> Vec<Value> {
        self.hist
            .get(path)
            .map(|h| {
                h.iter()
                    .map(|(ci, at, st)| {
                        let mut m = Self::commit_summary(&self.commits[*ci]);
                        m.insert("st".into(), json!(st));
                        m.insert("path".into(), json!(at));
                        Value::Object(m)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn folder_history(&self, folder: &str, limit: usize) -> Vec<Value> {
        let pre = if folder == "." { "" } else { folder };
        let mut out = Vec::new();
        for (ci, c) in self.commits.iter().enumerate() {
            let paths: &[String] = self
                .commit_paths
                .get(ci)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let mut set: HashSet<&str> = paths.iter().map(|s| s.as_str()).collect();
            for f in &c.f {
                set.insert(f[1].as_str());
                if f.len() > 2 {
                    set.insert(f[2].as_str());
                }
            }
            let n = set.iter().filter(|p| p.starts_with(pre)).count();
            if n > 0 {
                let mut m = Self::commit_summary(c);
                m.insert("n_in".into(), json!(n));
                out.push(Value::Object(m));
                if out.len() >= limit {
                    break;
                }
            }
        }
        out
    }

    // ---- link resolution --------------------------------------------------------------------
    /// (node id, how) where how is 'ok', 'external', 'outside', 'anchor', or 'broken'. historic: the
    /// text is an old version, so a target that was renamed since resolves to today's name.
    pub fn resolve(
        &self,
        src: &str,
        target: &str,
        kind: &str,
        historic: bool,
    ) -> (Option<String>, &'static str) {
        let t = py_strip(target);
        if t.is_empty() {
            return (None, "broken");
        }
        if t.starts_with('#') {
            return (None, "anchor");
        }
        if SCHEME_RE.is_match(t) && !DRIVE_RE.is_match(t) {
            return (None, "external");
        }
        let t0 = t.split('#').next().unwrap_or("");
        let t0 = t0.split('?').next().unwrap_or("");
        let t = percent_encoding::percent_decode_str(t0)
            .decode_utf8_lossy()
            .to_string();
        if t.is_empty() {
            return (None, "anchor");
        }
        let base = posix_dirname(src);
        if kind == "wiki" {
            return self.resolve_wiki(src, &t);
        }
        let mut cands = Vec::new();
        if let Some(rest) = t.strip_prefix("~/") {
            let mut home_rel: Option<String> = None;
            if let Some(home) = home_dir() {
                let full = resolve_lenient(&home.join(rest));
                if let Ok(rel) = full.strip_prefix(&self.root_real) {
                    let r = rel.to_string_lossy().to_string();
                    home_rel = Some(if r.is_empty() { ".".into() } else { r });
                }
            }
            if home_rel.is_none() {
                let kb = self.root_name();
                if t.starts_with(&format!("~/{kb}/")) || t == format!("~/{kb}") {
                    home_rel = Some(t.chars().skip(kb.chars().count() + 3).collect());
                }
            }
            match home_rel {
                None => return (None, "outside"),
                Some(h) => cands.push(h),
            }
        } else if t.starts_with('/') {
            cands.push(t.trim_start_matches('/').to_string());
        } else {
            cands.push(if base.is_empty() {
                t.clone()
            } else {
                posix_join(base, &t)
            });
            if kind == "mention" || kind == "link" || kind == "field" {
                cands.push(t.clone());
            }
        }
        let mut escaped = false;
        let ncands = cands.len();
        for c in cands {
            let c = if t.ends_with('/') { format!("{c}/") } else { c };
            let Some(n) = norm_rel(&c) else {
                escaped = true;
                continue;
            };
            let hit = self.lookup(&n).or_else(|| {
                if historic {
                    self.lookup_renamed(&n)
                } else {
                    None
                }
            });
            if let Some(h) = hit {
                return (Some(h), "ok");
            }
        }
        if escaped && ncands == 1 {
            return (None, "outside");
        }
        (None, "broken")
    }

    pub fn lookup(&self, n: &str) -> Option<String> {
        if self.files.contains_key(n) {
            return Some(n.to_string());
        }
        let bare = n.trim_end_matches('/');
        if bare.is_empty() {
            return Some(".".into());
        }
        let d = format!("{bare}/");
        if self.dirs.contains(&d) {
            return Some(d);
        }
        let m = format!("{bare}.md");
        if self.files.contains_key(&m) {
            return Some(m);
        }
        None
    }

    fn lookup_renamed(&self, n: &str) -> Option<String> {
        let r = self
            .renamed
            .get(n)
            .or_else(|| self.renamed.get(&format!("{n}.md")));
        if let Some(r) = r {
            if self.files.contains_key(r) {
                return Some(r.clone());
            }
        }
        let parts: Vec<&str> = n.trim_end_matches('/').split('/').collect();
        for i in (1..=parts.len()).rev() {
            let pre = format!("{}/", parts[..i].join("/"));
            if let Some(to) = self.renamed_dirs.get(&pre) {
                let rest = parts[i..].join("/");
                return if !rest.is_empty() {
                    self.lookup(&format!("{to}{rest}"))
                } else if self.dirs.contains(to) {
                    Some(to.clone())
                } else {
                    None
                };
            }
        }
        None
    }

    fn resolve_wiki(&self, src: &str, t: &str) -> (Option<String>, &'static str) {
        let t = py_strip(t);
        let hit = self.lookup(&norm_rel(t).unwrap_or_default());
        if let Some(h) = &hit {
            if h != "." {
                return (hit, "ok");
            }
        }
        let mut stem = t.rsplit('/').next().unwrap_or(t).to_lowercase();
        if !stem.ends_with(".md") {
            stem.push_str(".md");
        }
        let matches: Vec<&String> = self
            .files
            .keys()
            .filter(|p| basename(p).to_lowercase() == stem)
            .collect();
        if !matches.is_empty() {
            let sd = parent_dir(src);
            let same: Vec<&&String> = matches.iter().filter(|p| parent_dir(p) == sd).collect();
            if let Some(s) = same.first() {
                return (Some((**s).clone()), "ok");
            }
            let mut sorted = matches.clone();
            sorted.sort_by_key(|p| p.chars().count());
            return (Some(sorted[0].clone()), "ok");
        }
        (None, "broken")
    }

    fn resolve_all(&mut self) {
        let mut edges: IndexMap<(String, String, &'static str), Edge> = IndexMap::new();
        let mut broken = Vec::new();
        let add = |edges: &mut IndexMap<(String, String, &'static str), Edge>,
                   s: &str,
                   t: &str,
                   k: &'static str,
                   label: String| {
            if s == t {
                return;
            }
            let key = (s.to_string(), t.to_string(), k);
            if let Some(e) = edges.get_mut(&key) {
                e.n += 1;
            } else {
                edges.insert(
                    key,
                    Edge {
                        s: s.into(),
                        t: t.into(),
                        k,
                        n: 1,
                        label,
                    },
                );
            }
        };
        for (p, e) in &self.files {
            if e.kind == "symlink" {
                if let Some(lt) = &e.link_target {
                    if let Some(hit) = self.lookup(lt) {
                        add(&mut edges, p, &hit, "symlink", String::new());
                    }
                }
                continue;
            }
            for r in &e.raw {
                let (hit, how) = self.resolve(p, &r.target, r.kind, false);
                match hit {
                    Some(h) if h != "." => {
                        let label = if r.kind != "mention" {
                            r.text.clone()
                        } else {
                            String::new()
                        };
                        add(&mut edges, p, &h, r.kind, label);
                    }
                    _ => {
                        if how == "broken"
                            && (r.kind == "link" || r.kind == "wiki" || r.kind == "embed")
                            && !BROKEN_SKIP_RE.is_match(&r.target)
                        {
                            broken.push((p.clone(), r.target.clone(), r.line, r.kind));
                        }
                    }
                }
            }
            let empty = Map::new();
            let fm = e.fm.as_ref().unwrap_or(&empty);
            // frontmatter values that are KB paths
            for (key, val) in fm {
                for s in fm_strings(val) {
                    let ext = ext_of(&s);
                    if (s.contains('/') || MARKDOWN_EXT.contains(&ext.as_str()))
                        && PATHLIKE_RE.is_match(&s)
                        && !SCHEME_RE.is_match(&s)
                    {
                        let (hit, _) = self.resolve(p, &s, "field", false);
                        if let Some(h) = hit {
                            if h != "." {
                                add(&mut edges, p, &h, "field", format!("{key}:"));
                            }
                        }
                    }
                }
            }
            // surfaces
            for s in SURFACES {
                for key in s.keys {
                    if let Some(v) = fm.get(*key) {
                        if !is_todo(v) && !all_todo(v) {
                            add(
                                &mut edges,
                                p,
                                &format!("@{}", s.id),
                                "surface",
                                format!("{key}:"),
                            );
                        }
                    }
                }
            }
            if let Some(v) = fm.get(ASANA_FM_KEY) {
                if !is_todo(v) {
                    let mut tenants: BTreeSet<String> = fm_strings(v)
                        .into_iter()
                        .filter(|s| s.starts_with("asana-"))
                        .collect();
                    if tenants.is_empty() {
                        tenants.insert(ASANA_DEFAULT_TENANT.into());
                    }
                    for tnt in tenants {
                        if surface_label(&tnt).is_some() {
                            add(
                                &mut edges,
                                p,
                                &format!("@{tnt}"),
                                "surface",
                                "asana:".into(),
                            );
                        }
                    }
                }
            }
            for (sid, tok) in &e.tools {
                add(&mut edges, p, &format!("@{sid}"), "surface", tok.clone());
            }
            for u in &e.urls {
                let host = url_hostname(u);
                if let Some(sid) = domain_surface(&host) {
                    add(&mut edges, p, &format!("@{sid}"), "surface", host);
                }
            }
        }
        self.edges = edges.into_values().collect();
        self.broken = broken;
    }

    // ---- snapshot for the client ----------------------------------------------------------------
    pub fn snapshot(&self) -> Arc<Vec<u8>> {
        if let Some((v, data)) = &*self.json_cache.lock().unwrap() {
            if *v == self.version {
                return data.clone();
            }
        }
        let dirty = &self.status.files;
        let mut last: HashMap<&str, (usize, i64, i64, String)> = HashMap::new();
        for (p, h) in &self.hist {
            if !h.is_empty() {
                let c0 = &self.commits[h[h.len() - 1].0];
                let c1 = &self.commits[h[0].0];
                last.insert(
                    p.as_str(),
                    (h.len(), c0.t, c1.t, c1.h.chars().take(7).collect()),
                );
            }
        }
        let mut nodes: Vec<Value> = Vec::new();
        let mut root = Map::new();
        root.insert("id".into(), json!("."));
        root.insert("t".into(), json!("root"));
        root.insert("name".into(), json!(self.root_name()));
        root.insert("parent".into(), Value::Null);
        root.insert("family".into(), json!("core"));
        nodes.push(Value::Object(root));
        let mut dirs: Vec<&String> = self.dirs.iter().collect();
        dirs.sort();
        for d in dirs {
            let name = d.trim_end_matches('/').rsplit('/').next().unwrap_or("");
            let top = top_folder(d);
            let fam_key = if top.is_empty() {
                d.trim_end_matches('/')
            } else {
                top
            };
            let mut m = Map::new();
            m.insert("id".into(), json!(d));
            m.insert("t".into(), json!("dir"));
            m.insert("name".into(), json!(name));
            m.insert("parent".into(), json!(parent_dir(d)));
            m.insert(
                "family".into(),
                json!(folder_family(fam_key).unwrap_or("other")),
            );
            nodes.push(Value::Object(m));
        }
        let mut paths: Vec<&String> = self.files.keys().collect();
        paths.sort();
        for p in paths {
            let e = &self.files[p];
            let fam = if p.contains('/') {
                folder_family(top_folder(p)).unwrap_or("other")
            } else {
                "core"
            };
            let empty = Map::new();
            let fm = e.fm.as_ref().unwrap_or(&empty);
            let mut n = Map::new();
            n.insert("id".into(), json!(p));
            n.insert("t".into(), json!("file"));
            n.insert("name".into(), json!(basename(p)));
            n.insert("parent".into(), json!(parent_dir(p)));
            n.insert("family".into(), json!(fam));
            n.insert("kind".into(), json!(e.kind));
            n.insert("title".into(), json!(e.title));
            n.insert("type".into(), json!(e.type_));
            n.insert("size".into(), json!(e.size));
            n.insert("lines".into(), json!(e.lines));
            n.insert("words".into(), json!(e.words));
            n.insert(
                "mtime".into(),
                json!(e.mtime.div_euclid(1_000_000_000) as i64),
            );
            if !e.aliases.is_empty() {
                n.insert("aliases".into(), json!(e.aliases));
            }
            for k in ["updated", "status", "key"] {
                if let Some(v) = fm.get(k) {
                    let s = match v {
                        Value::Null => None,
                        Value::String(s) if s.is_empty() => None,
                        Value::String(_) | Value::Number(_) | Value::Bool(_) => Some(py_str(v)),
                        _ => None,
                    };
                    if let Some(s) = s {
                        n.insert(k.into(), json!(take_chars(&s, 80)));
                    }
                }
            }
            if let Some(err) = &e.fm_error {
                n.insert("fm_error".into(), json!(err));
            }
            if let Some(d) = dirty.get(p) {
                n.insert("dirty".into(), json!(d));
            }
            if let Some((c, ft, lt, lh)) = last.get(p.as_str()) {
                n.insert("c".into(), json!(c));
                n.insert("ft".into(), json!(ft));
                n.insert("lt".into(), json!(lt));
                n.insert("lh".into(), json!(lh));
            }
            if let Some(l) = &e.link_target {
                n.insert("link".into(), json!(l));
            }
            nodes.push(Value::Object(n));
        }
        let used: HashSet<&str> = self
            .edges
            .iter()
            .filter(|e| e.t.starts_with('@'))
            .map(|e| e.t.as_str())
            .collect();
        for s in SURFACES {
            let id = format!("@{}", s.id);
            if used.contains(id.as_str()) {
                nodes.push(json!({"id": id, "t": "surface", "name": s.label, "parent": null, "family": "world"}));
            }
        }
        let st = &self.status;
        let mut kb = Map::new();
        kb.insert("name".into(), json!(self.root_name()));
        kb.insert("root".into(), json!(self.root.display().to_string()));
        kb.insert("display".into(), json!(display_path(&self.root)));
        kb.insert("head".into(), json!(self.head));
        kb.insert("branch".into(), json!(st.branch));
        kb.insert("upstream".into(), json!(st.upstream));
        kb.insert("ahead".into(), json!(st.ahead));
        kb.insert("behind".into(), json!(st.behind));
        kb.insert(
            "dirty".into(),
            Value::Object(dirty.iter().map(|(k, v)| (k.clone(), json!(v))).collect()),
        );
        kb.insert("obsidian".into(), json!(self.obsidian));
        let mut payload = Map::new();
        payload.insert("version".into(), json!(self.version));
        payload.insert("kb".into(), Value::Object(kb));
        payload.insert(
            "families".into(),
            Value::Array(
                FAMILIES
                    .iter()
                    .map(|(f, l, b)| json!({"id": f, "label": l, "blurb": b}))
                    .collect(),
            ),
        );
        payload.insert("collapse".into(), json!(self.collapse));
        payload.insert("nodes".into(), Value::Array(nodes));
        payload.insert(
            "edges".into(),
            Value::Array(
                self.edges
                    .iter()
                    .map(|e| json!([e.s, e.t, e.k, e.n, e.label]))
                    .collect(),
            ),
        );
        payload.insert(
            "broken".into(),
            Value::Array(
                self.broken
                    .iter()
                    .map(|(p, t, l, k)| json!([p, t, l, k]))
                    .collect(),
            ),
        );
        payload.insert("commits".into(), json!(self.commits.len()));
        payload.insert(
            "built".into(),
            json!({"ms": py_round1(self.built_ms), "at": self.built_at}),
        );
        payload.insert(
            "change".into(),
            self.last_change
                .as_ref()
                .map(|c| c.to_json())
                .unwrap_or_else(|| json!({})),
        );
        let data = Arc::new(serde_json::to_vec(&Value::Object(payload)).unwrap());
        *self.json_cache.lock().unwrap() = Some((self.version, data.clone()));
        data
    }

    // ---- screener ---------------------------------------------------------------------------------
    pub fn table(&self, folder: &str, recursive: bool) -> Value {
        let pre = if folder == "." || folder.is_empty() {
            ""
        } else {
            folder
        };
        let mut rows: Vec<Map<String, Value>> = Vec::new();
        let mut paths: Vec<&String> = self.files.keys().collect();
        paths.sort();
        for p in paths {
            let e = &self.files[p];
            let Some(rest) = p.strip_prefix(pre) else {
                continue;
            };
            if !recursive && rest.contains('/') {
                continue;
            }
            if e.kind != "markdown" {
                continue;
            }
            let lt = self
                .hist
                .get(p.as_str())
                .filter(|h| !h.is_empty())
                .map(|h| self.commits[h[0].0].t);
            let mut r = Map::new();
            r.insert("path".into(), json!(p));
            r.insert("title".into(), json!(e.title));
            r.insert("type".into(), json!(e.type_));
            r.insert("fm".into(), Value::Object(e.fm.clone().unwrap_or_default()));
            r.insert("lines".into(), json!(e.lines));
            r.insert("lt".into(), json!(lt));
            r.insert("dirty".into(), json!(self.status.files.get(p.as_str())));
            rows.push(r);
        }
        let mut counts: IndexMap<String, usize> = IndexMap::new();
        let mut first: HashMap<String, usize> = HashMap::new();
        for (i, r) in rows.iter().enumerate() {
            if let Some(Value::Object(fm)) = r.get("fm") {
                for k in fm.keys() {
                    *counts.entry(k.clone()).or_insert(0) += 1;
                    first.entry(k.clone()).or_insert(i);
                }
            }
        }
        let mut cols: Vec<(usize, usize, Value)> = Vec::new();
        for (k, c) in &counts {
            let vals: Vec<&Value> = rows
                .iter()
                .filter_map(|r| r.get("fm").and_then(|f| f.get(k)))
                .collect();
            let nnum = vals
                .iter()
                .filter(|v| matches!(v, Value::Number(_)))
                .count() as f64;
            let numeric = nnum >= f64::max(1.0, vals.len() as f64 * 0.6);
            let compound = vals
                .iter()
                .filter(|v| matches!(v, Value::Array(_) | Value::Object(_)))
                .count() as f64
                > vals.len() as f64 * 0.4;
            let total: usize = vals.iter().map(|v| py_str(v).chars().count()).sum();
            let avglen = total as f64 / f64::max(1.0, vals.len() as f64);
            let order = *first.get(k).unwrap_or(&0);
            cols.push((
                *c,
                order,
                json!({"key": k, "count": c, "numeric": numeric, "compound": compound, "avglen": py_round1(avglen), "order": order}),
            ));
        }
        cols.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        json!({"folder": folder, "recursive": recursive, "rows": rows, "columns": cols.into_iter().map(|c| c.2).collect::<Vec<_>>()})
    }

    // ---- full-text search -------------------------------------------------------------------------
    pub fn search(&self, q: &str, regex: bool, case: bool, limit: usize) -> Value {
        let q = py_strip(q);
        if q.is_empty() {
            return json!({"q": q, "files": [], "total": 0});
        }
        let pattern = if regex {
            q.to_string()
        } else {
            regex::escape(q)
        };
        let rx = match fancy_regex::RegexBuilder::new(&pattern)
            .case_insensitive(!case)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                return json!({"q": q, "error": format!("bad regex: {e}"), "files": [], "total": 0});
            }
        };
        let is = |s: &str| rx.is_match(s).unwrap_or(false);
        let mut items: Vec<(&String, &Arc<FileEntry>)> = self.files.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        let mut name_hits = Vec::new();
        for (p, e) in &items {
            if is(p)
                || e.title.as_deref().map(is).unwrap_or(false)
                || e.aliases.iter().any(|a| is(a))
            {
                name_hits.push((*p).clone());
            }
        }
        let mut out: Vec<(usize, String, Vec<Value>)> = Vec::new();
        let mut total = 0usize;
        for (p, e) in &items {
            let Some(text) = &e.text else { continue };
            if text.is_empty() {
                continue;
            }
            let mut matches = Vec::new();
            let mut n = 0;
            for (i, line) in text.split('\n').enumerate() {
                if is(line) {
                    n += 1;
                    if matches.len() < 30 {
                        matches.push(json!([i + 1, snippet(line, &rx)]));
                    }
                }
            }
            total += n;
            if !matches.is_empty() {
                out.push((n, (*p).clone(), matches));
            }
            if total > 20000 {
                break;
            }
        }
        out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let truncated = out.len() > limit;
        let files: Vec<Value> = out
            .into_iter()
            .take(limit)
            .map(|(n, p, m)| json!({"path": p, "n": n, "matches": m}))
            .collect();
        name_hits.truncate(60);
        json!({"q": q, "files": files, "total": total, "names": name_hits, "truncated": truncated})
    }
}

fn snippet(line: &str, rx: &fancy_regex::Regex) -> String {
    let mut line = py_strip(line).to_string();
    if let Ok(Some(m)) = rx.find(&line) {
        if line.chars().count() > 220 {
            let start = char_index(&line, m.start());
            let a = start.saturating_sub(80);
            line = format!(
                "{}{}…",
                if a > 0 { "…" } else { "" },
                slice_chars(&line, a, a + 220)
            );
        }
    }
    let mut parts = String::new();
    let mut last = 0;
    let mut pos = 0;
    while pos <= line.len() {
        let Ok(Some(m)) = rx.find_from_pos(&line, pos) else {
            break;
        };
        if m.end() == m.start() {
            break;
        }
        parts.push_str(&py_html_escape(&line[last..m.start()]));
        parts.push_str("<mark>");
        parts.push_str(&py_html_escape(m.as_str()));
        parts.push_str("</mark>");
        last = m.end();
        pos = m.end();
    }
    parts.push_str(&py_html_escape(&line[last..]));
    parts
}

/// String leaves of a frontmatter value (stripped).
pub fn fm_strings(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => out.push(py_strip(s).to_string()),
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(v, &mut out);
    out
}

fn all_todo(v: &Value) -> bool {
    let vals = fm_strings(v);
    !vals.is_empty() && vals.iter().all(|x| is_todo(&Value::String(x.clone())))
}

/// `urllib.parse.urlsplit(u).hostname`
pub fn url_hostname(u: &str) -> String {
    let Some(i) = u.find("://") else {
        return String::new();
    };
    let rest = &u[i + 3..];
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let netloc = &rest[..end];
    let hostport = netloc.rsplit('@').next().unwrap_or(netloc);
    let host = if let Some(stripped) = hostport.strip_prefix('[') {
        stripped.split(']').next().unwrap_or("")
    } else {
        hostport.split(':').next().unwrap_or("")
    };
    host.to_lowercase()
}

fn domain_surface(host: &str) -> Option<&'static str> {
    let host = host.to_lowercase();
    let mut fallback = None;
    for s in SURFACES {
        for d in s.domains {
            if *d == "*" {
                fallback = Some(s.id);
            } else if host == *d || host.ends_with(&format!(".{d}")) {
                return Some(s.id);
            }
        }
    }
    if !host.is_empty() && host != "localhost" && host != "127.0.0.1" {
        fallback
    } else {
        None
    }
}

/// `Path.resolve()` (non-strict): resolve symlinks along the existing prefix, keep the rest.
pub fn resolve_lenient(p: &Path) -> PathBuf {
    let mut cur = PathBuf::from("/");
    for comp in p.components() {
        use std::path::Component::*;
        match comp {
            RootDir | Prefix(_) => cur = PathBuf::from("/"),
            CurDir => {}
            ParentDir => {
                cur.pop();
            }
            Normal(s) => {
                let next = cur.join(s);
                match std::fs::read_link(&next) {
                    Ok(target) => {
                        let t = if target.is_absolute() {
                            target
                        } else {
                            cur.join(target)
                        };
                        cur = resolve_lenient(&t);
                    }
                    Err(_) => cur = next,
                }
            }
        }
    }
    cur
}
