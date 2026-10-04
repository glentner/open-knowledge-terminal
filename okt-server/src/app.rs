//! Shared server state.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, RwLock};

use indexmap::IndexMap;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::Args;
use crate::git::Git;
use crate::index::Index;
use crate::render::{Renderer, parse_patch};

pub struct App {
    pub root: PathBuf,
    pub args: Args,
    pub index: RwLock<Index>,
    pub render: Renderer,
    pub bus: broadcast::Sender<(String, String)>,
    pub editor: Vec<String>,
    pub started: f64,
    pub build: String,
    /// `path@mtime` of the binary this server started from (see `exe_stamp`)
    pub exe: String,
    pub token: Option<String>,
    pub allowed_hosts: RwLock<HashSet<String>>,
    pub commit_cache: Mutex<IndexMap<String, Value>>,
    pub watching: bool,
    pub paused: AtomicBool,
    pub stopping: AtomicBool,
}

impl App {
    pub fn git(&self) -> Git {
        Git::new(&self.root)
    }

    pub fn publish(&self, kind: &str, data: &Value) {
        let _ = self.bus.send((
            kind.to_string(),
            serde_json::to_string(data).unwrap_or_default(),
        ));
    }

    /// Commits from HEAD back to (not including) `old`, newest first, at most 50.
    pub fn new_commits(&self, old: Option<&str>, new: Option<&str>) -> Vec<Value> {
        if new.is_none() {
            return vec![];
        }
        let idx = self.index.read().unwrap();
        let mut out = Vec::new();
        for c in &idx.commits {
            if Some(c.h.as_str()) == old {
                break;
            }
            out.push(Value::Object(Index::commit_summary(c)));
            if out.len() >= 50 {
                break;
            }
        }
        out
    }

    pub fn commit_detail(&self, sha: &str) -> Result<Value, (u16, String)> {
        let git = self.git();
        let full = git.resolve(sha).ok_or((404, format!("not found: {sha}")))?;
        if let Some(v) = self.commit_cache.lock().unwrap().get(&full) {
            return Ok(v.clone());
        }
        let meta = git
            .show_meta(&full)
            .map_err(|e| (500, format!("git: {e}")))?;
        let (files, patch) = git
            .diff_tree(&full, &meta.p)
            .map_err(|e| (500, format!("git: {e}")))?;
        let parsed = parse_patch(&patch, &files, crate::config::MAX_DIFF_LINES);
        let a: i64 = parsed.iter().map(|f| f["a"].as_i64().unwrap_or(0)).sum();
        let d: i64 = parsed.iter().map(|f| f["d"].as_i64().unwrap_or(0)).sum();
        let idx = self.index.read().unwrap();
        let mut exists = serde_json::Map::new();
        for f in &parsed {
            let p = f["path"].as_str().unwrap_or("").to_string();
            let e = idx.files.contains_key(&p);
            exists.insert(p, json!(e));
        }
        drop(idx);
        let out = json!({
            "h": meta.h, "p": meta.p, "t": meta.t, "an": meta.an, "ae": meta.ae, "cn": meta.cn, "ct": meta.ct,
            "s": meta.s, "x": meta.x, "body": meta.body, "files": parsed, "a": a, "d": d, "exists": exists,
        });
        let mut c = self.commit_cache.lock().unwrap();
        c.insert(full, out.clone());
        while c.len() > 64 {
            c.shift_remove_index(0);
        }
        Ok(out)
    }
}
