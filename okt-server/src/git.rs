//! Thin wrapper around the git CLI. Never takes optional locks, so it cannot collide with commits
//! that agents or editors make while the terminal is watching.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::util::prefix_of;

#[derive(Debug)]
pub struct GitError(pub String);

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GitError {}

pub type GitResult<T> = Result<T, GitError>;

#[derive(Clone, Debug)]
pub struct Git {
    pub root: PathBuf,
    exe: String,
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: i64,
    pub behind: i64,
    /// path -> one-letter code, in git's output order
    pub files: IndexMap<String, String>,
}

/// One entry of a commit's name-status: [status, path] or [R|C, new, old].
pub type FileChange = Vec<String>;

#[derive(Clone, Debug)]
#[allow(dead_code)] // the full record git gives (parents, author email): not all of it is served
pub struct Commit {
    pub h: String,
    pub p: Vec<String>,
    pub t: i64,
    pub an: String,
    pub ae: String,
    pub s: String,
    pub x: String,
    pub f: Vec<FileChange>,
    pub a: i64,
    pub d: i64,
}

#[derive(Clone, Debug)]
pub struct Meta {
    pub h: String,
    pub p: Vec<String>,
    pub t: i64,
    pub an: String,
    pub ae: String,
    pub cn: String,
    pub ct: i64,
    pub s: String,
    pub x: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct BlameCommit {
    pub h: String,
    pub t: i64,
    pub s: String,
    pub an: String,
    pub x: Option<String>,
}

pub static REV_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[\p{L}\p{N}_./~^@{}+-]{1,200}$").unwrap());
static SHA_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[0-9a-f]{40,64}$").unwrap());

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

impl Git {
    pub fn new(root: &Path) -> Self {
        Git {
            root: root.to_path_buf(),
            exe: which("git").unwrap_or_else(|| "git".into()),
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.exe);
        cmd.arg("--no-optional-locks")
            .args(["-c", "core.quotepath=false", "-c", "color.ui=never", "-C"])
            .arg(&self.root)
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("LC_ALL", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null());
        cmd
    }

    pub fn run_bytes(&self, args: &[&str], check: bool) -> GitResult<Vec<u8>> {
        let out = self.command(args).output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                GitError("git is not installed or not on PATH".into())
            } else {
                GitError(e.to_string())
            }
        })?;
        if check && !out.status.success() {
            let msg = lossy(&out.stderr).trim().to_string();
            return Err(GitError(if msg.is_empty() {
                format!("git {} failed", args[0])
            } else {
                msg
            }));
        }
        Ok(out.stdout)
    }

    pub fn run(&self, args: &[&str], check: bool) -> GitResult<String> {
        Ok(lossy(&self.run_bytes(args, check)?))
    }

    pub fn toplevel(path: &Path) -> Option<PathBuf> {
        let out = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(PathBuf::from(lossy(&out.stdout).trim()))
    }

    pub fn head(&self) -> Option<String> {
        let out = self
            .run(&["rev-parse", "--verify", "-q", "HEAD"], false)
            .ok()?;
        let out = out.trim();
        (!out.is_empty()).then(|| out.to_string())
    }

    pub fn ls_files(&self) -> GitResult<Vec<String>> {
        let out = self.run_bytes(
            &[
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
            ],
            true,
        )?;
        let mut seen = std::collections::HashSet::new();
        let mut files = Vec::new();
        for raw in out.split(|b| *b == 0) {
            if raw.is_empty() {
                continue;
            }
            let p = lossy(raw);
            if seen.insert(p.clone()) {
                files.push(p);
            }
        }
        Ok(files)
    }

    pub fn status(&self) -> Status {
        let out = self
            .run_bytes(
                &[
                    "status",
                    "--porcelain=v2",
                    "--branch",
                    "-z",
                    "--untracked-files=all",
                ],
                false,
            )
            .unwrap_or_default();
        let mut info = Status::default();
        let items: Vec<&[u8]> = out.split(|b| *b == 0).collect();
        let mut i = 0;
        static AB: Lazy<Regex> = Lazy::new(|| Regex::new(r"^# branch\.ab \+(\d+) -(\d+)").unwrap());
        while i < items.len() {
            let rec = lossy(items[i]);
            i += 1;
            if rec.is_empty() {
                continue;
            }
            if let Some(v) = rec.strip_prefix("# branch.head ") {
                info.branch = Some(v.to_string());
            } else if let Some(v) = rec.strip_prefix("# branch.upstream ") {
                info.upstream = Some(v.to_string());
            } else if rec.starts_with("# branch.ab ") {
                if let Some(m) = AB.captures(&rec) {
                    info.ahead = m[1].parse().unwrap_or(0);
                    info.behind = m[2].parse().unwrap_or(0);
                }
            } else if rec.starts_with("1 ") {
                let parts: Vec<&str> = rec.splitn(9, ' ').collect();
                if parts.len() == 9 {
                    info.files.insert(parts[8].to_string(), xy(parts[1]));
                }
            } else if rec.starts_with("2 ") {
                let parts: Vec<&str> = rec.splitn(10, ' ').collect();
                if parts.len() == 10 {
                    info.files.insert(parts[9].to_string(), "R".into());
                }
                i += 1; // the original path follows as its own NUL-terminated item
            } else if rec.starts_with("u ") {
                let parts: Vec<&str> = rec.splitn(11, ' ').collect();
                if parts.len() == 11 {
                    info.files.insert(parts[10].to_string(), "U".into());
                }
            } else if let Some(p) = rec.strip_prefix("? ") {
                info.files.insert(p.to_string(), "?".into());
            }
        }
        info
    }

    /// Every commit reachable from HEAD, newest first, with name-status and line counts.
    pub fn log_all(&self) -> GitResult<Vec<Commit>> {
        if self.head().is_none() {
            return Ok(vec![]);
        }
        let fmt = "--format=%x1e%H%x1f%P%x1f%at%x1f%an%x1f%ae%x1f%s";
        let out = self.run_bytes(&["log", "-z", "-M", "--name-status", fmt, "HEAD"], true)?;
        let mut commits = Vec::new();
        for chunk in out.split(|b| *b == 0x1e).skip(1) {
            let (header, entries) = match chunk.iter().position(|b| *b == 0) {
                Some(i) => (&chunk[..i], &chunk[i + 1..]),
                None => (chunk, &[][..]),
            };
            let entries = trim_start_newlines(entries);
            let header = lossy(header);
            let mut hf: Vec<String> = header.split('\u{1f}').map(String::from).collect();
            hf.resize(6, String::new());
            let toks: Vec<String> = entries.split(|b| *b == 0).map(lossy).collect();
            let mut files = Vec::new();
            let mut j = 0;
            while j < toks.len() {
                let st = toks[j].trim_matches('\n').to_string();
                j += 1;
                if st.is_empty() {
                    continue;
                }
                let first = st.chars().next().unwrap();
                if first == 'R' || first == 'C' {
                    let (old, new) = (
                        toks.get(j).cloned().unwrap_or_default(),
                        toks.get(j + 1).cloned().unwrap_or_default(),
                    );
                    j += 2;
                    files.push(vec![first.to_string(), new, old]);
                } else {
                    files.push(vec![
                        first.to_string(),
                        toks.get(j).cloned().unwrap_or_default(),
                    ]);
                    j += 1;
                }
            }
            let subject = hf[5].clone();
            commits.push(Commit {
                h: hf[0].clone(),
                p: hf[1].split_whitespace().map(String::from).collect(),
                t: hf[2].parse().unwrap_or(0),
                an: hf[3].clone(),
                ae: hf[4].clone(),
                x: prefix_of(&subject),
                s: subject,
                f: files,
                a: 0,
                d: 0,
            });
        }
        // line counts
        if let Ok(out) = self.run_bytes(
            &["log", "-z", "-M", "--numstat", "--format=%x1e%H", "HEAD"],
            true,
        ) {
            let mut stats = std::collections::HashMap::new();
            for chunk in out.split(|b| *b == 0x1e).skip(1) {
                let (header, entries) = match chunk.iter().position(|b| *b == 0) {
                    Some(i) => (&chunk[..i], &chunk[i + 1..]),
                    None => (chunk, &[][..]),
                };
                let entries = trim_start_newlines(entries);
                let h = lossy(header).trim().to_string();
                let (mut a, mut d) = (0i64, 0i64);
                let toks: Vec<&[u8]> = entries.split(|b| *b == 0).collect();
                let mut j = 0;
                while j < toks.len() {
                    let t = lossy(toks[j]);
                    let t = t.trim_matches('\n');
                    j += 1;
                    if t.is_empty() {
                        continue;
                    }
                    let fields: Vec<&str> = t.split('\t').collect();
                    if fields.len() >= 3 {
                        if fields[0].chars().all(|c| c.is_ascii_digit()) && !fields[0].is_empty() {
                            a += fields[0].parse::<i64>().unwrap_or(0);
                        }
                        if fields[1].chars().all(|c| c.is_ascii_digit()) && !fields[1].is_empty() {
                            d += fields[1].parse::<i64>().unwrap_or(0);
                        }
                        if fields[2].is_empty() {
                            j += 2; // rename: old and new paths follow
                        }
                    }
                }
                stats.insert(h, (a, d));
            }
            for c in commits.iter_mut() {
                if let Some((a, d)) = stats.get(&c.h) {
                    c.a = *a;
                    c.d = *d;
                }
            }
        }
        Ok(commits)
    }

    pub fn diff_tree(&self, sha: &str, parents: &[String]) -> GitResult<(Vec<FileChange>, String)> {
        let base: Vec<&str> = if parents.len() > 1 {
            vec![parents[0].as_str(), sha]
        } else {
            vec!["--root", sha]
        };
        let mut args = vec![
            "diff-tree",
            "-r",
            "-M",
            "--no-commit-id",
            "--name-status",
            "-z",
        ];
        args.extend(base.iter());
        let ns = self.run_bytes(&args, true)?;
        let toks: Vec<String> = ns.split(|b| *b == 0).map(lossy).collect();
        let mut files = Vec::new();
        let mut j = 0;
        while j < toks.len() {
            let st = toks[j].clone();
            j += 1;
            if st.is_empty() {
                continue;
            }
            let first = st.chars().next().unwrap();
            if first == 'R' || first == 'C' {
                files.push(vec![
                    first.to_string(),
                    toks.get(j + 1).cloned().unwrap_or_default(),
                    toks.get(j).cloned().unwrap_or_default(),
                ]);
                j += 2;
            } else {
                files.push(vec![
                    first.to_string(),
                    toks.get(j).cloned().unwrap_or_default(),
                ]);
                j += 1;
            }
        }
        let mut args = vec![
            "diff-tree",
            "-r",
            "-M",
            "-p",
            "--no-commit-id",
            "--no-color",
            "--no-ext-diff",
        ];
        args.extend(base.iter());
        let patch = self.run(&args, true)?;
        Ok((files, patch))
    }

    pub fn show_meta(&self, sha: &str) -> GitResult<Meta> {
        let out = self.run(
            &[
                "show",
                "-s",
                "--format=%H%x1f%P%x1f%at%x1f%an%x1f%ae%x1f%cn%x1f%ct%x1f%s%x1f%b",
                sha,
            ],
            true,
        )?;
        let mut f: Vec<String> = out.split('\u{1f}').map(String::from).collect();
        f.resize(9, String::new());
        let s = f[7].clone();
        Ok(Meta {
            h: f[0].clone(),
            p: f[1].split_whitespace().map(String::from).collect(),
            t: f[2].trim().parse().unwrap_or(0),
            an: f[3].clone(),
            ae: f[4].clone(),
            cn: f[5].clone(),
            ct: f[6].trim().parse().unwrap_or(0),
            x: prefix_of(&s),
            s,
            body: crate::util::py_strip(&f[8]).to_string(),
        })
    }

    pub fn cat(&self, rev: &str, path: &str) -> GitResult<Vec<u8>> {
        self.run_bytes(&["cat-file", "blob", &format!("{rev}:{path}")], true)
    }

    /// A commit sha for rev, or None. Only revision syntax is accepted (never an option).
    pub fn resolve(&self, rev: &str) -> Option<String> {
        if rev.is_empty() || rev.starts_with('-') || !REV_RE.is_match(rev) {
            return None;
        }
        let out = self
            .run(
                &["rev-parse", "--verify", "-q", &format!("{rev}^{{commit}}")],
                false,
            )
            .ok()?;
        let out = out.trim();
        SHA_RE.is_match(out).then(|| out.to_string())
    }

    pub fn blame(
        &self,
        path: &str,
        rev: Option<&str>,
    ) -> GitResult<(Vec<usize>, Vec<BlameCommit>)> {
        let mut args = vec!["blame", "--line-porcelain", "-M", "-C", "-w"];
        if let Some(r) = rev {
            args.push(r);
        }
        args.push("--");
        args.push(path);
        let out = self.run(&args, true)?;
        let mut commits: IndexMap<String, BlameCommit> = IndexMap::new();
        let mut order: IndexMap<String, usize> = IndexMap::new();
        let mut lines = Vec::new();
        let mut cur: Option<String> = None;
        for line in out.split('\n') {
            if line.is_empty() {
                continue;
            }
            if line.starts_with('\t') {
                if let Some(c) = &cur {
                    lines.push(order[c]);
                }
                continue;
            }
            let first = line.split(' ').next().unwrap_or("");
            if first.len() == 40 && first.chars().all(|c| "0123456789abcdef".contains(c)) {
                let c = first.to_string();
                if !order.contains_key(&c) {
                    let n = order.len();
                    order.insert(c.clone(), n);
                    commits.insert(
                        c.clone(),
                        BlameCommit {
                            h: c.clone(),
                            t: 0,
                            s: String::new(),
                            an: String::new(),
                            x: None,
                        },
                    );
                }
                cur = Some(c);
            } else if let Some(c) = &cur {
                let (key, val) = line.split_once(' ').unwrap_or((line, ""));
                let entry = commits.get_mut(c).unwrap();
                match key {
                    "author-time" => entry.t = val.parse().unwrap_or(0),
                    "summary" => {
                        entry.s = val.to_string();
                        entry.x = Some(prefix_of(val));
                    }
                    "author" => entry.an = val.to_string(),
                    _ => {}
                }
            }
        }
        Ok((lines, commits.into_values().collect()))
    }
}

fn trim_start_newlines(b: &[u8]) -> &[u8] {
    let n = b.iter().take_while(|c| **c == b'\n').count();
    &b[n..]
}

fn xy(code: &str) -> String {
    let chars: Vec<char> = code.chars().collect();
    let (x, y) = (
        chars.first().copied().unwrap_or('.'),
        chars.get(1).copied().unwrap_or('.'),
    );
    for c in [y, x] {
        if "MADRCT".contains(c) {
            return match c {
                'T' => "M".into(),
                'C' => "A".into(),
                c => c.to_string(),
            };
        }
    }
    "M".into()
}

pub fn which(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(name);
        if cand.is_file() {
            return Some(cand.display().to_string());
        }
    }
    None
}
