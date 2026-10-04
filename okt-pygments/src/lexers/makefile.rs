//! `MakefileLexer.get_tokens_unprocessed` (pygments/lexers/make.py) and
//! `pygments.lexer.do_insertions`.
//!
//! Lines that are make directives (`r_special`, plus continuations of such lines) or comments
//! (`r_comment`) are cut out and become single tokens; everything else is concatenated and
//! lexed with `BaseMakefileLexer`, and the cut lines are inserted back with `do_insertions`.
//! `do_insertions` does index arithmetic on Python string lengths, so it runs on character
//! counts here.

use std::borrow::Cow;

use crate::data::{LexerDef, TokId};
use crate::engine::{Inst, Runner, Tok};
use crate::{LexError, py_strip};

pub(crate) fn lex<'t>(
    run: &Runner,
    lx: &LexerDef,
    text: &'t str,
    out: &mut Vec<Tok<'t>>,
    base: usize,
) -> Result<(), LexError> {
    let rx = |k: &str| &run.data.regexes[lx.data[k].as_u64().expect("regex id") as usize];
    let tok = |k: &str| lx.data[k].as_u64().expect("token id") as TokId;
    let (r_special, r_comment) = (rx("r_special"), rx("r_comment"));
    let (preproc, comment) = (tok("preproc"), tok("comment"));

    // (len(done), [(0, token, line)]) with len() in characters
    let mut ins: Vec<(i64, TokId, &'t str)> = Vec::new();
    let mut done = String::new();
    let mut done_chars = 0i64;
    let mut backslashflag = false;
    for line in py_splitlines_keepends(text) {
        let exact = run.exact || line.is_empty();
        if r_special.match_end(line, 0, exact)?.is_some() || backslashflag {
            ins.push((done_chars, preproc, line));
            backslashflag = py_strip(line).ends_with('\\');
        } else if r_comment.match_end(line, 0, exact)?.is_some() {
            ins.push((done_chars, comment, line));
        } else {
            done.push_str(line);
            done_chars += line.chars().count() as i64;
        }
    }

    let base_lexer = run
        .data
        .lexer_id("BaseMakefileLexer")
        .expect("BaseMakefileLexer exported");
    let mut toks = Vec::new();
    run.tokens(Inst::new(base_lexer), &done, None, &mut toks, 0)?;
    // Token positions in characters, values owned (they are slices of `done`).
    let mut char_pos = CharPos::new(&done);
    let toks: Vec<(i64, TokId, String)> = toks
        .into_iter()
        .map(|t| (char_pos.of(t.pos) as i64, t.ty, t.val.into_owned()))
        .collect();

    for (pos, ty, val) in do_insertions(ins, toks) {
        out.push(Tok {
            pos: base + pos.max(0) as usize,
            ty,
            val,
        });
    }
    Ok(())
}

/// `do_insertions(insertions, tokens)` with one-token insertion lists.
fn do_insertions<'t>(
    insertions: Vec<(i64, TokId, &'t str)>,
    tokens: Vec<(i64, TokId, String)>,
) -> Vec<(i64, TokId, Cow<'t, str>)> {
    let mut res: Vec<(i64, TokId, Cow<'t, str>)> = Vec::new();
    let mut insertions = insertions.into_iter();
    let Some((mut index, mut it_tok, mut it_val)) = insertions.next() else {
        return tokens
            .into_iter()
            .map(|(p, t, v)| (p, t, Cow::Owned(v)))
            .collect();
    };
    let mut realpos: Option<i64> = None;
    let mut insleft = true;
    for (i, t, v) in tokens {
        // first iteration. store the position of first item
        let rp = realpos.get_or_insert(i);
        let vlen = v.chars().count() as i64;
        let mut oldi = 0i64;
        while insleft && i + vlen >= index {
            let tmpval = py_slice(&v, Some(oldi), Some(index - i));
            if !tmpval.is_empty() {
                res.push((*rp, t, Cow::Owned(tmpval.to_string())));
                *rp += tmpval.chars().count() as i64;
            }
            res.push((*rp, it_tok, Cow::Borrowed(it_val)));
            *rp += it_val.chars().count() as i64;
            oldi = index - i;
            match insertions.next() {
                Some((a, b, c)) => (index, it_tok, it_val) = (a, b, c),
                None => {
                    insleft = false;
                    break;
                }
            }
        }
        if oldi < vlen {
            let rest = py_slice(&v, Some(oldi), None);
            res.push((*rp, t, Cow::Owned(rest.to_string())));
            *rp += vlen - oldi;
        }
    }
    // leftover tokens
    while insleft {
        // no normal tokens, set realpos to zero
        let rp = realpos.get_or_insert(0);
        res.push((*rp, it_tok, Cow::Borrowed(it_val)));
        *rp += it_val.chars().count() as i64;
        match insertions.next() {
            Some((_, b, c)) => (it_tok, it_val) = (b, c),
            None => insleft = false,
        }
    }
    res
}

/// Python's `s[start:stop]` on characters (negative indices count from the end).
fn py_slice(s: &str, start: Option<i64>, stop: Option<i64>) -> &str {
    let n = s.chars().count() as i64;
    let norm = |i: i64| if i < 0 { (n + i).max(0) } else { i.min(n) };
    let a = start.map(norm).unwrap_or(0);
    let b = stop.map(norm).unwrap_or(n);
    if a >= b {
        return "";
    }
    let byte = |ci: i64| {
        s.char_indices()
            .nth(ci as usize)
            .map(|(b, _)| b)
            .unwrap_or(s.len())
    };
    &s[byte(a)..byte(b)]
}

/// `str.splitlines(keepends=True)`.
pub(crate) fn py_splitlines_keepends(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let end = match c {
            '\r' => {
                if let Some(&(_, '\n')) = it.peek() {
                    it.next();
                    i + 2
                } else {
                    i + 1
                }
            }
            '\n' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}'
            | '\u{2029}' => i + c.len_utf8(),
            _ => continue,
        };
        lines.push(&text[start..end]);
        start = end;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// Converts increasing byte offsets of a string to character offsets.
struct CharPos<'a> {
    s: &'a str,
    byte: usize,
    chars: usize,
}

impl<'a> CharPos<'a> {
    fn new(s: &'a str) -> Self {
        CharPos {
            s,
            byte: 0,
            chars: 0,
        }
    }

    fn of(&mut self, byte: usize) -> usize {
        if byte < self.byte {
            self.byte = 0;
            self.chars = 0;
        }
        self.chars += self.s[self.byte..byte].chars().count();
        self.byte = byte;
        self.chars
    }
}
