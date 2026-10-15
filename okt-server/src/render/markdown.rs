//! Markdown to HTML, matching markdown-it-py's commonmark preset with tables, strikethrough,
//! linkify, front matter, task lists, footnotes and definition lists, plus okt's token rewriting:
//! `data-line` on blocks, heading ids, KB links resolved to `#/f/...`, images served from `/raw/`,
//! path-like code spans and `[[wiki]]` links turned into KB links. The output is sanitized with
//! ammonia (the library Python's nh3 wraps).

use std::collections::{HashMap, HashSet};

use markdown_it::parser::core::{CoreRule, Root};
use markdown_it::parser::extset::NodeExt;
use markdown_it::parser::inline::builtin::InlineParserRule;
use markdown_it::parser::inline::{InlineRoot, InlineRule, InlineState, Text, TextSpecial};
use markdown_it::parser::linkfmt::LinkFormatter;
use markdown_it::plugins::cmark::block::blockquote::Blockquote;
use markdown_it::plugins::cmark::block::code::CodeBlock;
use markdown_it::plugins::cmark::block::fence::CodeFence;
use markdown_it::plugins::cmark::block::heading::ATXHeading;
use markdown_it::plugins::cmark::block::hr::ThematicBreak;
use markdown_it::plugins::cmark::block::lheading::SetextHeader;
use markdown_it::plugins::cmark::block::list::{BulletList, ListItem, OrderedList};
use markdown_it::plugins::cmark::block::paragraph::Paragraph;
use markdown_it::plugins::cmark::inline::autolink::Autolink;
use markdown_it::plugins::cmark::inline::backticks::CodeInline;
use markdown_it::plugins::cmark::inline::emphasis::{Em, Strong};
use markdown_it::plugins::cmark::inline::image::Image;
use markdown_it::plugins::cmark::inline::link::Link;
use markdown_it::plugins::extra::strikethrough::Strikethrough;
use markdown_it::plugins::extra::tables::{Table, TableBody, TableHead, TableRow};
use markdown_it::plugins::html::html_block::HtmlBlock;
use markdown_it::plugins::html::html_inline::HtmlInline;
use markdown_it::{MarkdownIt, Node, NodeValue, Renderer};
use markdown_it_front_matter::FrontMatter;
use once_cell::sync::Lazy;
use regex::Regex;

use super::linkify;
use super::punycode;
use crate::index::Index;
use crate::parse::{PATHLIKE_RE, WIKI_RE, slugify};
use crate::util::*;

// ---- link formatting with markdown-it-py's semantics ------------------------------------------

#[derive(Debug, Default)]
pub struct PyLinkFormatter;

fn recode_host(proto: &Option<String>) -> bool {
    match proto {
        None => true,
        Some(p) => p == "http:" || p == "https:" || p == "mailto:",
    }
}

pub fn normalize_link(url: &str) -> String {
    let mut parsed = mdurl::parse_url(url);
    if let Some(h) = &parsed.hostname {
        if !h.is_empty() && recode_host(&parsed.protocol) {
            parsed.hostname = Some(punycode::to_ascii(h));
        }
    }
    mdurl::urlencode::encode(
        &parsed.to_string(),
        mdurl::urlencode::ENCODE_DEFAULT_CHARS,
        true,
    )
    .into_owned()
}

pub fn normalize_link_text(url: &str) -> String {
    let mut parsed = mdurl::parse_url(url);
    if let Some(h) = &parsed.hostname {
        if !h.is_empty() && recode_host(&parsed.protocol) {
            parsed.hostname = Some(punycode::to_unicode(h));
        }
    }
    let exclude = mdurl::urlencode::DECODE_DEFAULT_CHARS.add(b'%');
    mdurl::urlencode::decode(&parsed.to_string(), exclude).into_owned()
}

pub fn validate_link(url: &str) -> bool {
    static BAD: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^(vbscript|javascript|file|data):").unwrap());
    static GOOD: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^data:image/(gif|png|jpeg|webp);").unwrap());
    let u = py_strip(url).to_lowercase();
    if BAD.is_match(&u) {
        GOOD.is_match(&u)
    } else {
        true
    }
}

impl LinkFormatter for PyLinkFormatter {
    fn validate_link(&self, url: &str) -> Option<()> {
        validate_link(url).then_some(())
    }
    fn normalize_link(&self, url: &str) -> String {
        normalize_link(url)
    }
    fn normalize_link_text(&self, url: &str) -> String {
        normalize_link_text(url)
    }
}

// ---- nodes rendered with an exact attribute order ---------------------------------------------

/// `<a ...>` with attributes in a fixed order, as markdown-it-py emits them.
#[derive(Debug)]
pub struct A {
    pub attrs: Vec<(&'static str, String)>,
    /// produced by linkify (inline or core)
    #[allow(dead_code)]
    pub auto: bool,
}

impl NodeValue for A {
    fn render(&self, node: &Node, fmt: &mut dyn Renderer) {
        fmt.open("a", &self.attrs);
        fmt.contents(&node.children);
        fmt.close("a");
    }
}

#[derive(Debug)]
pub struct Img {
    pub attrs: Vec<(&'static str, String)>,
}

impl NodeValue for Img {
    fn render(&self, _node: &Node, fmt: &mut dyn Renderer) {
        fmt.self_close("img", &self.attrs);
    }
}

/// `<ol start=.. data-line=..>`: markdown-it sets `start` when the list is created.
#[derive(Debug)]
pub struct Ol;

impl NodeValue for Ol {
    fn render(&self, node: &Node, fmt: &mut dyn Renderer) {
        fmt.cr();
        fmt.open("ol", &node.attrs);
        fmt.cr();
        fmt.contents(&node.children);
        fmt.cr();
        fmt.close("ol");
        fmt.cr();
    }
}

/// Indented code: markdown-it puts the block's attributes on `<pre>`.
#[derive(Debug)]
pub struct Pre {
    pub content: String,
}

impl NodeValue for Pre {
    fn render(&self, node: &Node, fmt: &mut dyn Renderer) {
        fmt.cr();
        fmt.open("pre", &node.attrs);
        fmt.open("code", &[]);
        fmt.text(&self.content);
        fmt.close("code");
        fmt.close("pre");
        fmt.cr();
    }
}

/// The heading's raw inline source, captured before inline parsing (markdown-it's `inline.content`).
#[derive(Debug, Default)]
struct HeadingSource(String);
impl NodeExt for HeadingSource {}

/// Raw inline source of a paragraph, for linkify's pretest and the task-list check.
#[derive(Debug, Default)]
struct InlineSource(String);
impl NodeExt for InlineSource {}

// ---- core rules ----------------------------------------------------------------------------------

/// Record the raw inline content of headings and paragraphs before the inline parser replaces it.
struct CaptureInline;
impl CoreRule for CaptureInline {
    fn run(root: &mut Node, _: &MarkdownIt) {
        root.walk_mut(|node, _| {
            let content = node
                .children
                .first()
                .and_then(|c| c.cast::<InlineRoot>())
                .map(|r| r.content.clone());
            if let Some(content) = content {
                if node.is::<ATXHeading>() || node.is::<SetextHeader>() {
                    node.ext.insert(HeadingSource(content.clone()));
                }
                node.ext.insert(InlineSource(content));
            }
        });
    }
}

static GFM_TODO_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\[[ xX]\][ \t\n\x0B\x0C\r]+").unwrap());

/// GitHub task lists with disabled checkboxes (mdit-py-plugins `tasklists`).
struct TaskLists;
impl CoreRule for TaskLists {
    fn run(root: &mut Node, _: &MarkdownIt) {
        fn visit(node: &mut Node) {
            let is_list = node.is::<BulletList>() || node.is::<OrderedList>();
            let mut any = false;
            for child in node.children.iter_mut() {
                if is_list && child.is::<ListItem>() && todoify_item(child) {
                    any = true;
                }
                visit(child);
            }
            if any {
                set_attr(&mut node.attrs, "class", "contains-task-list".into());
            }
        }
        visit(root);
    }
}

/// Returns true when the item became a task item.
fn todoify_item(item: &mut Node) -> bool {
    // loose: the first child is a paragraph; tight: the paragraph's inline nodes are the item's own
    let (content, inline_children): (Option<String>, &mut Vec<Node>) = {
        if item
            .children
            .first()
            .map(|c| c.is::<Paragraph>())
            .unwrap_or(false)
        {
            let p = &mut item.children[0];
            let c = p.ext.get::<InlineSource>().map(|s| s.0.clone());
            (c, &mut p.children)
        } else {
            let c = item.ext.get::<InlineSource>().map(|s| s.0.clone());
            (c, &mut item.children)
        }
    };
    let Some(content) = content else { return false };
    if !GFM_TODO_RE.is_match(&content) {
        return false;
    }
    let Some(first) = inline_children.first_mut() else {
        return false;
    };
    let Some(text) = first.cast_mut::<Text>() else {
        return false;
    };
    let checkbox = if content.starts_with("[ ] ") {
        r#"<input class="task-list-item-checkbox" disabled="disabled" type="checkbox">"#.to_string()
    } else if content.starts_with("[x] ") || content.starts_with("[X] ") {
        r#"<input class="task-list-item-checkbox" checked="checked" disabled="disabled" type="checkbox">"#.to_string()
    } else {
        String::new()
    };
    text.content = text.content.chars().skip(3).collect();
    inline_children.insert(0, Node::new(HtmlInline { content: checkbox }));
    set_attr(&mut item.attrs, "class", "task-list-item".into());
    true
}

fn set_attr(attrs: &mut Vec<(&'static str, String)>, k: &'static str, v: String) {
    if let Some(slot) = attrs.iter_mut().find(|(n, _)| *n == k) {
        slot.1 = v;
    } else {
        attrs.push((k, v));
    }
}

fn get_attr<'a>(attrs: &'a [(&'static str, String)], k: &str) -> Option<&'a str> {
    attrs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str())
}

/// Schema links found while parsing inline text (markdown-it-py's inline `linkify` rule).
struct LinkifyInline;
impl InlineRule for LinkifyInline {
    const MARKER: char = ':';

    fn run(state: &mut InlineState) -> Option<(Node, usize)> {
        static SCHEME_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"(?i)(?:^|[^a-z0-9.+-])([a-z][a-z0-9.+-]*)$").unwrap());
        if state.link_level > 0 {
            return None;
        }
        let pos = state.pos;
        if pos + 3 > state.pos_max || !state.src[pos..].starts_with("://") {
            return None;
        }
        let trailing = state.trailing_text_get().to_string();
        let m = SCHEME_RE.captures(&trailing)?;
        let proto = m.get(1).unwrap().as_str().to_string();
        if proto.len() > pos {
            return None;
        }
        let link = linkify::match_at_start(&state.src[pos - proto.len()..])?;
        let url = link.url.trim_end_matches('*').to_string();
        let full_url = normalize_link(&url);
        if !validate_link(&full_url) {
            return None;
        }
        let mut node = Node::new(A {
            attrs: vec![("href", full_url)],
            auto: true,
        });
        node.children.push(Node::new(Text {
            content: normalize_link_text(&url),
        }));
        state.trailing_text_pop(proto.len());
        state.pos -= proto.len();
        Some((node, url.len()))
    }
}

/// Fuzzy links and emails in text (markdown-it-py's core `linkify` rule).
struct LinkifyCore;
impl CoreRule for LinkifyCore {
    fn run(root: &mut Node, _: &MarkdownIt) {
        root.walk_mut(|node, _| {
            let Some(src) = node.ext.get::<InlineSource>().map(|s| s.0.clone()) else {
                return;
            };
            if !linkify::pretest(&src) {
                return;
            }
            let mut level = 0i32;
            linkify_children(&mut node.children, &mut level);
        });
    }
}

fn is_link_node(n: &Node) -> bool {
    n.is::<Link>() || n.is::<A>() || n.is::<Autolink>()
}

static LINK_OPEN_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^<a[>\s\x1c-\x1f]").unwrap());
static LINK_CLOSE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^</a[\s\x1c-\x1f]*>").unwrap());

/// Scan children from the end, like the flat token scan, recursing into emphasis containers.
fn linkify_children(children: &mut Vec<Node>, html_level: &mut i32) {
    let mut i = children.len();
    while i > 0 {
        i -= 1;
        if is_link_node(&children[i]) {
            continue;
        }
        if let Some(h) = children[i].cast::<HtmlInline>() {
            if LINK_OPEN_RE.is_match(&h.content) && *html_level > 0 {
                *html_level -= 1;
            }
            if LINK_CLOSE_RE.is_match(&h.content) {
                *html_level += 1;
            }
        }
        if *html_level > 0 {
            continue;
        }
        if children[i].is::<Em>() || children[i].is::<Strong>() || children[i].is::<Strikethrough>()
        {
            linkify_children(&mut children[i].children, html_level);
            continue;
        }
        let Some(text) = children[i].cast::<Text>().map(|t| t.content.clone()) else {
            continue;
        };
        if !linkify::test(&text) {
            continue;
        }
        let mut links = linkify::matches(&text);
        if !links.is_empty() && links[0].index == 0 && i > 0 && children[i - 1].is::<TextSpecial>()
        {
            links.remove(0);
        }
        let mut nodes = Vec::new();
        let mut last = 0;
        for l in links {
            let full = normalize_link(&l.url);
            if !validate_link(&full) {
                continue;
            }
            let url_text = if l.schema.is_empty() {
                let s = normalize_link_text(&format!("http://{}", l.text));
                s.strip_prefix("http://").map(String::from).unwrap_or(s)
            } else if l.schema == "mailto:" && l.text.to_lowercase().starts_with("mailto:") {
                // (markdown-it-py tests the text itself here, where markdown-it tests its negation)
                let s = normalize_link_text(&format!("mailto:{}", l.text));
                s.strip_prefix("mailto:").map(String::from).unwrap_or(s)
            } else {
                normalize_link_text(&l.text)
            };
            if l.index > last {
                nodes.push(Node::new(Text {
                    content: text[last..l.index].to_string(),
                }));
            }
            let mut a = Node::new(A {
                attrs: vec![("href", full)],
                auto: true,
            });
            a.children.push(Node::new(Text { content: url_text }));
            nodes.push(a);
            last = l.last_index;
        }
        if nodes.is_empty() && last == 0 {
            continue;
        }
        if last < text.len() {
            nodes.push(Node::new(Text {
                content: text[last..].to_string(),
            }));
        }
        children.splice(i..i + 1, nodes);
    }
}

/// text_special becomes text and adjacent text joins (markdown-it-py's `text_join`).
struct TextJoin;
impl CoreRule for TextJoin {
    fn run(root: &mut Node, _: &MarkdownIt) {
        root.walk_mut(|node, _| {
            if !node
                .children
                .iter()
                .any(|c| c.is::<Text>() || c.is::<TextSpecial>())
            {
                return;
            }
            let old = std::mem::take(&mut node.children);
            let mut out: Vec<Node> = Vec::with_capacity(old.len());
            for c in old {
                let content = if let Some(t) = c.cast::<Text>() {
                    Some(t.content.clone())
                } else {
                    c.cast::<TextSpecial>().map(|t| t.content.clone())
                };
                match content {
                    Some(s) => {
                        if let Some(prev) = out.last_mut().and_then(|p| p.cast_mut::<Text>()) {
                            prev.content.push_str(&s);
                        } else {
                            let mut n = Node::new(Text { content: s });
                            n.srcmap = c.srcmap;
                            out.push(n);
                        }
                    }
                    None => out.push(c),
                }
            }
            node.children = out;
        });
    }
}

// ---- the parser ------------------------------------------------------------------------------------

pub struct Markdown {
    md: MarkdownIt,
    sanitizer: ammonia::Builder<'static>,
}

pub const ALLOWED_TAGS: &[&str] = &[
    "a",
    "abbr",
    "b",
    "blockquote",
    "br",
    "code",
    "dd",
    "del",
    "details",
    "div",
    "dl",
    "dt",
    "em",
    "figure",
    "figcaption",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hr",
    "i",
    "img",
    "input",
    "ins",
    "kbd",
    "li",
    "mark",
    "ol",
    "p",
    "pre",
    "q",
    "s",
    "samp",
    "section",
    "small",
    "span",
    "strong",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "u",
    "ul",
    "var",
    "wbr",
    "caption",
    "colgroup",
    "col",
];

fn sanitizer() -> ammonia::Builder<'static> {
    let mut b = ammonia::Builder::default();
    b.tags(ALLOWED_TAGS.iter().copied().collect());
    b.generic_attributes(
        [
            "class",
            "id",
            "title",
            "data-line",
            "data-kb",
            "data-href",
            "lang",
            "dir",
        ]
        .into_iter()
        .collect(),
    );
    let mut ta: HashMap<&'static str, HashSet<&'static str>> = HashMap::new();
    ta.insert("a", ["href", "target", "rel"].into_iter().collect());
    ta.insert(
        "img",
        ["src", "alt", "width", "height", "loading"]
            .into_iter()
            .collect(),
    );
    ta.insert(
        "input",
        ["type", "checked", "disabled"].into_iter().collect(),
    );
    ta.insert(
        "th",
        ["colspan", "rowspan", "align", "style"]
            .into_iter()
            .collect(),
    );
    ta.insert(
        "td",
        ["colspan", "rowspan", "align", "style"]
            .into_iter()
            .collect(),
    );
    ta.insert("ol", ["start", "type"].into_iter().collect());
    ta.insert("li", ["value"].into_iter().collect());
    ta.insert("details", ["open"].into_iter().collect());
    ta.insert("col", ["span"].into_iter().collect());
    b.tag_attributes(ta);
    b.link_rel(None);
    b.url_schemes(
        ["http", "https", "mailto", "tel", "obsidian"]
            .into_iter()
            .collect(),
    );
    b.strip_comments(true);
    b.filter_style_properties(["text-align"].into_iter().collect());
    b
}

impl Markdown {
    pub fn new() -> Self {
        let mut md = MarkdownIt::new();
        md.link_formatter = Box::new(PyLinkFormatter);
        markdown_it::plugins::cmark::add(&mut md);
        markdown_it::plugins::html::add(&mut md);
        markdown_it::plugins::extra::tables::add(&mut md);
        markdown_it::plugins::extra::strikethrough::add(&mut md);
        markdown_it_front_matter::add(&mut md);
        markdown_it_footnote::add(&mut md);
        markdown_it_deflist::add(&mut md);
        md.inline.add_rule::<LinkifyInline>();
        md.add_rule::<CaptureInline>().before::<InlineParserRule>();
        md.add_rule::<TaskLists>().after::<InlineParserRule>();
        md.add_rule::<LinkifyCore>().after::<TaskLists>();
        md.add_rule::<TextJoin>().after_all();
        Markdown {
            md,
            sanitizer: sanitizer(),
        }
    }

    pub fn sanitize(&self, html: &str) -> String {
        self.sanitizer.clean(html).to_string()
    }

    /// Render `text` (a markdown file at KB path `src`) to sanitized HTML.
    pub fn render(&self, idx: &Index, src: &str, text: &str, historic: bool) -> String {
        let mut ast = self.md.parse(text);
        let line_starts = line_starts(text);
        let line_of = |byte: usize| line_starts.partition_point(|s| *s <= byte);
        // blocks: data-line, heading ids, ordered lists and indented code with markdown-it's markup
        let mut seen: HashMap<String, usize> = HashMap::new();
        fn walk_blocks(
            node: &mut Node,
            line_of: &dyn Fn(usize) -> usize,
            seen: &mut HashMap<String, usize>,
            top: bool,
        ) {
            let line = node.srcmap.map(|m| line_of(m.get_byte_offsets().0));
            if let Some(ol) = node.cast::<OrderedList>() {
                let start = ol.start;
                let mut attrs = Vec::new();
                if start != 1 {
                    attrs.push(("start", start.to_string()));
                }
                attrs.extend(node.attrs.drain(..));
                node.attrs = attrs;
                node.replace(Ol);
            }
            if let Some(cb) = node.cast::<CodeBlock>() {
                let content = cb.content.clone();
                node.replace(Pre { content });
            }
            let block = node.is::<Paragraph>()
                || node.is::<ATXHeading>()
                || node.is::<SetextHeader>()
                || node.is::<BulletList>()
                || node.is::<Ol>()
                || node.is::<ListItem>()
                || node.is::<Blockquote>()
                || node.is::<CodeFence>()
                || node.is::<Pre>()
                || node.is::<ThematicBreak>()
                || node.is::<Table>()
                || node.is::<TableHead>()
                || node.is::<TableBody>()
                || node.is::<TableRow>()
                || is_deflist_block(node);
            if !top && block {
                if let Some(l) = line {
                    set_attr(&mut node.attrs, "data-line", l.to_string());
                }
            }
            if node.is::<ATXHeading>() || node.is::<SetextHeader>() {
                let content = node
                    .ext
                    .get::<HeadingSource>()
                    .map(|h| h.0.clone())
                    .unwrap_or_default();
                let base = slugify(&content);
                let n = seen.entry(base.clone()).or_insert(0);
                *n += 1;
                let id = if *n == 1 {
                    base
                } else {
                    format!("{}-{}", base, *n - 1)
                };
                set_attr(&mut node.attrs, "id", id);
            }
            for c in node.children.iter_mut() {
                walk_blocks(c, line_of, seen, false);
            }
        }
        walk_blocks(&mut ast, &line_of, &mut seen, true);
        rewrite_inline(&mut ast, idx, src, historic);
        // markdown-it-py renders the (hidden) front-matter token as nothing, then gives the next
        // block-level tag a leading newline -- unless that block has its own render rule
        let lead = ast.children.first().is_some_and(|n| n.is::<FrontMatter>())
            && ast
                .children
                .get(1)
                .is_some_and(|n| !(n.is::<HtmlBlock>() || n.is::<CodeFence>() || n.is::<Pre>()));
        let mut html = ast.render();
        if lead {
            html.insert(0, '\n');
        }
        static COMMENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)<!--(.*?)-->").unwrap());
        let html = COMMENT_RE.replace_all(&html, |c: &regex::Captures| {
            format!(
                "<span class=\"hc\">&lt;!--{}--&gt;</span>",
                py_html_escape(&c[1])
            )
        });
        self.sanitize(&html)
    }
}

fn is_deflist_block(node: &Node) -> bool {
    let n = node.name();
    n.ends_with("DefinitionList")
        || n.ends_with("DefinitionTerm")
        || n.ends_with("DefinitionDescription")
}

fn line_starts(text: &str) -> Vec<usize> {
    let mut v = vec![0];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    v
}

/// `urllib.parse.quote(s)` (safe='/').
pub fn py_quote(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Links, images, path-like code spans and [[wiki]] text, resolved against the index.
fn rewrite_inline(node: &mut Node, idx: &Index, src: &str, historic: bool) {
    let old = std::mem::take(&mut node.children);
    let mut out = Vec::with_capacity(old.len());
    for mut c in old {
        if let Some(l) = c.cast::<Link>() {
            let mut attrs = vec![("href", l.url.clone())];
            if let Some(t) = &l.title {
                attrs.push(("title", t.clone()));
            }
            attrs.extend(c.attrs.drain(..));
            c.replace(A { attrs, auto: false });
        } else if let Some(a) = c.cast::<Autolink>() {
            let attrs = vec![("href", a.url.clone())];
            c.replace(A { attrs, auto: true });
        }
        if c.is::<A>() {
            let href = get_attr(&c.cast::<A>().unwrap().attrs, "href")
                .unwrap_or("")
                .to_string();
            let a = c.cast_mut::<A>().unwrap();
            decorate_link(idx, src, &mut a.attrs, &href, historic);
        } else if let Some(img) = c.cast::<Image>() {
            let url = img.url.clone();
            let title = img.title.clone();
            let alt = c.collect_text();
            let mut attrs = vec![("src", url.clone()), ("alt", alt)];
            if let Some(t) = title {
                attrs.push(("title", t));
            }
            let (hit, how) = idx.resolve(src, &url, "embed", historic);
            match hit {
                Some(h) if idx.files.contains_key(&h) => {
                    set_attr(&mut attrs, "src", format!("/raw/{}", py_quote(&h)));
                    set_attr(&mut attrs, "loading", "lazy".into());
                }
                _ if how == "external" => {
                    set_attr(&mut attrs, "src", String::new());
                    set_attr(&mut attrs, "class", "ext-img".into());
                    set_attr(
                        &mut attrs,
                        "title",
                        format!("External image not loaded: {url}"),
                    );
                }
                _ => {}
            }
            c.children.clear();
            c.replace(Img { attrs });
            out.push(c);
            continue;
        } else if c.is::<CodeInline>() {
            let code = py_strip(&c.collect_text()).to_string();
            if PATHLIKE_RE.is_match(&code)
                && (code.contains('/') || crate::config::is_known_file_ext(&ext_of(&code)))
            {
                let (hit, _) = idx.resolve(src, &code, "mention", historic);
                if let Some(h) = hit {
                    if h != "." {
                        let mut a = Node::new(A {
                            attrs: vec![
                                ("href", format!("#/f/{}", py_quote(&h))),
                                ("data-kb", h.clone()),
                                ("class", "kl mention".into()),
                            ],
                            auto: false,
                        });
                        a.children.push(c);
                        out.push(a);
                        continue;
                    }
                }
            }
            out.push(c);
            continue;
        } else if let Some(t) = c.cast::<Text>() {
            if t.content.contains("[[") {
                let content = t.content.clone();
                out.extend(wiki_nodes(idx, src, &content, historic));
                continue;
            }
        }
        rewrite_inline(&mut c, idx, src, historic);
        out.push(c);
    }
    node.children = out;
}

fn decorate_link(
    idx: &Index,
    src: &str,
    attrs: &mut Vec<(&'static str, String)>,
    href: &str,
    historic: bool,
) {
    let (hit, how) = idx.resolve(src, href, "link", historic);
    match hit {
        Some(h) if h != "." => {
            let frag = href.split_once('#').map(|(_, f)| f).unwrap_or("");
            let mut target = format!("#/f/{}", py_quote(&h));
            if !frag.is_empty() {
                target.push('#');
                target.push_str(frag);
            }
            set_attr(attrs, "href", target);
            set_attr(attrs, "data-kb", h);
            set_attr(attrs, "class", "kl".into());
        }
        _ => match how {
            "anchor" => {
                set_attr(attrs, "class", "anchor".into());
                set_attr(attrs, "data-href", href.to_string());
            }
            "external" => {
                set_attr(attrs, "target", "_blank".into());
                set_attr(attrs, "rel", "noopener noreferrer".into());
                set_attr(attrs, "class", "ext".into());
            }
            "outside" => {
                set_attr(attrs, "class", "outside".into());
                set_attr(attrs, "title", format!("Outside the KB: {href}"));
                set_attr(attrs, "href", String::new());
            }
            _ => {
                set_attr(attrs, "class", "broken".into());
                set_attr(attrs, "title", format!("Broken link: {href}"));
                set_attr(attrs, "href", String::new());
            }
        },
    }
}

fn wiki_nodes(idx: &Index, src: &str, text: &str, historic: bool) -> Vec<Node> {
    let mut out = Vec::new();
    let mut last = 0;
    for m in WIKI_RE.captures_iter(text) {
        let whole = m.get(0).unwrap();
        if whole.start() > last {
            out.push(Node::new(Text {
                content: text[last..whole.start()].to_string(),
            }));
        }
        let target = m.name("target").unwrap().as_str();
        let (hit, _) = idx.resolve(src, target, "wiki", historic);
        let label = py_strip(m.name("alias").map(|a| a.as_str()).unwrap_or(target)).to_string();
        let attrs = match hit {
            Some(h) => vec![
                ("href", format!("#/f/{}", py_quote(&h))),
                ("data-kb", h.clone()),
                ("class", "kl wiki".into()),
            ],
            None => vec![
                ("href", String::new()),
                ("class", "broken wiki".into()),
                ("title", format!("Broken wiki link: {}", whole.as_str())),
            ],
        };
        let mut a = Node::new(A { attrs, auto: false });
        a.children.push(Node::new(Text { content: label }));
        out.push(a);
        last = whole.end();
    }
    if last < text.len() {
        out.push(Node::new(Text {
            content: text[last..].to_string(),
        }));
    }
    out
}

#[allow(dead_code)]
fn root_content(root: &Node) -> &str {
    root.cast::<Root>()
        .map(|r| r.content.as_str())
        .unwrap_or("")
}
