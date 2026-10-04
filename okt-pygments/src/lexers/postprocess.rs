//! Token filters from `get_tokens_unprocessed` overrides that wrap
//! `RegexLexer.get_tokens_unprocessed`:
//!
//! * `CFamilyLexer` (C, C++): a `Name` in the stdlib / C99 / C11 atomic / Linux type lists
//!   becomes `Keyword.Type` (all four options default to on);
//! * `SwiftLexer`: a `Name` or `Name.Class` in the Cocoa lists becomes `Name.Builtin.Pseudo`;
//! * `PhpLexer`: a `Name.Other` in the PHP function lists becomes `Name.Builtin`;
//! * `LuaLexer`: a `Name.Builtin` that is not a known function becomes `Name` (split at the
//!   dot into `Name`, `Punctuation`, `Name`).

use std::borrow::Cow;
use std::collections::HashSet;

use crate::LexError;
use crate::data::{Data, LexerDef, TokId};
use crate::engine::Tok;

fn names(lx: &LexerDef, build: impl FnOnce(&LexerDef) -> HashSet<String>) -> &HashSet<String> {
    lx.names.get_or_init(|| build(lx))
}

fn strings(v: &serde_json::Value) -> impl Iterator<Item = String> + '_ {
    v.as_array()
        .expect("array")
        .iter()
        .map(|s| s.as_str().expect("string").to_string())
}

fn tok(lx: &LexerDef, key: &str) -> TokId {
    lx.data[key].as_u64().expect("token id") as TokId
}

pub(crate) fn cfamily(_d: &Data, lx: &LexerDef, toks: &mut [Tok]) {
    let set = names(lx, |lx| {
        lx.data["sets"]
            .as_array()
            .expect("sets")
            .iter()
            .flat_map(strings)
            .collect()
    });
    let (name, ty) = (tok(lx, "name"), tok(lx, "type"));
    for t in toks.iter_mut() {
        if t.ty == name && set.contains(t.val.as_ref()) {
            t.ty = ty;
        }
    }
}

pub(crate) fn swift(_d: &Data, lx: &LexerDef, toks: &mut [Tok]) {
    let set = names(lx, |lx| strings(&lx.data["names"]).collect());
    let (a, b) = (
        lx.data["match"][0].as_u64().unwrap() as TokId,
        lx.data["match"][1].as_u64().unwrap() as TokId,
    );
    let to = tok(lx, "to");
    for t in toks.iter_mut() {
        if (t.ty == a || t.ty == b) && set.contains(t.val.as_ref()) {
            t.ty = to;
        }
    }
}

/// PhpLexer's heredoc rule, `(<<<)(['"]?)(IDENT)(\2\n.*?\n\s*)(\3)(;?)(\n)` under
/// IGNORECASE, has a case-insensitive backreference. Python compares `lower()` per character;
/// fancy-regex compares simple case folds over the same number of bytes. With an ASCII
/// identifier the two agree except that Python also accepts U+212A KELVIN SIGN for `k` and
/// U+0130 for `i`; any other difference needs a non-ASCII character in the identifier. Refuse
/// exactly those inputs (the caller falls back to its own highlighter).
pub(crate) fn php_guard(text: &str) -> Result<(), LexError> {
    if text.is_ascii() || !text.contains("<<<") {
        return Ok(());
    }
    if text.contains(['\u{212A}', '\u{130}']) {
        return Err(LexError::Unsupported);
    }
    for (i, _) in text.match_indices("<<<") {
        let rest = &text[i + 3..];
        let rest = rest.strip_prefix(['\'', '"']).unwrap_or(rest);
        // the identifier: `(?:[_a-z]|[^\x00-\x7f])(?:\w|[^\x00-\x7f])*`
        for c in rest.chars() {
            if !c.is_ascii() {
                return Err(LexError::Unsupported);
            }
            if !(c.is_ascii_alphanumeric() || c == '_') {
                break;
            }
        }
    }
    Ok(())
}

pub(crate) fn php(_d: &Data, lx: &LexerDef, toks: &mut [Tok]) {
    let set = names(lx, |lx| strings(&lx.data["functions"]).collect());
    let (other, builtin) = (tok(lx, "other"), tok(lx, "builtin"));
    for t in toks.iter_mut() {
        if t.ty == other && set.contains(t.val.as_ref()) {
            t.ty = builtin;
        }
    }
}

/// Rewrites `out[start..]` in place.
pub(crate) fn lua<'t>(
    _d: &Data,
    lx: &LexerDef,
    out: &mut Vec<Tok<'t>>,
    start: usize,
) -> Result<(), LexError> {
    let set = names(lx, |lx| strings(&lx.data["functions"]).collect());
    let (builtin, name, punct) = (tok(lx, "builtin"), tok(lx, "name"), tok(lx, "punct"));
    if !out[start..]
        .iter()
        .any(|t| t.ty == builtin && !set.contains(t.val.as_ref()))
    {
        return Ok(());
    }
    let tail: Vec<Tok<'t>> = out.drain(start..).collect();
    for t in tail {
        if t.ty == builtin && !set.contains(t.val.as_ref()) {
            if let Some(dot) = t.val.find('.') {
                // a, b = value.split('.')  (ValueError unless exactly one dot)
                if t.val[dot + 1..].contains('.') {
                    return Err(LexError::Python("too many values to unpack"));
                }
                let p = t.pos;
                out.push(Tok {
                    pos: p,
                    ty: name,
                    val: sub(&t.val, 0, dot),
                });
                out.push(Tok {
                    pos: p + dot,
                    ty: punct,
                    val: sub(&t.val, dot, dot + 1),
                });
                out.push(Tok {
                    pos: p + dot + 1,
                    ty: name,
                    val: sub(&t.val, dot + 1, t.val.len()),
                });
            } else {
                out.push(Tok {
                    pos: t.pos,
                    ty: name,
                    val: t.val,
                });
            }
            continue;
        }
        out.push(t);
    }
    Ok(())
}

fn sub<'t>(v: &Cow<'t, str>, a: usize, b: usize) -> Cow<'t, str> {
    match v {
        Cow::Borrowed(s) => Cow::Borrowed(&s[a..b]),
        Cow::Owned(s) => Cow::Owned(s[a..b].to_string()),
    }
}
