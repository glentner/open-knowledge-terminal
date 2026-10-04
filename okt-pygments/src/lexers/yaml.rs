//! `YamlLexer` callbacks (pygments/lexers/data.py), operating on `YamlLexerContext`
//! (`indent_stack`, `indent`, `next_indent`, `block_scalar_indent` in [`Ctx`]).
//!
//! The `len(text)` computations here are on runs of spaces (or single ASCII indicators), so
//! byte lengths equal Python's character lengths.

use crate::LexError;
use crate::data::Callback;
use crate::engine::{Ctx, Runner, Tok};
use crate::regex::Groups;

pub(crate) fn callback<'t>(
    run: &Runner,
    cb: &Callback,
    text: &'t str,
    m: &Groups,
    ctx: &mut Ctx,
    out: &mut Vec<Tok<'t>>,
    base: usize,
) -> Result<(), LexError> {
    let (start, end) = (m.start(), m.end());
    let s = &text[start..end];
    let len = s.chars().count() as i64;
    match *cb {
        // "Do not produce empty tokens."
        Callback::YamlSomething(tok) => {
            if s.is_empty() {
                return Ok(());
            }
            out.push(Tok::new(base + start, tok, s));
            ctx.pos = end;
        }
        // "Reset the indentation levels."
        Callback::YamlResetIndent(tok) => {
            ctx.indent_stack.clear();
            ctx.indent = -1;
            ctx.next_indent = 0;
            ctx.block_scalar_indent = None;
            out.push(Tok::new(base + start, tok, s));
            ctx.pos = end;
        }
        // "Save a possible indentation level."
        Callback::YamlSaveIndent {
            tok,
            start: is_start,
        } => {
            let mut t = s;
            let mut extra = "";
            if is_start {
                ctx.next_indent = len;
                if ctx.next_indent < ctx.indent {
                    while ctx.next_indent < ctx.indent {
                        ctx.indent = ctx
                            .indent_stack
                            .pop()
                            .ok_or(LexError::Python("pop from empty list"))?;
                    }
                    if ctx.next_indent > ctx.indent {
                        let cut = byte_index(s, ctx.indent);
                        extra = &s[cut..];
                        t = &s[..cut];
                    }
                }
            } else {
                ctx.next_indent += len;
            }
            if !t.is_empty() {
                out.push(Tok::new(base + start, tok, t));
            }
            if !extra.is_empty() {
                // `token_class.Error`
                let name = format!("{}.Error", run.data.tokens[tok as usize].name);
                let err = run
                    .data
                    .tok_id(&name)
                    .ok_or(LexError::Python("token type not exported"))?;
                out.push(Tok::new(base + start + t.len(), err, extra));
            }
            ctx.pos = end;
        }
        // "Set the previously saved indentation level."
        Callback::YamlSetIndent { tok, implicit } => {
            if ctx.indent < ctx.next_indent {
                ctx.indent_stack.push(ctx.indent);
                ctx.indent = ctx.next_indent;
            }
            if !implicit {
                ctx.next_indent += len;
            }
            out.push(Tok::new(base + start, tok, s));
            ctx.pos = end;
        }
        // "Set an explicit indentation level for a block scalar."
        Callback::YamlSetBlockScalarIndent(tok) => {
            ctx.block_scalar_indent = None;
            if s.is_empty() {
                return Ok(());
            }
            // `if increment:` -- the group is optional
            if let Some((a, b)) = m.get(1)?
                && b > a
            {
                let current_indent = ctx.indent.max(0);
                let increment: i64 = text[a..b].parse().map_err(|_| LexError::Python("int()"))?;
                ctx.block_scalar_indent = Some(current_indent + increment);
            }
            out.push(Tok::new(base + start, tok, s));
            ctx.pos = end;
        }
        // "Process an empty line in a block scalar."
        Callback::YamlParseBlockScalarEmptyLine { indent, content } => {
            match ctx.block_scalar_indent {
                Some(bsi) if len > bsi => {
                    let cut = byte_index(s, bsi);
                    out.push(Tok::new(base + start, indent, &s[..cut]));
                    out.push(Tok::new(base + start + cut, content, &s[cut..]));
                }
                _ => {
                    if !s.is_empty() {
                        out.push(Tok::new(base + start, indent, s));
                    }
                }
            }
            ctx.pos = end;
        }
        // "Process indentation spaces in a block scalar."
        Callback::YamlParseBlockScalarIndent(tok) => {
            match ctx.block_scalar_indent {
                None => {
                    if len <= ctx.indent.max(0) {
                        pop2(ctx)?;
                        return Ok(());
                    }
                    ctx.block_scalar_indent = Some(len);
                }
                Some(bsi) => {
                    if len < bsi {
                        pop2(ctx)?;
                        return Ok(());
                    }
                }
            }
            if !s.is_empty() {
                out.push(Tok::new(base + start, tok, s));
                ctx.pos = end;
            }
        }
        // "Process indentation spaces in a plain scalar."
        Callback::YamlParsePlainScalarIndent(tok) => {
            if len <= ctx.indent {
                pop2(ctx)?;
                return Ok(());
            }
            if !s.is_empty() {
                out.push(Tok::new(base + start, tok, s));
                ctx.pos = end;
            }
        }
        _ => unreachable!("not a YAML callback"),
    }
    Ok(())
}

/// `context.stack.pop(); context.stack.pop()`
fn pop2(ctx: &mut Ctx) -> Result<(), LexError> {
    for _ in 0..2 {
        ctx.stack
            .pop()
            .ok_or(LexError::Python("pop from empty list"))?;
    }
    Ok(())
}

/// Byte offset of Python index `i` into `s` (with Python's clamping for slices; a negative
/// index counts from the end).
fn byte_index(s: &str, i: i64) -> usize {
    let n = s.chars().count() as i64;
    let i = if i < 0 { (n + i).max(0) } else { i.min(n) };
    s.char_indices()
        .nth(i as usize)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}
