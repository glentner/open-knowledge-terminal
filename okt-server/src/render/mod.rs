//! Per-file payloads for the inspector: rendered markdown, highlighted source, CSV tables, and
//! files as of any commit; plus commit diffs.

pub mod highlight;
pub mod linkify;
pub mod markdown;
pub mod punycode;
mod ucre_data;

use std::sync::Mutex;

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{Map, Value, json};

use crate::config::*;
use crate::git::FileChange;
use crate::index::Index;
use crate::parse::{self, FileEntry, classify, split_frontmatter};
use crate::util::*;

pub use markdown::py_quote;

pub struct Renderer {
    pub md: markdown::Markdown,
    cache: Mutex<IndexMap<(String, u64, i128, u64), Value>>,
}

#[derive(Debug)]
pub enum RenderError {
    NotFound(String),
    Git(String),
}

impl Renderer {
    pub fn new() -> Self {
        Renderer {
            md: markdown::Markdown::new(),
            cache: Mutex::new(IndexMap::new()),
        }
    }

    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
    }

    pub fn payload(
        &self,
        idx: &Index,
        path: &str,
        rev: Option<&str>,
    ) -> Result<Value, RenderError> {
        if let Some(rev) = rev {
            return self.payload_at(idx, path, rev);
        }
        let e = idx
            .files
            .get(path)
            .ok_or_else(|| RenderError::NotFound(path.to_string()))?;
        let key = (path.to_string(), e.size, e.mtime, idx.link_epoch);
        if let Some(v) = self.cache.lock().unwrap().get(&key) {
            return Ok(v.clone());
        }
        let out = self.payload_from(idx, path, &e.kind, None, Some(e), false);
        let mut c = self.cache.lock().unwrap();
        c.insert(key, out.clone());
        while c.len() > 96 {
            c.shift_remove_index(0);
        }
        Ok(out)
    }

    fn payload_at(&self, idx: &Index, path: &str, rev: &str) -> Result<Value, RenderError> {
        let sha = idx
            .git
            .resolve(rev)
            .ok_or_else(|| RenderError::NotFound(format!("unknown revision {rev}")))?;
        let at = idx.path_at(path, &sha);
        let meta = idx.git.show_meta(&sha).map_err(|e| RenderError::Git(e.0))?;
        let short: String = sha.chars().take(7).collect();
        let Some(at) = at else {
            return Err(RenderError::NotFound(format!(
                "{path} did not exist at {short} ({})",
                meta.s
            )));
        };
        let data = idx
            .git
            .cat(&sha, &at)
            .map_err(|_| RenderError::NotFound(format!("{at} is not in {short} ({})", meta.s)))?;
        let kind = classify(&at, &data[..data.len().min(8192)], false);
        let mut out = self.payload_from(idx, &at, &kind, Some(&data), None, true);
        let m = out.as_object_mut().unwrap();
        m.insert("path".into(), json!(path));
        m.insert(
            "at_path".into(),
            if at != path { json!(at) } else { Value::Null },
        );
        m.insert("raw".into(), json!(format!("/raw/{}", py_quote(path))));
        m.insert(
            "rev".into(),
            json!({"h": sha, "t": meta.t, "s": meta.s, "x": meta.x}),
        );
        Ok(out)
    }

    fn payload_from(
        &self,
        idx: &Index,
        path: &str,
        kind: &str,
        data: Option<&[u8]>,
        e: Option<&FileEntry>,
        historic: bool,
    ) -> Value {
        let mut out = Map::new();
        out.insert("path".into(), json!(path));
        out.insert("kind".into(), json!(kind));
        out.insert("rev".into(), Value::Null);
        out.insert("raw".into(), json!(format!("/raw/{}", py_quote(path))));
        if let Some(e) = e {
            out.insert("size".into(), json!(e.size));
            out.insert("lines".into(), json!(e.lines));
            out.insert("words".into(), json!(e.words));
            out.insert("title".into(), json!(e.title));
            out.insert("type".into(), json!(e.type_));
            out.insert(
                "fm".into(),
                e.fm.clone().map(Value::Object).unwrap_or(Value::Null),
            );
            out.insert("fm_error".into(), json!(e.fm_error));
            out.insert("headings".into(), headings_json(&e.headings));
            out.insert("dirty".into(), json!(idx.status.files.get(path)));
            if kind == "symlink" {
                out.insert("link".into(), json!(e.link_target));
                return Value::Object(out);
            }
        }
        if ["pdf", "image", "audio", "video", "binary"].contains(&kind) {
            out.insert(
                "mime".into(),
                json!(guess_mime(path).unwrap_or_else(|| "application/octet-stream".into())),
            );
            if let Some(d) = data {
                out.insert("size".into(), json!(d.len()));
            }
            return Value::Object(out);
        }
        let owned;
        let data: &[u8] = match data {
            Some(d) => d,
            None => match read_limited(&idx.root.join(path), MAX_RENDER_BYTES + 1) {
                Ok(d) => {
                    owned = d;
                    &owned
                }
                Err(err) => {
                    out.insert("error".into(), json!(err.to_string()));
                    return Value::Object(out);
                }
            },
        };
        let truncated = data.len() > MAX_RENDER_BYTES;
        let text = String::from_utf8_lossy(&data[..data.len().min(MAX_RENDER_BYTES)]).into_owned();
        out.insert("truncated".into(), json!(truncated));
        if e.is_none() {
            let lines = text.matches('\n').count()
                + if text.ends_with('\n') || text.is_empty() {
                    0
                } else {
                    1
                };
            out.insert("size".into(), json!(data.len()));
            out.insert("lines".into(), json!(lines));
            out.insert("words".into(), json!(py_split_ws(&text).count()));
        }
        let ext = ext_of(path);
        let mut source: Vec<String>;
        if kind == "markdown" {
            if e.is_none() {
                let (fm_text, body, offset) = split_frontmatter(&text);
                let (fm, err) = match fm_text {
                    Some(f) => parse::parse_frontmatter(&f),
                    None => (None, None),
                };
                let (_, _, heads) = parse::scan_markdown(&body, offset);
                let title = fm
                    .as_ref()
                    .and_then(|m| m.get("title"))
                    .and_then(|t| t.as_str())
                    .map(String::from);
                out.insert("fm".into(), fm.map(Value::Object).unwrap_or(Value::Null));
                out.insert("fm_error".into(), json!(err));
                out.insert("headings".into(), headings_json(&heads));
                out.insert("title".into(), json!(title));
            }
            out.insert(
                "html".into(),
                json!(self.md.render(idx, path, &text, historic)),
            );
            source = text.split('\n').map(py_html_escape).collect();
        } else if kind == "code"
            || kind == "html"
            || (kind == "text" && STRUCTURED_EXT.contains(&ext.as_str()))
        {
            let lang = if ext == ".jsonl" { Some("json") } else { None };
            source = highlight::highlight_lines(&text, Some(path), lang);
        } else {
            source = text.split('\n').map(py_html_escape).collect();
            if TABLE_EXT.contains(&ext.as_str()) {
                out.insert(
                    "table".into(),
                    parse_table(
                        &text,
                        if ext == ".tsv" { Some('\t') } else { None },
                        5000,
                        80,
                    ),
                );
            }
        }
        if source.last().map(|s| s.is_empty()).unwrap_or(false) && text.ends_with('\n') {
            source.pop();
        }
        out.insert("source".into(), json!(source));
        Value::Object(out)
    }
}

fn headings_json(h: &[parse::Heading]) -> Value {
    Value::Array(
        h.iter()
            .map(|h| json!([h.level, h.text, h.slug, h.line]))
            .collect(),
    )
}

fn read_limited(p: &std::path::Path, n: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let f = std::fs::File::open(p)?;
    let mut buf = Vec::new();
    f.take(n as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Python's `mimetypes.guess_type` for the types a KB holds.
pub fn guess_mime(path: &str) -> Option<String> {
    let e = ext_of(path);
    let known = match e.as_str() {
        ".md" | ".markdown" | ".mdown" | ".mkd" => "text/markdown",
        ".txt" | ".text" | ".log" => "text/plain",
        ".py" => "text/x-python",
        ".sh" => "application/x-sh",
        ".js" | ".mjs" => "text/javascript",
        ".json" => "application/json",
        ".yaml" | ".yml" => "application/yaml",
        ".toml" => "application/toml",
        ".csv" => "text/csv",
        ".tsv" => "text/tab-separated-values",
        ".html" | ".htm" => "text/html",
        ".css" => "text/css",
        ".xml" => "application/xml",
        ".svg" => "image/svg+xml",
        ".png" => "image/png",
        ".jpg" | ".jpeg" => "image/jpeg",
        ".gif" => "image/gif",
        ".webp" => "image/webp",
        ".avif" => "image/avif",
        ".bmp" => "image/bmp",
        ".ico" => "image/vnd.microsoft.icon",
        ".pdf" => "application/pdf",
        ".mp3" => "audio/mpeg",
        ".wav" => "audio/x-wav",
        ".ogg" => "audio/ogg",
        ".m4a" => "audio/mp4",
        ".flac" => "audio/flac",
        ".mp4" => "video/mp4",
        ".webm" => "video/webm",
        ".mov" => "video/quicktime",
        ".m4v" => "video/mp4",
        ".zip" => "application/zip",
        ".gz" => "application/gzip",
        ".tar" => "application/x-tar",
        ".docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ".xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ".pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ".doc" => "application/msword",
        ".xls" => "application/vnd.ms-excel",
        ".ppt" => "application/vnd.ms-powerpoint",
        ".ics" => "text/calendar",
        ".rst" => "text/x-rst",
        ".tex" => "text/x-tex",
        _ => "",
    };
    if !known.is_empty() {
        return Some(known.into());
    }
    mime_guess::from_path(path).first_raw().map(String::from)
}

static NUM_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[-+]?(\$|€|£)?\d[\d,]*(\.\d+)?%?$|^[-+]?\.\d+$|^[-+]?\d+(\.\d+)?[eE][-+]?\d+$")
        .unwrap()
});

/// csv.Sniffer's delimiter guess, simplified: the candidate with the most consistent per-line count.
fn sniff_delimiter(sample: &str) -> char {
    let lines: Vec<&str> = sample
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(50)
        .collect();
    let mut best: Option<(char, f64, usize)> = None;
    for d in [',', ';', '\t', '|'] {
        let counts: Vec<usize> = lines.iter().map(|l| l.matches(d).count()).collect();
        let mut freq: IndexMap<usize, usize> = IndexMap::new();
        for c in &counts {
            *freq.entry(*c).or_insert(0) += 1;
        }
        let Some((&mode, &n)) = freq
            .iter()
            .filter(|(c, _)| **c > 0)
            .max_by_key(|(_, n)| **n)
        else {
            continue;
        };
        let consistency = n as f64 / lines.len().max(1) as f64;
        if best
            .map(|b| consistency > b.1 || (consistency == b.1 && mode > b.2))
            .unwrap_or(true)
        {
            best = Some((d, consistency, mode));
        }
    }
    best.map(|b| b.0).unwrap_or(',')
}

pub fn parse_table(text: &str, delim: Option<char>, max_rows: usize, max_cols: usize) -> Value {
    let delim =
        delim.unwrap_or_else(|| sniff_delimiter(&text.chars().take(16384).collect::<String>()));
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delim as u8)
        .from_reader(text.as_bytes());
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (i, rec) in rdr.records().enumerate() {
        if i > max_rows {
            break;
        }
        let Ok(rec) = rec else { return Value::Null };
        rows.push(rec.iter().take(max_cols).map(String::from).collect());
    }
    let rows: Vec<Vec<String>> = rows
        .into_iter()
        .filter(|r| r.iter().any(|c| !py_strip(c).is_empty()))
        .collect();
    if rows.len() < 2 {
        return Value::Null;
    }
    let mut header = rows[0].clone();
    let body: Vec<Vec<String>> = rows[1..rows.len().min(max_rows + 1)].to_vec();
    let width = header
        .len()
        .max(body.iter().map(|r| r.len()).max().unwrap_or(0));
    header.resize(width, String::new());
    let mut num = Vec::new();
    for j in 0..width {
        let vals: Vec<&str> = body
            .iter()
            .filter_map(|r| r.get(j))
            .map(|c| py_strip(c))
            .filter(|c| !c.is_empty())
            .collect();
        let ok = vals.iter().filter(|v| NUM_RE.is_match(v)).count();
        num.push(!vals.is_empty() && ok as f64 >= 0.8 * vals.len() as f64);
    }
    let truncated = rows.len() > max_rows + 1 || text.matches('\n').count() > max_rows + 1;
    json!({"header": header, "rows": body, "numeric": num, "delim": delim.to_string(), "truncated": truncated})
}

/// Split a `git diff-tree -p` patch into per-file hunks with old/new line numbers.
pub fn parse_patch(patch: &str, files: &[FileChange], max_lines: usize) -> Vec<Value> {
    static SPLIT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^diff --git ").unwrap());
    static HUNK: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)").unwrap());
    let sections: Vec<&str> = SPLIT.split(patch).skip(1).collect();
    let mut out = Vec::new();
    let mut budget = max_lines as i64;
    for (i, f) in files.iter().enumerate() {
        let (st, path) = (&f[0], &f[1]);
        let sec = sections.get(i).copied().unwrap_or("");
        let binary = sec
            .split("\n@@")
            .next()
            .unwrap_or("")
            .contains("Binary files");
        let (mut a, mut d) = (0i64, 0i64);
        let mut truncated = false;
        let mut hunks: Vec<(String, i64, i64, Vec<Value>)> = Vec::new();
        let (mut o, mut n) = (0i64, 0i64);
        for line in sec.split('\n').skip(1) {
            if line.starts_with("@@") {
                let Some(m) = HUNK.captures(line) else {
                    continue;
                };
                o = m[1].parse().unwrap_or(0);
                n = m[2].parse().unwrap_or(0);
                hunks.push((line.to_string(), o, n, vec![]));
                continue;
            }
            let Some(h) = hunks.last_mut() else { continue };
            if line.starts_with('\\') {
                continue;
            }
            let tag = line.chars().next();
            if !matches!(tag, Some('+') | Some('-') | Some(' ')) {
                continue;
            }
            let tag = tag.unwrap();
            if tag == '+' {
                a += 1;
            } else if tag == '-' {
                d += 1;
            }
            if budget <= 0 {
                truncated = true;
                continue;
            }
            budget -= 1;
            let rest = &line[1..];
            match tag {
                '+' => {
                    h.3.push(json!(["+", null, n, rest]));
                    n += 1;
                }
                '-' => {
                    h.3.push(json!(["-", o, null, rest]));
                    o += 1;
                }
                _ => {
                    h.3.push(json!([" ", o, n, rest]));
                    o += 1;
                    n += 1;
                }
            }
        }
        out.push(json!({
            "st": st, "path": path, "old": f.get(2), "binary": binary, "a": a, "d": d,
            "hunks": hunks.into_iter().map(|(h, o, n, l)| json!({"h": h, "o": o, "n": n, "lines": l})).collect::<Vec<_>>(),
            "truncated": truncated,
        }));
    }
    out
}
