//! Pygments-compatible syntax highlighting.
//!
//! [`highlight_lines`] reproduces, byte for byte, this function of the Python reference
//! (`terminal.py`), which uses Pygments 2.21.0:
//!
//! ```python
//! def highlight_lines(text, path=None, lang=None):
//!     # lexer: get_lexer_by_name(lang) / get_lexer_for_filename(basename) / #! guess,
//!     #        all with stripnl=False, ensurenl=False
//!     if lexer is None:
//!         return [html.escape(line) for line in text.split('\n')]
//!     out = highlight(text, lexer, HtmlFormatter(nowrap=True))
//!     lines = out.split('\n')
//!     n = text.count('\n') + 1
//!     if len(lines) > n:
//!         lines = lines[:n]
//!     return lines
//! ```
//!
//! How it is built:
//!
//! * `tools/pygments/export.py` dumps, from the real Pygments, the lexer registry, the
//!   processed token tables of the supported lexers, their regexes (translated to
//!   fancy-regex syntax with exact Python semantics) and the CSS class of every token type
//!   into `data/pygments.json`, which is embedded in the crate ([`data`]).
//! * [`engine`] ports `RegexLexer` / `ExtendedRegexLexer` (`get_tokens_unprocessed`,
//!   `bygroups`, `using`, `default`), [`regex`] gives fancy-regex Python's
//!   `pattern.match(text, pos)` semantics, [`lexers`] holds the hand ports of non-table code
//!   (JSON, YAML callbacks, Makefile, Ruby heredocs, C/C++/Lua/PHP/Swift name filters, ...),
//!   [`format`] ports `HtmlFormatter._format_lines`, and [`registry`] the lexer selection.
//!
//! Lexers Pygments would select but that are not exported/ported give
//! [`Highlight::Unsupported`], so the caller can fall back to another highlighter instead of
//! showing output that differs from the reference.

mod data;
mod engine;
mod format;
mod lexers;
mod regex;
mod registry;

use std::borrow::Cow;

use once_cell::sync::Lazy;

use data::DATA;
use engine::{Inst, Runner};

/// Result of [`highlight_lines`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Highlight {
    /// Exactly what the reference returns, including its `html.escape` fallback (one escaped
    /// line per `\n`-separated line) when no lexer applies.
    Lines(Vec<String>),
    /// Pygments would pick this lexer (class name, e.g. `"CSharpLexer"`), but it isn't
    /// ported, or lexing failed where Python would have raised; the caller should use its
    /// own highlighter.
    Unsupported { lexer: String },
}

/// Highlight `text` to per-line HTML, like the reference's `highlight_lines(text, path, lang)`.
///
/// `lang` (a Pygments alias) takes precedence; otherwise the lexer is chosen from the file
/// name in `path` (only the part after the last `/` matters), falling back to the `#!` line.
pub fn highlight_lines(text: &str, path: Option<&str>, lang: Option<&str>) -> Highlight {
    let data = &*DATA;
    let Some(entry) = select(data, text, path, lang) else {
        return Highlight::Lines(text.split('\n').map(format::escape_quote).collect());
    };
    let unsupported = || Highlight::Unsupported {
        lexer: entry.class.clone(),
    };
    let Some(id) = data.lexer_id(&entry.class) else {
        return unsupported();
    };
    // A panic would be a bug in this crate; never let it take a server thread down.
    match std::panic::catch_unwind(|| highlight_with(id, text)) {
        Ok(Ok(lines)) => Highlight::Lines(lines),
        Ok(Err(_)) | Err(_) => unsupported(),
    }
}

/// Decode the embedded lexer data now instead of on the first [`highlight_lines`] call
/// (about 10 ms). Regexes are still compiled lazily, on first use.
pub fn preload() {
    Lazy::force(&DATA);
}

/// The class names of the lexers this crate can run (directly selectable or used through
/// `using()` by another lexer).
pub fn supported_lexers() -> Vec<&'static str> {
    DATA.lexers.iter().map(|l| l.name.as_str()).collect()
}

/// The lexer class Pygments would pick, without highlighting (for diagnostics).
pub fn select_lexer(text: &str, path: Option<&str>, lang: Option<&str>) -> Option<&'static str> {
    select(&DATA, text, path, lang).map(|e| e.class.as_str())
}

/// The lexer choice of the reference: `get_lexer_by_name(lang)` if `lang`, else
/// `get_lexer_for_filename(basename)`, else the `#!` guess; `None` when all fail.
fn select<'d>(
    data: &'d data::Data,
    text: &str,
    path: Option<&str>,
    lang: Option<&str>,
) -> Option<&'d data::RegEntry> {
    match (
        lang.filter(|l| !l.is_empty()),
        path.filter(|p| !p.is_empty()),
    ) {
        (Some(lang), _) => registry::by_name(data, lang),
        (None, Some(path)) => {
            let name = path.rsplit('/').next().unwrap_or(path);
            registry::for_filename(data, name).or_else(|| registry::for_shebang(data, text))
        }
        (None, None) => None,
    }
}

fn highlight_with(id: data::LexId, text: &str) -> Result<Vec<String>, LexError> {
    let data = &*DATA;
    let pre = preprocess(text);
    let run = Runner::new(data, regex::needs_exact_word_boundaries(&pre));
    let mut toks = Vec::new();
    run.tokens(Inst::new(id), &pre, None, &mut toks, 0)?;
    let mut lines = format::format_lines(data, &toks);
    // `out.split('\n')`: every formatter line ends with '\n', so there is a final ''.
    lines.push(String::new());
    let n = text.bytes().filter(|&b| b == b'\n').count() + 1;
    lines.truncate(n);
    Ok(lines)
}

/// `Lexer._preprocess_lexer_input` with `stripnl=False, ensurenl=False, tabsize=0`: drop a
/// leading BOM, normalize `\r\n` and `\r` to `\n`.
fn preprocess(text: &str) -> Cow<'_, str> {
    let t = text.strip_prefix('\u{feff}').unwrap_or(text);
    if t.contains('\r') {
        Cow::Owned(t.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(t)
    }
}

/// Why lexing failed (where Python would raise or hang).
#[derive(Debug)]
#[allow(dead_code)] // the payloads are for Debug output
pub(crate) enum LexError {
    /// A regex failed to compile or hit the backtracking limit.
    Regex(String),
    /// No progress for a long time (Pygments would loop forever).
    Stall,
    /// What would be a Python exception.
    Python(&'static str),
    /// The lexer has no implementation.
    Unsupported,
}

/// `str.isspace()` for one character (the same set as `\s` in Python's `re`).
pub(crate) fn py_isspace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\x0b' | '\x0c' | '\r' | '\x1c'..='\x20' | '\u{85}' | '\u{a0}' | '\u{1680}'
            | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// `str.strip()`.
pub(crate) fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}

/// `str.rstrip()`.
pub(crate) fn py_rstrip(s: &str) -> &str {
    s.trim_end_matches(py_isspace)
}

#[cfg(test)]
mod tests;
