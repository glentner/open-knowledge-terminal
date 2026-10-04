//! Reading one file: kind, frontmatter, links, mentions, wiki links, URLs, headings, and the
//! external-system tool tokens it names.

use std::path::Path;

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{Map, Value};

use crate::config::*;
use crate::util::*;
use crate::yaml;

// These mirror the reference's Python regexes, so the character classes are spelled with Python's
// meanings: `\w` is [\p{L}\p{N}_] (Rust's also takes marks and connector punctuation, Python's also
// takes No), and `\s` also takes \x1c-\x1f.
pub static FENCE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[\s\x1c-\x1f]{0,3}(`{3,}|~{3,})").unwrap());
pub static LINK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?P<img>!?)\[(?P<text>(?:[^\[\]\n]|\[[^\[\]\n]*\])*)\]\((?P<target><[^>\n]*>|[^)\s\x1c-\x1f]+)(?:[\s\x1c-\x1f]+(?:"[^"\n]*"|'[^'\n]*'))?\)"#).unwrap()
});
pub static REFDEF_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[\s\x1c-\x1f]{0,3}\[(?P<label>[^\]\n]+)\]:[\s\x1c-\x1f]*(?P<target><[^>\n]+>|[^\s\x1c-\x1f]+)").unwrap()
});
pub static WIKI_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\[\[(?P<target>[^\]|#\n]+)(?:#[^\]|\n]*)?(?:\|(?P<alias>[^\]\n]+))?\]\]").unwrap()
});
pub static CODE_RE: Lazy<fancy_regex::Regex> =
    Lazy::new(|| fancy_regex::Regex::new(r"(?<!`)`(?P<code>[^`\n]+)`(?!`)").unwrap());
pub static URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"https?://[^\s\x1c-\x1f<>()\[\]`"']+"#).unwrap());
pub static HEADING_RE_M: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^ {0,3}(#{1,6})[ \t]+(.+?)[ \t]*#*[ \t]*$").unwrap());
pub static PATHLIKE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[~.\p{L}\p{N}_@][\p{L}\p{N}_@.+~/-]*$").unwrap());
pub static SCHEME_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.-]*:").unwrap());
static REFDEF_TARGET_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[./#:]").unwrap());
static TITLE_TAG_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap());
static WS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\s\x1c-\x1f]+").unwrap());
static MD_EMPH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[`*_]").unwrap());

/// One raw reference found in a file: (kind, target, line, text).
#[derive(Clone, Debug)]
pub struct RawRef {
    pub kind: &'static str,
    pub target: String,
    pub line: usize,
    pub text: String,
}

/// A heading: [level, text, slug, line].
#[derive(Clone, Debug)]
pub struct Heading {
    pub level: usize,
    pub text: String,
    pub slug: String,
    pub line: usize,
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct FileEntry {
    pub path: String,
    pub kind: String,
    pub size: u64,
    pub mtime: i128, // nanoseconds
    pub title: Option<String>,
    pub fm: Option<Map<String, Value>>,
    pub fm_error: Option<String>,
    pub type_: Option<String>,
    pub headings: Vec<Heading>,
    pub raw: Vec<RawRef>,
    pub urls: Vec<String>,
    /// surface id -> token, in SURFACES order
    pub tools: Vec<(String, String)>,
    pub lines: usize,
    pub words: usize,
    pub text: Option<String>,
    pub link_target: Option<String>,
    pub aliases: Vec<String>,
}

pub fn slugify(text: &str) -> String {
    static TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").unwrap());
    static MARK: Lazy<Regex> = Lazy::new(|| Regex::new(r"[`*_~\[\]()]").unwrap());
    static NONWORD: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^\p{L}\p{N}_\- ]+").unwrap());
    static WS: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\s\x1c-\x1f]+").unwrap());
    let s = TAG.replace_all(text, "");
    let s = MARK.replace_all(&s, "");
    let s = py_strip(&s).to_lowercase();
    let s = NONWORD.replace_all(&s, "");
    let s = WS.replace_all(&s, "-");
    if s.is_empty() {
        "section".into()
    } else {
        s.into_owned()
    }
}

/// (frontmatter text or None, body, number of lines the frontmatter occupied)
pub fn split_frontmatter(text: &str) -> (Option<String>, String, usize) {
    if !text.starts_with("---") {
        return (None, text.to_string(), 0);
    }
    let lines: Vec<&str> = text.split('\n').collect();
    if py_strip(lines[0]) != "---" {
        return (None, text.to_string(), 0);
    }
    for i in 1..lines.len().min(2000) {
        let l = py_strip(lines[i]);
        if l == "---" || l == "..." {
            return (
                Some(lines[1..i].join("\n")),
                lines[i + 1..].join("\n"),
                i + 1,
            );
        }
    }
    (None, text.to_string(), 0)
}

pub fn parse_frontmatter(fm: &str) -> (Option<Map<String, Value>>, Option<String>) {
    yaml::parse_frontmatter(fm)
}

pub fn classify(path: &str, head: &[u8], is_link: bool) -> String {
    if is_link {
        return "symlink".into();
    }
    let e = ext_of(path);
    let name = basename(path);
    let e = e.as_str();
    if MARKDOWN_EXT.contains(&e) {
        return "markdown".into();
    }
    if PDF_EXT.contains(&e) {
        return "pdf".into();
    }
    if HTML_EXT.contains(&e) {
        return "html".into();
    }
    if IMAGE_EXT.contains(&e) {
        return "image".into();
    }
    if AUDIO_EXT.contains(&e) {
        return "audio".into();
    }
    if VIDEO_EXT.contains(&e) {
        return "video".into();
    }
    let h = &head[..head.len().min(8192)];
    if h.contains(&0) {
        return "binary".into();
    }
    if CODE_EXT.contains(&e) || CODE_NAMES.contains(&name) {
        return "code".into();
    }
    if head.starts_with(b"#!") {
        return "code".into();
    }
    if TEXT_EXT.contains(&e) || e.is_empty() {
        return "text".into();
    }
    if std::str::from_utf8(&head[..head.len().min(4096)]).is_ok()
        || utf8_prefix_ok(&head[..head.len().min(4096)])
    {
        return "text".into();
    }
    "binary".into()
}

/// Python's `head[:4096].decode('utf-8')` fails on a truncated trailing sequence too; only a fully
/// valid prefix counts.
fn utf8_prefix_ok(_b: &[u8]) -> bool {
    false
}

/// Links, mentions, wiki links, URLs, and headings from a markdown body. Fenced code is blanked
/// (line count preserved) so its contents never become links or headings.
pub fn scan_markdown(body: &str, offset: usize) -> (Vec<RawRef>, Vec<String>, Vec<Heading>) {
    let mut lines: Vec<String> = body.split('\n').map(String::from).collect();
    let mut in_fence = false;
    let mut fence = String::new();
    for i in 0..lines.len() {
        let line = lines[i].clone();
        let head4 = take_chars(&line, 4);
        let lead = take_chars(head4.trim_start_matches(is_py_space), 3).to_string();
        if lead == "```" || lead == "~~~" {
            if let Some(m) = FENCE_RE.captures(&line) {
                let tick = m.get(1).unwrap().as_str();
                if !in_fence {
                    in_fence = true;
                    fence = tick
                        .chars()
                        .next()
                        .unwrap()
                        .to_string()
                        .repeat(tick.chars().count());
                    lines[i].clear();
                    continue;
                }
                if py_strip(&line).starts_with(fence.as_str()) {
                    in_fence = false;
                    lines[i].clear();
                    continue;
                }
            }
        }
        if in_fence {
            lines[i].clear();
        }
    }
    let prose = lines.join("\n");
    let mut starts = vec![0usize];
    for line in &lines[..lines.len().saturating_sub(1)] {
        let last = *starts.last().unwrap();
        starts.push(last + line.len() + 1);
    }
    let ln = |pos: usize| -> usize { starts.partition_point(|s| *s <= pos) + offset };
    let mut raw = Vec::new();
    let mut urls = Vec::new();
    let mut headings = Vec::new();
    if prose.contains('#') {
        for m in HEADING_RE_M.captures_iter(&prose) {
            let whole = m.get(0).unwrap();
            headings.push(Heading {
                level: m[1].len(),
                text: py_strip(&m[2]).to_string(),
                slug: String::new(),
                line: ln(whole.start()),
            });
        }
    }
    // inline code: a path in backticks is a mention; anything else in backticks is literal text,
    // so code spans are blanked (same length, so positions hold) before looking for links
    let mut plain = prose.clone();
    if prose.contains('`') {
        let mut blanks: Vec<(usize, usize)> = Vec::new();
        for m in CODE_RE.captures_iter(&prose).flatten() {
            let whole = m.get(0).unwrap();
            let code = py_strip(m.name("code").unwrap().as_str());
            if PATHLIKE_RE.is_match(code)
                && (code.contains('/') || is_known_file_ext(&ext_of(code)))
            {
                raw.push(RawRef {
                    kind: "mention",
                    target: code.to_string(),
                    line: ln(whole.start()),
                    text: String::new(),
                });
            }
            blanks.push((whole.start(), whole.end()));
        }
        if !blanks.is_empty() {
            let mut bytes = prose.clone().into_bytes();
            for (a, b) in blanks {
                for x in &mut bytes[a..b] {
                    *x = b' ';
                }
            }
            plain = String::from_utf8(bytes).unwrap_or(plain);
        }
    }
    if plain.contains("](") {
        for m in LINK_RE.captures_iter(&plain) {
            let whole = m.get(0).unwrap();
            let target = m
                .name("target")
                .unwrap()
                .as_str()
                .trim_matches(|c| c == '<' || c == '>');
            raw.push(RawRef {
                kind: if m.name("img").unwrap().as_str().is_empty() {
                    "link"
                } else {
                    "embed"
                },
                target: target.to_string(),
                line: ln(whole.start()),
                text: take_chars(m.name("text").unwrap().as_str(), 80).to_string(),
            });
        }
    }
    if plain.contains("[[") {
        for m in WIKI_RE.captures_iter(&plain) {
            let whole = m.get(0).unwrap();
            raw.push(RawRef {
                kind: "wiki",
                target: py_strip(m.name("target").unwrap().as_str()).to_string(),
                line: ln(whole.start()),
                text: m
                    .name("alias")
                    .map(|a| py_strip(a.as_str()).to_string())
                    .unwrap_or_default(),
            });
        }
    }
    if prose.contains("://") {
        urls = URL_RE
            .find_iter(&prose)
            .map(|u| {
                u.as_str()
                    .trim_end_matches(|c| ".,;:!?*_".contains(c))
                    .to_string()
            })
            .collect();
    }
    if plain.contains("]:") {
        for m in REFDEF_RE.captures_iter(&plain) {
            let whole = m.get(0).unwrap();
            let tgt = m
                .name("target")
                .unwrap()
                .as_str()
                .trim_matches(|c| c == '<' || c == '>');
            if REFDEF_TARGET_RE.is_match(tgt) {
                raw.push(RawRef {
                    kind: "link",
                    target: tgt.to_string(),
                    line: ln(whole.start()),
                    text: take_chars(m.name("label").unwrap().as_str(), 80).to_string(),
                });
            }
        }
    }
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for h in headings.iter_mut() {
        let base = slugify(&h.text);
        let n = seen.entry(base.clone()).or_insert(0);
        *n += 1;
        h.slug = if *n == 1 {
            base
        } else {
            format!("{}-{}", base, *n - 1)
        };
    }
    (raw, urls, headings)
}

pub fn tool_hits(text: &str) -> Vec<(String, String)> {
    let low = text.to_lowercase();
    let mut hits = Vec::new();
    for s in SURFACES {
        for tok in s.tokens {
            if low.contains(&tok.to_lowercase()) {
                hits.push((s.id.to_string(), tok.to_lowercase()));
                break;
            }
        }
    }
    hits
}

pub struct Stat {
    pub size: u64,
    pub mtime_ns: i128,
    pub is_link: bool,
}

pub fn lstat(full: &Path) -> std::io::Result<Stat> {
    let md = std::fs::symlink_metadata(full)?;
    let mtime_ns = {
        use std::os::unix::fs::MetadataExt;
        md.mtime() as i128 * 1_000_000_000 + md.mtime_nsec() as i128
    };
    Ok(Stat {
        size: md.len(),
        mtime_ns,
        is_link: md.file_type().is_symlink(),
    })
}

pub fn parse_file(root: &Path, path: &str, st: &Stat) -> FileEntry {
    let mut e = FileEntry {
        path: path.to_string(),
        size: st.size,
        mtime: st.mtime_ns,
        ..Default::default()
    };
    let full = root.join(path);
    let name = basename(path).to_string();
    if st.is_link {
        e.kind = "symlink".into();
        e.link_target = match std::fs::read_link(&full) {
            Ok(t) => {
                let t = t.to_string_lossy().to_string();
                if t.starts_with('/') {
                    None
                } else {
                    norm_rel(&posix_join(posix_dirname(path), &t))
                }
            }
            Err(_) => None,
        };
        e.title = Some(name);
        return e;
    }
    let data = match read_head(&full, st.size) {
        Ok(d) => d,
        Err(_) => {
            e.kind = "binary".into();
            e.title = Some(name);
            return e;
        }
    };
    let head = &data[..data.len().min(8192)];
    e.kind = classify(path, head, false);
    if ["markdown", "code", "text", "html"].contains(&e.kind.as_str()) {
        let text = String::from_utf8_lossy(&data).into_owned();
        e.lines = text.matches('\n').count()
            + if text.ends_with('\n') || text.is_empty() {
                0
            } else {
                1
            };
        e.words = py_split_ws(&text).count();
        if e.kind == "markdown" {
            let (fm_text, body, offset) = split_frontmatter(&text);
            if let Some(fm_text) = fm_text {
                let (fm, err) = parse_frontmatter(&fm_text);
                e.fm = fm;
                e.fm_error = err;
            }
            let (raw, urls, headings) = scan_markdown(&body, offset);
            e.raw = raw;
            e.urls = urls;
            e.headings = headings;
            let fm = e.fm.clone().unwrap_or_default();
            e.type_ = match fm.get("type") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) if s.is_empty() => None,
                Some(t) => Some(py_strip(py_str(t).split('#').next().unwrap_or("")).to_string()),
            };
            for key in ["title", "name"] {
                if let Some(Value::String(v)) = fm.get(key) {
                    if !py_strip(v).is_empty() && !is_todo(&Value::String(v.clone())) {
                        e.title = Some(py_strip(v).to_string());
                        break;
                    }
                }
            }
            if e.title.is_none() {
                e.title = e
                    .headings
                    .iter()
                    .find(|h| h.level == 1)
                    .map(|h| MD_EMPH_RE.replace_all(&h.text, "").into_owned());
            }
            match fm.get("aliases") {
                Some(Value::Array(al)) => {
                    e.aliases = al
                        .iter()
                        .filter(|a| py_truthy(a))
                        .map(py_str)
                        .take(12)
                        .collect();
                }
                Some(Value::String(s)) if !py_strip(s).is_empty() => {
                    e.aliases = vec![py_strip(s).to_string()]
                }
                _ => {}
            }
            e.tools = tool_hits(&text);
        } else if e.kind == "code" || e.kind == "text" {
            e.tools = if e.kind == "code" {
                tool_hits(&text)
            } else {
                vec![]
            };
            e.urls = URL_RE
                .find_iter(&text)
                .map(|u| {
                    u.as_str()
                        .trim_end_matches(|c| ".,;:!?".contains(c))
                        .to_string()
                })
                .take(200)
                .collect();
        } else if e.kind == "html" {
            if let Some(m) = TITLE_TAG_RE.captures(&text) {
                let t = html_unescape(&WS_RE.replace_all(&m[1], " "));
                let t = take_chars(py_strip(&t), 120).to_string();
                e.title = if t.is_empty() { None } else { Some(t) };
            }
        }
        e.text = Some(text);
    }
    if e.title.as_deref().map(|t| t.is_empty()).unwrap_or(true) {
        e.title = Some(name);
    }
    e
}

fn read_head(full: &Path, size: u64) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut f = std::fs::File::open(full)?;
    let mut head = vec![0u8; 8192];
    let mut n = 0;
    while n < head.len() {
        let k = f.read(&mut head[n..])?;
        if k == 0 {
            break;
        }
        n += k;
    }
    head.truncate(n);
    if size > 8192 {
        let mut rest = Vec::new();
        f.take(MAX_TEXT_BYTES as u64).read_to_end(&mut rest)?;
        head.extend(rest);
    }
    Ok(head)
}

/// Python's `html.unescape` for the common cases.
pub fn html_unescape(s: &str) -> String {
    html_escape_decode(s)
}

fn html_escape_decode(s: &str) -> String {
    // Named entities beyond the basics are rare in <title>; numeric and the common named ones
    // cover what appears in practice.
    static ENT: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"&(#[0-9]+|#[xX][0-9a-fA-F]+|[a-zA-Z]+);").unwrap());
    ENT.replace_all(s, |c: &regex::Captures| {
        let e = &c[1];
        if let Some(num) = e.strip_prefix('#') {
            let v = if let Some(h) = num.strip_prefix(['x', 'X']) {
                u32::from_str_radix(h, 16).ok()
            } else {
                num.parse().ok()
            };
            return v
                .and_then(char::from_u32)
                .map(|c| c.to_string())
                .unwrap_or_else(|| c[0].to_string());
        }
        match e {
            "amp" => "&".into(),
            "lt" => "<".into(),
            "gt" => ">".into(),
            "quot" => "\"".into(),
            "apos" => "'".into(),
            "nbsp" => "\u{a0}".into(),
            "mdash" => "—".into(),
            "ndash" => "–".into(),
            "hellip" => "…".into(),
            "copy" => "©".into(),
            _ => c[0].to_string(),
        }
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(
            slugify("Agentic AI Config — Puppet Wiring Playbook"),
            "agentic-ai-config-puppet-wiring-playbook"
        );
        assert_eq!(slugify("`code` and *em*"), "code-and-em");
        assert_eq!(slugify("!!!"), "section");
    }

    #[test]
    fn scan() {
        let (raw, urls, heads) = scan_markdown(
            "# Title\nsee `contact/x.md` and [y](../y.md)\n```\n[z](z.md)\n```\nhttps://a.b/c.\n",
            3,
        );
        assert_eq!(heads.len(), 1);
        assert_eq!(heads[0].line, 4);
        assert_eq!(raw.len(), 2);
        assert_eq!(raw[0].kind, "mention");
        assert_eq!(raw[1].target, "../y.md");
        assert_eq!(raw[1].line, 5);
        assert_eq!(urls, vec!["https://a.b/c".to_string()]);
    }
}
